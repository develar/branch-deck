//! The git processes of a detection, by subcommand: one cherry-mark walk per branch, the batch reads once per detection

use super::test_helpers::{detected_branch_info, setup_test_repos};
use branch_integration::detector::{DetectConfig, detect_integrated_branches};
use branch_integration::strategy::DetectionStrategy;
use domain_types::branch_integration::BranchIntegrationStatus;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, HashSet};
use sync_utils::timestamp::utc_today;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::TestReporter;

/// Detects the archived branches of `repo` against `origin/master` with `strategy`; returns the events and the
/// processes by subcommand
fn detect(repo: &TestRepo, strategy: DetectionStrategy) -> (Vec<domain_types::SyncEvent>, BTreeMap<String, u32>) {
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

/// The full name of the branch `name` archived today
fn archived(name: &str) -> String {
  format!("user/archived/{}/{name}", utc_today())
}

/// Archived branches: `integrated` (its commit cherry-picked), `partial` (one of two) and `orphaned` (none); returns
/// their full names
fn archived_branches(repo: &TestRepo) -> [String; 3] {
  let base = repo.head();
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
  // the branches and their notes, the baseline, the stack notes; one cherry-mark walk per branch, which finds the
  // integration commit too; the notes in one `fast-import` with the committer of `var`
  assert_eq!(
    by_command,
    processes(&[("fast-import", 1), ("for-each-ref", 1), ("log", 5), ("rev-parse", 1), ("show-ref", 1), ("var", 1)])
  );

  // every result is cached: nothing is scanned
  let (_, by_command) = detect(&local_repo, DetectionStrategy::Rebase);
  assert_eq!(by_command, processes(&[("for-each-ref", 1), ("log", 1), ("rev-parse", 1)]));
}

/// Two more archived branches without an equivalent commit, for the squash fallback: `unmerged` (its change is not in
/// the baseline) and `squashed` (its two commits are one baseline commit with the subject of its tip); returns their
/// full names and the squash commit
fn squash_candidates(repo: &TestRepo) -> ([String; 2], String) {
  let baseline = repo.head();
  let names = [archived("unmerged"), archived("squashed")];
  let unmerged = repo.create_commit("Add five", "five.rs", "// five");
  repo.create_branch_at(&names[0], &unmerged).unwrap();
  repo.reset_hard(&baseline).unwrap();
  repo.create_commit("Add six", "six.rs", "// six");
  let squashed = repo.create_commit("Add seven", "seven.rs", "// seven");
  repo.create_branch_at(&names[1], &squashed).unwrap();
  repo.reset_hard(&baseline).unwrap();
  let squash = repo.create_commit_with_files("Add seven", &[("six.rs", "// six"), ("seven.rs", "// seven")]);
  repo.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
  (names, squash)
}

/// The squash fallback of the full detection, for the three branches without an equivalent commit: one `merge-tree`
/// each, the baseline tree read once, the subject of the tip known from the branch listing
#[test]
fn detecting_archived_branches_in_full() {
  let (_upstream_repo, local_repo, _) = setup_test_repos();
  let [_, _, orphaned] = archived_branches(&local_repo);
  let ([unmerged, squashed], squash) = squash_candidates(&local_repo);

  let (events, by_command) = detect(&local_repo, DetectionStrategy::All);

  let status = |name: &str| detected_branch_info(&events, name).status.clone();
  for name in [&orphaned, &unmerged] {
    assert!(matches!(status(name), BranchIntegrationStatus::NotIntegrated { integrated_count: 0, .. }), "{name}");
  }
  let squashed = status(&squashed);
  let BranchIntegrationStatus::Integrated {
    commit_count: 2,
    commit: Some(commit),
    ..
  } = &squashed
  else {
    panic!("{squashed:?}");
  };
  assert_eq!(commit.id, squash);

  assert_eq!(
    by_command,
    processes(&[
      ("fast-import", 1),
      ("for-each-ref", 2),
      ("log", 8),
      ("merge-tree", 3),
      ("rev-parse", 2),
      ("show-ref", 1),
      ("var", 1),
    ])
  );
}
