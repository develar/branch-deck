//! Tests for archive cleanup functionality

use anyhow::Result;
use sync_utils::timestamp::{days_ago, format_utc_date};
use test_log::test;

/// Cached-status-based cleanup during detection deletes only fully integrated branches older than retention
#[test(tokio::test)]
async fn test_detection_cleanup_respects_cached_status() -> Result<()> {
  use crate::detection_notes::CacheOps;
  use crate::detection_notes::CachedDetection;
  use crate::detector::{DetectConfig, detect_integrated_branches};
  use crate::strategy::DetectionStrategy;
  use test_utils::sync_fixtures::TestReporter;
  use test_utils::sync_fixtures::{integrated_branch_info, not_integrated_branch_info};

  // Set up repos and executor
  let (_upstream_repo, local_repo, git_executor) = test_utils::sync_fixtures::setup_test_repos();

  // Create distinct commits to serve as tips for archived branches
  let initial_commit = local_repo.create_commit("Initial commit", "README.md", "# Test");
  let commit_a = local_repo.create_commit("A", "a.txt", "a");
  let commit_b = local_repo.create_commit("B", "b.txt", "b");
  let commit_c = local_repo.create_commit("C", "c.txt", "c");

  // Dates for branches
  let old_date = format_utc_date(days_ago(10));
  let recent_date = format_utc_date(days_ago(2));

  // Archived branch names
  let old_integrated = format!("user/archived/{old_date}/old-integrated");
  let old_orphaned = format!("user/archived/{old_date}/old-orphaned");
  let old_uncached = format!("user/archived/{old_date}/old-uncached");
  let recent_integrated = format!("user/archived/{recent_date}/recent-integrated");

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
  let integrated_info = CachedDetection {
    info: integrated_branch_info(),
    check: None,
  };
  let not_integrated_info = CachedDetection {
    info: not_integrated_branch_info(3, 1, 2),
    check: None,
  };

  cache_ops.write(&old_integrated_tip, &integrated_info).unwrap();
  cache_ops.write(&old_orphaned_tip, &not_integrated_info).unwrap();
  cache_ops.write(&recent_integrated_tip, &integrated_info).unwrap();

  // Run detection (which triggers cleanup using cached notes). No active branches.
  // the upstream of the cloned branch, which a sync takes for the baseline
  let baseline = git_executor.execute_command(&["rev-parse", "--abbrev-ref", "@{u}"], repo_path).unwrap().trim().to_string();
  let active_branches = std::collections::HashSet::new();
  let progress = TestReporter::new();
  let cfg = DetectConfig {
    active_branches: &active_branches,
    progress: &progress,
    strategy: DetectionStrategy::Rebase,
    retention_days: 7,
  };
  detect_integrated_branches(&git_executor, repo_path, "user", &baseline, cfg).unwrap();

  // Verify: only old_integrated was deleted. All others remain.
  let remaining = local_repo.list_branches("user/archived/*").unwrap();
  assert!(!remaining.contains(&old_integrated), "old-integrated should be deleted");
  assert!(remaining.contains(&old_orphaned), "old-orphaned should remain");
  assert!(remaining.contains(&old_uncached), "old-uncached (no cache) should remain");
  assert!(remaining.contains(&recent_integrated), "recent-integrated should remain (not old enough)");

  // Deleted branches must not be reported to the UI
  let reported: Vec<String> = progress
    .get_events()
    .into_iter()
    .filter_map(|event| match event {
      domain_types::SyncEvent::ArchivedBranchesFound { branch_names, .. } => Some(branch_names),
      _ => None,
    })
    .flatten()
    .collect();
  assert!(!reported.contains(&old_integrated), "deleted old-integrated must not be reported: {reported:?}");
  assert!(reported.contains(&old_orphaned), "old-orphaned should be reported: {reported:?}");
  assert!(reported.contains(&recent_integrated), "recent-integrated should be reported: {reported:?}");
  // a note without a baseline beyond the retention period is reported as is, not detected again
  let events = progress.get_events();
  let status = &test_utils::sync_fixtures::detected_branch_info(&events, &old_orphaned).status;
  assert!(
    matches!(
      status,
      domain_types::branch_integration::BranchIntegrationStatus::NotIntegrated {
        total_commit_count: 3,
        integrated_count: 1,
        orphaned_count: 2,
        ..
      }
    ),
    "{status:?}"
  );

  Ok(())
}
