use crate::git::git_command::GitCommandExecutor;
use crate::git::model::{BranchError, CommitDetail, CommitInfo, CommitSyncStatus};
use crate::git::notes::{CommitNoteInfo, find_existing_commit};
use crate::progress::SyncEvent;
use anyhow::anyhow;
use git2::{Oid, Repository, Signature};
use std::sync::Mutex;
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

impl From<git2::Error> for CopyCommitError {
  fn from(err: git2::Error) -> Self {
    CopyCommitError::Other(err.into())
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
  pub new_parent_oid: Oid,
  pub reuse_if_possible: bool,
  pub repo: &'a Repository,
  pub progress: &'a Channel<SyncEvent>,
  pub progress_info: &'a ProgressInfo<'a>,
  pub task_index: i16,
  pub git_notes_mutex: &'a Mutex<()>,
  pub git_executor: &'a GitCommandExecutor,
}

// Create or update a commit based on an original commit
// Returns commit detail, new OID, and note info for later writing
#[instrument(skip(params), fields(commit_id = %params.commit_info.id, branch = %params.progress_info.branch_name))]
pub(crate) fn create_or_update_commit(params: CreateCommitParams) -> Result<(CommitDetail, Oid, Option<CommitNoteInfo>), CopyCommitError> {
  let CreateCommitParams {
    commit_info,
    new_parent_oid,
    reuse_if_possible,
    repo,
    progress,
    progress_info,
    task_index,
    git_notes_mutex: _,
    git_executor,
  } = params;

  if reuse_if_possible {
    if let Some(hash) = find_existing_commit(repo, commit_info.id) {
      debug!(original_id = %commit_info.id, mapped_id = %hash, "reusing existing commit");
      return Ok((
        CommitDetail {
          original_hash: commit_info.id.to_string(),
          hash: hash.to_string(),
          time: commit_info.time,
          message: commit_info.message.clone(),
          status: CommitSyncStatus::Unchanged,
          error: None,
        },
        hash.parse()?,
        None, // No need to write note for unchanged commits
      ));
    }
  }

  let original_commit = &repo.find_commit(commit_info.id)?;

  let new_parent_commit = repo.find_commit(new_parent_oid)?;
  let original_commit_parent = original_commit.parent(0)?;

  // Commits are processed in order (oldest to newest).
  // We can directly compare if the new parent tree is the same as the cherry-picked original parent tree.
  // This helps us identify if the parent relationship is preserved.
  // If the tree IDs match, we can skip the merge and reuse the original tree directly.

  // If the trees match, it means the new parent has exactly the same content as the original parent.
  // In this case, we can apply the original commit directly without merging.
  let new_tree = if original_commit_parent.tree_id() == new_parent_commit.tree_id() {
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
    // trees are identical, we can skip the merge and just use the original tree
    original_commit.tree()?
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
    // trees are different, use fast cherry-pick for better performance
    crate::git::cherry_pick::perform_fast_cherry_pick_with_context(
      repo,
      original_commit,
      &new_parent_commit,
      git_executor,
      Some((progress, progress_info.branch_name, task_index)),
    )?
  };

  let author = original_commit.author();
  let committer = Signature::now("branch-deck", author.email().unwrap_or_default())?;

  let new_commit_oid = repo.commit(
    None, // don't update any references
    &author,
    &committer,
    &commit_info.message,
    &new_tree,
    &[&new_parent_commit],
  )?;

  // Prepare note info for later batch writing
  let note_info = CommitNoteInfo {
    original_oid: commit_info.id,
    new_oid: new_commit_oid,
    author: author.name().unwrap_or("unknown").to_string(),
    author_email: author.email().unwrap_or("unknown@example.com").to_string(),
  };

  Ok((
    CommitDetail {
      original_hash: commit_info.id.to_string(),
      hash: new_commit_oid.to_string(),
      time: commit_info.time,
      message: commit_info.message.clone(),
      status: CommitSyncStatus::Created,
      error: None,
    },
    new_commit_oid,
    Some(note_info),
  ))
}
