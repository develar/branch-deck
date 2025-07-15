use crate::git::cache::TreeIdCache;
use crate::git::git_command::GitCommandExecutor;
use crate::git::model::{BranchError, CommitDetail, CommitInfo, CommitSyncStatus};
use crate::git::notes::CommitNoteInfo;
use crate::progress::SyncEvent;
use anyhow::anyhow;
use tauri::ipc::Channel;
use tracing::{debug, instrument};

/// Custom error type for copy commit operations
#[derive(Debug)]
pub enum CopyCommitError {
  BranchError(BranchError),
  Other(anyhow::Error),
}

impl From<anyhow::Error> for CopyCommitError {
  fn from(err: anyhow::Error) -> Self {
    CopyCommitError::Other(err)
  }
}

impl From<tauri::Error> for CopyCommitError {
  fn from(err: tauri::Error) -> Self {
    CopyCommitError::Other(anyhow!("Tauri error: {}", err))
  }
}

impl std::fmt::Display for CopyCommitError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      CopyCommitError::BranchError(e) => write!(f, "Branch error: {e:?}"),
      CopyCommitError::Other(e) => write!(f, "{e}"),
    }
  }
}

impl std::error::Error for CopyCommitError {}

// Progress information for logging and user feedback
#[derive(Debug)]
pub struct ProgressInfo<'a> {
  pub branch_name: &'a str,
  pub current_commit_idx: usize,
  pub total_commits_in_branch: usize,
  pub current_branch_idx: usize,
  pub total_branches: usize,
}

// Parameters for creating or updating a commit
pub struct CreateCommitParams<'a> {
  pub commit_info: &'a CommitInfo,
  pub new_parent_oid: String,
  pub reuse_if_possible: bool,
  pub repo_path: &'a str,
  pub progress: &'a Channel<SyncEvent>,
  pub progress_info: &'a ProgressInfo<'a>,
  pub task_index: i16,
  pub git_executor: &'a GitCommandExecutor,
  pub tree_id_cache: &'a TreeIdCache,
}

// Create or update a commit based on an original commit
// Returns commit detail, new commit hash, and note info for later writing
#[instrument(skip(params), fields(commit_id = %params.commit_info.id, branch = %params.progress_info.branch_name))]
pub(crate) fn create_or_update_commit(params: CreateCommitParams) -> Result<(CommitDetail, String, Option<CommitNoteInfo>), CopyCommitError> {
  let CreateCommitParams {
    commit_info,
    new_parent_oid,
    reuse_if_possible,
    repo_path,
    progress,
    progress_info,
    task_index,
    git_executor,
    tree_id_cache,
  } = params;

  if reuse_if_possible {
    if let Some(mapped_id) = &commit_info.mapped_commit_id {
      debug!(original_id = %commit_info.id, mapped_id = %mapped_id, "reusing existing commit");
      return Ok((
        CommitDetail {
          original_hash: commit_info.id.to_string(),
          hash: mapped_id.clone(),
          message: commit_info.message.clone(),
          author_time: commit_info.author_time,
          committer_time: commit_info.committer_time,
          status: CommitSyncStatus::Unchanged,
          error: None,
        },
        mapped_id.clone(),
        None, // No need to write note for unchanged commits
      ));
    }
  }

  // Get tree IDs for comparison using cache
  let original_parent_tree_id = if let Some(parent_id) = &commit_info.parent_id {
    tree_id_cache.get_tree_id(git_executor, repo_path, parent_id)?
  } else {
    // No parent means this is the initial commit, use empty tree
    String::new()
  };
  let new_parent_tree_id = tree_id_cache.get_tree_id(git_executor, repo_path, &new_parent_oid)?;

  // Check if trees match for fast path
  let tree_id = if original_parent_tree_id == new_parent_tree_id {
    debug!(commit_id = %commit_info.id, "parent tree matches, reusing original commit tree");
    progress.send(SyncEvent::Progress {
      message: format!(
        "[{}/{}] {}: Creating commit {}/{} ({:.7}) with existing tree",
        progress_info.current_branch_idx + 1,
        progress_info.total_branches,
        progress_info.branch_name,
        progress_info.current_commit_idx + 1,
        progress_info.total_commits_in_branch,
        commit_info.id
      ),
      index: task_index,
    })?;
    // Trees are identical, reuse original tree
    commit_info.tree_id.clone()
  } else {
    debug!(commit_id = %commit_info.id, "parent tree differs, performing merge");
    progress.send(SyncEvent::Progress {
      message: format!(
        "[{}/{}] {}: Creating commit {}/{} ({:.7}) using merge",
        progress_info.current_branch_idx + 1,
        progress_info.total_branches,
        progress_info.branch_name,
        progress_info.current_commit_idx + 1,
        progress_info.total_commits_in_branch,
        commit_info.id
      ),
      index: task_index,
    })?;

    // Use cherry-pick for efficient 3-way merge with conflict handling
    use crate::git::cherry_pick::perform_fast_cherry_pick_with_context;
    let progress_tuple = Some((progress, progress_info.branch_name, task_index));
    perform_fast_cherry_pick_with_context(
      git_executor,
      repo_path,
      &commit_info.id,
      &new_parent_oid,
      progress_tuple.as_ref().map(|(p, b, i)| (*p, *b, *i)),
      tree_id_cache,
    )?
  };

  // Create new commit using git commit-tree
  let commit_args = vec!["commit-tree", &tree_id, "-p", &new_parent_oid, "-m", &commit_info.message];

  // Use Unix timestamp directly (Git accepts this format)
  let author_date = commit_info.author_time.to_string();

  let env_vars = vec![
    ("GIT_AUTHOR_NAME", commit_info.author_name.as_str()),
    ("GIT_AUTHOR_EMAIL", commit_info.author_email.as_str()),
    ("GIT_AUTHOR_DATE", &author_date),
    ("GIT_COMMITTER_NAME", "branch-deck"),
    ("GIT_COMMITTER_EMAIL", commit_info.author_email.as_str()),
  ];

  let output = git_executor
    .execute_command_with_env(&commit_args, repo_path, &env_vars)
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to create commit: {}", e)))?;

  let new_commit_hash = output.trim().to_string();

  // Prepare note info for later batch writing
  let note_info = CommitNoteInfo {
    original_oid: commit_info.id.clone(),
    new_oid: new_commit_hash.clone(),
    author: commit_info.author_name.clone(),
    author_email: commit_info.author_email.clone(),
  };

  Ok((
    CommitDetail {
      original_hash: commit_info.id.to_string(),
      hash: new_commit_hash.clone(),
      message: commit_info.message.clone(),
      author_time: commit_info.author_time,
      committer_time: commit_info.committer_time,
      status: CommitSyncStatus::Created,
      error: None,
    },
    new_commit_hash,
    Some(note_info),
  ))
}
