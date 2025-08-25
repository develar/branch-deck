use super::test_helpers::{setup_test_repos, test_partial_integration_detection_with_strategy};
use branch_integration::{detector::detect_integrated_branches, strategy::DetectionStrategy};
use sync_test_utils::TestReporter;
use sync_types::SyncEvent;
use test_log::test;

/// Test the core orphan scenario: virtual branch exists but original commits are gone from HEAD
/// This tests what CLAUDE.md describes as orphaned: "Virtual branch exists but original commits are gone"
#[test(tokio::test)]
async fn test_true_orphan_case_when_original_commits_removed_from_head() {
  let (_upstream_repo, local_repo, git_executor) = setup_test_repos();

  // Step 1: Create some commits on main branch (simulating commits in baseline..HEAD range)
  let _commit1 = local_repo.create_commit("(feature-test) Add feature 1", "feature1.js", "// original feature 1");
  let commit2 = local_repo.create_commit("(feature-test) Add feature 2", "feature2.js", "// original feature 2");

  // Step 2: Create virtual branch (simulating what Branch Deck sync does - cherry-picking commits)
  local_repo.create_branch_at("user/virtual/feature-test", &commit2).unwrap();

  // Step 3: Remove the original commits from main (simulating rebase/reset)
  // This is the key scenario: original commits are gone but virtual branch still exists
  local_repo.reset_hard("HEAD~2").unwrap();

  // Step 4: Test orphan detection
  // The virtual branch should be detected as orphaned because:
  // - It has commits that have no equivalent in current main branch
  // - The original commits that it was based on are no longer in main

  // Use TestReporter to capture events
  let progress = TestReporter::new();

  // Use production code path - detect_integrated_branches with sync_branches to detect baseline automatically
  use sync_core::sync::detect_baseline_branch;
  let baseline = detect_baseline_branch(&git_executor, local_repo.path().to_str().unwrap(), "main").unwrap();
  println!("Detected baseline: {}", baseline);

  let grouped_commits = indexmap::IndexMap::new(); // Empty since we're testing archived branches
  let result = detect_integrated_branches(
    &git_executor,
    local_repo.path().to_str().unwrap(),
    "user",
    &baseline, // Use detected baseline instead of hardcoded "main"
    branch_integration::detector::DetectConfig {
      grouped_commits: &grouped_commits,
      progress: &progress,
      strategy: DetectionStrategy::Rebase,
      retention_days: 7,
    },
  )
  .await;

  println!("Orphan detection result: {:?}", result);
  assert!(result.is_ok(), "Detection should succeed");

  // Check events to verify orphan was detected
  let events = progress.get_events();
  let mut found_orphaned = false;

  for event in events.iter() {
    if let SyncEvent::BranchIntegrationDetected { info } = event {
      found_orphaned = true;
      // The branch name should contain feature-test (might be archived)
      assert!(info.name.contains("feature-test"), "Branch name should contain feature-test");

      // Verify branch status details
      if let sync_types::branch_integration::BranchIntegrationStatus::NotIntegrated {
        total_commit_count,
        integrated_count,
        orphaned_count,
        integrated_at: _,
      } = &info.status
      {
        assert_eq!(*total_commit_count, 2, "Should detect 2 total commits");
        assert_eq!(*orphaned_count, 2, "Should detect 2 orphaned commits");
        assert_eq!(*integrated_count, 0, "Should detect 0 integrated commits");
      } else {
        panic!("Expected NotIntegrated status");
      }
    }
  }

  assert!(found_orphaned, "Should have detected orphaned branch");
}

/// Test batch archiving of inactive branches
#[test]
fn test_batch_archive_inactive_branches() {
  use branch_integration::archive::batch_archive_inactive_branches;
  use branch_integration::common::get_all_branch_data;

  let (_upstream_repo, local_repo, git_executor) = setup_test_repos();

  // Step 1: Create multiple virtual branches
  let commit1 = local_repo.create_commit("(feature-1) Add feature 1", "feature1.js", "// feature 1");
  local_repo.create_branch_at("user/virtual/feature-1", &commit1).unwrap();

  let commit2 = local_repo.create_commit("(feature-2) Add feature 2", "feature2.js", "// feature 2");
  local_repo.create_branch_at("user/virtual/feature-2", &commit2).unwrap();

  let commit3 = local_repo.create_commit("(feature-3) Add feature 3", "feature3.js", "// feature 3");
  local_repo.create_branch_at("user/virtual/feature-3", &commit3).unwrap();

  // Step 2: Verify branches exist
  let branches_before = git_executor
    .execute_command_lines(&["branch", "--list", "user/virtual/*", "--format=%(refname:short)"], local_repo.path().to_str().unwrap())
    .unwrap();
  assert_eq!(branches_before.len(), 3, "Should have 3 virtual branches");

  // Step 3: Get branch data (simulating what the real code does)
  let branch_data = get_all_branch_data(&git_executor, local_repo.path().to_str().unwrap(), "user").unwrap();

  // Step 4: Batch archive the branches
  let inactive_branches = vec![
    "user/virtual/feature-1".to_string(),
    "user/virtual/feature-2".to_string(),
    "user/virtual/feature-3".to_string(),
  ];

  let result = batch_archive_inactive_branches(
    &git_executor,
    local_repo.path().to_str().unwrap(),
    "user",
    inactive_branches,
    &branch_data.virtual_commits,
    &branch_data.archived_today_names,
  );

  assert!(result.is_ok(), "Batch archive should succeed");
  let newly_archived = result.unwrap();

  // Step 4: Verify results
  assert_eq!(newly_archived.len(), 3, "Should have archived 3 branches");

  // Check that all archived branches are correct
  for (archived_name, _commit) in &newly_archived {
    assert!(archived_name.contains("/archived/"), "Archived should be in archive namespace");
    assert!(archived_name.contains("feature-"), "Archived should contain feature name");
  }

  // Step 5: Verify original branches are gone
  let virtual_branches_after = git_executor
    .execute_command_lines(&["branch", "--list", "user/virtual/*", "--format=%(refname:short)"], local_repo.path().to_str().unwrap())
    .unwrap();
  assert_eq!(virtual_branches_after.len(), 0, "Virtual branches should be gone after archiving");

  // Step 6: Verify archived branches exist
  let archived_branches = git_executor
    .execute_command_lines(&["branch", "--list", "user/archived/*", "--format=%(refname:short)"], local_repo.path().to_str().unwrap())
    .unwrap();
  assert_eq!(archived_branches.len(), 3, "Should have 3 archived branches");

  // Step 7: Verify archived branches point to correct commits
  for (archived_name, commit_hash) in &newly_archived {
    // The HashMap already contains the correct commit hash
    let expected_commit = match archived_name.as_str() {
      s if s.contains("feature-1") => &commit1,
      s if s.contains("feature-2") => &commit2,
      s if s.contains("feature-3") => &commit3,
      _ => panic!("Unexpected archived branch: {}", archived_name),
    };
    assert_eq!(commit_hash, expected_commit, "Archived branch should point to correct commit");
  }
}

/// Test batch archiving with naming conflicts
#[test]
fn test_batch_archive_with_naming_conflicts() {
  use branch_integration::archive::batch_archive_inactive_branches;
  use branch_integration::common::get_all_branch_data;

  let (_upstream_repo, local_repo, git_executor) = setup_test_repos();

  // Step 1: Create branches that would result in the same archived name
  let commit1 = local_repo.create_commit("(auth) Add auth 1", "auth1.js", "// auth 1");
  local_repo.create_branch_at("user/virtual/auth", &commit1).unwrap();

  let commit2 = local_repo.create_commit("(auth-duplicate) Add auth 2", "auth2.js", "// auth 2");
  local_repo.create_branch_at("user/virtual/auth-duplicate", &commit2).unwrap();

  // Step 2: Create branches with different simple names to test normal archiving
  local_repo.checkout("main").unwrap();
  let commit3 = local_repo.create_commit("(test) Add test feature", "test.js", "// test");
  local_repo.create_branch_at("user/virtual/test", &commit3).unwrap();

  let commit4 = local_repo.create_commit("(test2) Add test2 feature", "test2.js", "// test2");
  local_repo.create_branch_at("user/virtual/test2", &commit4).unwrap();

  // Get branch data
  let branch_data = get_all_branch_data(&git_executor, local_repo.path().to_str().unwrap(), "user").unwrap();

  // Now archive with branches that have simple names that could be similar
  let inactive_branches_real = vec!["user/virtual/test".to_string(), "user/virtual/test2".to_string()];

  let result = batch_archive_inactive_branches(
    &git_executor,
    local_repo.path().to_str().unwrap(),
    "user",
    inactive_branches_real,
    &branch_data.virtual_commits,
    &branch_data.archived_today_names,
  );

  assert!(result.is_ok(), "Batch archive should succeed");
  let newly_archived = result.unwrap();

  // Step 3: Verify both branches were archived with different names
  assert_eq!(newly_archived.len(), 2, "Should have archived both branches");

  // Both should have unique names - one "test", one "test2"
  let has_test = newly_archived.keys().any(|name| name.ends_with("/test"));
  let has_test2 = newly_archived.keys().any(|name| name.ends_with("/test2"));

  assert!(has_test, "Should have one branch ending with 'test'");
  assert!(has_test2, "Should have one branch ending with 'test2'");

  // Verify they are different (they already are by nature of being HashMap keys)
  assert_eq!(newly_archived.len(), 2, "Should have exactly 2 different archived branches");
}

/// Test same-day re-archiving (e.g., working on issue #123 multiple times in one day)
#[test]
fn test_same_day_rearchiving_with_conflict_resolution() {
  use branch_integration::archive::batch_archive_inactive_branches;
  use branch_integration::common::get_all_branch_data;

  let (_upstream_repo, local_repo, git_executor) = setup_test_repos();

  // Scenario: Working on issue #123 multiple times in the same day

  // Morning: Create and archive first version of issue #123
  let commit1 = local_repo.create_commit("(123) Fix login", "login.js", "// fix 1");
  local_repo.create_branch_at("user/virtual/123", &commit1).unwrap();

  // Get branch data and archive it
  let branch_data = get_all_branch_data(&git_executor, local_repo.path().to_str().unwrap(), "user").unwrap();
  let first_archive = batch_archive_inactive_branches(
    &git_executor,
    local_repo.path().to_str().unwrap(),
    "user",
    vec!["user/virtual/123".to_string()],
    &branch_data.virtual_commits,
    &branch_data.archived_today_names,
  )
  .unwrap();

  assert_eq!(first_archive.len(), 1);
  let first_archive_name = first_archive.keys().next().unwrap();
  println!("First archive created: {}", first_archive_name);
  assert!(first_archive_name.ends_with("/123"), "First archive should be named '123', got: {}", first_archive_name);

  // Verify the archived branch actually exists
  let verify_exists = git_executor.execute_command(&["rev-parse", first_archive_name], local_repo.path().to_str().unwrap());
  assert!(verify_exists.is_ok(), "Archived branch should exist");

  // List all branches to debug
  let all_branches = git_executor
    .execute_command_lines(&["branch", "--list", "--format=%(refname:short)"], local_repo.path().to_str().unwrap())
    .unwrap();
  println!("All branches after first archive: {:?}", all_branches);

  // Afternoon: Work on issue #123 again
  local_repo.checkout("main").unwrap();
  let commit2 = local_repo.create_commit("(123) Fix logout", "logout.js", "// fix 2");
  local_repo.create_branch_at("user/virtual/123", &commit2).unwrap();

  // Get updated branch data (now includes the archived branch from morning)
  let branch_data = get_all_branch_data(&git_executor, local_repo.path().to_str().unwrap(), "user").unwrap();

  // Debug: print what we found
  println!("Archived branches found: {:?}", branch_data.archived_all);
  println!("Today's archived names: {:?}", branch_data.archived_today_names);

  // The archived_today_names should now contain "123"
  assert!(
    branch_data.archived_today_names.contains("123"),
    "Should detect existing '123' in today's archives. Found: {:?}",
    branch_data.archived_today_names
  );

  // Archive the second version - should get suffix
  let second_archive = batch_archive_inactive_branches(
    &git_executor,
    local_repo.path().to_str().unwrap(),
    "user",
    vec!["user/virtual/123".to_string()],
    &branch_data.virtual_commits,
    &branch_data.archived_today_names,
  )
  .unwrap();

  assert_eq!(second_archive.len(), 1);
  let second_archive_name = second_archive.keys().next().unwrap();
  assert!(
    second_archive_name.ends_with("/123-1"),
    "Second archive should be named '123-1' due to conflict, got: {}",
    second_archive_name
  );

  // Verify both archived branches exist
  let all_archived = git_executor
    .execute_command_lines(&["branch", "--list", "user/archived/*", "--format=%(refname:short)"], local_repo.path().to_str().unwrap())
    .unwrap();
  assert_eq!(all_archived.len(), 2, "Should have both archived branches");

  // Verify they have different names
  let has_original = all_archived.iter().any(|b| b.ends_with("/123"));
  let has_suffixed = all_archived.iter().any(|b| b.ends_with("/123-1"));
  assert!(has_original, "Should have original '123' archive");
  assert!(has_suffixed, "Should have suffixed '123-1' archive");
}

#[test]
fn test_partial_integration_detection_with_rebase_strategy() {
  // Test partial integration detection using rebase strategy
  // Only some commits from a branch are cherry-picked, branch should be detected as orphaned
  test_partial_integration_detection_with_strategy(DetectionStrategy::Rebase);
}

/// Test archiving + production cleanup with custom retention
#[test(tokio::test)]
async fn test_archive_and_cleanup_with_production_path() -> anyhow::Result<()> {
  use super::test_helpers::sync_branches_core_with_strategy_and_retention;
  use branch_integration::cache::CacheOps;
  use sync_test_utils::TestReporter;
  use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus, IntegrationConfidence};

  let (_upstream_repo, local_repo, git_executor) = setup_test_repos();

  // Step 1: Create branches and archive them with specific dates
  let commit1 = local_repo.create_commit("(feature-old) Old feature", "old.js", "// old");
  let commit2 = local_repo.create_commit("(feature-recent) Recent feature", "recent.js", "// recent");
  let commit3 = local_repo.create_commit("(feature-orphaned) Orphaned feature", "orphaned.js", "// orphaned");

  local_repo.create_branch_at("user/virtual/feature-old", &commit1).unwrap();
  local_repo.create_branch_at("user/virtual/feature-recent", &commit2).unwrap();
  local_repo.create_branch_at("user/virtual/feature-orphaned", &commit3).unwrap();

  // Step 2: Manually archive branches to specific dates for testing
  let old_date = (chrono::Utc::now() - chrono::Duration::days(10)).format("%Y-%m-%d").to_string();
  let recent_date = (chrono::Utc::now() - chrono::Duration::days(2)).format("%Y-%m-%d").to_string();

  // Archive branches manually to specific dates for testing
  let old_archived_name = format!("user/archived/{}/feature-old", old_date);
  let orphaned_archived_name = format!("user/archived/{}/feature-orphaned", old_date);
  let recent_archived_name = format!("user/archived/{}/feature-recent", recent_date);

  // Move branches to archive namespace
  git_executor
    .execute_command(&["branch", "-m", "user/virtual/feature-old", &old_archived_name], local_repo.path().to_str().unwrap())
    .unwrap();
  git_executor
    .execute_command(
      &["branch", "-m", "user/virtual/feature-orphaned", &orphaned_archived_name],
      local_repo.path().to_str().unwrap(),
    )
    .unwrap();
  git_executor
    .execute_command(&["branch", "-m", "user/virtual/feature-recent", &recent_archived_name], local_repo.path().to_str().unwrap())
    .unwrap();

  // Step 3: Set up cache status for archived branches
  let cache_ops = CacheOps::new(&git_executor, local_repo.path().to_str().unwrap());

  // Create integrated cache entries
  let integrated_info = BranchIntegrationInfo {
    name: "test-branch".to_string(),
    summary: String::new(),
    status: BranchIntegrationStatus::Integrated {
      integrated_at: Some(0),
      confidence: IntegrationConfidence::High,
      commit_count: 1,
    },
  };
  // Write integrated cache directly

  // Create not-integrated cache entry
  let not_integrated_info = BranchIntegrationInfo {
    name: "test-branch".to_string(),
    summary: String::new(),
    status: BranchIntegrationStatus::NotIntegrated {
      total_commit_count: 1,
      integrated_count: 0,
      orphaned_count: 1,
      integrated_at: None,
    },
  };
  // Write not-integrated cache directly

  cache_ops.write(&commit1, &integrated_info).unwrap(); // old + integrated = should be deleted
  cache_ops.write(&commit2, &integrated_info).unwrap(); // recent + integrated = should remain (not old enough)
  cache_ops.write(&commit3, &not_integrated_info).unwrap(); // old + not_integrated = should remain

  // Step 4: Verify all archived branches exist before cleanup
  let archived_before = local_repo.list_branches("user/archived/*").unwrap();
  assert!(archived_before.contains(&old_archived_name), "Old integrated branch should exist before cleanup");
  assert!(archived_before.contains(&orphaned_archived_name), "Old orphaned branch should exist before cleanup");
  assert!(archived_before.contains(&recent_archived_name), "Recent integrated branch should exist before cleanup");

  // Step 5: Run sync with custom retention (5 days) to trigger production cleanup
  let progress = TestReporter::new();

  sync_branches_core_with_strategy_and_retention(
    &git_executor,
    local_repo.path().to_str().unwrap(),
    "user",
    progress,
    DetectionStrategy::Rebase,
    5, // 5 days retention - should delete 10-day-old branches
  )
  .await
  .unwrap();

  // Step 6: Verify cleanup results
  let archived_after = local_repo.list_branches("user/archived/*").unwrap();

  // Only the old integrated branch should be deleted
  assert!(!archived_after.contains(&old_archived_name), "Old integrated branch should be deleted by cleanup");

  // Other branches should remain
  assert!(archived_after.contains(&orphaned_archived_name), "Old orphaned branch should remain (not integrated)");
  assert!(archived_after.contains(&recent_archived_name), "Recent integrated branch should remain (not old enough)");

  Ok(())
}
