use crate::host_objects::{
  IngestConfig, ShowRefusal, Stored, delete_host_refs, ingest_config, ingest_pack, is_host_ref_name, parse_pack_header, point_host_ref, present_commits, show_commit,
};
use pretty_assertions::assert_eq;
use std::io::{Cursor, Read};
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::{assert_processes, counting};

/// A client with two commits and a host that has the first one: the thin pack of the second against the first
struct Fixture {
  host: TestRepo,
  client: TestRepo,
  base: String,
  commit: String,
}

impl Fixture {
  fn new() -> Self {
    let (client, host) = TestRepo::upstream_and_clone(&[("a.txt", "a\n")]);
    let base = client.head();
    let commit = client.create_commit_with_files("Change a, add b", &[("a.txt", "a\nchanged\n"), ("b.txt", "b\n")]);
    Self { host, client, base, commit }
  }

  /// `pack-objects --revs --thin --stdout` of the commit against the base, as `bd host pack` builds it
  fn thin_pack(&self) -> Vec<u8> {
    let input = format!("{}\n^{}\n", self.commit, self.base);
    self
      .client
      .git_executor()
      .cmd(&["pack-objects", "--revs", "--thin", "--stdout", "-q"], self.client.path_str())
      .stdin(input.as_bytes())
      .bytes()
      .unwrap()
  }

  fn objects(&self) -> (usize, usize) {
    let counts = self.host.git(&["count-objects", "-v"]);
    let value = |key: &str| {
      counts
        .lines()
        .find_map(|line| line.strip_prefix(key))
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
    };
    (value("count:"), value("packs:"))
  }
}

/// A short reader: the pack arrives a few bytes at a time, as from a pipe, so the header is read across reads
struct Trickle(Cursor<Vec<u8>>);

impl Read for Trickle {
  fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
    let len = buffer.len().min(5);
    self.0.read(&mut buffer[..len])
  }
}

/// A pack below the unpack limit lands as loose objects, as `receive-pack` lands a small push, and one at the limit
/// as a pack of the repository; a thin pack resolves its bases from the object store either way, also when the pack
/// arrives a few bytes at a time
#[test]
fn a_small_pack_lands_as_loose_objects_and_a_large_one_as_a_pack() {
  let fixture = Fixture::new();
  let pack = fixture.thin_pack();
  let objects = parse_pack_header(pack[..12].try_into().unwrap()).unwrap().objects;
  let (loose, packs) = fixture.objects();

  let git = counting();
  let ingested = ingest_pack(&git, fixture.host.path_str(), &mut Trickle(Cursor::new(pack)), objects + 1).unwrap();
  assert_processes(&git, &[("unpack-objects", 1)]);
  assert_eq!((ingested.objects, ingested.stored), (objects, Stored::Loose));
  assert_eq!(fixture.objects(), (loose + objects as usize, packs));
  assert_eq!(fixture.host.git(&["cat-file", "-t", &fixture.commit]), "commit");

  let other = Fixture::new();
  let pack = other.thin_pack();
  let ingested = ingest_pack(other.host.git_executor(), other.host.path_str(), &mut Trickle(Cursor::new(pack)), objects).unwrap();
  assert_eq!(ingested.stored, Stored::Pack);
  assert_eq!(other.objects().1, packs + 1);
  assert_eq!(other.host.git(&["cat-file", "-t", &format!("{}:b.txt", other.commit)]), "blob");
}

/// A stream that is no pack is refused before git starts, and a pack that git refuses after its header is refused
/// with git's words; either way the rest of the stream is read to its end, so the sender is never blocked
#[test]
fn a_stream_that_is_no_pack_is_refused_and_read_to_its_end() {
  let fixture = Fixture::new();
  let mut input = Cursor::new(b"NOPE but a long text after it".to_vec());
  let error = ingest_pack(fixture.host.git_executor(), fixture.host.path_str(), &mut input, 100).unwrap_err();
  assert!(format!("{error:#}").contains("no pack"), "{error:#}");
  assert_eq!(input.position(), 29);

  let mut broken = fixture.thin_pack();
  broken.truncate(broken.len() - 30);
  broken.extend_from_slice(&[0; 30]);
  let length = broken.len() as u64;
  let mut input = Cursor::new(broken);
  ingest_pack(fixture.host.git_executor(), fixture.host.path_str(), &mut input, 100).unwrap_err();
  assert_eq!(input.position(), length);
}

/// The commits a host has, in the order asked, from one `cat-file --batch-check`: a tree and a missing id are none; a
/// short id is refused
#[test]
fn the_commits_a_host_has_are_its_commit_objects_in_order() {
  let fixture = Fixture::new();
  let tree = fixture.host.git(&["rev-parse", "HEAD^{tree}"]);
  let ids = [fixture.commit.clone(), tree, fixture.base.clone()];
  let git = counting();
  assert_eq!(present_commits(&git, fixture.host.path_str(), &ids).unwrap(), std::slice::from_ref(&fixture.base));
  assert_processes(&git, &[("cat-file", 1)]);
  present_commits(&git, fixture.host.path_str(), &[fixture.base[..8].to_string()]).unwrap_err();
}

/// The refs of the host move without a compare and go in one transaction, also one that is gone already
#[test]
fn the_refs_of_the_host_move_and_go_in_one_transaction() {
  let fixture = Fixture::new();
  let repo = fixture.host.path_str();
  let git = fixture.host.git_executor();
  point_host_ref(git, repo, "refs/branch-deck/host/run/r1", &fixture.base).unwrap();
  point_host_ref(git, repo, "refs/branch-deck/host/base/client", &fixture.base).unwrap();
  delete_host_refs(git, repo, &["refs/branch-deck/host/run/r1".to_string(), "refs/branch-deck/host/run/gone".to_string()]).unwrap();
  assert_eq!(
    fixture.host.git(&["for-each-ref", "--format=%(refname)", "refs/branch-deck/host/"]),
    "refs/branch-deck/host/base/client"
  );
}

/// The text of a file and the ids of paths of a commit from one `cat-file --batch`: a directory answers its tree, a
/// missing path none; a commit the repository lacks, a path with `..` and a file over 1 MiB are refused
#[test]
fn show_answers_the_text_of_a_file_and_the_ids_of_paths() {
  let repo = TestRepo::new();
  let commit = repo.create_commit_with_files(
    "Add files",
    &[(".yonder.toml", "outputs = [\"out\"]\n"), ("dir/a.txt", "a\n"), ("big.bin", &"x".repeat(1024 * 1024 + 1))],
  );
  let tree = repo.git(&["rev-parse", &format!("{commit}:dir")]);
  let file = repo.git(&["rev-parse", &format!("{commit}:dir/a.txt")]);
  let git = counting();
  let shown = show_commit(
    &git,
    repo.path_str(),
    &commit,
    Some(".yonder.toml"),
    &["dir/".to_string(), "dir/a.txt".to_string(), "missing".to_string()],
  )
  .unwrap();
  assert_processes(&git, &[("cat-file", 1)]);
  assert_eq!(shown.blob.map(|blob| blob.text), Some("outputs = [\"out\"]\n".to_string()));
  assert_eq!(shown.ids, [Some(tree), Some(file), None]);
  assert_eq!(show_commit(&git, repo.path_str(), &commit, Some("missing.toml"), &[]).unwrap().blob, None);

  let refusal = |result: anyhow::Result<_>| result.unwrap_err().downcast::<ShowRefusal>().unwrap();
  let missing = "0".repeat(40);
  assert_eq!(refusal(show_commit(&git, repo.path_str(), &missing, None, &[])), ShowRefusal::NoCommit(missing));
  assert_eq!(
    refusal(show_commit(&git, repo.path_str(), &commit, Some("../x"), &[])),
    ShowRefusal::BadPath("../x".to_string())
  );
  assert!(matches!(
    refusal(show_commit(&git, repo.path_str(), &commit, Some("big.bin"), &[])),
    ShowRefusal::TooLarge { .. }
  ));
}

/// The unpack limit is `receive.unpackLimit`, else `transfer.unpackLimit`, else 100, from one `config`
#[test]
fn the_unpack_limit_is_the_one_of_receive_else_transfer_else_100() {
  let repo = TestRepo::new();
  let config = || ingest_config(repo.git_executor(), repo.path_str()).unwrap();
  assert_eq!(
    config(),
    IngestConfig {
      unpack_limit: 100,
      maintenance: false
    }
  );
  repo.git(&["config", "transfer.unpackLimit", "7"]);
  repo.git(&["config", "branchdeck.leaseMaintenance", "true"]);
  assert_eq!(
    config(),
    IngestConfig {
      unpack_limit: 7,
      maintenance: true
    }
  );
  repo.git(&["config", "receive.unpackLimit", "3"]);
  assert_eq!(config().unpack_limit, 3);
}

#[test]
fn a_host_ref_name_is_one_safe_component() {
  for name in ["run-1", "1792-81.a_b", "client", &"a".repeat(128)] {
    assert!(is_host_ref_name(name), "{name}");
  }
  for name in ["", "-run", ".run", "a..b", "a/b", "a.lock", "a b", "a:b", &"a".repeat(129)] {
    assert!(!is_host_ref_name(name), "{name}");
  }
}
