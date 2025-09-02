use crate::branch_processor::{BranchProcessingParams, process_single_branch};
use crate::commit_grouper::CommitGrouper;
use crate::issue_navigation::load_issue_navigation_config;
use anyhow::{Result, anyhow};
use branch_integration::{detector::detect_integrated_branches, strategy::DetectionStrategy};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cache::TreeIdCache;
use git_ops::commit_list::{Commit, get_commit_list_with_handler};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use sync_types::issue_navigation::IssueNavigationConfig;
use sync_types::ordered_progress_reporter::OrderedProgressReporter;
use sync_types::{GroupedBranchInfo, ProgressReporter, SyncEvent};
use sync_utils::issue_pattern::{find_issue_range, has_issue_reference};
use tokio::task::JoinSet;
use tracing::{debug, error, info, instrument, warn};

/// Options for configuring sync behavior
pub struct SyncOptions {
  /// Optional cached issue navigation configuration
  pub cached_issue_config: Option<IssueNavigationConfig>,
  /// Detection strategy for integration detection
  pub detection_strategy: DetectionStrategy,
  /// Archive cleanup retention in days (older archived branches will be deleted)
  /// Defaults to the current retention used by branch-integration (7 days).
  pub archive_retention_days: u64,
}

impl Default for SyncOptions {
  fn default() -> Self {
    Self {
      cached_issue_config: None,
      detection_strategy: branch_integration::strategy::get_detection_strategy(),
      // Keep in sync with branch_integration::archive::ARCHIVE_RETENTION_DAYS (currently 7)
      archive_retention_days: 7,
    }
  }
}

/// Detect the baseline branch for a repository
///
/// This function attempts to find the appropriate baseline branch using the following strategies:
/// 1. For repositories without remotes: checks for local preferred branch, then master/main
/// 2. For repositories with remotes:
///    - First tries to get the upstream tracking branch (@{u})
///    - Then tries remote/preferred_branch (e.g., origin/master)
///    - Finally tries remote/master and remote/main
///
/// # Arguments
/// * `git_executor` - The git command executor
/// * `repository_path` - Path to the repository
/// * `preferred_branch` - The preferred branch name to use as baseline (typically "master")
///
/// # Returns
/// The name of the baseline branch (e.g., "master", "origin/master", "upstream/main")
///
/// # Errors
/// Returns an error if no suitable baseline branch can be found
#[instrument(skip(git_executor))]
pub fn detect_baseline_branch(git_executor: &GitCommandExecutor, repository_path: &str, preferred_branch: &str) -> Result<String> {
  // First, check if we have any remotes
  let remotes_output = git_executor.execute_command(&["--no-pager", "remote"], repository_path)?;
  let has_remotes = !remotes_output.trim().is_empty();

  if !has_remotes {
    // Local repository without remotes
    // Check if the preferred branch exists locally
    if git_executor
      .execute_command(&["--no-pager", "rev-parse", "--verify", preferred_branch], repository_path)
      .is_ok()
    {
      return Ok(preferred_branch.to_string());
    }

    // Try common branch names
    for branch in &["master", "main"] {
      if git_executor.execute_command(&["--no-pager", "rev-parse", "--verify", branch], repository_path).is_ok() {
        return Ok(branch.to_string());
      }
    }

    return Err(anyhow!("No baseline branch found. Repository has no remotes and no main branch."));
  }

  // Try to get the upstream branch for the current branch
  if let Ok(upstream) = git_executor.execute_command(&["--no-pager", "rev-parse", "--abbrev-ref", "@{u}"], repository_path) {
    return Ok(upstream);
  }

  // Try to find the remote tracking branch for the preferred branch
  // Get the first remote (usually "origin")
  let first_remote = remotes_output.lines().next().unwrap_or("origin");

  // Try the preferred branch with the remote
  let remote_branch = format!("{first_remote}/{preferred_branch}");
  if git_executor
    .execute_command(&["--no-pager", "rev-parse", "--verify", &remote_branch], repository_path)
    .is_ok()
  {
    return Ok(remote_branch);
  }

  // Try common branch names with the remote
  for branch in &["master", "main"] {
    let remote_branch = format!("{first_remote}/{branch}");
    if git_executor
      .execute_command(&["--no-pager", "rev-parse", "--verify", &remote_branch], repository_path)
      .is_ok()
    {
      return Ok(remote_branch);
    }
  }

  Err(anyhow!(
    "No baseline branch found. Tried upstream tracking, {}/{{{},master,main}}",
    first_remote,
    preferred_branch
  ))
}

/// Core sync branches logic without Tauri dependencies
#[instrument(skip(git_executor, progress), fields(repository_path = %repository_path, branch_prefix = %branch_prefix))]
pub async fn sync_branches_core<P: ProgressReporter + Clone + 'static>(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str, progress: P) -> Result<()> {
  // Delegate to the version with cache support, passing None for cached state
  sync_branches_core_with_cache(git_executor, repository_path, branch_prefix, progress, None).await
}

/// Core sync branches logic with optional cached issue config
#[instrument(skip(git_executor, progress, cached_issue_config), fields(repository_path = %repository_path, branch_prefix = %branch_prefix, cached_issue_config = cached_issue_config.is_some()))]
pub async fn sync_branches_core_with_cache<P: ProgressReporter + Clone + 'static>(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  progress: P,
  cached_issue_config: Option<IssueNavigationConfig>,
) -> Result<()> {
  sync_branches(
    git_executor,
    repository_path,
    branch_prefix,
    progress,
    SyncOptions {
      cached_issue_config,
      ..Default::default()
    },
  )
  .await
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
    if (summary.len() >= 7 && summary[..7].eq_ignore_ascii_case("cleanup"))
      || (summary.len() >= 8 && summary[..8].eq_ignore_ascii_case("refactor"))
      || (summary.len() >= 6 && summary[..6].eq_ignore_ascii_case("format"))
    {
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
fn prepare_branches_for_ui(grouped_commits: &IndexMap<String, Vec<Commit>>, branch_emails: &HashMap<String, Option<String>>) -> Vec<GroupedBranchInfo> {
  let mut grouped_branches_for_ui = Vec::with_capacity(grouped_commits.len());

  for (branch_name, commits) in grouped_commits {
    // Compute summary for issue-based branches
    let summary = compute_branch_summary(branch_name, commits);

    // Single pass to find latest commit time only (author emails are pre-computed)
    let mut latest_commit_time = 0u32;
    for c in commits.iter() {
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
          commits.iter().all(|c| has_issue_reference(&c.subject))
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
  // Finally, use name for a stable ordering.
  grouped_branches_for_ui.sort_by(|a, b| {
    use std::cmp::Ordering;

    let primary = b.latest_commit_time.cmp(&a.latest_commit_time);
    if primary != Ordering::Equal {
      return primary;
    }

    let a_author_latest = a.commits.iter().map(|c| c.author_timestamp).max().unwrap_or(0);
    let b_author_latest = b.commits.iter().map(|c| c.author_timestamp).max().unwrap_or(0);

    let secondary = b_author_latest.cmp(&a_author_latest);
    if secondary != Ordering::Equal {
      return secondary;
    }

    // Stable final tie-breaker
    a.name.cmp(&b.name)
  });

  grouped_branches_for_ui
}

/// Get the parent commit hash of the oldest commit
fn get_parent_commit_hash(git_executor: &GitCommandExecutor, repository_path: &str, oldest_commit: Option<&Commit>) -> Result<String> {
  let oldest_head_commit = oldest_commit.ok_or_else(|| anyhow::anyhow!("No oldest commit found despite having commits"))?;

  let parent_ref = format!("{}^", oldest_head_commit.id);
  Ok(git_executor.execute_command(&["rev-parse", &parent_ref], repository_path)?.trim().to_string())
}

/// Core sync branches logic
#[instrument(skip(git_executor, progress, options), fields(repository_path = %repository_path, branch_prefix = %branch_prefix, cached_issue_config = options.cached_issue_config.is_some()))]
pub async fn sync_branches<P: ProgressReporter + Clone + 'static>(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  progress: P,
  options: SyncOptions,
) -> Result<()> {
  // Use cached issue config if available, otherwise load it
  let issue_config = if let Some(cached) = options.cached_issue_config {
    debug!("Using cached issue navigation config");
    Some(cached)
  } else {
    debug!("Loading issue navigation config");
    load_issue_navigation_config(repository_path)
  };

  // Send issue navigation config at the beginning
  progress.send(SyncEvent::IssueNavigationConfig { config: issue_config })?;

  // Detect the baseline branch (origin/master, origin/main, or local master/main)
  let baseline_branch = detect_baseline_branch(git_executor, repository_path, "master")?;

  // Use streaming commit processing
  let mut grouper = CommitGrouper::new();

  get_commit_list_with_handler(git_executor, repository_path, &baseline_branch, |commit| {
    grouper.add_commit(commit);
    Ok(())
  })?;

  // Check if we have any commits
  if grouper.commit_count == 0 {
    info!(commit_count = 0, "No commits ahead of baseline, checking for integrated branches");
    detect_integrated_branches(
      git_executor,
      repository_path,
      branch_prefix,
      &baseline_branch,
      branch_integration::detector::DetectConfig {
        grouped_commits: &IndexMap::new(),
        progress: &progress,
        strategy: options.detection_strategy,
        retention_days: options.archive_retention_days,
      },
    )
    .await?;

    // Send empty unassigned commits to clear any stale data from previous sync
    progress.send(SyncEvent::UnassignedCommits { commits: Vec::new() })?;

    return Ok(());
  }

  // Extract oldest commit before consuming grouper
  let oldest_commit = grouper.oldest_commit.clone();

  // group commits by prefix first to get all branch names
  let (grouped_commits, unassigned_commits, branch_emails) = grouper.finish();

  let total_branches = grouped_commits.len();

  info!(total_branches, "Fetched and grouped commits");

  // Always send unassigned commits (even if empty) to ensure frontend updates
  let unassigned_commits_for_ui: Vec<Commit> = if unassigned_commits.is_empty() {
    Vec::new()
  } else {
    unassigned_commits
      .into_iter()
      .rev() // Reverse to show newest commits first
      .collect()
  };

  progress.send(SyncEvent::UnassignedCommits {
    commits: unassigned_commits_for_ui,
  })?;

  // Create ordered progress reporter to ensure correct event ordering
  let ordered_progress = OrderedProgressReporter::new(progress.clone());

  let ui_preparation_handle = tokio::spawn({
    let grouped_commits = grouped_commits.clone();
    let branch_emails = branch_emails.clone();
    let baseline_branch = baseline_branch.clone();
    let ordered_progress = ordered_progress.clone();

    async move {
      let grouped_branches_for_ui = prepare_branches_for_ui(&grouped_commits, &branch_emails);
      ordered_progress.send(SyncEvent::BranchesGrouped {
        branches: grouped_branches_for_ui,
        baseline_branch,
      })
    }
  });

  let branch_processing_handle = tokio::spawn({
    let repository_path = repository_path.to_string();
    let branch_prefix = branch_prefix.to_string();
    let oldest_commit = oldest_commit.clone();
    let ordered_progress = ordered_progress.clone();
    let git_executor = git_executor.clone();
    let grouped_commits = grouped_commits.clone();
    let baseline_branch = baseline_branch.to_string();
    let branch_emails = branch_emails.clone();

    async move {
      // Compute parent commit hash inside the spawned task
      let parent_commit_hash = get_parent_commit_hash(&git_executor, &repository_path, oldest_commit.as_ref())?;

      // Create git notes mutex inside the spawned task
      let git_notes_mutex = Arc::new(Mutex::new(()));

      // Create tree ID cache inside the spawned task
      let tree_id_cache = TreeIdCache::new();

      // Process branches in parallel using JoinSet
      let mut set = JoinSet::new();

      for (current_branch_idx, (branch_name, commits)) in grouped_commits.into_iter().enumerate() {
        // Use pre-computed author email with O(1) HashMap lookup
        let branch_my_email = branch_emails.get(&branch_name).cloned().flatten();

        let params = BranchProcessingParams {
          repository_path: repository_path.clone(),
          branch_prefix: branch_prefix.clone(),
          branch_name,
          commits,
          parent_commit_hash: parent_commit_hash.clone(),
          current_branch_idx,
          total_branches,
          progress: ordered_progress.clone(),
          git_executor: git_executor.clone(),
          tree_id_cache: tree_id_cache.clone(),
          git_notes_mutex: git_notes_mutex.clone(),
          my_email: branch_my_email,
          baseline_branch: baseline_branch.clone(),
        };

        // Use spawn_blocking since process_single_branch is a sync function doing blocking I/O
        set.spawn_blocking(move || process_single_branch(params));
      }

      // Wait for all branches to complete
      let mut has_error = false;
      while let Some(result) = set.join_next().await {
        match result {
          Ok(Ok(())) => {
            // Branch processed successfully
          }
          Ok(Err(e)) => {
            // Error status has already been sent by process_single_branch
            error!(error = ?e, "Branch processing failed");
            has_error = true;
          }
          Err(e) => {
            error!(error = %e, "JoinSet spawn_blocking error during branch processing");
            has_error = true;
          }
        }
      }

      if has_error {
        Err(anyhow!("One or more branch processing tasks failed"))
      } else {
        Ok(())
      }
    }
  });

  // Clone values needed for integration detection task
  let grouped_commits_clone = grouped_commits.clone();
  let git_executor_clone = git_executor.clone();
  let repository_path_str = repository_path.to_string();
  let branch_prefix_str = branch_prefix.to_string();
  let baseline_branch_str = baseline_branch.to_string();
  let progress_clone = progress.clone();

  // Spawn integration detection task - runs concurrently with branch processing
  let integration_detection_handle = tokio::spawn(async move {
    detect_integrated_branches(
      &git_executor_clone,
      &repository_path_str,
      &branch_prefix_str,
      &baseline_branch_str,
      branch_integration::detector::DetectConfig {
        grouped_commits: &grouped_commits_clone,
        progress: &progress_clone,
        strategy: options.detection_strategy,
        retention_days: options.archive_retention_days,
      },
    )
    .await
  });

  // Wait for all three tasks to complete using try_join
  let (branch_result, ui_result, integration_result) = tokio::try_join!(branch_processing_handle, ui_preparation_handle, integration_detection_handle)?;

  // Check results
  branch_result?;
  ui_result?;
  integration_result?;

  Ok(())
}
