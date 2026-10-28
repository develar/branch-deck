//! `bd lease host have`, `ingest`, `show` and `git` end to end through the Cli: the commits a host has, the pack of a
//! client under the ref of its run until the give-back of the run, the reclaim of the refs no run needs, the text of a
//! file and the ids of paths of a commit, the git of the host, and the refusals by their codes

use crate::host_command_test::host;
use crate::host_slots_test::host_fixture;
use crate::lease_test::Fixture;
use bd_cli::fixture_test::{assert_error, git};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::PathBuf;
use test_utils::git_test_utils::TestRepo;

/// A client of the host: a clone of its repository with a commit of its own, and the thin pack of that commit against
/// the HEAD of the host in a file
struct Client {
  repo: TestRepo,
  base: String,
  commit: String,
  pack: PathBuf,
  _dir: tempfile::TempDir,
}

impl Client {
  fn of(fixture: &Fixture, file: &str) -> Self {
    let repo = TestRepo::new_empty();
    repo.clone_from(fixture.repo.path()).unwrap();
    let base = repo.head();
    let commit = repo.create_commit("A change of the client", file, "the client's\n");
    let input = format!("{commit}\n^{base}\n");
    let bytes = repo
      .git_executor()
      .cmd(&["pack-objects", "--revs", "--thin", "--stdout", "-q"], repo.path_str())
      .stdin(input.as_bytes())
      .bytes()
      .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("run.pack");
    std::fs::write(&pack, bytes).unwrap();
    Self {
      repo,
      base,
      commit,
      pack,
      _dir: dir,
    }
  }

  fn ingest(&self, fixture: &Fixture, target: &str) -> Value {
    host(
      fixture,
      "lease_host_ingest",
      &["ingest", "--ref", target, "--commit", &self.commit, "--pack", self.pack.to_str().unwrap()],
    )
  }
}

fn host_refs(fixture: &Fixture) -> String {
  git(&fixture.repo, &["for-each-ref", "--format=%(refname) %(objectname)", "refs/branch-deck/host/"])
}

/// have, ingest and the give-back: the host lacks the commit of the client until its pack lands as loose objects under
/// the run ref, which the give-back of the run deletes; a run that sent no pack deletes none
#[test_log::test]
fn a_pack_lands_under_the_ref_of_its_run_until_the_give_back_of_the_run() {
  let fixture = host_fixture(1);
  host(&fixture, "lease_host_fill", &["fill"]);
  let client = Client::of(&fixture, "c.txt");
  let have = |commits: &[&str]| host(&fixture, "lease_host_have", &[&["have"][..], commits].concat());
  let had = have(&[&client.commit, &client.base]);
  assert_eq!(had["commits"], json!([client.base]), "{had}");

  let ingested = client.ingest(&fixture, "run:run-1");
  assert_eq!(
    (&ingested["ref"], &ingested["stored"], ingested.get("maintenance_started")),
    (&json!("refs/branch-deck/host/run/run-1"), &json!("loose"), None),
    "{ingested}"
  );
  assert_eq!(have(&[&client.commit])["commits"], json!([client.commit]));
  assert_eq!(host_refs(&fixture), format!("refs/branch-deck/host/run/run-1 {}", client.commit));

  let pid = std::process::id().to_string();
  host(&fixture, "lease_host_lend", &["lend", "--run", "run-1", "--pid", &pid]);
  let gave_back = host(&fixture, "lease_host_give_back", &["give-back", "--run", "run-1"]);
  assert_eq!(gave_back["deleted_ref"], json!("refs/branch-deck/host/run/run-1"), "{gave_back}");
  assert_eq!(host_refs(&fixture), "");
  let none = host(&fixture, "lease_host_give_back", &["give-back", "--run", "run-2"]);
  assert_eq!(none.get("deleted_ref"), None, "{none}");
  drop(client.repo);
}

/// reclaim deletes a run ref whose run has no lend an hour after its pack and a base ref that no pack moved for 14
/// days, and keeps the ref of a run that waits in the queue and a base ref a pack moved since; a pack with
/// `branchdeck.leaseMaintenance` starts the maintenance
#[test_log::test]
fn reclaim_deletes_the_refs_that_no_run_needs() {
  let fixture = host_fixture(1);
  git(&fixture.repo, &["config", "branchdeck.leaseMaintenance", "true"]);
  let client = Client::of(&fixture, "c.txt");
  for target in ["run:gone", "run:waiting", "base:old", "base:new"] {
    assert_eq!(client.ingest(&fixture, target)["maintenance_started"], json!(true));
  }
  let pid = std::process::id().to_string();
  let queued = host(&fixture, "lease_host_lend", &["lend", "--run", "waiting", "--pid", &pid]);
  assert_eq!(queued["state"], json!("queued"), "no slot is filled: {queued}");

  let file = fixture.repo.path().join(".git/branch-deck/host-refs.json");
  let mut refs: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
  let now_ms = u64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()).unwrap();
  let hours_ago = |hours: u64| json!(now_ms - hours * 3_600_000);
  for (full_ref, moved) in [
    ("run/gone", hours_ago(2)),
    ("run/waiting", hours_ago(2)),
    ("base/old", hours_ago(15 * 24)),
    ("base/new", hours_ago(13 * 24)),
  ] {
    refs["refs"][format!("refs/branch-deck/host/{full_ref}")]["moved_ms"] = moved;
  }
  std::fs::write(&file, serde_json::to_vec(&refs).unwrap()).unwrap();

  let reclaimed = host(&fixture, "lease_host_reclaim", &["reclaim"]);
  assert_eq!(
    reclaimed["deleted_refs"],
    json!(["refs/branch-deck/host/base/old", "refs/branch-deck/host/run/gone"]),
    "{reclaimed}"
  );
  let kept: Vec<String> = host_refs(&fixture).lines().map(|line| line.split(' ').next().unwrap().to_string()).collect();
  assert_eq!(kept, ["refs/branch-deck/host/base/new", "refs/branch-deck/host/run/waiting"]);
  assert_eq!(host(&fixture, "lease_host_reclaim", &["reclaim"]).get("deleted_refs"), None);
}

/// show answers the text of a file and the ids of paths of a commit, git the git of bd and the floor of a host
#[test_log::test]
fn show_answers_a_file_and_ids_and_git_answers_the_git_of_the_host() {
  let fixture = Fixture::new();
  let head = fixture.repo.head();
  let shown = host(&fixture, "lease_host_show", &["show", "--commit", &head, "--blob", "a.txt", "--ids", "a.txt", "missing"]);
  let a = git(&fixture.repo, &["rev-parse", &format!("{head}:a.txt")]);
  assert_eq!(
    (&shown["blob"], &shown["ids"]),
    (
      &json!({ "path": "a.txt", "id": a, "text": "a\n" }),
      &json!([{ "path": "a.txt", "id": a }, { "path": "missing", "id": null }])
    ),
    "{shown}"
  );
  assert_eq!(host(&fixture, "lease_host_show", &["show", "--commit", &head, "--blob", "none.toml"])["blob"], Value::Null);
  assert_eq!(host(&fixture, "lease_host_show", &["show", "--commit", &head]).get("blob"), None);

  let found = host(&fixture, "lease_host_git", &["git"]);
  assert_eq!(found["floor"], json!("2.56.0"));
  assert!(found["path"].as_str().is_some_and(|path| path.ends_with("git")), "{found}");
  assert!(found["meets_floor"].is_boolean() && found["upgraded"] == json!(false), "{found}");
}

/// The refusals by their codes: a short id, a ref that is no run or base, a commit the repository lacks, a directory
/// that is no repository
#[test_log::test]
fn the_refusals_of_the_host_objects_have_their_codes() {
  let fixture = Fixture::new();
  let head = fixture.repo.head();
  let refused = |args: &[&str], expected: (i32, &str)| {
    let (exit_code, output) = fixture.run(&[&["lease", "host"][..], args].concat());
    assert_error(&output, exit_code, expected);
  };
  refused(&["have", &head[..8]], (2, "invalid_arguments"));
  refused(&["ingest", "--ref", "run:../x", "--commit", &head, "--pack", "/dev/null"], (2, "invalid_arguments"));
  refused(&["ingest", "--ref", "branch:x", "--commit", &head, "--pack", "/dev/null"], (2, "invalid_arguments"));
  refused(&["ingest", "--ref", "run:x", "--commit", &head[..8], "--pack", "/dev/null"], (2, "invalid_arguments"));
  refused(&["show", "--commit", &"0".repeat(40)], (2, "invalid_arguments"));
  refused(&["show", "--commit", &head, "--blob", "../a.txt"], (2, "invalid_arguments"));
  let outside = tempfile::tempdir().unwrap();
  let (exit_code, output) = fixture.run(&["-C", outside.path().to_str().unwrap(), "lease", "host", "have", &head]);
  assert_error(&output, exit_code, (2, "not_a_repository"));
}
