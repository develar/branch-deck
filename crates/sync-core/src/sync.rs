use crate::baseline::detect_baseline_branch;
use crate::branch_processor::{BranchJob, BranchSyncContext, process_single_branch};
use crate::commit_grouper::{CommitGrouper, GroupedCommits};
use crate::issue_navigation::load_issue_navigation_config;
use anyhow::{Result, anyhow};
use branch_integration::archive::ARCHIVE_RETENTION_DAYS;
use branch_integration::detector::{DetectConfig, detect_integrated_branches};
use branch_integration::strategy::DetectionStrategy;
use git_executor::concurrency::max_parallel_git_processes;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cache::TreeIdCache;
use git_ops::commit_list::{Commit, get_commit_list_with_handler};
use indexmap::IndexMap;
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use sync_types::issue_navigation::IssueNavigationConfig;
use sync_types::{GroupedBranchInfo, ProgressReporter, SyncEvent};
use sync_utils::issue_pattern::{find_issue_range, has_issue_reference};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{debug, error, info, instrument};

/// Where the sync takes the issue navigation configuration from
pub enum IssueConfigSource {
  /// Read `.idea/vcs.xml` from the repository
  Load,
  /// Use an already loaded configuration; `None` means the repository has none
  Cached(Option<IssueNavigationConfig>),
}

/// Options for configuring sync behavior
pub struct SyncOptions {
  /// Issue navigation configuration source
  pub issue_config: IssueConfigSource,
  /// Detection strategy for integration detection
  pub detection_strategy: DetectionStrategy,
  /// Archive cleanup retention in days (older fully integrated archived branches are deleted)
  pub archive_retention_days: u64,
}

impl Default for SyncOptions {
  fn default() -> Self {
    Self {
      issue_config: IssueConfigSource::Load,
      detection_strategy: branch_integration::strategy::get_detection_strategy(),
      archive_retention_days: ARCHIVE_RETENTION_DAYS,
    }
  }
}

/// Compute summary for a branch based on its name and commits
/// For issue-based branches, extracts the commit message after the issue reference
/// Searches in reverse order to skip cleanup commits and find meaningful ones
fn compute_branch_summary(branch_name: &str, commits: &[Commit]) -> String {
  if !has_issue_reference(branch_name) || commits.is_empty() {
    return String::new();
  }

  let mut fallback_summary: Option<String> = None;

  // Search commits in reverse order (oldest first) to find a suitable summary
  for commit in commits.iter().rev() {
    // Strip issue number if present
    let summary = if let Some((_, end)) = find_issue_range(&commit.stripped_subject) {
      commit.stripped_subject[end..].trim_start_matches([' ', ':']).trim()
    } else {
      commit.stripped_subject.as_str()
    };

    // Skip empty summaries
    if summary.is_empty() {
      continue;
    }

    // Check if it's a cleanup/refactor/format commit AFTER stripping issue prefix
    // Use ASCII case-insensitive comparison for efficiency
    // `get` instead of slicing: the byte length of the keyword may fall inside a multi-byte character
    let starts_with_keyword = |keyword: &str| summary.get(..keyword.len()).is_some_and(|start| start.eq_ignore_ascii_case(keyword));
    if starts_with_keyword("cleanup") || starts_with_keyword("refactor") || starts_with_keyword("format") {
      // Save the first non-empty summary as fallback (even if it's cleanup)
      if fallback_summary.is_none() {
        fallback_summary = Some(summary.to_string());
      }
      continue;
    }

    // Found a non-cleanup summary, return it
    return summary.to_string();
  }

  // If all commits are cleanup/refactor/format, use the fallback (first non-empty)
  fallback_summary.unwrap_or_default()
}

/// Prepare grouped commits for UI display with sorting and metadata
pub(crate) fn prepare_branches_for_ui(grouped_commits: &IndexMap<String, Vec<Commit>>, branch_emails: &HashMap<String, Option<String>>) -> Vec<GroupedBranchInfo> {
  let mut grouped_branches_for_ui = Vec::with_capacity(grouped_commits.len());

  for (branch_name, commits) in grouped_commits {
    // Compute summary for issue-based branches
    let summary = compute_branch_summary(branch_name, commits);

    // Single pass to find latest commit time only (author emails are pre-computed)
    let mut latest_commit_time = 0u32;
    for c in commits {
      // Track latest committer timestamp
      if c.committer_timestamp > latest_commit_time {
        latest_commit_time = c.committer_timestamp;
      }
    }

    // Get pre-computed most frequent author email for this branch
    let branch_my_email = branch_emails.get(branch_name).cloned().flatten();

    grouped_branches_for_ui.push(GroupedBranchInfo {
      name: branch_name.clone(),
      latest_commit_time,
      summary,
      all_commits_have_issue_references: {
        if commits.is_empty() {
          false
        } else if has_issue_reference(branch_name) {
          // If branch is grouped by issue reference, all commits likely have issue references
          // (they were grouped for this reason, so this is a strong heuristic)
          true
        } else {
          // For non-issue branches (like "(feature-auth)"), check each commit
          commits.iter().all(|c| has_issue_reference(&c.stripped_subject))
        }
      },
      my_email: branch_my_email,
      commits: commits
        .iter()
        .rev() // Reverse to show newest commits first within branch
        .cloned()
        .collect(),
    });
  }

  // Sort branches by latest commit time (newest first). If committer times are equal (e.g.,
  // after a rebase that rewrites many commits at once), tie-break by latest author time.
  // Finally, use name for a stable ordering. The key is computed once per branch.
  grouped_branches_for_ui.sort_by_cached_key(|branch| {
    let latest_author_time = branch.commits.iter().map(|c| c.author_timestamp).max().unwrap_or(0);
    (Reverse(branch.latest_commit_time), Reverse(latest_author_time), branch.name.clone())
  });

  grouped_branches_for_ui
}

/// Result of the sequential first phase of a sync
struct PreparedSync {
  baseline_branch: String,
  branches: IndexMap<String, Vec<Commit>>,
  branch_emails: HashMap<String, Option<String>>,
  /// Parent of the oldest commit ahead of the baseline; `None` without commits (or when that commit is a root)
  first_parent_id: Option<String>,
}

/// Phase 1 (blocking): issue config, baseline, commit grouping. Sends `IssueNavigationConfig`, `UnassignedCommits`
/// and then `BranchesGrouped` (also when empty, so the UI drops stale branches) before any per-branch event can exist.
fn prepare_sync(git_executor: &GitCommandExecutor, repository_path: &str, issue_config: IssueConfigSource, progress: &dyn ProgressReporter) -> Result<PreparedSync> {
  // Use cached issue config if available (including a cached "none"), otherwise load it
  let issue_config = match issue_config {
    IssueConfigSource::Cached(cached) => {
      debug!("Using cached issue navigation config");
      cached
    }
    IssueConfigSource::Load => {
      debug!("Loading issue navigation config");
      load_issue_navigation_config(repository_path)
    }
  };
  progress.send(SyncEvent::IssueNavigationConfig { config: issue_config })?;

  // Detect the baseline branch (origin/master, origin/main, or local master/main)
  let baseline_branch = detect_baseline_branch(git_executor, repository_path, "master")?;

  // Use streaming commit processing
  let mut grouper = CommitGrouper::new();
  get_commit_list_with_handler(git_executor, repository_path, &baseline_branch, |commit| {
    grouper.add_commit(commit);
    Ok(())
  })?;

  // The virtual branches are rebuilt on top of the parent of the oldest commit
  let first_parent_id = grouper.first_parent_id.take();
  let commit_count = grouper.commit_count;
  let GroupedCommits {
    branches,
    unassigned,
    branch_emails,
  } = grouper.finish();
  info!(commit_count, total_branches = branches.len(), "Fetched and grouped commits");

  // Always send unassigned commits (even if empty) to ensure the frontend updates; newest first
  progress.send(SyncEvent::UnassignedCommits {
    commits: unassigned.into_iter().rev().collect(),
  })?;

  progress.send(SyncEvent::BranchesGrouped {
    branches: prepare_branches_for_ui(&branches, &branch_emails),
    baseline_branch: baseline_branch.clone(),
  })?;

  Ok(PreparedSync {
    baseline_branch,
    branches,
    branch_emails,
    first_parent_id,
  })
}

/// Rebuild every virtual branch on the blocking pool, at most `max_parallel_git_processes()` at a time.
/// Returns the first failure, naming its branch.
async fn process_branches<P: ProgressReporter + Clone + 'static>(
  context: BranchSyncContext<P>,
  branches: IndexMap<String, Vec<Commit>>,
  mut branch_emails: HashMap<String, Option<String>>,
) -> Result<()> {
  let context = Arc::new(context);
  let semaphore = Arc::new(Semaphore::new(max_parallel_git_processes()));
  let mut tasks = JoinSet::new();

  for (branch_idx, (branch_name, commits)) in branches.into_iter().enumerate() {
    let permit = semaphore.clone().acquire_owned().await?;
    let job = BranchJob {
      my_email: branch_emails.remove(&branch_name).flatten(),
      branch_name,
      commits,
      branch_idx,
    };
    let context = context.clone();
    tasks.spawn_blocking(move || {
      let _permit = permit;
      let branch_name = job.branch_name.clone();
      (branch_name, process_single_branch(&context, job))
    });
  }

  let mut first_error = None;
  while let Some(joined) = tasks.join_next().await {
    match joined {
      Ok((_, Ok(()))) => {}
      Ok((branch_name, Err(e))) => {
        // The error status has already been sent by process_single_branch
        error!(branch = %branch_name, error = ?e, "Branch processing failed");
        first_error.get_or_insert(e.context(format!("Failed to sync branch {branch_name}")));
      }
      Err(e) => {
        error!(error = %e, "Branch processing task failed");
        first_error.get_or_insert(anyhow!("Branch processing task failed: {e}"));
      }
    }
  }
  first_error.map_or(Ok(()), Err)
}

/// Sync virtual branches: group the commits ahead of the baseline by prefix, rebuild one virtual branch per group,
/// and detect integration of archived branches concurrently. All git work runs on the blocking pool.
#[instrument(skip(git_executor, progress, options), fields(repository_path = %repository_path, branch_prefix = %branch_prefix, cached_issue_config = matches!(options.issue_config, IssueConfigSource::Cached(_))))]
pub async fn sync_branches<P: ProgressReporter + Clone + 'static>(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  progress: P,
  options: SyncOptions,
) -> Result<()> {
  let SyncOptions {
    issue_config,
    detection_strategy,
    archive_retention_days,
  } = options;

  // Phase 1: sequential, BranchesGrouped is on the wire before any per-branch task starts
  let PreparedSync {
    baseline_branch,
    branches,
    branch_emails,
    first_parent_id,
  } = tokio::task::spawn_blocking({
    let git_executor = git_executor.clone();
    let repository_path = repository_path.to_string();
    let progress = progress.clone();
    move || prepare_sync(&git_executor, &repository_path, issue_config, &progress)
  })
  .await
  .map_err(|e| anyhow!("Sync preparation task failed: {e}"))??;

  // Phase 2: integration detection and branch processing run concurrently
  let active_branches: HashSet<String> = branches.keys().cloned().collect();
  let detection = tokio::task::spawn_blocking({
    let git_executor = git_executor.clone();
    let repository_path = repository_path.to_string();
    let branch_prefix = branch_prefix.to_string();
    let baseline_branch = baseline_branch.clone();
    let progress = progress.clone();
    move || {
      detect_integrated_branches(
        &git_executor,
        &repository_path,
        &branch_prefix,
        &baseline_branch,
        DetectConfig {
          active_branches: &active_branches,
          progress: &progress,
          strategy: detection_strategy,
          retention_days: archive_retention_days,
        },
      )
    }
  });

  let processing = async {
    if branches.is_empty() {
      info!("No commits ahead of baseline, only checking for integrated branches");
      return Ok(());
    }
    let parent_commit_hash = first_parent_id.ok_or_else(|| anyhow!("The oldest commit ahead of the baseline has no parent"))?;
    let context = BranchSyncContext {
      repository_path: repository_path.to_string(),
      branch_prefix: branch_prefix.to_string(),
      parent_commit_hash,
      baseline_branch,
      total_branches: branches.len(),
      progress,
      git_executor: git_executor.clone(),
      tree_id_cache: TreeIdCache::new(),
      git_notes_mutex: Mutex::new(()),
    };
    process_branches(context, branches, branch_emails).await
  };

  // join (not try_join) so a failure on one side doesn't orphan the other
  let (processing_result, detection_result) = tokio::join!(processing, detection);
  processing_result?;
  detection_result.map_err(|e| anyhow!("Integration detection task failed: {e}"))?
}
