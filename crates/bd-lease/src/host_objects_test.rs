//! The objects of a host (`lease_pool::host_refs`): the commits a host has, the pack of a client under the ref of its
//! run until the give-back of the run, and the reclaim of the refs no run needs

use crate::host_slots_test::{context, host_fixture, lend_to, params};
use crate::lease_test::Fixture;
use bd_cli::fixture_test::git;
use git_ops::host_objects::Stored;
use lease_pool::host_refs::{HostRef, IngestedPack, have, ingest};
use lease_pool::host_runs::reclaim_all;
use lease_pool::host_slots::{LendAnswer, fill, give_back_run};
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

  /// The ingest of the pack under `target` (`run:<id>` or `base:<name>`)
  fn ingest(&self, fixture: &Fixture, target: &str) -> IngestedPack {
    let context = context(fixture);
    let mut pack = std::fs::File::open(&self.pack).unwrap();
    ingest(&context.git, &context.dirs, &HostRef::parse(target).unwrap(), &self.commit, &mut pack, context.lock_timeout).unwrap()
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
  let context = context(&fixture);
  let params = params(&fixture, &context);
  fill(&context.git, &params, None).unwrap();
  let client = Client::of(&fixture, "c.txt");
  let have = |commits: &[String]| have(&context.git, &context.dirs, commits, context.lock_timeout).unwrap();
  assert_eq!(have(&[client.commit.clone(), client.base.clone()]), std::slice::from_ref(&client.base));

  let ingested = client.ingest(&fixture, "run:run-1");
  assert_eq!(
    (ingested.full_ref.as_str(), ingested.ingested.stored, ingested.maintenance_started),
    ("refs/branch-deck/host/run/run-1", Stored::Loose, false)
  );
  assert_eq!(have(std::slice::from_ref(&client.commit)), std::slice::from_ref(&client.commit));
  assert_eq!(host_refs(&fixture), format!("refs/branch-deck/host/run/run-1 {}", client.commit));

  assert!(matches!(lend_to(&params, "run-1", None), LendAnswer::Lent(_)));
  let gave_back = give_back_run(&context.git, &params, "run-1").unwrap();
  assert_eq!(gave_back.deleted_ref.as_deref(), Some("refs/branch-deck/host/run/run-1"));
  assert_eq!(host_refs(&fixture), "");
  assert_eq!(give_back_run(&context.git, &params, "run-2").unwrap().deleted_ref, None);
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
    assert!(client.ingest(&fixture, target).maintenance_started, "{target}");
  }
  let context = context(&fixture);
  let params = params(&fixture, &context);
  assert!(matches!(lend_to(&params, "waiting", None), LendAnswer::Queued(_)), "no slot is filled");

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

  assert_eq!(
    reclaim_all(&context.git, &params).unwrap().deleted_refs,
    ["refs/branch-deck/host/base/old", "refs/branch-deck/host/run/gone"]
  );
  let kept: Vec<String> = host_refs(&fixture).lines().map(|line| line.split(' ').next().unwrap().to_string()).collect();
  assert_eq!(kept, ["refs/branch-deck/host/base/new", "refs/branch-deck/host/run/waiting"]);
  assert_eq!(reclaim_all(&context.git, &params).unwrap().deleted_refs, Vec::<String>::new());
}
