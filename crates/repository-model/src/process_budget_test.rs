//! The git processes of the checks of a start, by subcommand (docs/design/git-operations.md, "Batch"): the start right
//! after the check of its popover reuses that check (`RepositoryViews::landing_check`) instead of a session and a trial
//! of its own

use crate::views::RepositoryViews;
use domain_types::patronus::LandingAllowances;
use object_cache::cache::ObjectCache;
use std::collections::HashSet;
use sync_core::sync::{SyncOptions, sync_branches};
use test_utils::process_budget::{alpha_and_beta_on_origin, assert_processes, counting, processes};
use test_utils::sync_fixtures::TestReporter;
use trial_check::landing_gate::LandingGateParams;

const PREFIX: &str = "test";

/// The check of the popover runs its session and its trial once; the start's check of the same inputs runs the two
/// processes of the inputs (the refs and the config), and a commit in between is another input, checked anew
#[tokio::test]
async fn the_landing_check_of_a_start_reuses_the_check_of_its_popover() {
  let (repo, _) = alpha_and_beta_on_origin();
  sync_branches(&counting(), repo.path_str(), PREFIX, TestReporter::new(), SyncOptions::default())
    .await
    .unwrap();
  let views = RepositoryViews::default();
  let params = LandingGateParams {
    repository_path: repo.path_str().to_string(),
    branch_name: "alpha".to_string(),
    stack: false,
    allow: LandingAllowances::default(),
  };
  let check = |landed: &HashSet<String>| {
    let git = counting();
    let check = views.landing_check(&git, &ObjectCache::disabled(), PREFIX, &params, landed).unwrap();
    (check, git)
  };

  // the inputs, the session (the snapshot, the config, the tips) and the trial, which reuses the copies of the sync
  const SESSION: [(&str, u32); 6] = [("cat-file", 2), ("config", 3), ("for-each-ref", 3), ("log", 5), ("remote", 1), ("rev-parse", 4)];
  let (popover, git) = check(&HashSet::new());
  assert_processes(&git, &SESSION);

  let (start, git) = check(&HashSet::new());
  assert_eq!(start, popover);
  assert_processes(&git, &[("config", 1), ("for-each-ref", 1)]);

  // other landed commits are another input
  let (_, git) = check(&HashSet::from(["a".repeat(40)]));
  assert_processes(&git, &SESSION);

  // so is a commit: its copy is built in the trial
  repo.create_commit("(alpha) Alpha 4", "alpha-4.txt", "alpha\n");
  let (_, git) = check(&HashSet::new());
  assert!(processes(&git).iter().any(|(subcommand, _)| subcommand == "merge-tree"), "{:?}", processes(&git));
}
