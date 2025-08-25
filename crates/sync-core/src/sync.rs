use crate::branch_processor::{BranchProcessingParams, process_single_branch};
use crate::commit_grouper::CommitGrouper;
use crate::issue_navigation::load_issue_navigation_config;
use anyhow::{Result, anyhow};
use branch_integration::{detector::detect_integrated_branches, strategy::DetectionStrategy};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cache::TreeIdCache;
use git_ops::commit_list::{Commit, get_commit_list_with_handler};
use indexmap::IndexMap;
use std::sync::{Arc, Mutex};
use sync_types::issue_navigation::IssueNavigationConfig;
use sync_types::{GroupedBranchInfo, ProgressReporter, SyncEvent};
use sync_utils::issue_pattern::{find_issue_number, find_issue_range};
use tokio::task::JoinSet;
use tracing::{debug, error, info, instrument, warn};

/// Options for configuring sync behavior
pub struct SyncOptions {
  /// Optional cached issue navigation configuration
  pub cached_issue_config: Option<IssueNavigationConfig>,
  /// Optional detection strategy override (defaults to production strategy)
  pub detection_strategy: Option<DetectionStrategy>,
  /// Archive cleanup retention in days (older archived branches will be deleted)
  /// Defaults to the current retention used by branch-integration (7 days).
  pub archive_retention_days: u64,
}

impl Default for SyncOptions {
  fn default() -> Self {
    Self {
      cached_issue_config: None,
      detection_strategy: None,
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

    return Err(anyhow!("No baseline branch found. Repository has no remotes and no master/main branch."));
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
      detection_strategy: None,
      ..Default::default()
    },
  )
  .await
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
    // No commits to sync, but still check for integrated branches
    let empty_map = IndexMap::new();

    // Use provided strategy or default to production strategy
    let strategy = options.detection_strategy.unwrap_or_else(branch_integration::strategy::get_detection_strategy);

    detect_integrated_branches(
      git_executor,
      repository_path,
      branch_prefix,
      &baseline_branch,
      branch_integration::detector::DetectConfig {
        grouped_commits: &empty_map,
        progress: &progress,
        strategy,
        retention_days: options.archive_retention_days,
      },
    )
    .await?;

    progress.send(SyncEvent::Completed)?;
    return Ok(());
  }

  let oldest_head_commit = grouper
    .oldest_commit
    .as_ref()
    .ok_or_else(|| anyhow::anyhow!("No oldest commit found despite having commits"))?;

  // Get parent commit hash using git CLI
  let parent_commit_hash = {
    let parent_ref = format!("{}^", oldest_head_commit.id);
    let args = vec!["rev-parse", &parent_ref];
    let output = git_executor.execute_command(&args, repository_path)?;
    output.trim().to_string()
  };

  // group commits by prefix first to get all branch names
  let (grouped_commits, unassigned_commits) = grouper.finish();

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

  // Build author email frequency map to derive identity
  let mut author_freq: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

  // Send grouped branch info so UI can render the structure
  // Create branches with latest commit time for sorting
  let mut grouped_branches_for_ui: Vec<GroupedBranchInfo> = grouped_commits
    .iter()
    .map(|(branch_name, commits)| {
      // Find the latest committer time in this branch
      let latest_commit_time = commits.iter().map(|commit| commit.committer_timestamp).max().unwrap_or(0);

      // Compute summary for issue-based branches
      let summary = if find_issue_number(branch_name).is_some() && !commits.is_empty() {
        // For issue-based branches, use the first commit's stripped_subject as summary
        let first_commit = &commits[0];
        let subject = &first_commit.stripped_subject;

        // Remove the issue number from the beginning if present
        if let Some((_, end)) = find_issue_range(subject) {
          let after_issue = subject[end..].trim_start_matches([' ', ':']).trim();
          if after_issue.is_empty() { String::new() } else { after_issue.to_string() }
        } else {
          subject.to_string()
        }
      } else {
        String::new()
      };

      // Tally author emails for identity derivation
      for c in commits.iter() {
        if !c.author_email.is_empty() {
          *author_freq.entry(c.author_email.clone()).or_insert(0) += 1;
        }
      }

      GroupedBranchInfo {
        name: branch_name.clone(),
        latest_commit_time,
        summary,
        commits: commits
          .iter()
          .rev() // Reverse to show newest commits first within branch
          .cloned()
          .collect(),
      }
    })
    .collect();

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

  progress.send(SyncEvent::BranchesGrouped {
    branches: grouped_branches_for_ui.clone(),
  })?;

  // Derive identity (most frequent author email)
  let my_email: Option<String> = author_freq.into_iter().max_by_key(|(_, count)| *count).map(|(email, _)| email);

  // Create a shared cache for tree IDs for this sync operation
  let tree_id_cache = TreeIdCache::new();

  // Create a mutex for git notes writing
  let git_notes_mutex = Arc::new(Mutex::new(()));

  // Clone values needed for both concurrent tasks
  let grouped_commits_clone = grouped_commits.clone();
  let git_executor_clone = git_executor.clone();
  let repository_path_str = repository_path.to_string();
  let branch_prefix_str = branch_prefix.to_string();
  let baseline_branch_str = baseline_branch.to_string();
  let progress_clone = progress.clone();
  let strategy = options.detection_strategy.unwrap_or_else(branch_integration::strategy::get_detection_strategy);

  // Spawn branch processing task
  let branch_processing_handle = tokio::spawn({
    let repository_path = repository_path.to_string();
    let branch_prefix = branch_prefix.to_string();
    let parent_commit_hash = parent_commit_hash.clone();
    let progress = progress.clone();
    let git_executor = git_executor.clone();
    let grouped_commits = grouped_commits.clone();
    let baseline_branch = baseline_branch.to_string();

    async move {
      // Process branches in parallel using JoinSet
      let mut set = JoinSet::new();

      for (current_branch_idx, (branch_name, commits)) in grouped_commits.into_iter().enumerate() {
        let params = BranchProcessingParams {
          repository_path: repository_path.clone(),
          branch_prefix: branch_prefix.clone(),
          branch_name,
          commits,
          parent_commit_hash: parent_commit_hash.clone(),
          current_branch_idx,
          total_branches,
          progress: progress.clone(),
          git_executor: git_executor.clone(),
          tree_id_cache: tree_id_cache.clone(),
          git_notes_mutex: git_notes_mutex.clone(),
          my_email: my_email.clone(),
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
        strategy,
        retention_days: options.archive_retention_days,
      },
    )
    .await
  });

  // Wait for both tasks to complete using try_join
  let (branch_result, integration_result) = tokio::try_join!(branch_processing_handle, integration_detection_handle)?;

  // Check results
  branch_result?;
  integration_result?;

  progress.send(SyncEvent::Completed)?;

  Ok(())
}
