//! Amend and drop commits of the checked out branch without touching the working tree
//! (docs/design/git-operations.md). The descendants of a changed commit are replayed with `git replay`, which writes
//! only objects; the branch moves in one ref transaction, and the index follows only where HEAD's tree changed.

use crate::commit_writer::{NewCommit, NewParent, current_committer, read_raw_commits, write_commits};
use crate::copy_commit::CopyCommitError;
use crate::model::BranchError;
use crate::ref_transaction::RefTransaction;
use crate::rewrite_guard::{IndexUpdate, RewriteTarget, check_rewrite};
use anyhow::anyhow;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{debug, instrument};

/// `git replay --ref-action=print` reports the new tip as an update of this ref; the ref itself is never written
const REPLAY_REF: &str = "refs/branch-deck/replay";

/// A rewrite of the checked out branch whose new commits exist, but whose branch has not moved yet
#[derive(Debug)]
pub struct PreparedRewrite {
  pub target: RewriteTarget,
  pub new_head: String,
  index_update: IndexUpdate,
  /// Refs keeping the new objects reachable until the branch moves
  temp_refs: Vec<String>,
}

impl PreparedRewrite {
  /// Move the branch in one transaction with `transaction` (other ref changes of the same operation), then let the
  /// index follow. Nothing changes if a staged change would be lost or the branch moved meanwhile.
  #[instrument(skip(self, git_executor, transaction), fields(new_head = %self.new_head))]
  pub fn apply(&self, git_executor: &GitCommandExecutor, repo_path: &str, mut transaction: RefTransaction, operation: &str) -> Result<(), CopyCommitError> {
    if let Err(e) = self.index_update.preflight(git_executor, repo_path) {
      self.discard(git_executor, repo_path);
      return Err(CopyCommitError::BranchError(BranchError::Generic(format!("{e:#}"))));
    }

    transaction.update(&self.target.full_ref(), &self.new_head, Some(&self.target.head));
    for temp_ref in &self.temp_refs {
      transaction.delete(temp_ref, None);
    }
    if let Err(e) = transaction.commit(git_executor, repo_path, operation) {
      self.discard(git_executor, repo_path);
      return Err(CopyCommitError::Other(anyhow!(
        "Branch {} changed while rewriting, sync and try again: {e:#}",
        self.target.branch
      )));
    }

    self.index_update.apply(git_executor, repo_path).map_err(CopyCommitError::Other)
  }

  /// Abandon the rewrite: delete the temporary refs, leaving the unreferenced objects to `git gc`
  pub fn discard(&self, git_executor: &GitCommandExecutor, repo_path: &str) {
    for temp_ref in &self.temp_refs {
      if let Err(e) = git_executor.execute_command(&["update-ref", "-d", temp_ref], repo_path) {
        tracing::warn!(temp_ref = %temp_ref, error = %e, "failed to delete temporary ref");
      }
    }
  }
}

/// First-parent history of HEAD, newest first, with each commit's position
struct History {
  commits: Vec<String>,
  positions: HashMap<String, usize>,
}

impl History {
  fn read(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Self, CopyCommitError> {
    let commits = git_executor.execute_command_lines(&["rev-list", "--first-parent", "HEAD"], repo_path)?;
    let positions = commits.iter().enumerate().map(|(index, id)| (id.clone(), index)).collect();
    Ok(Self { commits, positions })
  }

  fn position(&self, commit_id: &str) -> Result<usize, CopyCommitError> {
    self
      .positions
      .get(commit_id)
      .copied()
      .ok_or_else(|| CopyCommitError::Other(anyhow!("Commit {} is not on the current branch, sync and try again", short_id(commit_id))))
  }

  /// First parent of the commit at `position`; `None` for the root commit
  fn parent(&self, position: usize) -> Option<&str> {
    self.commits.get(position + 1).map(String::as_str)
  }

  /// Read the rewrite target and check that HEAD is still the tip this history was read from
  fn check_rewrite(&self, git_executor: &GitCommandExecutor, repo_path: &str, oldest_position: usize) -> Result<RewriteTarget, CopyCommitError> {
    let target = check_rewrite(git_executor, repo_path, &self.commits[oldest_position]).map_err(|e| CopyCommitError::BranchError(BranchError::Generic(format!("{e:#}"))))?;
    if self.commits.first() != Some(&target.head) {
      return Err(CopyCommitError::Other(anyhow!("Branch {} changed meanwhile, sync and try again", target.branch)));
    }
    Ok(target)
  }
}

fn short_id(id: &str) -> &str {
  id.get(..8).unwrap_or(id)
}

/// Replay the commits of `from..to` (linear history) onto `onto` with `git replay`; returns the new tip.
/// The replayed commits keep their authors; the committer is the current user. `None` on a conflict.
#[instrument(skip(git_executor))]
fn replay_onto(git_executor: &GitCommandExecutor, repo_path: &str, onto: &str, from: &str, to: &str) -> Result<Option<String>, CopyCommitError> {
  let onto_arg = format!("--onto={onto}");
  let ref_arg = format!("--ref={REPLAY_REF}");
  let range = format!("{from}..{to}");
  let (output, exit_code) = git_executor.execute_command_with_status(&["replay", "--ref-action=print", &onto_arg, &ref_arg, &range], repo_path)?;
  match exit_code {
    0 => {
      // `update <ref> <new> <old>`
      let new_tip = output
        .lines()
        .find_map(|line| line.strip_prefix("update ")?.strip_prefix(REPLAY_REF)?.split_whitespace().next())
        .ok_or_else(|| CopyCommitError::Other(anyhow!("git replay printed no new tip: {output}")))?;
      Ok(Some(new_tip.to_string()))
    }
    1 => Ok(None),
    _ => Err(CopyCommitError::Other(anyhow!("git replay failed: {output}"))),
  }
}

/// Prepare dropping `commit_ids_to_drop` from the checked out branch: their descendants are replayed onto the
/// remaining history. The branch is not moved; `PreparedRewrite::apply` does it.
/// The working tree keeps the dropped content, so it shows up as unstaged changes (files the dropped commits added as
/// untracked); staged changes on other paths stay staged.
#[instrument(skip(git_executor))]
pub fn prepare_drop_commits(git_executor: &GitCommandExecutor, repo_path: &str, commit_ids_to_drop: &[String]) -> Result<PreparedRewrite, CopyCommitError> {
  if commit_ids_to_drop.is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No commits specified to drop")));
  }

  let history = History::read(git_executor, repo_path)?;
  let drop_set: HashSet<&str> = commit_ids_to_drop.iter().map(String::as_str).collect();
  // oldest first
  let mut drop_positions = drop_set.iter().map(|id| history.position(id)).collect::<Result<Vec<_>, _>>()?;
  drop_positions.sort_unstable_by(|a, b| b.cmp(a));

  let oldest_position = drop_positions[0];
  let target = history.check_rewrite(git_executor, repo_path, oldest_position)?;
  let mut new_head = history
    .parent(oldest_position)
    .ok_or_else(|| CopyCommitError::Other(anyhow!("Cannot drop the root commit {}", short_id(&history.commits[oldest_position]))))?
    .to_string();

  // replay each run of kept commits between two dropped ones (or after the last one) onto the new history
  for (index, &drop_position) in drop_positions.iter().enumerate() {
    let segment_end = match drop_positions.get(index + 1) {
      // the parent of the next dropped commit, unless it is this dropped commit itself (adjacent drops)
      Some(&next_drop) if next_drop + 1 == drop_position => continue,
      Some(&next_drop) => &history.commits[next_drop + 1],
      None if drop_position == 0 => continue,
      None => &target.head,
    };
    let dropped = &history.commits[drop_position];
    new_head = replay_onto(git_executor, repo_path, &new_head, dropped, segment_end)?.ok_or_else(|| {
      CopyCommitError::BranchError(BranchError::Generic(format!(
        "Removing commit {} conflicts with later commits on {}: they depend on its changes",
        short_id(dropped),
        target.branch
      )))
    })?;
  }

  let old_tree = git_executor.resolve_tree_id(repo_path, &target.head)?;
  let new_tree = git_executor.resolve_tree_id(repo_path, &new_head)?;
  debug!(%new_head, "prepared dropping commits");
  Ok(PreparedRewrite {
    target,
    new_head,
    index_update: IndexUpdate::TwoWay { old_tree, new_tree },
    temp_refs: Vec::new(),
  })
}

/// Drop `commit_ids_to_drop` from the checked out branch; returns the new HEAD
#[instrument(skip(git_executor))]
pub fn drop_commits_from_head(git_executor: &GitCommandExecutor, repo_path: &str, commit_ids_to_drop: &[String]) -> Result<String, CopyCommitError> {
  let prepared = prepare_drop_commits(git_executor, repo_path, commit_ids_to_drop)?;
  prepared.apply(git_executor, repo_path, RefTransaction::new(), &format!("drop {} commits", commit_ids_to_drop.len()))?;
  Ok(prepared.new_head)
}

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

/// Three-way merge of trees without a working tree; `None` on a conflict
fn merge_trees(git_executor: &GitCommandExecutor, repo_path: &str, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, CopyCommitError> {
  let merge_base = format!("--merge-base={base}");
  let (output, exit_code) = git_executor.execute_command_with_status(&["merge-tree", "--write-tree", "--no-messages", &merge_base, ours, theirs], repo_path)?;
  match exit_code {
    0 => Ok(Some(output.lines().next().unwrap_or_default().to_string())),
    1 => Ok(None),
    _ => Err(CopyCommitError::Other(anyhow!("git merge-tree failed: {output}"))),
  }
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
