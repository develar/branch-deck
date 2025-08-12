//! Tests for archive cleanup functionality

use anyhow::Result;
use test_log::test;

/// Cached-status-based cleanup during detection deletes only fully integrated branches older than retention
#[test(tokio::test)]
async fn test_detection_cleanup_respects_cached_status() -> Result<()> {
  use branch_integration::cache::CacheOps;
  use branch_integration::detector::{DetectConfig, detect_integrated_branches};
  use branch_integration::strategy::DetectionStrategy;
  use sync_core::sync::detect_baseline_branch;
  use sync_test_utils::TestReporter;
  use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus, IntegrationConfidence};

  // Set up repos and executor
  let (_upstream_repo, local_repo, git_executor) = crate::test_helpers::setup_test_repos();

  // Create distinct commits to serve as tips for archived branches
  let initial_commit = local_repo.create_commit("Initial commit", "README.md", "# Test");
  let commit_a = local_repo.create_commit("A", "a.txt", "a");
  let commit_b = local_repo.create_commit("B", "b.txt", "b");
  let commit_c = local_repo.create_commit("C", "c.txt", "c");

  // Dates for branches
  let old_date = (chrono::Utc::now() - chrono::Duration::days(10)).format("%Y-%m-%d").to_string();
  let recent_date = (chrono::Utc::now() - chrono::Duration::days(2)).format("%Y-%m-%d").to_string();

  // Archived branch names
  let old_integrated = format!("user/archived/{}/old-integrated", old_date);
  let old_orphaned = format!("user/archived/{}/old-orphaned", old_date);
  let old_uncached = format!("user/archived/{}/old-uncached", old_date);
  let recent_integrated = format!("user/archived/{}/recent-integrated", recent_date);

  // Create the archived branches pointing to distinct commits
  local_repo.create_branch_at(&old_integrated, &commit_a).unwrap();
  local_repo.create_branch_at(&old_orphaned, &commit_b).unwrap();
  local_repo.create_branch_at(&old_uncached, &initial_commit).unwrap();
  local_repo.create_branch_at(&recent_integrated, &commit_c).unwrap();

  // Helper to resolve tip hash
  let repo_path = local_repo.path().to_str().unwrap();
  let tip = |name: &str| -> String { git_executor.execute_command(&["rev-parse", name], repo_path).unwrap().trim().to_string() };

  // Write detection cache notes: Integrated for old_integrated and recent_integrated; Orphaned for old_orphaned; None for old_uncached
  let cache_ops = CacheOps::new(&git_executor, repo_path);
  let old_integrated_tip = tip(&old_integrated);
  let old_orphaned_tip = tip(&old_orphaned);
  let recent_integrated_tip = tip(&recent_integrated);

  // Create cache entries
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

  let not_integrated_info = BranchIntegrationInfo {
    name: "test-branch".to_string(),
    summary: String::new(),
    status: BranchIntegrationStatus::NotIntegrated {
      total_commit_count: 3,
      integrated_count: 1,
      orphaned_count: 2,
      integrated_at: None,
    },
  };
  // Write not-integrated cache directly

  cache_ops.write(&old_integrated_tip, &integrated_info).unwrap();
  cache_ops.write(&old_orphaned_tip, &not_integrated_info).unwrap();
  cache_ops.write(&recent_integrated_tip, &integrated_info).unwrap();

  // Run detection (which triggers cleanup using cached notes). Use empty grouped_commits.
  let baseline = detect_baseline_branch(&git_executor, repo_path, "main").unwrap_or_else(|_| "origin/main".to_string());
  let grouped_commits = indexmap::IndexMap::new();
  let progress = TestReporter::new();
  let cfg = DetectConfig {
    grouped_commits: &grouped_commits,
    progress: &progress,
    strategy: DetectionStrategy::Rebase,
    retention_days: 7,
  };
  detect_integrated_branches(&git_executor, repo_path, "user", &baseline, cfg).await.unwrap();

  // Verify: only old_integrated was deleted. All others remain.
  let remaining = local_repo.list_branches("user/archived/*").unwrap();
  assert!(!remaining.contains(&old_integrated), "old-integrated should be deleted");
  assert!(remaining.contains(&old_orphaned), "old-orphaned should remain");
  assert!(remaining.contains(&old_uncached), "old-uncached (no cache) should remain");
  assert!(remaining.contains(&recent_integrated), "recent-integrated should remain (not old enough)");

  Ok(())
}
