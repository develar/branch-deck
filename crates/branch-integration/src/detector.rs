use super::{cache::CacheOps, common, merge, rebase, squash, strategy::DetectionStrategy};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::Commit;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus};
use sync_types::{ProgressReporter, SyncEvent};
use sync_utils::issue_pattern::{find_issue_number, find_issue_range};
use tokio::task::JoinSet;
use tracing::{debug, info, instrument, trace, warn};

/// Configuration for integration detection
pub struct DetectConfig<'a> {
  pub grouped_commits: &'a IndexMap<String, Vec<Commit>>,
  pub progress: &'a dyn ProgressReporter,
  pub strategy: DetectionStrategy,
  pub retention_days: u64,
}

/// Parameters for parallel branch processing
struct BranchProcessingParams<'a> {
  git_executor: &'a GitCommandExecutor,
  repo_path: &'a str,
  baseline_branch: &'a str,
  branches: Vec<String>,
  branch_commits: &'a HashMap<String, String>,
  cached_notes: &'a HashMap<String, BranchIntegrationInfo>,
  merged_branches: &'a HashSet<String>,
  strategy: DetectionStrategy,
  progress: &'a dyn ProgressReporter,
}

/// Process a list of branches in parallel and return collected cache writes
#[instrument(skip(params), fields(branch_count = params.branches.len()))]
async fn process_branches_parallel(params: BranchProcessingParams<'_>) -> Result<Vec<(String, BranchIntegrationInfo)>> {
  let BranchProcessingParams {
    git_executor,
    repo_path,
    baseline_branch,
    branches,
    branch_commits,
    cached_notes,
    merged_branches,
    strategy,
    progress,
  } = params;
  if branches.is_empty() {
    return Ok(Vec::new());
  }

  // Process branches in parallel
  let mut set: JoinSet<std::result::Result<DetectionResult, anyhow::Error>> = JoinSet::new();
  let mut all_caches_to_write = Vec::new();

  for archived_branch in branches {
    let repo = repo_path.to_string();
    let baseline = baseline_branch.to_string();
    let strategy_clone = strategy.clone();

    // Precompute cheap values before spawning
    let is_merged = merged_branches.contains(&archived_branch);

    // Get branch tip commit - we already have it from branch_commits
    let branch_tip = branch_commits.get(&archived_branch).cloned();

    // Skip if we couldn't get the branch tip
    let Some(branch_tip) = branch_tip else {
      warn!(branch = %archived_branch, "Could not resolve branch tip");
      continue;
    };

    let cache_entry = cached_notes.get(&branch_tip).cloned();

    // Check cache BEFORE spawning blocking task - major optimization
    if let Some(cache) = cache_entry.clone()
      && handle_cache_hit(progress, &archived_branch, cache)?
    {
      continue;
    }

    // Build inputs and deps and spawn the per-branch task
    let inputs = BranchWorkInputs {
      archived_branch: archived_branch.clone(),
      branch_tip: branch_tip.clone(),
      is_merged,
      strategy: strategy_clone.clone(),
      repo: repo.clone(),
      baseline: baseline.clone(),
    };
    set.spawn(run_branch_task(inputs, git_executor.clone()));
  }

  // Process results as they complete, sending individual events immediately
  while let Some(res) = set.join_next().await {
    match res {
      Ok(Ok(result)) => {
        // Send unified event immediately as each branch completes detection
        progress.send(SyncEvent::BranchIntegrationDetected { info: result.info })?;
        // Collect cache to write later (always present for fresh detection)
        all_caches_to_write.push(result.cache_to_write);
      }
      Ok(Err(e)) => {
        warn!(error = %e, "Task returned error during integration detection");
      }
      Err(e) => {
        warn!(error = %e, "JoinSet spawn_blocking error during integration detection");
      }
    }
  }

  Ok(all_caches_to_write)
}

// ===== Helpers extracted to simplify process_branches_parallel =====

#[derive(Debug)]
struct BranchWorkInputs {
  archived_branch: String,
  branch_tip: String,
  is_merged: bool,
  strategy: DetectionStrategy,
  repo: String,
  baseline: String,
}

fn compute_summary_blocking(git: &GitCommandExecutor, repo: &str, branch_tip: &str, should_compute: bool) -> String {
  if !should_compute {
    return String::new();
  }
  git
    .execute_command(["--no-pager", "log", "-1", "--format=%s", branch_tip].as_slice(), repo)
    .ok()
    .map(|subject| {
      if let Some((_, end)) = find_issue_range(&subject) {
        let cleaned = subject[end..].trim_start_matches([' ', ':']).trim();
        if cleaned.is_empty() { String::new() } else { cleaned.to_string() }
      } else {
        subject.trim().to_string()
      }
    })
    .unwrap_or_default()
}

fn handle_cache_hit(progress: &dyn ProgressReporter, archived_branch: &str, mut info: BranchIntegrationInfo) -> Result<bool> {
  // Ensure the branch name is set correctly
  info.name = archived_branch.to_string();
  progress.send(SyncEvent::BranchIntegrationDetected { info })?;
  Ok(true)
}

#[instrument(skip(git), fields(branch = %inputs.archived_branch, baseline = %inputs.baseline, merged = inputs.is_merged, strategy = ?inputs.strategy, tip = %inputs.branch_tip))]
async fn run_branch_task(inputs: BranchWorkInputs, git: GitCommandExecutor) -> std::result::Result<DetectionResult, anyhow::Error> {
  let leaf = inputs.archived_branch.rsplit('/').next().unwrap_or(&inputs.archived_branch);
  let should_compute_summary = matches!(find_issue_number(leaf), Some(issue) if issue == leaf);

  let git_for_det = git.clone();
  let repo_for_det = inputs.repo.clone();
  let branch_for_det = inputs.archived_branch.clone();
  let baseline_for_det = inputs.baseline.clone();
  let strategy_for_det = inputs.strategy.clone();
  let is_merged = inputs.is_merged;

  let det_handle = tokio::task::spawn_blocking(move || perform_fresh_detection(&git_for_det, &repo_for_det, &branch_for_det, &baseline_for_det, is_merged, strategy_for_det));

  let sum_handle = if should_compute_summary {
    let git_for_sum = git.clone();
    let repo_for_sum = inputs.repo.clone();
    let tip_for_sum = inputs.branch_tip.clone();
    Some(tokio::task::spawn_blocking(move || {
      compute_summary_blocking(&git_for_sum, &repo_for_sum, &tip_for_sum, should_compute_summary)
    }))
  } else {
    None
  };

  let status = det_handle.await.map_err(|e| anyhow::anyhow!("join error in detection: {}", e))??;

  let summary = if let Some(h) = sum_handle {
    h.await.map_err(|e| anyhow::anyhow!("join error in summary: {}", e))?
  } else {
    String::new()
  };

  Ok(create_detection_result(status, inputs.archived_branch.clone(), inputs.branch_tip.clone(), summary))
}

/// Write all collected caches sequentially to avoid race conditions
fn write_caches_sequentially(git_executor: &GitCommandExecutor, repo_path: &str, caches_to_write: Vec<(String, BranchIntegrationInfo)>) -> Result<()> {
  if !caches_to_write.is_empty() {
    let cache_ops = CacheOps::new(git_executor, repo_path);
    for (branch_tip, cache) in caches_to_write {
      cache_ops.write(&branch_tip, &cache).map_err(|e| {
        trace!(error = %e, branch_tip = %branch_tip, "Failed to write cache");
        e
      })?;
      debug!(branch_tip = %branch_tip, status = ?cache.status, "Cached detection result");
    }
  }
  Ok(())
}

/// Archive inactive branches using pre-fetched branch data
/// Returns map of newly archived branch names to their commits
async fn archive_inactive_branches(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch_prefix: &str,
  branch_data: &common::BranchData,
  grouped_commits: &IndexMap<String, Vec<Commit>>,
) -> Result<HashMap<String, String>> {
  // Find inactive virtual branches
  let virtual_prefix = format!("{branch_prefix}/virtual/");
  let inactive_branches: Vec<String> = branch_data
    .virtual_commits
    .keys()
    .filter(|full_name| {
      let simple_name = full_name.strip_prefix(&virtual_prefix).unwrap_or(full_name);
      !grouped_commits.contains_key(simple_name)
    })
    .cloned()
    .collect();

  if inactive_branches.is_empty() {
    return Ok(HashMap::new());
  }

  debug!(count = inactive_branches.len(), "Found inactive branches to archive");

  // Archive with pre-fetched data
  let newly_archived = super::archive::batch_archive_inactive_branches(
    git_executor,
    repo_path,
    branch_prefix,
    inactive_branches,
    &branch_data.virtual_commits,
    &branch_data.archived_today_names,
  )?;

  if !newly_archived.is_empty() {
    debug!(archived_count = newly_archived.len(), "Successfully archived inactive branches");
  }

  Ok(newly_archived)
}

/// Detect integrated and not-integrated branches with a specific detection strategy
#[instrument(skip(git_executor, config), fields(branch_prefix = %branch_prefix, baseline_branch = %baseline_branch, grouped_count = config.grouped_commits.len(), strategy = ?config.strategy))]
pub async fn detect_integrated_branches(git_executor: &GitCommandExecutor, repo_path: &str, branch_prefix: &str, baseline_branch: &str, config: DetectConfig<'_>) -> Result<()> {
  // Step 0: Get ALL branch data including parsed cached notes in a single git call
  let branch_data = common::get_all_branch_data(git_executor, repo_path, branch_prefix)?;

  // Step 0.5: Clean up old archived branches, but only those fully integrated
  // Compute cutoff date based on retention
  let cutoff_date = chrono::Utc::now() - chrono::Duration::days(config.retention_days as i64);
  let archive_prefix = format!("{branch_prefix}/archived/");

  // Build list of archived branches to delete: older than cutoff AND cache status Integrated
  let mut branches_to_delete: Vec<String> = Vec::new();
  for branch in &branch_data.archived_all {
    // Extract date from branch path: <prefix>/archived/YYYY-MM-DD/...
    if let Some(date_part) = branch.strip_prefix(&archive_prefix).and_then(|p| p.split('/').next())
      && let Ok(branch_date) = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d")
    {
      let branch_datetime = branch_date.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(chrono::Utc).single().unwrap();
      if branch_datetime < cutoff_date {
        // Check detection cache status on the branch tip commit
        if let Some(tip_commit) = branch_data.all_branch_commits.get(branch)
          && let Some(cache) = branch_data.branch_notes.get(tip_commit)
          && matches!(cache.status, BranchIntegrationStatus::Integrated { .. })
        {
          branches_to_delete.push(branch.clone());
        }
      }
    }
  }

  if !branches_to_delete.is_empty() {
    let deleted = super::archive::batch_delete_archived_branches(git_executor, repo_path, &branches_to_delete)?;
    if deleted > 0 {
      info!(
        deleted_count = deleted,
        retention_days = config.retention_days,
        "Cleaned up old fully integrated archived branches"
      );
    }
  }

  // Step 2: Archive inactive branches using the pre-fetched data
  let newly_archived = archive_inactive_branches(git_executor, repo_path, branch_prefix, &branch_data, config.grouped_commits).await?;

  // Step 3: Build a complete branch commit map including newly archived branches
  let mut all_branch_commits = branch_data.all_branch_commits.clone();
  all_branch_commits.extend(newly_archived.clone());

  // Step 4: Combine newly archived with previously archived
  let archived_all_count = branch_data.archived_all.len();
  let mut all_archived_branches = Vec::with_capacity(archived_all_count + newly_archived.len());

  // Don't sort by archive date - it has no correlation with integration date
  // Frontend will sort by actual integration date once detection completes
  all_archived_branches.extend(newly_archived.keys().cloned());
  all_archived_branches.extend(branch_data.archived_all);

  // Send event with archived branch names (even if empty) to update UI
  config.progress.send(SyncEvent::ArchivedBranchesFound {
    branch_names: all_archived_branches.clone(),
  })?;

  // If no archived branches exist, nothing to detect
  if all_archived_branches.is_empty() {
    return Ok(());
  }

  // Fast path for rebase-only detection (the common case)
  if config.strategy == DetectionStrategy::Rebase {
    // Use the shared helper with empty merged_branches (rebase doesn't need merge detection)
    let empty_merged_branches = HashSet::new();
    let all_caches_to_write = process_branches_parallel(BranchProcessingParams {
      git_executor,
      repo_path,
      baseline_branch,
      branches: all_archived_branches,
      branch_commits: &all_branch_commits,
      cached_notes: &branch_data.branch_notes,
      merged_branches: &empty_merged_branches,
      strategy: DetectionStrategy::Rebase,
      progress: config.progress,
    })
    .await?;

    // Write all caches sequentially to avoid race conditions
    write_caches_sequentially(git_executor, repo_path, all_caches_to_write)?;

    return Ok(());
  }

  // Get list of branches merged into baseline using git's native detection
  // This is only useful for merge-based workflows, not rebase workflows
  // Note: Use archived branch names for the check
  let merged_branches: HashSet<String> = if config.strategy == DetectionStrategy::Merge || config.strategy == DetectionStrategy::All {
    let merged_branches_list = git_executor.execute_command_lines(&["branch", "--merged", baseline_branch, "--format=%(refname:short)"], repo_path)?;
    let merged_branches: HashSet<String> = merged_branches_list.into_iter().collect();
    debug!(baseline = %baseline_branch, count = merged_branches.len(), "Found branches merged into baseline");
    merged_branches
  } else {
    HashSet::new()
  };

  // Use the shared helper for processing branches
  let all_caches_to_write = process_branches_parallel(BranchProcessingParams {
    git_executor,
    repo_path,
    baseline_branch,
    branches: all_archived_branches,
    branch_commits: &all_branch_commits,
    cached_notes: &branch_data.branch_notes,
    merged_branches: &merged_branches,
    strategy: config.strategy,
    progress: config.progress,
  })
  .await?;

  // Write all caches sequentially to avoid race conditions
  write_caches_sequentially(git_executor, repo_path, all_caches_to_write)?;

  Ok(())
}

/// Result of single branch integration detection
#[derive(Debug)]
struct DetectionResult {
  info: BranchIntegrationInfo,
  cache_to_write: (String, BranchIntegrationInfo),
}

impl DetectionResult {
  fn new(info: BranchIntegrationInfo, branch_tip: String) -> Self {
    Self {
      info: info.clone(),
      cache_to_write: (branch_tip, info),
    }
  }
}

/// Create detection result based on integration/not-integrated info and branch tip
fn create_detection_result(status: BranchIntegrationStatus, branch_name: String, branch_tip: String, summary: String) -> DetectionResult {
  let info = BranchIntegrationInfo {
    name: branch_name,
    summary: summary.clone(),
    status: status.clone(),
  };
  DetectionResult::new(info, branch_tip)
}

/// Perform fresh detection using the specified strategy
/// Returns (integrated_info, not_integrated_info) based on detection results
fn perform_fresh_detection(
  git: &GitCommandExecutor,
  repo: &str,
  branch_name: &str,
  baseline: &str,
  is_merged: bool,
  strategy: DetectionStrategy,
) -> Result<BranchIntegrationStatus> {
  // 1) Merge detection first if enabled and branch is known merged
  if (strategy == DetectionStrategy::Merge || strategy == DetectionStrategy::All)
    && is_merged
    && let Some(merge_integrated) = merge::detect_merge_status(git, repo, branch_name, baseline, is_merged)?
  {
    return Ok(merge_integrated);
  }

  // 2) Rebase/cherry-pick detection via marker scan
  let (total_right, orphaned_right, integrated_right) = rebase::scan_right_side_marks(git, repo, baseline, branch_name)?;
  let mut status = rebase::detect_rebase_status_with_marks(git, repo, branch_name, baseline, total_right, orphaned_right, integrated_right)?;

  // 3) Squash detection fallback for branches with no integrated commits
  if matches!(status, BranchIntegrationStatus::NotIntegrated { integrated_count: 0, .. }) && (strategy == DetectionStrategy::Squash || strategy == DetectionStrategy::All) {
    let right_count = orphaned_right as usize;
    if let Some(squash_integrated) = squash::detect_squash_status(git, repo, branch_name, baseline, right_count)? {
      status = squash_integrated;
    }
  }

  Ok(status)
}
