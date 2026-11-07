//! The git processes of the cores of this crate, by subcommand: the numbers behind the batching rule
//! (docs/design/git-operations.md, "Batch"), as `sync_core`'s `process_budget_test` pins those of its cores. A change
//! that runs fewer processes lowers the numbers here; one that runs more has to say why.

use crate::resolve_session::unapply_branch_suggestion;
use crate::update_project::update_project_test::{path, setup};
use crate::update_project::{UpdateProjectParams, update_project_core};
use domain_types::update_project::UpdateState;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::LockWait;
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::{assert_processes, assert_signed_commits, counting, with_added_processes};

/// The processes of Update Project after its fetch (`an_update`, `an_update_after_its_fetch`): one `update-index`, the
/// refresh of the user's index that the dry run of the local changes copies and the move takes as it is
const UPDATE: &[(&str, u32)] = &[
  ("cat-file", 1),
  ("config", 2),
  ("diff-tree", 2),
  ("for-each-ref", 1),
  ("log", 3),
  ("merge-base", 2),
  ("read-tree", 2),
  ("remote", 1),
  ("replay", 1),
  ("rev-list", 3),
  ("rev-parse", 5),
  ("symbolic-ref", 4),
  ("update-index", 1),
  ("update-ref", 2),
];

/// `processes` and one `fetch`, sorted by subcommand
fn with_fetch(processes: &[(&'static str, u32)]) -> Vec<(&'static str, u32)> {
  let mut all = processes.to_vec();
  all.push(("fetch", 1));
  all.sort_unstable();
  all
}

fn run_update(git: &GitCommandExecutor, repo: &TestRepo, fetch: bool) {
  let result = update_project_core(
    git,
    &UpdateProjectParams {
      repository_path: path(repo).to_string(),
      fetch,
    },
    &|_| {},
    LockWait::default(),
  )
  .unwrap();
  assert!(result.applied.is_some(), "{result:?}");
}

/// Update Project: the fetch, the check and the rebase of the local commit onto the upstream, the files following. The
/// directories are read once at the start (one `rev-parse`): the guards, the lock, the pending moves and the index of
/// the dry run take them from there. Before its replay the check pairs the local commit with the upstream commits by
/// its author: no twin, and the sets of paths of both sides (`diff-tree --name-only`) differ, so no patch id. The check
/// runs before the lock; under it, its witness (the guards, one `rev-parse` of HEAD and the upstream) costs one
/// `rev-parse` and one `symbolic-ref` more. The record of what landed reads HEAD once more under the lock of the landed
/// state (`record_update`), as it runs after the repository lock: one `rev-parse`
#[test]
fn an_update() {
  let fixture = setup();
  fixture.clone.create_commit("(auth) Add login", "login.txt", "login\n");
  fixture.upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  let git = counting();
  run_update(&git, &fixture.clone, true);
  assert_processes(&git, &with_fetch(UPDATE));
}

/// Update Project of the app after its fetch (the `FetchBaseline` of its operation, `operations::update`): the same
/// update without the fetch, the network before it holds HEAD
#[test]
fn an_update_after_its_fetch() {
  let fixture = setup();
  fixture.clone.create_commit("(auth) Add login", "login.txt", "login\n");
  fixture.upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  fixture.clone.git(&["fetch", "--quiet", "origin"]);
  let git = counting();
  run_update(&git, &fixture.clone, false);
  // the processes of `an_update` without the fetch: the time of the last fetch is that of `FETCH_HEAD` in the
  // directories read at the start
  assert_processes(&git, UPDATE);
}

#[cfg(any(unix, windows))]
#[test]
fn signing_an_update_adds_one_batch_read_and_write() {
  #[cfg(windows)]
  if sync_utils::child_process::windowless_command("ssh-keygen").arg("-?").output().is_err() {
    return;
  }

  let fixture = setup();
  fixture.clone.create_commit("Local change", "local.txt", "local\n");
  fixture.upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  fixture.clone.git(&["fetch", "--quiet", "origin"]);
  fixture.clone.enable_ssh_signing();
  let git = counting();

  run_update(&git, &fixture.clone, false);
  let tip = fixture.clone.head();
  assert_signed_commits(&fixture.clone, &fixture.clone.git(&["rev-parse", "refs/remotes/origin/master"]), &tip);
  let expected = with_added_processes(UPDATE, &[("cat-file", 1), ("rev-list", 1), ("fast-import", 1)]);
  assert_processes(&git, &expected);
}

/// Update Project of `upstream_and_clone_with_twins` after its fetch: the check pairs the local commits once. Before the
/// replay, the pairing reads the local commits (`log`) and the upstream commits by the local authors (`log`), compares
/// the patch ids of both pairs by identity (`diff-tree -p` with `patch-id --stable`, none equal), replays them
/// (`merge-tree --stdin`, one `rev-parse` of the upstream trees), and proves the one whose cherry-pick conflicts by its
/// changed lines (`diff-tree -p -U0` with `patch-id --verbatim`, `diff-tree --raw`). The commit left compares its set of
/// paths with the upstream commits no twin took (`diff-tree --name-only`); the upstream change of the same file has
/// another patch id (`diff-tree -p` with `patch-id --stable`). The replay without the twins conflicts, so the pairing
/// looks for the commits of the same patch by other authors (`rev-list --cherry-mark`, none here), and the
/// resolve by words stops at the real conflict (`var`, `merge-tree`, `check-attr`, `cat-file`): the first conflict,
/// which needs no search. It is described by its merge (`merge-tree`), the commit (`log`) and the upstream commits that
/// change its files (`log`); the outline of the whole branch is gone (one `log` less). A conflict moves nothing: no
/// lock, no witness
#[test]
fn a_conflicting_update_pairs_its_commits_once() {
  let (_upstream, clone, _, _) = TestRepo::upstream_and_clone_with_twins(false);
  clone.git(&["fetch", "--quiet", "origin"]);
  let git = counting();
  let result = update_project_core(
    &git,
    &UpdateProjectParams {
      repository_path: path(&clone).to_string(),
      fetch: false,
    },
    &|_| {},
    LockWait::default(),
  )
  .unwrap();
  assert!(matches!(result.check.state, UpdateState::Conflict { .. }), "{result:?}");
  assert_processes(
    &git,
    &[
      ("cat-file", 2),
      ("check-attr", 1),
      ("config", 2),
      ("diff-tree", 5),
      ("for-each-ref", 1),
      ("log", 4),
      ("merge-base", 2),
      ("merge-tree", 3),
      ("patch-id", 3),
      ("remote", 1),
      ("replay", 1),
      ("rev-list", 2),
      ("rev-parse", 4),
      ("symbolic-ref", 2),
      ("var", 1),
    ],
  );
}

/// A suggested unapply reads the unsupported commit once and checks the branch with the existing dry-run core
#[test]
fn an_unapply_suggestion_reads_its_commit_once() {
  let repo = TestRepo::new();
  let commit = repo.create_commit("(auth) Add login", "login.txt", "login\n");
  let git = counting();
  let suggestion = unapply_branch_suggestion(&git, repo.path_str(), "branch-deck", &commit, LockWait::default());
  assert_eq!(suggestion, None);
  assert_processes(
    &git,
    &[
      ("cat-file", 2),
      ("for-each-ref", 1),
      ("log", 2),
      ("remote", 1),
      ("rev-list", 1),
      ("rev-parse", 1),
      ("symbolic-ref", 1),
    ],
  );
}
