use crate::cache::TreeIdCache;
use crate::commit_list::Commit;
use crate::model::{BranchError, CommitSyncStatus};
use crate::notes::CommitNoteInfo;
use crate::progress::ProgressCallback;
use anyhow::anyhow;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashSet;
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
  pub commit: &'a Commit,
  pub new_parent_oid: String,
  pub reuse_if_possible: bool,
  pub repo_path: &'a str,
  pub progress: &'a dyn ProgressCallback,
  pub progress_info: &'a ProgressInfo<'a>,
  pub task_index: i16,
  pub git_executor: &'a GitCommandExecutor,
  pub tree_id_cache: &'a TreeIdCache,
  pub existing_virtual_commits: Option<&'a HashSet<String>>, // For efficient batch verification
}

// Create or update a commit based on an original commit
// Returns new commit hash, sync status, and note info for later writing
#[instrument(skip(params), fields(commit_id = %params.commit.id, branch = %params.progress_info.branch_name))]
pub fn create_or_update_commit(params: CreateCommitParams<'_>) -> Result<(String, CommitSyncStatus, Option<CommitNoteInfo>), CopyCommitError> {
  let CreateCommitParams {
    commit,
    new_parent_oid,
    reuse_if_possible,
    repo_path,
    progress,
    progress_info,
    task_index,
    git_executor,
    tree_id_cache,
    existing_virtual_commits,
  } = params;

  if reuse_if_possible {
    // First check if we have a mapped commit from git notes
    if let Some(mapped_id) = &commit.mapped_commit_id {
      // Verify the mapped commit still exists
      let mapped_exists = if let Some(existing_commits) = existing_virtual_commits {
        existing_commits.contains(mapped_id)
      } else {
        git_executor.execute_command(&["rev-parse", "--verify", mapped_id], repo_path).is_ok()
      };

      if mapped_exists {
        debug!(original_id = %commit.id, mapped_id = %mapped_id, "reusing existing commit from git note");
        // Create note info even though we're reusing
        let note_info = CommitNoteInfo {
          original_oid: commit.id.clone(),
          new_oid: mapped_id.clone(),
          author: commit.author_name.clone(),
          author_email: commit.author_email.clone(),
          tree_id: commit.tree_id.clone(),
          subject: commit.stripped_subject.clone(),
        };
        return Ok((
          mapped_id.clone(),
          CommitSyncStatus::Unchanged,
          Some(note_info), // Return note info for database tracking
        ));
      } else {
        debug!(original_id = %commit.id, mapped_id = %mapped_id, "mapped commit no longer exists, will re-copy");
      }
    }
  }

  // Get tree IDs for comparison using cache
  let original_parent_tree_id = if let Some(parent_id) = &commit.parent_id {
    tree_id_cache.get_tree_id(git_executor, repo_path, parent_id)?
  } else {
    // No parent means this is the initial commit, use empty tree
    String::new()
  };
  let new_parent_tree_id = tree_id_cache.get_tree_id(git_executor, repo_path, &new_parent_oid)?;

  // Check if trees match for fast path
  let tree_id = if original_parent_tree_id == new_parent_tree_id {
    debug!(commit_id = %commit.id, "parent tree matches, reusing original commit tree");
    // Trees are identical, reuse original tree
    commit.tree_id.clone()
  } else {
    debug!(commit_id = %commit.id, "parent tree differs, performing merge");
    // Use cherry-pick for efficient 3-way merge with conflict handling
    use crate::cherry_pick::perform_fast_cherry_pick_with_context;
    use crate::progress::CherryPickProgress;
    let cherry_progress = CherryPickProgress::new(progress, progress_info.branch_name, task_index);
    perform_fast_cherry_pick_with_context(git_executor, repo_path, &commit.id, &new_parent_oid, Some(&cherry_progress), tree_id_cache)?
  };

  // Reconstruct message with stripped subject for the actual git commit
  let commit_message = if commit.message.contains('\n') {
    // Multi-line message: replace first line with stripped subject
    let body_start = commit.message.find('\n').unwrap_or(commit.message.len());
    format!("{}{}", commit.stripped_subject, &commit.message[body_start..])
  } else {
    // Single line message: use the stripped subject
    commit.stripped_subject.clone()
  };

  // Create new commit using git commit-tree
  let commit_args = vec!["commit-tree", &tree_id, "-p", &new_parent_oid, "-m", &commit_message];

  // Use Unix timestamp directly (Git accepts this format)
  let author_date = commit.author_timestamp.to_string();

  let env_vars = vec![
    ("GIT_AUTHOR_NAME", commit.author_name.as_str()),
    ("GIT_AUTHOR_EMAIL", commit.author_email.as_str()),
    ("GIT_AUTHOR_DATE", &author_date),
    ("GIT_COMMITTER_NAME", "branch-deck"),
    ("GIT_COMMITTER_EMAIL", commit.author_email.as_str()),
  ];

  let output = git_executor
    .execute_command_with_env(&commit_args, repo_path, &env_vars)
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to create commit: {}", e)))?;

  let new_commit_hash = output.trim().to_string();

  // Prepare note info for later batch writing
  // Use stripped_subject if available (without branch prefix), otherwise use original subject
  let note_info = CommitNoteInfo {
    original_oid: commit.id.clone(),
    new_oid: new_commit_hash.clone(),
    author: commit.author_name.clone(),
    author_email: commit.author_email.clone(),
    tree_id: commit.tree_id.clone(),
    subject: if !commit.stripped_subject.is_empty() {
      commit.stripped_subject.clone()
    } else {
      commit.subject.clone()
    },
  };

  Ok((new_commit_hash, CommitSyncStatus::Created, Some(note_info)))
}
