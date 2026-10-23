//! The git processes of the cores of this crate, by subcommand: the numbers behind the batching rule
//! (docs/design/git-operations.md, "Batch"), as `sync_core`'s `process_budget_test` pins those of its cores. A change
//! that runs fewer processes lowers the numbers here; one that runs more has to say why.

use crate::update_project::update_project_test::{path, setup};
use crate::update_project::{UpdateProjectParams, update_project_core};
use test_utils::process_budget::{assert_processes, counting};

/// Update Project: the fetch, the check and the rebase of the local commit onto the upstream, the files following. The
/// check runs before the lock; under it, its witness (the guards, one `rev-parse` of HEAD and the upstream) costs two
/// `rev-parse` and one `symbolic-ref` more. The record of what landed reads HEAD once more under the lock of the
/// landed state (`record_update`), as it runs after the repository lock: one `rev-parse`
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
    },
    &|_| {},
  )
  .unwrap();
  assert!(result.applied.is_some(), "{result:?}");
  assert_processes(
    &git,
    &[
      ("cat-file", 1),
      ("config", 1),
      ("diff-tree", 1),
      ("fetch", 1),
      ("for-each-ref", 1),
      ("log", 3),
      ("merge-base", 2),
      ("read-tree", 2),
      ("remote", 2),
      ("replay", 1),
      ("rev-list", 3),
      ("rev-parse", 13),
      ("symbolic-ref", 6),
      ("update-index", 3),
      ("update-ref", 2),
    ],
  );
}
