use crate::git::commit_list::get_commit_list;
use crate::git::copy_commit::{CopyCommitError, CreateCommitParams, create_or_update_commit};
use crate::git::git_command::GitCommandExecutor;
use crate::git::model::{BranchError, BranchSyncStatus, CommitDetail, CommitInfo, CommitSyncStatus, to_final_branch_name};
use crate::git::notes::{CommitNoteInfo, write_commit_notes};
use crate::progress::{GroupedBranchInfo, SyncEvent};
use git2::Oid;
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
  parent_commit_hash: Oid,
  current_branch_idx: usize,
  total_branches: usize,
  progress: Channel<SyncEvent>,
  git_notes_mutex: Arc<Mutex<()>>,
  git_executor: GitCommandExecutor,
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
    message: "get commits".to_string(),
    index: -1,
  })?;

  // extract all the data we need from git2 types before spawning tasks
  let (grouped_commit_data, parent_commit_hash) = {
    let repo = git2::Repository::open(repository_path)?;
    let commits = get_commit_list(&repo, "master")?;

    let Some(oldest_head_commit) = commits.first() else {
      // No commits to sync
      progress.send(SyncEvent::Completed)?;
      return Ok(());
    };

    let parent_commit_hash = oldest_head_commit.parent(0)?.id();

    // group commits by prefix first to get all branch names
    // for each prefix, create a branch with only the relevant commits
    let grouped_commits = group_commits_by_prefix(&commits);

    // extract commit IDs and messages to avoid lifetime issues with git2 types
    let grouped_commit_data: Vec<(String, Vec<CommitInfo>)> = grouped_commits
      .into_iter()
      .map(|(branch_name, branch_commits)| {
        let commit_data = branch_commits
          .into_iter()
          .map(|(message, commit)| CommitInfo {
            message,
            id: commit.id(),
            time: commit.author().when().seconds() as u32,
          })
          .collect();
        (branch_name, commit_data)
      })
      .collect();

    (grouped_commit_data, parent_commit_hash)
  }; // repo and commits are dropped here, before we spawn tasks

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
          time: commit.time,
          status: CommitSyncStatus::Pending,
          error: None,
        })
        .collect(),
    })
    .collect();

  progress.send(SyncEvent::BranchesGrouped {
    branches: grouped_branches_for_ui,
  })?;

  // process branches in parallel
  let repository_path_owned = repository_path.to_string();
  let branch_prefix_owned = branch_prefix.to_string();

  // Create a mutex for this sync operation to serialize git notes access
  let git_notes_mutex = Arc::new(Mutex::new(()));

  let mut tasks = Vec::new();

  for (current_branch_idx, (branch_name, commit_data)) in grouped_commit_data.into_iter().enumerate() {
    debug!("Processing branch {} of {total_branches}: {branch_name}", current_branch_idx + 1);
    let repository_path = repository_path_owned.clone();
    let branch_prefix = branch_prefix_owned.clone();
    let progress_clone = progress.clone();
    let branch_name_for_error = branch_name.clone();
    let git_notes_mutex_clone = git_notes_mutex.clone();
    let git_executor_clone = (*git_executor).clone();

    let params = BranchProcessingParams {
      repository_path,
      branch_prefix,
      branch_name,
      commit_data,
      parent_commit_hash,
      current_branch_idx,
      total_branches,
      progress: progress_clone,
      git_notes_mutex: git_notes_mutex_clone,
      git_executor: git_executor_clone,
    };
    let task = tauri::async_runtime::spawn(async move { process_single_branch(params).await });

    tasks.push((task, branch_name_for_error));
  }

  // wait for all tasks to complete
  for (task, branch_name) in tasks {
    match task.await {
      Ok(Ok(_)) => {}
      Ok(Err(e)) => {
        warn!("Branch {branch_name} processing returned an error: {e:?}");
        // Send branch error event
        let _ = progress.send(SyncEvent::BranchStatusUpdate {
          branch_name,
          status: BranchSyncStatus::Error,
        });
      }
      Err(e) => {
        error!("Task for branch {branch_name} failed to join or panicked: {e:?}");
        // Send branch error event
        let _ = progress.send(SyncEvent::BranchStatusUpdate {
          branch_name,
          status: BranchSyncStatus::Error,
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
  } = params;

  let task_index = current_branch_idx as i16;
  let repo = git2::Repository::open(&repository_path)?;
  let full_branch_name = to_final_branch_name(&branch_prefix, &branch_name)?;

  let is_existing_branch = check_branch_exists(&repo, &full_branch_name);
  debug!("Branch {full_branch_name} exists: {is_existing_branch}");

  let mut current_parent_hash = parent_commit_hash;
  let mut last_commit_hash: Oid = Oid::zero();
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
      repo: &repo,
      progress: &progress,
      progress_info: &progress_info,
      task_index,
      git_notes_mutex: &git_notes_mutex,
      git_executor: &git_executor,
    };

    let original_hash = commit_info.id.to_string();

    let (detail, new_id, note_info) = match create_or_update_commit(commit_params) {
      Ok((detail, oid, note)) => {
        // Send success event with status
        let _ = progress.send(SyncEvent::CommitSynced {
          branch_name: branch_name.clone(),
          commit_hash: original_hash.clone(),
          new_hash: oid.to_string(),
          status: detail.status.clone(),
        });
        (detail, oid, note)
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

    current_parent_hash = new_id;
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

    repo.find_commit(last_commit_hash).and_then(|commit| repo.branch(&full_branch_name, &commit, true))?;
  }

  // Write all commit notes after successful branch sync
  if !pending_notes.is_empty() {
    debug!("Writing {} commit notes for branch {branch_name}", pending_notes.len());
    write_commit_notes(&repo, pending_notes, &git_notes_mutex)?;
  }

  // Send branch status update event
  let _ = progress.send(SyncEvent::BranchStatusUpdate {
    branch_name: branch_name.clone(),
    status: branch_sync_status.clone(),
  });

  // send end-of-task progress with empty message
  progress.send(SyncEvent::Progress {
    message: String::new(),
    index: task_index,
  })?;

  Ok(())
}

pub(crate) fn check_branch_exists(repo: &git2::Repository, branch_name: &str) -> bool {
  repo.find_branch(branch_name, git2::BranchType::Local).is_ok()
}

// Static regex patterns - compiled once on first use
static PREFIX_PATTERN: OnceLock<Regex> = OnceLock::new();
static ISSUE_PATTERN: OnceLock<Regex> = OnceLock::new();

#[instrument(skip(commits))]
pub(crate) fn group_commits_by_prefix<'repo>(commits: &[git2::Commit<'repo>]) -> IndexMap<String, Vec<(String, git2::Commit<'repo>)>> {
  debug!("Grouping {} commits by prefix", commits.len());

  if commits.is_empty() {
    return IndexMap::new();
  }

  // Initialize regex patterns on first use
  let prefix_pattern = PREFIX_PATTERN.get_or_init(|| Regex::new(r"\((.+?)\)(.*?)(?:\r?\n|$)").unwrap());
  let issue_pattern = ISSUE_PATTERN.get_or_init(|| Regex::new(r"\b([A-Z]+-\d+)\b").unwrap());

  // use index map - preserve insertion order
  let mut prefix_to_commits: IndexMap<String, Vec<(String, git2::Commit)>> = IndexMap::new();

  // group commits by prefix
  for commit in commits {
    let Some(message) = commit.message() else {
      continue;
    };

    // First try to find explicit prefix in parentheses
    if let Some(captures) = prefix_pattern.captures(message) {
      if let (Some(prefix_match), Some(message_match)) = (captures.get(1), captures.get(2)) {
        let prefix = prefix_match.as_str().trim();
        let message_text = message_match.as_str().trim();

        prefix_to_commits.entry(prefix.to_string()).or_default().push((message_text.to_string(), commit.clone()));
        continue;
      }
    }

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
    }
  }

  info!("Grouped commits into {} branches", prefix_to_commits.len());
  for (prefix, commits) in &prefix_to_commits {
    debug!("Branch '{prefix}' has {} commits", commits.len());
  }
  prefix_to_commits
}
