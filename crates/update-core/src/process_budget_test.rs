//! The git processes of the cores of this crate, by subcommand: the numbers behind the batching rule
//! (docs/design/git-operations.md, "Batch"), as `sync_core`'s `process_budget_test` pins those of its cores. A change
//! that runs fewer processes lowers the numbers here; one that runs more has to say why.

use crate::update_project::update_project_test::{path, setup};
use crate::update_project::{UpdateProjectParams, update_project_core};
use history_rewrite::repository_lock::LockWait;
use test_utils::process_budget::{assert_processes, counting};

/// The processes of Update Project after its fetch (`an_update`, `an_update_after_its_fetch`)
const UPDATE: &[(&str, u32)] = &[
  ("cat-file", 1),
  ("config", 1),
  ("diff-tree", 1),
  ("for-each-ref", 1),
  ("log", 3),
  ("merge-base", 2),
  ("read-tree", 2),
  ("remote", 1),
  ("replay", 1),
  ("rev-list", 3),
  ("rev-parse", 5),
  ("symbolic-ref", 5),
  ("update-index", 3),
  ("update-ref", 2),
];

/// `processes` and one `fetch`, sorted by subcommand
fn with_fetch(processes: &[(&'static str, u32)]) -> Vec<(&'static str, u32)> {
  let mut all = processes.to_vec();
  all.push(("fetch", 1));
  all.sort_unstable();
  all
}

/// Update Project: the fetch, the check and the rebase of the local commit onto the upstream, the files following. The
/// directories are read once at the start (one `rev-parse`): the guards, the lock, the pending moves and the index of
/// the dry run take them from there. The check runs before the lock; under it, its witness (the guards, one `rev-parse`
/// of HEAD and the upstream) costs one `rev-parse` and one `symbolic-ref` more. The record of what landed reads HEAD once
/// more under the lock of the landed state (`record_update`), as it runs after the repository lock: one `rev-parse`
#[test]
fn an_update() {
  let fixture = setup();
  fixture.clone.create_commit("(auth) Add login", "login.txt", "login\n");
  fixture.upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  let git = counting();
  let result = update_project_core(
    &git,
    &UpdateProjectParams {
      repository_path: path(&fixture.clone).to_string(),
      fetch: true,
    },
    &|_| {},
    LockWait::default(),
  )
  .unwrap();
  assert!(result.applied.is_some(), "{result:?}");
  assert_processes(&git, &with_fetch(UPDATE));
}

/// Update Project of the app after its `fetch_baseline`: the same update without the fetch, the network outside the
/// slot of the page's rewrites
#[test]
fn an_update_after_its_fetch() {
  let fixture = setup();
  fixture.clone.create_commit("(auth) Add login", "login.txt", "login\n");
  fixture.upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  fixture.clone.git(&["fetch", "--quiet", "origin"]);
  let git = counting();
  let result = update_project_core(
    &git,
    &UpdateProjectParams {
      repository_path: path(&fixture.clone).to_string(),
      fetch: false,
    },
    &|_| {},
    LockWait::default(),
  )
  .unwrap();
  assert!(result.applied.is_some(), "{result:?}");
  // the processes of `an_update` without the fetch: the time of the last fetch is that of `FETCH_HEAD` in the
  // directories read at the start
  assert_processes(&git, UPDATE);
}
