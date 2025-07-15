use crate::git::cache::TreeIdCache;
use crate::git::commit_list::{Commit, get_commit_list};
use crate::git::copy_commit::{CopyCommitError, CreateCommitParams, create_or_update_commit};
use crate::git::git_command::GitCommandExecutor;
use crate::git::model::{BranchError, BranchSyncStatus, CommitDetail, CommitInfo, CommitSyncStatus, to_final_branch_name};
use crate::git::notes::{CommitNoteInfo, write_commit_notes};
use crate::progress::{GroupedBranchInfo, SyncEvent};
use indexmap::IndexMap;
use regex::Regex;
use std::sync::Mutex;
use std::sync::{Arc, OnceLock};
use tauri::State;
use tauri::ipc::Channel;
use tracing::{debug, error, info, instrument, warn};

/// Parameters for processing a single branch
struct BranchProcessingParams {
  repository_path: String,
  branch_prefix: String,
  branch_name: String,
  commit_data: Vec<CommitInfo>,
  parent_commit_hash: String,
  current_branch_idx: usize,
  total_branches: usize,
  progress: Channel<SyncEvent>,
  git_notes_mutex: Arc<Mutex<()>>,
  git_executor: GitCommandExecutor,
  tree_id_cache: TreeIdCache,
}

/// Synchronizes branches by grouping commits by prefix and creating/updating branches
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, progress))]
pub async fn sync_branches(git_executor: State<'_, GitCommandExecutor>, repository_path: &str, branch_prefix: &str, progress: Channel<SyncEvent>) -> Result<(), String> {
  info!("Starting branch synchronization for repository: {repository_path}, prefix: {branch_prefix}");
  do_sync_branches(git_executor, repository_path, branch_prefix, progress).await.map_err(|e| {
    error!("Branch synchronization failed: {e}");
    format!("{e:?}")
  })
}

async fn do_sync_branches(git_executor: State<'_, GitCommandExecutor>, repository_path: &str, branch_prefix: &str, progress: Channel<SyncEvent>) -> anyhow::Result<()> {
  progress.send(SyncEvent::Progress {
    message: "detecting baseline branch".to_string(),
    index: -1,
  })?;

  // Detect the baseline branch (origin/master, origin/main, or local master/main)
  let baseline_branch = git_executor.detect_baseline_branch(repository_path, "master")?;

  progress.send(SyncEvent::Progress {
    message: format!("getting commits from {baseline_branch}"),
    index: -1,
  })?;

  // Use the new GitCommandExecutor-based API
  let commits = get_commit_list(&git_executor, repository_path, &baseline_branch)?;

  let Some(oldest_head_commit) = commits.first() else {
    // No commits to sync
    progress.send(SyncEvent::Completed)?;
    return Ok(());
  };

  // Get parent commit hash using git CLI
  let parent_commit_hash = {
    let parent_ref = format!("{}^", oldest_head_commit.id);
    let args = vec!["rev-parse", &parent_ref];
    let output = git_executor.execute_command(&args, repository_path)?;
    output.trim().to_string()
  };

  // group commits by prefix first to get all branch names
  // for each prefix, create a branch with only the relevant commits
  let (grouped_commits, unassigned_commits) = group_commits_by_prefix_new(&commits);

  // extract commit IDs and messages for processing
  let grouped_commit_data: Vec<(String, Vec<CommitInfo>)> = grouped_commits
    .into_iter()
    .map(|(branch_name, branch_commits)| {
      let commit_data = branch_commits
        .into_iter()
        .map(|(message, commit)| CommitInfo {
          message,
          id: commit.id.clone(),
          author_name: commit.author_name.clone(),
          author_email: commit.author_email.clone(),
          author_time: commit.author_timestamp,
          committer_time: commit.committer_timestamp,
          parent_id: commit.parent_id.clone(),
          tree_id: commit.tree_id.clone(),
          mapped_commit_id: commit.mapped_commit_id.clone(),
        })
        .collect();
      (branch_name, commit_data)
    })
    .collect();

  let total_branches = grouped_commit_data.len();

  info!("Fetched and grouped commits. Total branches: {total_branches}.");

  // Send grouped branch info immediately so UI can render the structure
  // Reverse to show newest branches first (commits are processed oldest to newest)
  let grouped_branches_for_ui: Vec<GroupedBranchInfo> = grouped_commit_data
    .iter()
    .rev() // Reverse the order to show newest branches first
    .map(|(branch_name, commits)| GroupedBranchInfo {
      name: branch_name.clone(),
      commits: commits
        .iter()
        .rev() // Reverse to show newest commits first
        .map(|commit| CommitDetail {
          original_hash: commit.id.to_string(),
          hash: String::new(), // Will be filled after sync
          message: commit.message.clone(),
          author: commit.author_name.clone(),
          author_time: commit.author_time,
          committer_time: commit.committer_time,
          status: CommitSyncStatus::Pending,
          error: None,
        })
        .collect(),
    })
    .collect();

  progress.send(SyncEvent::BranchesGrouped {
    branches: grouped_branches_for_ui,
  })?;

  // Send unassigned commits if any
  if !unassigned_commits.is_empty() {
    let unassigned_commits_for_ui: Vec<CommitDetail> = unassigned_commits
      .iter()
      .rev() // Reverse to show newest commits first
      .map(|commit| CommitDetail {
        original_hash: commit.id.to_string(),
        hash: String::new(), // No synced hash for unassigned commits
        message: commit.message.clone(),
        author: commit.author_name.clone(),
        author_time: commit.author_timestamp,
        committer_time: commit.committer_timestamp,
        status: CommitSyncStatus::Pending,
        error: None,
      })
      .collect();

    progress.send(SyncEvent::UnassignedCommits {
      commits: unassigned_commits_for_ui,
    })?;
  }

  // process branches in parallel
  let repository_path_owned = repository_path.to_string();
  let branch_prefix_owned = branch_prefix.to_string();

  // Create a mutex for this sync operation to serialize git notes access
  let git_notes_mutex = Arc::new(Mutex::new(()));

  // Create a shared cache for tree IDs for this sync operation
  let tree_id_cache = TreeIdCache::new();

  let mut tasks = Vec::new();

  for (current_branch_idx, (branch_name, commit_data)) in grouped_commit_data.into_iter().enumerate() {
    debug!("Processing branch {} of {total_branches}: {branch_name}", current_branch_idx + 1);
    let repository_path = repository_path_owned.clone();
    let branch_prefix = branch_prefix_owned.clone();
    let progress_clone = progress.clone();
    let branch_name_for_error = branch_name.clone();
    let git_notes_mutex_clone = git_notes_mutex.clone();
    let git_executor_clone = (*git_executor).clone();
    let tree_id_cache_clone = tree_id_cache.clone();

    let params = BranchProcessingParams {
      repository_path,
      branch_prefix,
      branch_name,
      commit_data,
      parent_commit_hash: parent_commit_hash.clone(),
      current_branch_idx,
      total_branches,
      progress: progress_clone,
      git_notes_mutex: git_notes_mutex_clone,
      git_executor: git_executor_clone,
      tree_id_cache: tree_id_cache_clone,
    };
    let task = tauri::async_runtime::spawn(async move { process_single_branch(params).await });

    tasks.push((task, branch_name_for_error));
  }

  // wait for all tasks to complete
  for (task, branch_name) in tasks {
    match task.await {
      Ok(Ok(_)) => {
        // Branch processing completed successfully
        // Status updates have already been sent by process_single_branch
      }
      Ok(Err(e)) => {
        // This should rarely happen since process_single_branch handles its own errors
        // Only send error status if the function returned an actual error
        error!("Branch {branch_name} processing returned an unexpected error: {e:?}");
        // Send branch error event only for unexpected errors
        let _ = progress.send(SyncEvent::BranchStatusUpdate {
          branch_name,
          status: BranchSyncStatus::Error,
          error: Some(BranchError::Generic(e.to_string())),
        });
      }
      Err(e) => {
        // Task panicked or was cancelled
        error!("Task for branch {branch_name} failed to join or panicked: {e:?}");
        // Send branch error event for task failures
        let _ = progress.send(SyncEvent::BranchStatusUpdate {
          branch_name,
          status: BranchSyncStatus::Error,
          error: Some(BranchError::Generic(format!("Task failed: {e}"))),
        });
      }
    }
  }

  progress.send(SyncEvent::Completed)?;

  Ok(())
}

#[instrument(skip(params), fields(branch_name = %params.branch_name, commitCount=%params.commit_data.len()))]
async fn process_single_branch(params: BranchProcessingParams) -> anyhow::Result<()> {
  let BranchProcessingParams {
    repository_path,
    branch_prefix,
    branch_name,
    commit_data,
    parent_commit_hash,
    current_branch_idx,
    total_branches,
    progress,
    git_notes_mutex,
    git_executor,
    tree_id_cache,
  } = params;

  let task_index = current_branch_idx as i16;
  let full_branch_name = to_final_branch_name(&branch_prefix, &branch_name)?;

  let is_existing_branch = check_branch_exists(&git_executor, &repository_path, &full_branch_name);
  debug!("Branch {full_branch_name} exists: {is_existing_branch}");

  let mut current_parent_hash = parent_commit_hash;
  let mut last_commit_hash = String::new();
  let mut commit_details: Vec<CommitDetail> = Vec::new();
  let mut is_any_commit_changed = false;
  let mut pending_notes: Vec<CommitNoteInfo> = Vec::new();

  // recreate each commit on top of the last one
  let total_commits_in_branch = commit_data.len();

  // Collect all commit hashes for potential blocking notifications
  let all_commit_hashes: Vec<String> = commit_data.iter().map(|c| c.id.to_string()).collect();

  for (current_commit_idx, commit_info) in commit_data.into_iter().enumerate() {
    debug!(
      "Processing commit {}/{total_commits_in_branch} in branch {branch_name}: {}",
      current_commit_idx + 1,
      commit_info.id
    );
    // If any commit in the branch's history up to this point has changed, we still need to copy this commit —
    // even if its own content didn't change — so that its parent reference is updated.
    let reuse_if_possible = is_existing_branch && !is_any_commit_changed;
    // check if we can reuse the tree directly (avoid merge)
    let progress_info = crate::git::copy_commit::ProgressInfo {
      branch_name: &branch_name,
      current_commit_idx,
      total_commits_in_branch,
      current_branch_idx,
      total_branches,
    };

    let commit_params = CreateCommitParams {
      commit_info: &commit_info,
      new_parent_oid: current_parent_hash,
      reuse_if_possible,
      repo_path: &repository_path,
      progress: &progress,
      progress_info: &progress_info,
      task_index,
      git_executor: &git_executor,
      tree_id_cache: &tree_id_cache,
    };

    let original_hash = commit_info.id.to_string();

    let (detail, new_id, note_info) = match create_or_update_commit(commit_params) {
      Ok((detail, commit_hash, note)) => {
        // Send success event with status
        let _ = progress.send(SyncEvent::CommitSynced {
          branch_name: branch_name.clone(),
          commit_hash: original_hash.clone(),
          new_hash: commit_hash.clone(),
          status: detail.status.clone(),
        });
        (detail, commit_hash, note)
      }
      Err(CopyCommitError::BranchError(branch_error)) => {
        // Send error event for this commit
        let _ = progress.send(SyncEvent::CommitError {
          branch_name: branch_name.clone(),
          commit_hash: original_hash.clone(),
          error: branch_error.clone(),
        });

        // Send blocked events for remaining commits
        let blocked_hashes: Vec<String> = all_commit_hashes.iter().skip(current_commit_idx + 1).cloned().collect();

        if !blocked_hashes.is_empty() {
          let _ = progress.send(SyncEvent::CommitsBlocked {
            branch_name: branch_name.clone(),
            blocked_commit_hashes: blocked_hashes,
          });
        }

        // Send branch completed event with appropriate error status
        let status = match &branch_error {
          BranchError::MergeConflict(_) => BranchSyncStatus::MergeConflict,
          BranchError::Generic(_) => BranchSyncStatus::Error,
        };

        let _ = progress.send(SyncEvent::BranchStatusUpdate {
          branch_name: branch_name.clone(),
          status,
          error: None,
        });

        // Return early - error already sent via events
        return Ok(());
      }
      Err(CopyCommitError::Other(e)) => return Err(e),
    };

    if detail.status == CommitSyncStatus::Created {
      is_any_commit_changed = true;
    }

    // Collect note info if present
    if let Some(note) = note_info {
      pending_notes.push(note);
    }

    current_parent_hash = new_id.clone();
    last_commit_hash = new_id;
    commit_details.push(detail);
  }

  let branch_sync_status: BranchSyncStatus;
  if is_existing_branch {
    if is_any_commit_changed {
      branch_sync_status = BranchSyncStatus::Updated;
      debug!("Branch {branch_name} was updated with new commits");
    } else {
      branch_sync_status = BranchSyncStatus::Unchanged;
      debug!("Branch {branch_name} is unchanged");
    }
  } else {
    branch_sync_status = BranchSyncStatus::Created;
    debug!("Branch {branch_name} was created");
  }

  // only update the branch if it's new or changed
  if branch_sync_status != BranchSyncStatus::Unchanged {
    let _ = progress.send(SyncEvent::Progress {
      message: format!("[{}/{}] {}: Setting branch reference", current_branch_idx + 1, total_branches, branch_name),
      index: task_index,
    });

    // Use git CLI to update branch reference
    let commit_hash_str = last_commit_hash.to_string();
    let args = vec!["branch", "-f", &full_branch_name, &commit_hash_str];
    git_executor.execute_command(&args, &repository_path)?;
  }

  // Write all commit notes after successful branch sync
  if !pending_notes.is_empty() {
    debug!("Writing {} commit notes for branch {branch_name}", pending_notes.len());
    if let Err(e) = write_commit_notes(&git_executor, &repository_path, pending_notes, &git_notes_mutex) {
      error!("Failed to write commit notes for branch {branch_name}: {e}");
      // Send error status for git notes failure
      let _ = progress.send(SyncEvent::BranchStatusUpdate {
        branch_name: branch_name.clone(),
        status: BranchSyncStatus::Error,
        error: Some(BranchError::Generic(format!("Failed to write commit notes: {e}"))),
      });

      // send end-of-task progress with empty message
      progress.send(SyncEvent::Progress {
        message: String::new(),
        index: task_index,
      })?;

      return Err(e);
    }
  }

  // Send branch status update event
  debug!("Sending final branch status for {branch_name}: {:?}", branch_sync_status);
  let _ = progress.send(SyncEvent::BranchStatusUpdate {
    branch_name: branch_name.clone(),
    status: branch_sync_status.clone(),
    error: None,
  });

  // send end-of-task progress with empty message
  progress.send(SyncEvent::Progress {
    message: String::new(),
    index: task_index,
  })?;

  Ok(())
}

pub(crate) fn check_branch_exists(git_executor: &GitCommandExecutor, repo_path: &str, branch_name: &str) -> bool {
  // Use git show-ref to check if branch exists - more efficient than rev-parse
  let branch_ref = format!("refs/heads/{branch_name}");
  let args = vec!["show-ref", "--verify", "--quiet", &branch_ref];
  git_executor.execute_command(&args, repo_path).is_ok()
}

// Static regex patterns - compiled once on first use
static PREFIX_PATTERN: OnceLock<Regex> = OnceLock::new();
static ISSUE_PATTERN: OnceLock<Regex> = OnceLock::new();

// Type alias for grouped commits result
type GroupedCommitsResult = (IndexMap<String, Vec<(String, Commit)>>, Vec<Commit>);

#[instrument(skip(commits))]

/// Group commits by prefix using the new Commit struct
/// Returns (grouped commits, unassigned commits)
pub(crate) fn group_commits_by_prefix_new(commits: &[Commit]) -> GroupedCommitsResult {
  debug!("Grouping {} commits by prefix", commits.len());

  if commits.is_empty() {
    return (IndexMap::new(), Vec::new());
  }

  // Initialize regex patterns on first use
  let prefix_pattern = PREFIX_PATTERN.get_or_init(|| Regex::new(r"\((.+?)\)(.*?)(?:\r?\n|$)").unwrap());
  let issue_pattern = ISSUE_PATTERN.get_or_init(|| Regex::new(r"\b([A-Z]+-\d+)\b").unwrap());

  // use index map - preserve insertion order
  let mut prefix_to_commits: IndexMap<String, Vec<(String, Commit)>> = IndexMap::new();
  let mut unassigned_commits: Vec<Commit> = Vec::new();

  // group commits by prefix
  for commit in commits {
    let message = &commit.message;
    let mut has_prefix = false;

    // First try to find explicit prefix in parentheses
    if let Some(captures) = prefix_pattern.captures(message) {
      if let (Some(prefix_match), Some(message_match)) = (captures.get(1), captures.get(2)) {
        let prefix = prefix_match.as_str().trim();
        let message_text = message_match.as_str().trim();

        prefix_to_commits.entry(prefix.to_string()).or_default().push((message_text.to_string(), commit.clone()));
        has_prefix = true;
      }
    }

    if !has_prefix {
      // If no explicit parentheses prefix, look for issue number pattern in the first line only
      // More efficient than lines().next() - find first newline directly
      let first_line_end = message.find('\n').unwrap_or(message.len());
      let first_line = &message[..first_line_end];

      if let Some(issue_match) = issue_pattern.find(first_line) {
        let issue_number = issue_match.as_str();
        prefix_to_commits
          .entry(issue_number.to_string())
          .or_default()
          .push((message.trim().to_string(), commit.clone()));
        has_prefix = true;
      }
    }

    // If no prefix found, add to unassigned commits
    if !has_prefix {
      unassigned_commits.push(commit.clone());
    }
  }

  info!("Grouped commits into {} branches, {} unassigned commits", prefix_to_commits.len(), unassigned_commits.len());
  for (prefix, commits) in &prefix_to_commits {
    debug!("Branch '{prefix}' has {} commits", commits.len());
  }
  (prefix_to_commits, unassigned_commits)
}
