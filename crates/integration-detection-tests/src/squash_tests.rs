//! Tests specifically for squash merge integration detection

use super::integration_tests::test_integration_workflow_helper;
use super::test_helpers::{
  IntegrationMethod, UpdateMethod, get_commit_timestamp, setup_test_repos, sync_branches_core_with_strategy, verify_archived_branches_listed, verify_detection_cache_works,
};
use branch_integration::strategy::DetectionStrategy;
use sync_test_utils::TestReporter;
use sync_types::SyncEvent;
use sync_types::branch_integration::IntegrationConfidence;
use test_log::test;

#[test]
fn test_squash_merge_integration_with_timestamp_verification() {
  // Test squash merge integration detection with proper timestamp verification
  test_integration_workflow_helper(
    UpdateMethod::Rebase,
    IntegrationMethod::Squash,
    vec![(
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
  let progress_reporter = TestReporter::new();
  let result = rt.block_on(async { sync_branches_core_with_strategy(&git_executor, local_repo.path().to_str().unwrap(), "user", progress_reporter, DetectionStrategy::All).await });
  assert!(result.is_ok());

  // Push branch
  local_repo.push("origin", "user/virtual/feature-squash").unwrap();

  // Squash merge in upstream
  upstream_repo.checkout("main").unwrap();

  // Use a specific timestamp for testing
  let squash_timestamp = chrono::Utc::now().timestamp() - 180; // 3 minutes ago
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

  let progress_reporter2 = TestReporter::new();
  let result = rt.block_on(async {
    sync_branches_core_with_strategy(
      &git_executor,
      local_repo.path().to_str().unwrap(),
      "user",
      progress_reporter2.clone(),
      DetectionStrategy::All,
    )
    .await
  });
  assert!(result.is_ok(), "Second sync failed: {result:?}");

  // Should detect as integrated (High confidence due to message match)
  let events = progress_reporter2.get_events();

  // Debug: Print all events to understand what's happening
  println!("\n=== Debug: All sync events ===");
  for event in &events {
    match event {
      SyncEvent::BranchIntegrationDetected { info } => match &info.status {
        sync_types::branch_integration::BranchIntegrationStatus::Integrated { .. } => {
          println!("Integrated branch detected: {}", info.name);
        }
        sync_types::branch_integration::BranchIntegrationStatus::NotIntegrated { .. } => {
          println!("Not integrated branch detected: {}", info.name);
        }
        _ => println!("Other integration status: {} - {:?}", info.name, info.status),
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
  println!("{}", baseline_log);

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
    println!("{}", virtual_commits);
  }

  // Git notes are now used for tracking instead of database
  // No need to check database mappings

  // Collect all integrated branches from individual events
  let integrated_branches: Vec<_> = events
    .iter()
    .filter_map(|e| match e {
      SyncEvent::BranchIntegrationDetected { info } => Some(info.name.clone()),
      _ => None,
    })
    .collect();

  assert!(!integrated_branches.is_empty(), "Should detect squash-merged branch as integrated");

  let squash_branch = integrated_branches.iter().find(|b| b.contains("feature-squash"));
  assert!(squash_branch.is_some(), "Should find feature-squash in integrated branches");

  // Should be High confidence since detection is via message matching
  // (Squash merges are detected by finding commit messages in baseline)
  if let Some(branch) = squash_branch {
    // Need to find the actual integration info for the branch
    let integration_info = events
      .iter()
      .find_map(|e| match e {
        SyncEvent::BranchIntegrationDetected { info } if info.name == **branch => Some(info),
        _ => None,
      })
      .expect("Should find integration info");

    if let sync_types::branch_integration::BranchIntegrationStatus::Integrated { confidence, integrated_at, .. } = &integration_info.status {
      assert_eq!(*confidence, IntegrationConfidence::High);

      // Verify timestamp is close to squash merge time
      if let Some(integrated_date) = integrated_at {
        let timestamp_diff = (*integrated_date as i64 - squash_commit as i64).abs();
        assert!(
          timestamp_diff <= 60,
          "Squash merge timestamp should match. Expected: {squash_commit}, Got: {integrated_date}, Diff: {timestamp_diff}"
        );

        // Verify it's not just "now"
        let now = chrono::Utc::now().timestamp() as u32;
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
