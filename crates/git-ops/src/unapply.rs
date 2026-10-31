//! Unapply a virtual branch: its original commits leave the checked out branch, the files following, and the virtual
//! branch moves to the unapplied namespace in the transaction that moves HEAD (docs/design/git-operations.md, "Unapply
//! and reapply"). The drop is the rewrite plan of `rewrite_plan` (`git replay` of the kept runs, objects only); a dry
//! run on a copy of the index proves the files can follow, and only then does HEAD move, through the guarded update of
//! `update_branch`. Only `check_unapply` builds a plan, so no other crate can move HEAD through the working-tree
//! exemption.

use crate::copy_commit::CopyCommitError;
use crate::paths::changed_paths;
use crate::ref_transaction::RefTransaction;
use crate::rewrite_plan::{PreparedRewrite, prepare_drop_commits};
use crate::update_branch::{
  FileMove, HeadRef, MAX_BLOCKED_PATHS, UNAPPLY, UndoCheck, UpdateUndo, apply_move_undo_with, blocking_paths, check_move_undo, check_update_guards, move_branch_and_files_carrying,
};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_utils::list_cut::kept;
use tracing::{debug, instrument};

/// Uncommitted, staged or untracked files sit on paths the unapply (or the reapply) writes; nothing was changed
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Uncommitted, staged or untracked files sit on paths the {operation} writes: {}{}", .paths.join(", "), more(*.path_count, .paths.len()))]
pub struct MoveBlocked {
  pub operation: &'static str,
  /// At most the first `MAX_BLOCKED_PATHS` (all of them when only one more)
  pub paths: Vec<String>,
  pub path_count: u32,
}

/// ` and N more` when the list was cut
fn more(path_count: u32, shown: usize) -> String {
  let rest = (path_count as usize).saturating_sub(shown);
  if rest == 0 { String::new() } else { format!(" and {rest} more") }
}

impl MoveBlocked {
  /// `paths` cut to `MAX_BLOCKED_PATHS`, all of them when only one more
  pub fn new(operation: &'static str, mut paths: Vec<String>) -> Self {
    let path_count = u32::try_from(paths.len()).unwrap_or(u32::MAX);
    paths.truncate(kept(paths.len(), MAX_BLOCKED_PATHS));
    Self { operation, paths, path_count }
  }
}

/// The drop of the commits is refused: a later commit depends on the changes of one of them (a conflict), or the range
/// holds a merge; nothing was changed
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct DropRefused(pub String);

/// A checked unapply: the drop is written (objects on temporary refs) and the files can follow. `apply_unapply`
/// applies it, `discard` abandons it
#[derive(Debug)]
pub struct UnapplyPlan {
  branch: String,
  prepared: PreparedRewrite,
  files_change: bool,
}

impl UnapplyPlan {
  /// The checked out branch
  pub fn branch(&self) -> &str {
    &self.branch
  }

  /// HEAD before the unapply
  pub fn head(&self) -> &str {
    self.prepared.old_head()
  }

  /// HEAD after the unapply
  pub fn new_head(&self) -> &str {
    self.prepared.new_head()
  }

  /// Whether the files follow the unapply: the index and the working tree change
  pub const fn files_change(&self) -> bool {
    self.files_change
  }

  /// Delete the temporary refs of the drop, leaving its objects to `git gc`
  pub fn discard(&self, git_executor: &GitCommandExecutor, repo_path: &str) {
    self.prepared.discard(git_executor, repo_path);
  }
}

/// Check the unapply of `commit_ids` (the original commits of a virtual branch) from the checked out branch: the guards
/// (no operation in progress, HEAD on a branch, no merge in the range), the drop replayed on objects (a conflict refuses
/// it), then the dry run of the files on a copy of the index. Local changes on a path the drop changes refuse it with
/// `MoveBlocked`, staged or not; work on other paths stays as it is. Nothing is changed but git objects
#[instrument(skip(git_executor))]
pub fn check_unapply(git_executor: &GitCommandExecutor, repo_path: &str, commit_ids: &[String]) -> Result<UnapplyPlan> {
  let branch = check_update_guards(git_executor, repo_path)?;
  let prepared = prepare_drop_commits(git_executor, repo_path, commit_ids).map_err(|e| match e {
    CopyCommitError::Other(e) => e,
    e @ CopyCommitError::BranchError(_) => anyhow::Error::new(DropRefused(e.to_string())),
  })?;
  let checked = || -> Result<bool> {
    let changed = changed_paths(git_executor, repo_path, prepared.old_head(), prepared.new_head())?;
    if changed.is_empty() {
      return Ok(false);
    }
    let blocked = blocking_paths(git_executor, repo_path, prepared.old_head(), prepared.new_head(), &changed)?;
    if !blocked.is_empty() {
      return Err(anyhow::Error::new(MoveBlocked::new(UNAPPLY.done, blocked)));
    }
    Ok(true)
  };
  let files_change = checked().inspect_err(|_| prepared.discard(git_executor, repo_path))?;
  debug!(new_head = %prepared.new_head(), files_change, "checked the unapply");
  Ok(UnapplyPlan { branch, prepared, files_change })
}

/// The virtual branch an unapply moves, and where to
#[derive(Debug, Clone, Copy)]
pub struct UnappliedMove<'a> {
  /// `refs/heads/{prefix}/virtual/<name>`
  pub virtual_ref: &'a str,
  /// `refs/heads/{prefix}/unapplied/<name>`, a name no ref takes
  pub unapplied_ref: &'a str,
  /// The tip of the virtual branch, which the unapplied branch gets
  pub tip: &'a str,
}

/// Move HEAD to the checked unapply, the files following, with the virtual branch moved to `refs.unapplied_ref` in the
/// same transaction (the delete checked against its tip). If the files can't follow (a file changed since the check),
/// HEAD and the virtual branch move back and nothing is changed
#[instrument(skip(git_executor, plan), fields(branch = %plan.branch))]
pub fn apply_unapply(git_executor: &GitCommandExecutor, repo_path: &str, plan: &UnapplyPlan, refs: &UnappliedMove<'_>) -> Result<()> {
  let mut with = RefTransaction::new();
  with.create(refs.unapplied_ref, refs.tip).delete(refs.virtual_ref, Some(refs.tip));
  for written in &plan.prepared.written {
    written.delete_temp_refs(&mut with);
  }
  let mut rollback_with = RefTransaction::new();
  rollback_with.delete(refs.unapplied_ref, Some(refs.tip)).create(refs.virtual_ref, refs.tip);
  let moved = move_branch_and_files_carrying(
    git_executor,
    repo_path,
    &FileMove {
      head_ref: &HeadRef::Branch(plan.branch.clone()),
      head: plan.head(),
      new_head: plan.new_head(),
      files_change: plan.files_change,
      operation: UNAPPLY.done,
      with: &with,
      rollback_with: &rollback_with,
    },
  );
  // a move that failed before the transaction leaves the temporary refs; after a rollback they are gone already
  moved.inspect_err(|_| plan.discard(git_executor, repo_path))
}

/// Check the undo of an unapply that moved the checked out branch from `previous_head` to `new_head`: the last entry of
/// its reflog must be that unapply, as for the undo of an update (`update_branch::check_update_undo`)
#[instrument(skip(git_executor))]
pub fn check_unapply_undo(git_executor: &GitCommandExecutor, repo_path: &str, previous_head: &str, new_head: &str) -> Result<UndoCheck> {
  let branch = check_update_guards(git_executor, repo_path)?;
  check_move_undo(git_executor, repo_path, UNAPPLY, &HeadRef::Branch(branch), previous_head, new_head)
}

/// Move HEAD back as checked, the files following, with the unapplied branch moved back to the virtual one in the same
/// transaction (`refs` as the unapply moved them). Refused while local changes block it; if the files can't follow, HEAD
/// and the branches move forward again and nothing is changed
#[instrument(skip(git_executor, undo))]
pub fn apply_unapply_undo(git_executor: &GitCommandExecutor, repo_path: &str, undo: &UpdateUndo, refs: &UnappliedMove<'_>) -> Result<()> {
  if !undo.blocked().is_empty() {
    return Err(anyhow::Error::new(MoveBlocked::new(UNAPPLY.undo, undo.blocked().to_vec())));
  }
  let mut with = RefTransaction::new();
  with.delete(refs.unapplied_ref, Some(refs.tip)).create(refs.virtual_ref, refs.tip);
  let mut rollback_with = RefTransaction::new();
  rollback_with.delete(refs.virtual_ref, Some(refs.tip)).create(refs.unapplied_ref, refs.tip);
  apply_move_undo_with(git_executor, repo_path, undo, &with, &rollback_with)
}

#[cfg(test)]
#[path = "unapply_test.rs"]
mod unapply_test;
