//! Amend uncommitted changes into a commit of the checked out branch without touching the working tree
//! (docs/design/git-operations.md). The descendants of the amended commit are replayed with `git replay`, which writes
//! only objects; the branch moves in one ref transaction, and the index stages exactly the amended files.

use crate::commit_writer::{NewCommit, NewParent, current_committer, read_raw_commits, write_commits};
use crate::copy_commit::CopyCommitError;
use crate::history::History;
use crate::ref_transaction::RefTransaction;
use crate::replay::{merge_trees, replay_onto};
use crate::revs::{resolve_tree_id, short_id};
use crate::rewrite_guard::{IndexUpdate, stage_worktree_files};
use crate::rewrite_plan::{PreparedRewrite, check_target};
use anyhow::anyhow;
use domain_types::BranchError;
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
  let original_commit_id = params.original_commit_id.clone();
  let history = History::read_to(git_executor, repo_path, &[original_commit_id.as_str()])?;
  let prepared = prepare_amend(git_executor, repo_path, &history, params)?;
  apply_amend(git_executor, repo_path, &prepared, &original_commit_id)
}

/// Move the branch to the amend `prepare_amend` wrote for `original_commit_id` (`branch-deck: amend <id>`) and stage
/// the amended files
pub fn apply_amend(git_executor: &GitCommandExecutor, repo_path: &str, prepared: &PreparedRewrite, original_commit_id: &str) -> Result<AmendResult, CopyCommitError> {
  prepared.apply(git_executor, repo_path, RefTransaction::new(), &format!("amend {}", short_id(original_commit_id)))?;
  Ok(AmendResult {
    amended_commit_id: prepared.replacement_of(original_commit_id).unwrap_or_else(|| prepared.new_head()).to_string(),
    rebased_to_commit: prepared.new_head().to_string(),
  })
}

/// Prepare `amend_to_commit_in_main` on `history`, the first-parent history from HEAD down to the amended commit
/// (`History::read_to`), which the caller reads, so that it can check its HEAD before the commit is looked for on it:
/// the amended commit and its replayed descendants are written, the branch is not moved and the index not touched;
/// `PreparedRewrite::apply` does both
#[instrument(skip(git_executor, history), fields(original_commit = %params.original_commit_id))]
pub fn prepare_amend(git_executor: &GitCommandExecutor, repo_path: &str, history: &History, params: AmendToCommitParams) -> Result<PreparedRewrite, CopyCommitError> {
  let AmendToCommitParams { original_commit_id, files } = params;
  if files.is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No files specified to amend")));
  }

  let position = history.position(&original_commit_id)?;
  let target = check_target(git_executor, repo_path, history, position)?;

  // HEAD with the working-tree versions of the files: the tree HEAD has after the amend
  let head_tree = &history.commit(0).tree;
  let worktree_tree = tree_with_worktree_files(git_executor, repo_path, head_tree, &files)?;
  if worktree_tree == *head_tree {
    return Err(CopyCommitError::Other(anyhow!("No uncommitted changes to amend")));
  }

  // the amended commit's tree: its own tree plus the changes made on top of HEAD
  let original_tree = &history.commit(position).tree;
  let amended_tree = if position == 0 {
    worktree_tree.clone()
  } else {
    merge_trees(git_executor, repo_path, head_tree, original_tree, &worktree_tree)?.ok_or_else(|| {
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
    written: vec![written],
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
    if resolve_tree_id(git_executor, repo_path, &prepared.new_head)? != worktree_tree {
      prepared.discard(git_executor, repo_path);
      return Err(CopyCommitError::BranchError(BranchError::Generic(format!(
        "Amending commit {} would change later commits differently than the working tree; amend into a later commit instead",
        short_id(&original_commit_id)
      ))));
    }
  }

  debug!(%amended_commit_id, new_head = %prepared.new_head, "prepared the amend");
  Ok(prepared)
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
  const fn env(&self) -> [(&str, &str); 1] {
    [("GIT_INDEX_FILE", self.path.as_str())]
  }
}

/// `base_tree` with the working-tree versions of `files` (a missing file is removed), built in a temporary index.
/// Reads the working tree, never writes it.
#[instrument(skip(git_executor))]
fn tree_with_worktree_files(git_executor: &GitCommandExecutor, repo_path: &str, base_tree: &str, files: &[String]) -> Result<String, CopyCommitError> {
  let tmp_idx = TempIndex::new()?;
  git_executor
    .cmd(&["read-tree", base_tree], repo_path)
    .env(&tmp_idx.env())
    .text()
    .map_err(CopyCommitError::Other)?;

  stage_worktree_files(git_executor, repo_path, &tmp_idx.env(), files).map_err(|e| CopyCommitError::Other(anyhow!("Failed to read the working tree changes: {e}")))?;

  let tree = git_executor.cmd(&["write-tree"], repo_path).env(&tmp_idx.env()).text().map_err(CopyCommitError::Other)?;
  Ok(tree.trim().to_string())
}
