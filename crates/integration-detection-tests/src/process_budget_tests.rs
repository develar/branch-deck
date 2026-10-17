//! The git processes of a detection, by subcommand: one cherry-mark walk per branch, the batch reads once per detection

use super::test_helpers::{detected_branch_info, setup_test_repos};
use branch_integration::detector::{DetectConfig, detect_integrated_branches};
use branch_integration::strategy::DetectionStrategy;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, HashSet};
use sync_test_utils::TestReporter;
use sync_types::branch_integration::BranchIntegrationStatus;
use sync_utils::timestamp::utc_today;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// Detects the archived branches of `repo` against `origin/master` with `strategy`; returns the events and the
/// processes by subcommand
fn detect(repo: &TestRepo, strategy: DetectionStrategy) -> (Vec<sync_types::SyncEvent>, BTreeMap<String, u32>) {
  let git = GitCommandExecutor::new().with_process_stats("test");
  let progress = TestReporter::new();
  let active_branches = HashSet::new();
  let config = DetectConfig {
    active_branches: &active_branches,
    progress: &progress,
    strategy,
    retention_days: 7,
  };
  detect_integrated_branches(&git, repo.path().to_str().unwrap(), "user", "origin/master", config).unwrap();
  let processes = git
    .process_stats()
    .unwrap()
    .by_command()
    .into_iter()
    .map(|(command, stats)| (command, stats.count))
    .collect();
  (progress.get_events(), processes)
}

fn processes(counts: &[(&str, u32)]) -> BTreeMap<String, u32> {
  counts.iter().map(|(command, count)| ((*command).to_string(), *count)).collect()
}

/// Archived branches: `integrated` (its commit cherry-picked), `partial` (one of two) and `orphaned` (none); returns
/// their full names
fn archived_branches(repo: &TestRepo) -> [String; 3] {
  let base = repo.head();
  let archived = |name: &str| format!("user/archived/{}/{name}", utc_today());
  let names = [archived("integrated"), archived("partial"), archived("orphaned")];
  let integrated = repo.create_commit("Add one", "one.rs", "// one");
  repo.create_branch_at(&names[0], &integrated).unwrap();
  repo.reset_hard(&base).unwrap();
  repo.create_commit("Add two", "two.rs", "// two");
  let partial = repo.create_commit("Add three", "three.rs", "// three");
  repo.create_branch_at(&names[1], &partial).unwrap();
  repo.reset_hard(&base).unwrap();
  let orphaned = repo.create_commit("Add four", "four.rs", "// four");
  repo.create_branch_at(&names[2], &orphaned).unwrap();
  repo.reset_hard(&base).unwrap();
  // the baseline takes `one` and `two` with other messages
  repo.create_commit("Take one", "one.rs", "// one");
  repo.create_commit("Take two", "two.rs", "// two");
  repo.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
  names
}

#[test]
fn detecting_archived_branches() {
  let (_upstream_repo, local_repo, _) = setup_test_repos();
  let [integrated, partial, orphaned] = archived_branches(&local_repo);

  let (events, by_command) = detect(&local_repo, DetectionStrategy::Rebase);

  let status = |name: &str| detected_branch_info(&events, name).status.clone();
  assert!(matches!(
    status(&integrated),
    BranchIntegrationStatus::Integrated {
      commit_count: 1,
      commit: Some(_),
      ..
    }
  ));
  assert!(matches!(
    status(&partial),
    BranchIntegrationStatus::NotIntegrated {
      total_commit_count: 2,
      integrated_count: 1,
      integrated_at: Some(_),
      ..
    }
  ));
  assert!(matches!(status(&orphaned), BranchIntegrationStatus::NotIntegrated { integrated_count: 0, .. }));
  // the branches and their notes, the baseline, the stack notes; a cherry-mark walk per branch, and one more for the
  // integration commit of the two with an equivalent; the notes in one `fast-import` with the committer of `var`
  assert_eq!(
    by_command,
    processes(&[
      ("fast-import", 1),
      ("for-each-ref", 1),
      ("log", 2),
      ("rev-list", 5),
      ("rev-parse", 1),
      ("show-ref", 1),
      ("var", 1)
    ])
  );

  // every result is cached: nothing is scanned
  let (_, by_command) = detect(&local_repo, DetectionStrategy::Rebase);
  assert_eq!(by_command, processes(&[("for-each-ref", 1), ("log", 1), ("rev-parse", 1)]));
}

/// The squash fallback of the full detection, for the branch without an equivalent commit
#[test]
fn detecting_archived_branches_in_full() {
  let (_upstream_repo, local_repo, _) = setup_test_repos();
  archived_branches(&local_repo);

  let (_, by_command) = detect(&local_repo, DetectionStrategy::All);

  assert_eq!(
    by_command,
    processes(&[
      ("diff", 1),
      ("fast-import", 1),
      ("for-each-ref", 2),
      ("log", 2),
      ("merge-base", 1),
      ("merge-tree", 1),
      ("rev-list", 5),
      ("rev-parse", 2),
      ("show-ref", 1),
      ("var", 1),
    ])
  );
}
