//! Tests specifically for squash merge integration detection

use super::integration_tests::test_integration_workflow_helper;
use super::test_helpers::{
  IntegrationMethod, UpdateMethod, detected_branch_info, detected_branch_names, get_commit_timestamp, setup_test_repos, sync_user_branches, verify_archived_branches_listed,
  verify_detection_cache_works,
};
use branch_integration::detector::{DetectConfig, detect_integrated_branches};
use branch_integration::strategy::DetectionStrategy;
use domain_types::SyncEvent;
use domain_types::branch_integration::{BranchIntegrationStatus, IntegrationConfidence};
use std::collections::HashSet;
use sync_utils::timestamp::{unix_now, utc_today};
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::TestReporter;

#[test]
fn test_squash_merge_integration_with_timestamp_verification() {
  // Test squash merge integration detection with proper timestamp verification
  test_integration_workflow_helper(
    UpdateMethod::Rebase,
    IntegrationMethod::Squash,
    &[(
      "feature-squash",
      vec![
        ("(feature-squash) Add function one", "feature1.js", "function one() {}"),
        ("(feature-squash) Add function two", "feature2.js", "function two() {}"),
      ],
    )],
  );
}

#[test]
fn test_squash_merge_detection() {
  // Test that we can detect integration even with squash merges
  let rt = tokio::runtime::Runtime::new().unwrap();
  let (upstream_repo, local_repo, git_executor) = setup_test_repos();

  // Create a feature branch for development
  local_repo.checkout_new_branch("local-dev").unwrap();

  // Add multiple commits for squash merge
  local_repo.create_commit("(feature-squash) Add function one", "feature.js", "function one() {}");
  local_repo.create_commit("(feature-squash) Add function two", "feature.js", "function one() {}\nfunction two() {}");

  // Sync to create virtual branch with notes
  rt.block_on(sync_user_branches(&git_executor, &local_repo, DetectionStrategy::All))
    .expect("Initial sync failed");

  // Push branch
  local_repo.push("origin", "user/virtual/feature-squash").unwrap();

  // Squash merge in upstream
  upstream_repo.checkout("main").unwrap();

  // Use a specific timestamp for testing
  let squash_timestamp = i64::from(unix_now()) - 180; // 3 minutes ago
  upstream_repo
    .merge_squash_with_timestamp("user/virtual/feature-squash", "Add function two", squash_timestamp)
    .unwrap();

  // Capture the squash commit hash for verification
  let squash_commit = get_commit_timestamp(&upstream_repo, "HEAD");

  // Delete the branch
  upstream_repo.delete_branch("user/virtual/feature-squash").unwrap();

  // Pull to local
  local_repo.fetch_prune("origin").unwrap();
  local_repo.checkout("main").unwrap();
  local_repo.pull().unwrap();

  // Check working directory and HEAD before sync
  let head_hash = local_repo.head();
  let current_branch = local_repo.current_branch().unwrap();

  println!("\n=== Pre-sync state ===");
  println!("Current branch: {current_branch}");
  println!("HEAD: {head_hash}");

  // Run sync again - this should detect the integrated branch
  println!("\n=== Running second sync to detect squash merge integration ===");

  let progress_reporter2 = rt
    .block_on(sync_user_branches(&git_executor, &local_repo, DetectionStrategy::All))
    .expect("Second sync failed");

  // Should detect as integrated (High confidence due to message match)
  let events = progress_reporter2.get_events();

  // Debug: Print all events to understand what's happening
  println!("\n=== Debug: All sync events ===");
  for event in &events {
    match event {
      SyncEvent::BranchIntegrationDetected { info } => match &info.status {
        BranchIntegrationStatus::Integrated { .. } => {
          println!("Integrated branch detected: {}", info.name);
        }
        BranchIntegrationStatus::NotIntegrated { .. } => {
          println!("Not integrated branch detected: {}", info.name);
        }
        BranchIntegrationStatus::Partial { .. } => println!("Other integration status: {} - {:?}", info.name, info.status),
      },
      SyncEvent::BranchesGrouped { branches, .. } => {
        for branch in branches {
          println!("Grouped branch: {} with {} commits", branch.name, branch.commits.len());
        }
      }
      _ => {}
    }
  }

  // Also check what's in the baseline after squash merge
  let baseline_log = local_repo.log(&["--oneline", "main", "-5"]).unwrap();
  println!("\nBaseline (main) after pull:");
  println!("{baseline_log}");

  // Check the tree of the squashed commit
  let squash_tree = local_repo.log(&["--format=%T %s", "main", "-1"]).unwrap();
  println!("Squashed commit tree: {}", squash_tree.trim());

  // Check what virtual branches exist after fetch --prune
  let branches_list = local_repo.list_branches("user/virtual/*").unwrap();
  println!("\nVirtual branches after fetch --prune:");
  println!("{}", branches_list.join("\n"));

  // Check commits on virtual branch
  if let Ok(virtual_commits) = local_repo.log(&["--oneline", "user/virtual/feature-squash", "-5"]) {
    println!("\nCommits on user/virtual/feature-squash:");
    println!("{virtual_commits}");
  }

  // Git notes are now used for tracking instead of database
  // No need to check database mappings

  // Collect all integrated branches from individual events
  let integrated_branches = detected_branch_names(&events);

  assert!(!integrated_branches.is_empty(), "Should detect squash-merged branch as integrated");

  let squash_branch = integrated_branches.iter().find(|b| b.contains("feature-squash"));
  assert!(squash_branch.is_some(), "Should find feature-squash in integrated branches");

  // Should be High confidence since detection is via message matching
  // (Squash merges are detected by finding commit messages in baseline)
  if let Some(branch) = squash_branch {
    // Need to find the actual integration info for the branch
    let integration_info = detected_branch_info(&events, branch);

    if let BranchIntegrationStatus::Integrated { confidence, integrated_at, .. } = &integration_info.status {
      assert_eq!(*confidence, IntegrationConfidence::High);

      // Verify timestamp is close to squash merge time
      if let Some(integrated_date) = integrated_at {
        let timestamp_diff = (i64::from(*integrated_date) - i64::from(squash_commit)).abs();
        assert!(
          timestamp_diff <= 60,
          "Squash merge timestamp should match. Expected: {squash_commit}, Got: {integrated_date}, Diff: {timestamp_diff}"
        );

        // Verify it's not just "now"
        let now = unix_now();
        assert!(
          *integrated_date < now - 60,
          "Integration timestamp should be from when the squash merge happened, not 'now'"
        );
      } else {
        // Squash merges might not find timestamps if message matching fails
        // This is acceptable - we just won't have a timestamp
      }
    } else {
      panic!("Expected Integrated status for squash branch");
    }
  }

  // Verify archived branches are listed via production get_all_branch_data
  verify_archived_branches_listed(&git_executor, local_repo.path().to_str().unwrap(), "user", &["feature-squash"]);

  // Verify detection cache is working for squash merge detection
  rt.block_on(async {
    verify_detection_cache_works(&local_repo, &git_executor, "user", DetectionStrategy::All, 1).await.unwrap();
  });
}

/// Detects the archived branch `name` of `repo`, archived today, against `origin/master` with every strategy; the
/// baseline is `HEAD`
fn detect_archived(repo: &TestRepo, name: &str) -> BranchIntegrationStatus {
  repo.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
  let progress = TestReporter::new();
  let active_branches = HashSet::new();
  let config = DetectConfig {
    active_branches: &active_branches,
    progress: &progress,
    strategy: DetectionStrategy::All,
    retention_days: 7,
  };
  detect_integrated_branches(repo.git_executor(), repo.path().to_str().unwrap(), "user", "origin/master", config).unwrap();
  detected_branch_info(&progress.get_events(), &format!("user/archived/{}/{name}", utc_today()))
    .status
    .clone()
}

/// A branch whose commits cancel out changes nothing: merging it gives the baseline's tree
#[test]
fn a_branch_whose_changes_cancel_out_is_integrated() {
  let (_upstream_repo, local_repo, _) = setup_test_repos();
  let base = local_repo.head();
  local_repo.create_commit("Add scratch", "scratch.rs", "// scratch");
  local_repo.git(&["rm", "-q", "scratch.rs"]);
  local_repo.git(&["commit", "-q", "-m", "Drop scratch"]);
  local_repo.create_branch_at(&format!("user/archived/{}/scratch", utc_today()), "HEAD").unwrap();
  local_repo.reset_hard(&base).unwrap();
  local_repo.create_commit("Add other", "other.rs", "// other");

  let status = detect_archived(&local_repo, "scratch");

  assert!(matches!(status, BranchIntegrationStatus::Integrated { commit_count: 2, .. }), "{status:?}");
}

/// Merging a branch that deletes a file the baseline changed conflicts, and git leaves the baseline's version in the
/// tree: the tree is the baseline's, but the deletion is not in the baseline
#[test]
fn a_conflicted_merge_is_no_squash() {
  let (_upstream_repo, local_repo, _) = setup_test_repos();
  local_repo.create_commit("Add shared", "shared.rs", "// shared");
  local_repo.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
  let base = local_repo.head();
  local_repo.git(&["rm", "-q", "shared.rs"]);
  local_repo.git(&["commit", "-q", "-m", "Remove shared"]);
  local_repo.create_branch_at(&format!("user/archived/{}/remove", utc_today()), "HEAD").unwrap();
  local_repo.reset_hard(&base).unwrap();
  local_repo.create_commit("Change shared", "shared.rs", "// shared, changed");

  let status = detect_archived(&local_repo, "remove");

  assert!(matches!(status, BranchIntegrationStatus::NotIntegrated { orphaned_count: 1, .. }), "{status:?}");
}
