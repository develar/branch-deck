//! Test helper functions for integration detection tests

use anyhow::Result;
use branch_integration::cache::CachedDetection;
use branch_integration::strategy::DetectionStrategy;
use git_executor::git_command_executor::GitCommandExecutor;
use std::time::Instant;
use sync_core::sync::{IssueConfigSource, SyncOptions, sync_branches};
use sync_test_utils::TestReporter;
use sync_types::SyncEvent;
use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus, IntegrationConfidence};
use test_utils::git_test_utils::TestRepo;
use tracing::{debug, info};

/// Common test setup that creates upstream and local repositories with proper clone relationship
/// This ensures tests use realistic git configuration with remotes
pub(crate) fn setup_test_repos() -> (TestRepo, TestRepo, GitCommandExecutor) {
  let git_executor = GitCommandExecutor::new();

  // Create upstream repository
  let upstream_repo = TestRepo::new();
  let initial_commit = upstream_repo.create_commit("Initial commit", "README.md", "# Project");
  upstream_repo.create_branch_at("main", &initial_commit).unwrap();

  // Clone to create local repository
  let local_repo = TestRepo::new_empty();
  local_repo.clone_from(upstream_repo.path()).unwrap();

  (upstream_repo, local_repo, git_executor)
}

/// Sync options for tests: load the issue config from the repository and use the given detection strategy
pub(crate) fn options_with_strategy(strategy: DetectionStrategy) -> SyncOptions {
  SyncOptions {
    issue_config: IssueConfigSource::Load,
    detection_strategy: strategy,
    ..Default::default()
  }
}

/// Helper to list all detection notes using `TestRepo` API
fn list_detection_notes(repo: &TestRepo) -> Vec<String> {
  repo.list_notes_with_ref(branch_integration::cache::NOTES_REF).unwrap_or_default()
}

/// Syncs the "user" branches of `repo` with the given detection strategy and returns the reporter with the sync events
pub(crate) async fn sync_user_branches(git_executor: &GitCommandExecutor, repo: &TestRepo, strategy: DetectionStrategy) -> Result<TestReporter> {
  let progress_reporter = TestReporter::new();
  sync_branches(
    git_executor,
    repo.path().to_str().unwrap(),
    "user",
    progress_reporter.clone(),
    options_with_strategy(strategy),
  )
  .await?;
  Ok(progress_reporter)
}

/// Names of the branches reported by `BranchIntegrationDetected` events, integrated or orphaned
pub(crate) fn detected_branch_names(events: &[SyncEvent]) -> Vec<String> {
  events
    .iter()
    .filter_map(|e| match e {
      SyncEvent::BranchIntegrationDetected { info } => Some(info.name.clone()),
      _ => None,
    })
    .collect()
}

/// Names of the branches that `ArchivedBranchesFound` reports as archived by this sync
pub(crate) fn newly_archived_names(events: &[SyncEvent]) -> Vec<String> {
  events
    .iter()
    .filter_map(|e| match e {
      SyncEvent::ArchivedBranchesFound { newly_archived, .. } => Some(newly_archived.clone()),
      _ => None,
    })
    .flatten()
    .collect()
}

/// Names of the branches reported as integrated by `BranchIntegrationDetected` events
pub(crate) fn integrated_branch_names(events: &[SyncEvent]) -> Vec<String> {
  events
    .iter()
    .filter_map(|e| match e {
      SyncEvent::BranchIntegrationDetected { info } if matches!(info.status, BranchIntegrationStatus::Integrated { .. }) => Some(info.name.clone()),
      _ => None,
    })
    .collect()
}

/// Integration info of the detected branch `name`
pub(crate) fn detected_branch_info<'a>(events: &'a [SyncEvent], name: &str) -> &'a BranchIntegrationInfo {
  events
    .iter()
    .find_map(|e| match e {
      SyncEvent::BranchIntegrationDetected { info } if info.name == name => Some(info),
      _ => None,
    })
    .expect("Should find integration info")
}

/// The detection note on `commit`, parsed
pub(crate) fn detection_note(repo: &TestRepo, commit: &str) -> CachedDetection {
  let note = repo
    .git_executor()
    .execute_command(
      &["notes", &format!("--ref={}", branch_integration::cache::NOTES_REF), "show", commit],
      repo.path().to_str().unwrap(),
    )
    .unwrap_or_else(|e| panic!("no detection note on {commit}: {e}"));
  branch_integration::cache::deserialize_from_cache(note.trim()).unwrap()
}

/// Detection cache entry of a fully integrated branch
pub(crate) fn integrated_cache_info() -> CachedDetection {
  CachedDetection {
    info: BranchIntegrationInfo {
      name: "test-branch".to_string(),
      summary: String::new(),
      status: BranchIntegrationStatus::Integrated {
        integrated_at: Some(0),
        confidence: IntegrationConfidence::High,
        commit_count: 1,
        commit: None,
      },
    },
    check: None,
  }
}

/// Detection cache entry of a branch whose commits are partly or not at all integrated, without the baseline it was
/// checked against, as version 1 wrote it
pub(crate) fn not_integrated_cache_info(total_commit_count: u32, integrated_count: u32, orphaned_count: u32) -> CachedDetection {
  CachedDetection {
    info: BranchIntegrationInfo {
      name: "test-branch".to_string(),
      summary: String::new(),
      status: BranchIntegrationStatus::NotIntegrated {
        total_commit_count,
        integrated_count,
        orphaned_count,
        integrated_at: None,
      },
    },
    check: None,
  }
}

/// Verify that detection cache is working correctly after an initial detection run
/// This should be called at the end of integration tests to ensure cache is working
pub(crate) async fn verify_detection_cache_works(
  repo: &TestRepo,
  git_executor: &GitCommandExecutor,
  branch_prefix: &str,
  strategy: DetectionStrategy,
  expected_detection_count: usize,
) -> Result<()> {
  use branch_integration::common::get_all_branch_data;

  info!("verifying detection cache behavior");

  // Get initial cache state
  let notes_before = list_detection_notes(repo);
  assert!(!notes_before.is_empty(), "Detection cache notes should exist after initial detection");
  info!(cache_notes_count = notes_before.len(), "found cache notes before second detection");

  // Get branch data to access cached notes with commit hashes
  let branch_data = get_all_branch_data(git_executor, repo.path().to_str().unwrap(), branch_prefix)?;

  // Parse and verify cache contents have proper commit counts
  let mut integrated_cache_count = 0;
  let mut orphaned_cache_count = 0;

  for (commit, cache) in &branch_data.branch_notes {
    match &cache.info.status {
      BranchIntegrationStatus::Integrated { commit_count, integrated_at, .. } => {
        integrated_cache_count += 1;
        assert!(*commit_count > 0, "Integrated branch cache should have commit_count > 0, got {commit_count}");
        assert!(integrated_at.is_some(), "Integrated branch cache should have integrated_date, got {integrated_at:?}");
        info!(commit = %commit, commit_count = commit_count, integrated_date = ?integrated_at, "verified integrated cache has commit count and date");
      }
      BranchIntegrationStatus::NotIntegrated {
        total_commit_count,
        integrated_count,
        orphaned_count,
        integrated_at: _,
      } => {
        orphaned_cache_count += 1;
        assert!(
          *total_commit_count > 0,
          "NotIntegrated branch cache should have total_commit_count > 0, got {total_commit_count}"
        );
        assert_eq!(
          *total_commit_count,
          integrated_count + orphaned_count,
          "total_commit_count should equal integrated_count + orphaned_count"
        );
        info!(
          commit = %commit,
          total = total_commit_count,
          integrated = integrated_count,
          orphaned = orphaned_count,
          "verified not-integrated cache has commit counts"
        );
      }
      BranchIntegrationStatus::Partial { .. } => {} // Not checking Partial caches
    }
  }

  // Run detection again and measure time
  let progress = TestReporter::new();
  let start_time = Instant::now();
  sync_branches(
    git_executor,
    repo.path().to_str().unwrap(),
    branch_prefix,
    progress.clone(),
    options_with_strategy(strategy),
  )
  .await?;
  let cached_detection_time = start_time.elapsed();

  // Verify same detection results
  let events = progress.get_events();
  let detection_count = events.iter().filter(|e| matches!(e, SyncEvent::BranchIntegrationDetected { .. })).count();

  assert_eq!(
    detection_count, expected_detection_count,
    "Second detection should find same number of integrated/orphaned branches"
  );
  assert_eq!(newly_archived_names(&events), Vec::<String>::new(), "The branches were archived by the previous sync");

  // Branch details are now validated at the event level, not the branch name level

  // Verify cache notes still exist
  let notes_after = list_detection_notes(repo);
  assert_eq!(notes_before.len(), notes_after.len(), "Cache notes should be preserved");

  info!(
    detection_count,
    duration_ms = cached_detection_time.as_millis(),
    integrated_cache_count,
    orphaned_cache_count,
    "cache verification passed with commit counts"
  );

  Ok(())
}

/// Update method for pulling changes from upstream
#[derive(Debug, Clone, Copy)]
pub enum UpdateMethod {
  Rebase,
  Merge,
}

/// Integration method for how branches are integrated upstream
#[derive(Debug, Clone, Copy)]
pub enum IntegrationMethod {
  Merge,  // --no-ff merge (default)
  Rebase, // cherry-pick commits
  Squash, // squash merge
}

/// Type alias for branch test data: (`branch_name`, vec![(subject, filename, content)])
pub(crate) type BranchTestData<'a> = [(&'a str, Vec<(&'a str, &'a str, &'a str)>)];

/// Helper function to get commit timestamp
pub(crate) fn get_commit_timestamp(repo: &TestRepo, commit_ref: &str) -> u32 {
  repo.get_commit_timestamp(commit_ref).unwrap()
}

/// Verify that archived branches are found using production `get_all_branch_data`
pub(crate) fn verify_archived_branches_listed(git_executor: &GitCommandExecutor, repo_path: &str, branch_prefix: &str, expected_branches: &[&str]) {
  use branch_integration::common::get_all_branch_data;

  let branch_data = get_all_branch_data(git_executor, repo_path, branch_prefix).expect("get_all_branch_data should work");
  let all_archived = &branch_data.archived_all;

  assert_eq!(
    all_archived.len(),
    expected_branches.len(),
    "get_all_branch_data should find exactly {} archived branches",
    expected_branches.len()
  );

  for branch_name in expected_branches {
    assert!(
      all_archived.iter().any(|b| b.contains(branch_name)),
      "get_all_branch_data should find archived branch {branch_name}"
    );
  }
}

/// Test partial integration detection with a specific strategy
/// Tests scenario where only some commits from a branch are integrated (cherry-picked)
pub(crate) fn test_partial_integration_detection_with_strategy(strategy: DetectionStrategy) {
  let rt = tokio::runtime::Runtime::new().unwrap();

  rt.block_on(async {
    use branch_integration::common::get_all_branch_data;

    // Test scenario where only some commits from a branch are integrated
    let (upstream_repo, local_repo, git_executor) = setup_test_repos();

    // Create a feature branch for development
    local_repo.checkout_new_branch("local-dev").unwrap();

    // Add multiple commits on feature branch
    local_repo.create_commit("(feature-partial) Add feature 1", "feature1.js", "// feature 1");
    local_repo.create_commit("(feature-partial) Add feature 2", "feature2.js", "// feature 2");
    local_repo.create_commit("(feature-partial) Add feature 3", "feature3.js", "// feature 3");

    // Sync to create virtual branch
    sync_user_branches(&git_executor, &local_repo, strategy).await.unwrap();

    // Push branch to upstream
    local_repo.push("origin", "user/virtual/feature-partial").unwrap();

    // In upstream, cherry-pick only first two commits (partial integration)
    upstream_repo.checkout("main").unwrap();

    // Get commit hashes
    let log_output = upstream_repo.log(&["--oneline", "--reverse", "main..user/virtual/feature-partial"]).unwrap();
    let commits: Vec<&str> = log_output.lines().collect();
    let first_hash = commits[0].split(' ').next().unwrap();
    let second_hash = commits[1].split(' ').next().unwrap();

    // Cherry-pick only first two commits
    upstream_repo.cherry_pick(first_hash).unwrap();
    upstream_repo.cherry_pick(second_hash).unwrap();

    // Delete the branch
    upstream_repo.delete_branch("user/virtual/feature-partial").unwrap();

    // Pull changes to local
    local_repo.fetch_prune("origin").unwrap();
    local_repo.checkout("main").unwrap();

    // Update based on strategy
    match strategy {
      DetectionStrategy::Merge => {
        if let Err(e) = local_repo.merge_ff_only("origin/main") {
          debug!(
            error = %e,
            strategy = ?strategy,
            "merge command output"
          );
        }
      }
      DetectionStrategy::Rebase => {
        if let Err(e) = local_repo.rebase("origin/main") {
          debug!(
            error = %e,
            strategy = ?strategy,
            "rebase command output"
          );
        }
      }
      _ => {
        // For All and Squash strategies, use default pull
        local_repo.pull().unwrap();
      }
    }

    // Run sync again with the specific strategy
    let progress_reporter2 = sync_user_branches(&git_executor, &local_repo, strategy).await.unwrap();

    // Check for orphaned branches detection (not fully integrated)
    let events = progress_reporter2.get_events();

    // Collect all orphaned branches from individual events
    let orphaned_branches = detected_branch_names(&events);

    assert!(!orphaned_branches.is_empty(), "Should detect partially integrated branch as orphaned");

    // The branch name will be archived, so check if it contains the original branch name
    assert!(
      orphaned_branches.iter().any(|b| b.contains("feature-partial")),
      "Should detect feature-partial branch as orphaned, found: {:?}",
      orphaned_branches.iter().collect::<Vec<_>>()
    );

    // Field validation is now done at the event level, not the branch level

    // Branch should be archived (all inactive branches are archived)
    let branches = local_repo.list_branches("user/virtual/feature-partial").unwrap();
    assert!(branches.is_empty(), "Virtual branch should be archived, not remain active");

    let archived_branches = local_repo.list_branches("user/archived/*").unwrap();
    let archived = archived_branches.join("\n");
    assert!(archived.contains("feature-partial"), "Partially integrated branch should be archived");

    // Verify archived branches are listed via production get_all_branch_data
    verify_archived_branches_listed(&git_executor, local_repo.path().to_str().unwrap(), "user", &["feature-partial"]);

    // Verify cache was created for the detection
    let branch_data = get_all_branch_data(&git_executor, local_repo.path().to_str().unwrap(), "user").unwrap();
    let archived_branch_name = branch_data
      .archived_all
      .iter()
      .find(|b| b.contains("feature-partial"))
      .expect("Should find archived branch");

    let branch_tip = git_executor
      .execute_command(&["rev-parse", archived_branch_name], local_repo.path().to_str().unwrap())
      .unwrap()
      .trim()
      .to_string();

    // Check if cache note exists
    let notes_ref_arg = format!("--ref={}", branch_integration::cache::NOTES_REF);
    let cache_note = git_executor.execute_command(&["notes", &notes_ref_arg, "show", &branch_tip], local_repo.path().to_str().unwrap());
    assert!(cache_note.is_ok(), "Detection cache should be created for branch");

    // Run sync once more to verify cache is used
    let progress_reporter3 = sync_user_branches(&git_executor, &local_repo, strategy).await.unwrap();

    // Should get same orphaned result (from cache this time)
    let events3 = progress_reporter3.get_events();
    let orphaned_cached = detected_branch_names(&events3).iter().any(|b| b.contains("feature-partial"));
    assert!(orphaned_cached, "Should still detect partial integration from cache");
  });
}
