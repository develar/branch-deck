//! Amend uncommitted changes into a commit of the checked out branch without touching the working tree
//! (docs/design/git-operations.md). The descendants of the amended commit are replayed with `git replay`, which writes
//! only objects; the branch moves in one ref transaction, and the index stages exactly the amended files.

use crate::commit_writer::{NewCommit, NewParent, current_committer, read_raw_commits, write_commits};
use crate::copy_commit::CopyCommitError;
use crate::model::BranchError;
use crate::ref_transaction::RefTransaction;
use crate::rewrite_guard::IndexUpdate;
use crate::rewrite_plan::{History, PreparedRewrite, merge_trees, replay_onto, short_id};
use anyhow::anyhow;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

/// Parameters for amending uncommitted changes to a specific commit in main branch
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AmendToCommitParams {
  pub original_commit_id: String,
  pub files: Vec<String>,
}

/// Result of amending operation
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AmendResult {
  pub amended_commit_id: String,
  pub rebased_to_commit: String,
}

/// Amend the working-tree versions of `files` into a commit of the checked out branch.
/// Only the difference between HEAD and the working tree for those files goes into the commit; its descendants are
/// replayed on top. Afterwards the files match HEAD, other staged changes stay staged, and the working tree is not
/// written. The amended commit keeps its author and message; its committer is the current user.
#[instrument(skip(git_executor), fields(original_commit = %params.original_commit_id))]
pub fn amend_to_commit_in_main(git_executor: &GitCommandExecutor, repo_path: &str, params: AmendToCommitParams) -> Result<AmendResult, CopyCommitError> {
  let AmendToCommitParams { original_commit_id, files } = params;
  if files.is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No files specified to amend")));
  }

  let history = History::read(git_executor, repo_path)?;
  let position = history.position(&original_commit_id)?;
  let target = history.check_rewrite(git_executor, repo_path, position)?;

  // HEAD with the working-tree versions of the files: the tree HEAD has after the amend
  let head_tree = git_executor.resolve_tree_id(repo_path, &target.head)?;
  let worktree_tree = tree_with_worktree_files(git_executor, repo_path, &head_tree, &files)?;
  if worktree_tree == head_tree {
    return Err(CopyCommitError::Other(anyhow!("No uncommitted changes to amend")));
  }

  // the amended commit's tree: its own tree plus the changes made on top of HEAD
  let original_tree = git_executor.resolve_tree_id(repo_path, &original_commit_id)?;
  let amended_tree = if position == 0 {
    worktree_tree.clone()
  } else {
    merge_trees(git_executor, repo_path, &head_tree, &original_tree, &worktree_tree)?.ok_or_else(|| {
      CopyCommitError::BranchError(BranchError::Generic(format!(
        "The changes can't be amended into commit {}: they conflict with later commits that changed the same lines",
        short_id(&original_commit_id)
      )))
    })?
  };

  let raw = read_raw_commits(git_executor, repo_path, &[original_commit_id.as_str()])?
    .remove(&original_commit_id)
    .ok_or_else(|| CopyCommitError::Other(anyhow!("Commit {original_commit_id} not found")))?;
  let committer = current_committer(git_executor, repo_path)?;
  let written = write_commits(
    git_executor,
    repo_path,
    &[NewCommit {
      tree: &amended_tree,
      parent: history.parent(position).map(NewParent::Existing),
      author: &raw.author,
      committer: &committer,
      encoding: raw.encoding.as_deref(),
      message: &raw.message,
    }],
  )?;
  let amended_commit_id = written.ids[0].clone();

  let mut prepared = PreparedRewrite {
    new_head: amended_commit_id.clone(),
    target,
    index_update: IndexUpdate::FromWorktree(files),
    temp_refs: vec![written.temp_ref],
    replacements: vec![(original_commit_id.clone(), amended_commit_id.clone())],
  };
  if position > 0 {
    match replay_onto(git_executor, repo_path, &amended_commit_id, &original_commit_id, &prepared.target.head) {
      Ok(Some(new_head)) => prepared.new_head = new_head,
      Ok(None) => {
        prepared.discard(git_executor, repo_path);
        return Err(CopyCommitError::BranchError(BranchError::Generic(format!(
          "The changes can't be amended into commit {}: later commits conflict with the amended commit",
          short_id(&original_commit_id)
        ))));
      }
      Err(e) => {
        prepared.discard(git_executor, repo_path);
        return Err(e);
      }
    }
    // the replayed history must end with exactly the working-tree versions of the files, nothing else changed
    if git_executor.resolve_tree_id(repo_path, &prepared.new_head)? != worktree_tree {
      prepared.discard(git_executor, repo_path);
      return Err(CopyCommitError::BranchError(BranchError::Generic(format!(
        "Amending commit {} would change later commits differently than the working tree; amend into a later commit instead",
        short_id(&original_commit_id)
      ))));
    }
  }

  prepared.apply(git_executor, repo_path, RefTransaction::new(), &format!("amend {}", short_id(&original_commit_id)))?;
  debug!(%amended_commit_id, new_head = %prepared.new_head, "amended commit");
  Ok(AmendResult {
    amended_commit_id,
    rebased_to_commit: prepared.new_head,
  })
}

/// Temporary index file in its own temporary directory, removed with the directory on drop.
/// The file itself is not created up front: git rejects an empty index file, and `read-tree` creates it.
struct TempIndex {
  _dir: tempfile::TempDir,
  path: String,
}

impl TempIndex {
  fn new() -> Result<Self, CopyCommitError> {
    let dir = tempfile::Builder::new()
      .prefix("branchdeck-amend-")
      .tempdir()
      .map_err(|e| CopyCommitError::Other(anyhow!("Failed to create temporary index directory: {e}")))?;
    let path = dir.path().join("index");
    let path = path
      .to_str()
      .ok_or_else(|| CopyCommitError::Other(anyhow!("Temporary index path is not valid UTF-8: {}", path.display())))?
      .to_string();
    Ok(Self { _dir: dir, path })
  }

  /// Environment that points git at this index
  fn env(&self) -> [(&str, &str); 1] {
    [("GIT_INDEX_FILE", self.path.as_str())]
  }
}

/// `base_tree` with the working-tree versions of `files` (a missing file is removed), built in a temporary index.
/// Reads the working tree, never writes it.
#[instrument(skip(git_executor))]
fn tree_with_worktree_files(git_executor: &GitCommandExecutor, repo_path: &str, base_tree: &str, files: &[String]) -> Result<String, CopyCommitError> {
  let tmp_idx = TempIndex::new()?;
  git_executor
    .execute_command_with_env(&["read-tree", base_tree], repo_path, &tmp_idx.env())
    .map_err(CopyCommitError::Other)?;

  let mut input = String::new();
  for file in files {
    input.push_str(file);
    input.push('\0');
  }
  git_executor
    .execute_command_with_env_and_stdin(&["update-index", "--add", "--remove", "-z", "--stdin"], repo_path, &tmp_idx.env(), &input)
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to read the working tree changes: {e}")))?;

  let tree = git_executor
    .execute_command_with_env(&["write-tree"], repo_path, &tmp_idx.env())
    .map_err(CopyCommitError::Other)?;
  Ok(tree.trim().to_string())
}
