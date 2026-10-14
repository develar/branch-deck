use crate::cache::TreeIdCache;
use crate::commit_list::Commit;
use crate::model::{BranchError, BranchSyncStatus, CommitSyncStatus};
use anyhow::anyhow;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
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

impl std::fmt::Display for CopyCommitError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      CopyCommitError::BranchError(BranchError::Generic(message)) => f.write_str(message),
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
#[derive(Clone, Copy)]
pub struct CreateCommitParams<'a> {
  pub commit: &'a Commit,
  pub new_parent_oid: &'a str,
  pub reuse_if_possible: bool,
  pub repo_path: &'a str,
  /// Receives intermediate branch statuses (e.g. `AnalyzingConflict`)
  pub on_status: &'a dyn Fn(BranchSyncStatus),
  pub progress_info: &'a ProgressInfo<'a>,
  pub git_executor: &'a GitCommandExecutor,
  pub tree_id_cache: &'a TreeIdCache,
  /// Commits of the existing virtual branch -> their first parent; `None` copies the commit
  pub existing_virtual_commits: Option<&'a HashMap<String, String>>,
  /// Committer date (unix seconds) of a new commit: the same input then writes the same commit
  pub committer_timestamp: u32,
}

/// Result of copying one commit onto the virtual branch
#[derive(Debug)]
pub struct CopiedCommit {
  pub hash: String,
  pub status: CommitSyncStatus,
  /// `(original, new)` mapping to record as a git note; only for newly created commits
  pub note: Option<(String, String)>,
}

fn git_date(timestamp: u32, timezone: &str) -> String {
  if timezone.is_empty() {
    timestamp.to_string()
  } else {
    format!("{timestamp} {timezone}")
  }
}

// Create or update a commit based on an original commit
#[instrument(skip(params), fields(commit_id = %params.commit.id, branch = %params.progress_info.branch_name))]
pub fn create_or_update_commit(params: CreateCommitParams<'_>) -> Result<CopiedCommit, CopyCommitError> {
  use crate::cherry_pick::perform_fast_cherry_pick_with_context;
  let CreateCommitParams {
    commit,
    new_parent_oid,
    reuse_if_possible,
    repo_path,
    on_status,
    progress_info: _, // only read by the span
    git_executor,
    tree_id_cache,
    existing_virtual_commits,
    committer_timestamp,
  } = params;

  // the commit the note maps to is reused while it is still in the virtual branch, on the same parent
  if reuse_if_possible
    && let Some(mapped_id) = &commit.mapped_commit_id
    && let Some(existing_commits) = existing_virtual_commits
  {
    if existing_commits.get(mapped_id).is_some_and(|parent| parent == new_parent_oid) {
      debug!(original_id = %commit.id, mapped_id = %mapped_id, "reusing existing commit from git note");
      // The note already maps to this commit, nothing to write
      return Ok(CopiedCommit {
        hash: mapped_id.clone(),
        status: CommitSyncStatus::Unchanged,
        note: None,
      });
    }
    debug!(original_id = %commit.id, mapped_id = %mapped_id, "mapped commit is gone or has another parent, will re-copy");
  }

  // the commit list read the tree with the commit
  if !commit.tree_id.is_empty() {
    tree_id_cache.insert(&commit.id, &commit.tree_id);
  }

  // Get tree IDs for comparison using cache
  let original_parent_tree_id = if let Some(parent_id) = &commit.parent_id {
    tree_id_cache.get_tree_id(git_executor, repo_path, parent_id)?
  } else {
    // No parent means this is the initial commit, use empty tree
    String::new()
  };
  let new_parent_tree_id = tree_id_cache.get_tree_id(git_executor, repo_path, new_parent_oid)?;

  // Check if trees match for fast path
  let tree_id = if original_parent_tree_id == new_parent_tree_id {
    debug!(commit_id = %commit.id, "parent tree matches, reusing original commit tree");
    // Trees are identical, reuse original tree
    commit.tree_id.clone()
  } else {
    debug!(commit_id = %commit.id, "parent tree differs, performing merge");
    // Use cherry-pick for efficient 3-way merge with conflict handling
    let parent_id = commit
      .parent_id
      .as_deref()
      .ok_or_else(|| CopyCommitError::Other(anyhow!("Cannot cherry-pick root commit {}: it has no parent", commit.id)))?;
    perform_fast_cherry_pick_with_context(git_executor, repo_path, &commit.id, parent_id, new_parent_oid, on_status, tree_id_cache)?
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
  let commit_args = vec!["commit-tree", &tree_id, "-p", new_parent_oid, "-m", &commit_message];

  // `<unix seconds> <offset>` keeps the original timezone; without the offset git takes the local one
  let author_date = git_date(commit.author_timestamp, &commit.author_timezone);
  let committer_date = git_date(committer_timestamp, &commit.committer_timezone);

  let env_vars = vec![
    ("GIT_AUTHOR_NAME", commit.author_name.as_str()),
    ("GIT_AUTHOR_EMAIL", commit.author_email.as_str()),
    ("GIT_AUTHOR_DATE", &author_date),
    ("GIT_COMMITTER_NAME", "branch-deck"),
    ("GIT_COMMITTER_EMAIL", commit.author_email.as_str()),
    ("GIT_COMMITTER_DATE", &committer_date),
  ];

  let output = git_executor
    .execute_command_with_env(&commit_args, repo_path, &env_vars)
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to create commit: {e}")))?;

  let new_commit_hash = output.trim().to_string();
  // the next commit of the branch is copied onto this one; its tree is known, so don't ask git for it
  tree_id_cache.insert(&new_commit_hash, &tree_id);

  // Note for later batch writing
  let note = Some((commit.id.clone(), new_commit_hash.clone()));

  Ok(CopiedCommit {
    hash: new_commit_hash,
    status: CommitSyncStatus::Created,
    note,
  })
}
