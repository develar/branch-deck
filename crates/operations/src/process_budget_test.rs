use crate::events::NoEvents;
use crate::push::push_test::{OFFLINE, registry_of};
use crate::rewrite::execute_rewrite;
use crate::witness::read_refs;
use domain_types::operation::Outcome;
use domain_types::rewrite::{ParkBranchLine, RewriteIntent};
use pretty_assertions::assert_eq;
use std::sync::Arc;
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::{alpha_and_beta_on_origin, assert_processes, counting};

/// The witness of a plan reads the tips of every ref it names with one `for-each-ref`, under the lock too
#[test]
fn a_witness_reads_its_refs_with_one_process() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test\n");
  let git = counting();
  let refs: Vec<String> = ["auth", "login", "ui"]
    .iter()
    .flat_map(|name| [format!("refs/heads/user/virtual/{name}"), format!("refs/remotes/origin/user/virtual/{name}")])
    .collect();
  read_refs(&git, repo.path_str(), &refs).unwrap();
  assert_processes(&git, &[("for-each-ref", 1)]);
}

/// The git processes of the operation of `outcome`, by subcommand (`OperationStats`, counted wherever it ran)
fn processes_of(outcome: &Outcome) -> Vec<(&str, u32)> {
  outcome.stats.git_processes.iter().map(|(subcommand, count)| (subcommand.as_str(), *count)).collect()
}

/// A rewrite of the engine costs what its core costs and one `for-each-ref` of the checked out branch and the three
/// stores, its witness (`rewrite::read_witness`); a core that moves HEAD's branch reads its own witness under the lock
/// (one more `for-each-ref`); the sync it asks runs on the actor of the session, outside it. A park
/// and an unpark of alpha and beta (the core's processes: `a_park_and_an_unpark` of sync-core), a squash of two commits
/// of alpha (`a_squash`) and its undo, which reads no witness
#[tokio::test(flavor = "multi_thread")]
async fn a_rewrite_costs_its_core_and_one_read_of_its_witness() {
  let (repo, commits) = alpha_and_beta_on_origin();
  let (registry, key) = registry_of(&repo, true).await;
  let witness = || async { registry.sessions().read(&key, |state, _| state.header().witness()).await.unwrap() };
  let rewrite = |rewrite: RewriteIntent| execute_rewrite(&registry, &key, rewrite, OFFLINE.hosts(), Arc::new(NoEvents));
  let line = |branch: &str, note: &str| ParkBranchLine {
    branch_name: branch.to_owned(),
    note: note.to_owned(),
  };
  let parked = rewrite(RewriteIntent::Park {
    branches: vec![line("alpha", "later"), line("beta", "")],
    witness: witness().await,
  })
  .await
  .unwrap();
  assert_eq!(
    processes_of(&parked),
    [
      ("cat-file", 1),
      ("config", 1),
      ("fast-import", 1),
      ("for-each-ref", 2),
      ("log", 1),
      ("remote", 1),
      ("rev-parse", 2),
      ("update-ref", 1),
      ("var", 1),
    ]
  );
  let unparked = rewrite(RewriteIntent::Unpark {
    branch_names: vec!["alpha".to_owned(), "beta".to_owned()],
    witness: witness().await,
  })
  .await
  .unwrap();
  assert_eq!(
    processes_of(&unparked),
    [("cat-file", 1), ("fast-import", 1), ("for-each-ref", 1), ("rev-parse", 1), ("update-ref", 1), ("var", 1)]
  );

  let squashed = rewrite(RewriteIntent::Squash {
    commit_ids: vec![commits[0].clone(), commits[2].clone()],
    message: "(alpha) Alpha 1 and 2".to_owned(),
    witness: witness().await,
  })
  .await
  .unwrap();
  assert_eq!(
    processes_of(&squashed),
    [
      ("cat-file", 1),
      ("fast-import", 2), // the squashed commit; the mapping notes of the commits after it, carried
      ("for-each-ref", 2),
      ("log", 2), // the history with its notes; the new one with its trees
      ("merge-tree", 1),
      ("notes", 1),
      ("replay", 2),
      ("rev-parse", 2),
      ("update-ref", 1),
      ("var", 1),
    ]
  );
  let undone = rewrite(RewriteIntent::Undo { token: squashed.undo.unwrap() }).await.unwrap();
  // the restore of HEAD, as the app's undo of a squash: its checks, then under the lock the core's witness of HEAD and
  // the move; the engine reads no witness for an undo
  assert_eq!(processes_of(&undone), [("for-each-ref", 1), ("rev-list", 1), ("rev-parse", 3), ("update-ref", 1)]);
}
