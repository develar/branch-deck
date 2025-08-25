use super::test_helpers::{setup_test_repos, sync_branches_core_with_strategy, verify_detection_cache_works};
use branch_integration::archive::get_archived_branch_commits;
use branch_integration::strategy::DetectionStrategy;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::to_final_branch_name;
use pretty_assertions::assert_eq;
use sync_core::sync::sync_branches_core;
use sync_test_utils::TestReporter;
use sync_types::SyncEvent;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use tracing::debug;

use super::test_helpers::{
  BranchTestData, IntegrationMethod, UpdateMethod, get_commit_timestamp, test_partial_integration_detection_with_strategy, verify_archived_branches_listed,
};

/// Helper function for integration workflow tests
/// Tests the complete workflow of creating commits, syncing, pushing, integrating upstream, and detecting integration
pub fn test_integration_workflow_helper(update_method: UpdateMethod, integration_method: IntegrationMethod, branch_names: BranchTestData) {
  let rt = tokio::runtime::Runtime::new().unwrap();
  let (upstream_repo, local_repo, git_executor) = setup_test_repos();

  // Step 3: Create a feature branch for development
  local_repo.checkout_new_branch("local-dev").unwrap();

  // Step 4: Add commits with prefixes
  for (_branch_name, commits) in &branch_names {
    for (subject, filename, content) in commits {
      local_repo.create_commit(subject, filename, content);
    }
  }

  // Step 5: Run sync to create virtual branches
  let progress_reporter = TestReporter::new();
  let result = rt.block_on(async { sync_branches_core_with_strategy(&git_executor, local_repo.path().to_str().unwrap(), "user", progress_reporter, DetectionStrategy::All).await });
  assert!(result.is_ok(), "Initial sync should succeed");

  // Verify branches were created
  for (branch_name, _) in &branch_names {
    let full_branch_name = to_final_branch_name("user", branch_name).unwrap();
    let branches = local_repo.list_branches(&full_branch_name).unwrap();
    assert!(
      !branches.is_empty() && branches.iter().any(|b| b.contains(&full_branch_name)),
      "Branch {branch_name} should be created"
    );
  }

  // Verify mappings were created for first branch in database
  if let Some((first_branch, _)) = branch_names.first() {
    let full_branch_name = to_final_branch_name("user", first_branch).unwrap();
    // Git notes are now used for tracking instead of database
    // Verify branch was created
    assert!(local_repo.branch_exists(&full_branch_name), "Virtual branch should exist");
  }

  // Step 6: Push virtual branches to upstream
  for (branch_name, _) in &branch_names {
    let full_branch_name = to_final_branch_name("user", branch_name).unwrap();
    local_repo.push("origin", &full_branch_name).unwrap();
  }

  // Step 7: Integrate branches in upstream and capture timestamps
  upstream_repo.checkout("main").unwrap();

  let mut integration_timestamps = std::collections::HashMap::new();
  for (branch_name, _) in &branch_names {
    let full_branch_name = to_final_branch_name("user", branch_name).unwrap();

    match integration_method {
      IntegrationMethod::Merge => {
        // --no-ff merge
        upstream_repo.merge_no_ff(&full_branch_name, &format!("Merge {branch_name}")).unwrap();
      }
      IntegrationMethod::Rebase => {
        // Cherry-pick all commits from the branch
        let log_output = upstream_repo.log(&["--reverse", "--format=%H", &format!("main..{full_branch_name}")]).unwrap();
        let commits: Vec<&str> = log_output.lines().filter(|line| !line.trim().is_empty()).collect();

        for commit in commits {
          upstream_repo.cherry_pick(commit).unwrap();
        }
      }
      IntegrationMethod::Squash => {
        // Get the tip commit subject from the branch to match squash detection logic
        let subject_output = upstream_repo.log(&["-1", "--format=%s", &full_branch_name]).unwrap();
        let subject = subject_output.trim();

        // Extract the stripped subject (remove prefix)
        let stripped_subject = if let Some(start) = subject.find(") ") { &subject[start + 2..] } else { subject };

        // Squash merge with the stripped subject
        upstream_repo.merge_squash(&full_branch_name, stripped_subject).unwrap();
      }
    }

    // Capture the integration timestamp
    let integration_timestamp = get_commit_timestamp(&upstream_repo, "HEAD");
    integration_timestamps.insert(branch_name.to_string(), integration_timestamp);

    // Delete integrated branch
    upstream_repo.delete_branch(&full_branch_name).unwrap();
  }

  // Step 8: Pull changes to local
  local_repo.fetch_prune("origin").unwrap();
  local_repo.checkout("main").unwrap();

  // Update based on method
  match update_method {
    UpdateMethod::Rebase => {
      if let Err(e) = local_repo.rebase("origin/main") {
        debug!(
          error = %e,
          update_method = ?update_method,
          "rebase command output"
        );
      }
    }
    UpdateMethod::Merge => {
      if let Err(e) = local_repo.merge_ff_only("origin/main") {
        debug!(
          error = %e,
          update_method = ?update_method,
          "merge command output"
        );
      }
    }
  }

  // Step 9: Run sync again - should detect integrated branches
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
  assert!(result.is_ok(), "Second sync should succeed");

  // Check for integrated branches detection
  let events = progress_reporter2.get_events();

  // Collect all integrated branches from individual events
  let integrated_branches: Vec<_> = events
    .iter()
    .filter_map(|e| match e {
      SyncEvent::BranchIntegrationDetected { info } if matches!(info.status, sync_types::branch_integration::BranchIntegrationStatus::Integrated { .. }) => Some(info.name.clone()),
      _ => None,
    })
    .collect();

  assert_eq!(integrated_branches.len(), branch_names.len(), "Should detect all branches as integrated");

  // Verify each branch is detected with correct timestamp
  for (branch_name, _) in &branch_names {
    // The integrated branch name will be the archived path, so check if it contains the simple name
    let found = integrated_branches.iter().find(|b| b.contains(branch_name));
    assert!(found.is_some(), "Branch {branch_name} should be detected as integrated");

    if let Some(integrated_branch) = found {
      let expected_timestamp = integration_timestamps.get(*branch_name).unwrap();

      // Check if we have a timestamp
      // Find the integration info to get the timestamp
      let integration_info = events
        .iter()
        .find_map(|e| match e {
          SyncEvent::BranchIntegrationDetected { info } if info.name == **integrated_branch => Some(info),
          _ => None,
        })
        .expect("Should find integration info");

      if let sync_types::branch_integration::BranchIntegrationStatus::Integrated { integrated_at, .. } = &integration_info.status {
        if let Some(integrated_date) = integrated_at {
          // Allow some tolerance (within 60 seconds) for timestamp detection
          let timestamp_diff = (*integrated_date as i64 - *expected_timestamp as i64).abs();

          assert!(
            timestamp_diff <= 60,
            "Branch {branch_name} integration timestamp should match integration commit timestamp. Expected: {expected_timestamp}, Got: {integrated_date}, Diff: {timestamp_diff}"
          );

          // Verify it's not just "now" (should be from the past, not the current time)
          // Since the test runs quickly, we allow for very small time differences
          let now = chrono::Utc::now().timestamp() as u32;

          // The integration happened in the past (during the test), so it should be at least 0 seconds ago
          // and not in the future
          assert!(*integrated_date <= now, "Integration timestamp should not be in the future");

          // It should be reasonably close to the expected timestamp, not just "now"
          // If the difference is less than 5 seconds from expected, it's the actual merge time
          assert!(
            timestamp_diff <= 5,
            "Integration timestamp is too far from the actual merge time ({timestamp_diff}s difference)"
          );
        } else {
          panic!("Expected integration timestamp for branch {branch_name}");
        }
      } else {
        panic!("Expected Integrated status for branch {branch_name}");
      }
    }
  }

  // Verify branches were archived
  let archived_branches = local_repo.list_branches("user/archived/*").unwrap();
  let archived = archived_branches.join("\n");
  for (branch_name, commits) in &branch_names {
    assert!(archived.contains(&format!("/{branch_name}")), "{branch_name} should be archived");

    // Test that we can get commits from the archived branch
    // Find the actual archived branch name (includes date prefix)
    let archived_lines: Vec<&str> = archived.lines().collect();
    let archived_branch = archived_lines.iter().find(|line| line.contains(branch_name)).expect("Should find archived branch");

    // Extract the full archived branch name
    let full_archived_branch_name = archived_branch.trim().strip_prefix("* ").unwrap_or(archived_branch.trim());

    // Pass the full archived branch name (e.g., "user/archived/2025-08-11/feature-auth")
    // Note: The test syncs against origin/master initially, not origin/main
    let retrieved_commits = get_archived_branch_commits(
      &git_executor,
      local_repo.path().to_str().unwrap(),
      full_archived_branch_name,
      "origin/master", // The test syncs against origin/master (git default)
    );

    // The archived branch should still exist and have commits
    assert!(retrieved_commits.is_ok(), "Should be able to get commits from archived branch {branch_name}");
    let retrieved = retrieved_commits.unwrap();
    assert_eq!(retrieved.len(), commits.len(), "Archived branch {} should have {} commits", branch_name, commits.len());

    // Verify the commit messages match what we created
    for (i, (subject, _, _)) in commits.iter().enumerate() {
      // Remove the prefix from the subject for comparison
      let expected_subject = subject.strip_prefix(&format!("({branch_name}) ")).unwrap_or(subject);
      assert_eq!(
        retrieved[i].stripped_subject, expected_subject,
        "Commit {} in archived branch {} should match",
        i, branch_name
      );
    }
  }

  // Original virtual branches should no longer exist
  for (branch_name, _) in &branch_names {
    let full_branch_name = to_final_branch_name("user", branch_name).unwrap();
    let branches = local_repo.list_branches(&full_branch_name).unwrap();
    assert!(branches.is_empty(), "Original {branch_name} branch should be gone");
  }

  // Verify archived branches are listed via production get_all_branch_data
  let expected: Vec<&str> = branch_names.iter().map(|(name, _)| *name).collect();
  verify_archived_branches_listed(&git_executor, local_repo.path().to_str().unwrap(), "user", &expected);

  // Verify detection cache is working with the appropriate strategy
  let strategy = match integration_method {
    IntegrationMethod::Rebase => DetectionStrategy::Rebase,
    IntegrationMethod::Merge => DetectionStrategy::Merge,
    IntegrationMethod::Squash => DetectionStrategy::All,
  };

  rt.block_on(async {
    verify_detection_cache_works(&local_repo, &git_executor, "user", strategy, branch_names.len())
      .await
      .unwrap();
  });
}

// TODO: Fix this test - it's failing because local-only repos don't detect integration properly
// when the virtual branch is merged locally to main without going through a remote
#[ignore]
#[test]
fn test_integration_timestamp_accuracy() {
  // Test that integration timestamps reflect actual integration time, not detection time
  let rt = tokio::runtime::Runtime::new().unwrap();
  let git_executor = GitCommandExecutor::new();

  // Create test repository
  let repo = TestRepo::new();
  let initial_commit = repo.create_commit("Initial", "README.md", "# Init");
  repo.create_branch_at("main", &initial_commit).unwrap();

  // Create a feature branch with commits
  repo.checkout_new_branch("feature").unwrap();

  // Add commit with specific timestamp (old timestamp)
  let old_timestamp = 1700000000u32; // Some time in the past
  repo.create_commit_with_timestamp("(feature-test) Add feature", "feature.txt", "feature content", Some(old_timestamp as i64));

  // Run initial sync to create virtual branch
  let progress_reporter = TestReporter::new();
  let result = rt.block_on(async { sync_branches_core(&git_executor, repo.path().to_str().unwrap(), "user", progress_reporter).await });
  assert!(result.is_ok(), "Initial sync should succeed");

  // Merge the virtual branch back to main with a newer timestamp
  repo.checkout("main").unwrap();

  let merge_timestamp = (chrono::Utc::now().timestamp() - 300) as u32; // 5 minutes ago
  repo
    .merge_no_ff_with_timestamp("user/virtual/feature-test", "Merge feature-test", merge_timestamp as i64)
    .unwrap();

  // Don't delete the virtual branch yet - we need it to exist for detection

  // Wait a moment to ensure timestamps are different
  std::thread::sleep(std::time::Duration::from_secs(2));

  // Run sync again - should detect integration with correct timestamp
  let progress_reporter2 = TestReporter::new();
  let result = rt.block_on(async { sync_branches_core(&git_executor, repo.path().to_str().unwrap(), "user", progress_reporter2.clone()).await });
  assert!(result.is_ok(), "Second sync should succeed");

  // Check integrated branch detection
  let events = progress_reporter2.get_events();
  let integrated_branches: Vec<String> = events
    .iter()
    .filter_map(|e| match e {
      SyncEvent::BranchIntegrationDetected { info } if matches!(info.status, sync_types::branch_integration::BranchIntegrationStatus::Integrated { .. }) => Some(info.name.clone()),
      _ => None,
    })
    .collect();

  assert_eq!(integrated_branches.len(), 1, "Should detect one integrated branch");
  let branch = &integrated_branches[0];

  // Find the integration info for verification
  let integration_info = events
    .iter()
    .find_map(|e| match e {
      SyncEvent::BranchIntegrationDetected { info } if info.name == *branch => Some(info),
      _ => None,
    })
    .expect("Should find integration info");

  // Verify the timestamp is close to merge time, not current time
  let now = chrono::Utc::now().timestamp() as u32;
  let time_since_merge = now - merge_timestamp;

  if let sync_types::branch_integration::BranchIntegrationStatus::Integrated { integrated_at, .. } = &integration_info.status {
    if let Some(integrated_date) = integrated_at {
      let time_since_detected = now - integrated_date;

      assert!(
        time_since_detected >= time_since_merge - 60,
        "Integration timestamp should be close to merge time. Merge was {time_since_merge} seconds ago, but timestamp shows {time_since_detected} seconds ago"
      );

      // Verify it's not the old commit timestamp
      assert!(*integrated_date > old_timestamp + 1000, "Integration timestamp should not be the original commit time");
    } else {
      panic!("Expected integration timestamp for test, but got None");
    }
  } else {
    panic!("Expected Integrated status");
  }
}

#[test]
fn test_confidence_ordering() {
  use sync_types::branch_integration::IntegrationConfidence::*;
  assert!(Exact > High);
  assert_eq!(Exact, Exact);
  assert_eq!(High, High);
}

#[test]
fn test_rebase_integration_with_timestamp_verification() {
  // Test rebase/cherry-pick integration detection with proper timestamp verification
  test_integration_workflow_helper(
    UpdateMethod::Rebase,
    IntegrationMethod::Rebase,
    vec![(
      "feature-rebase",
      vec![
        ("(feature-rebase) Add feature 1", "feature1.txt", "feature 1 content"),
        ("(feature-rebase) Add feature 2", "feature2.txt", "feature 2 content"),
      ],
    )],
  );
}

#[test]
fn test_partial_integration_detection_with_all_strategy() {
  // Test partial integration detection using all strategies (comprehensive detection)
  // Only some commits from a branch are cherry-picked, branch should be detected as orphaned
  test_partial_integration_detection_with_strategy(DetectionStrategy::All);
}
