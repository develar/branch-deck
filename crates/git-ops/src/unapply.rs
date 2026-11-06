//! Drop commits from the checked out branch, the files following: the unapply of a virtual branch, whose branch moves
//! to the unapplied namespace in the transaction that moves HEAD, and the drop of commits for good (`brd drop`, the
//! app's Drop Commit; docs/design/git-operations.md, "Drop and unapply"). The drop is the rewrite plan of
//! `rewrite_plan` (`git replay` of the kept runs, objects only); a dry run on a copy of the index proves the files can
//! follow, and only then does HEAD move, through the guarded update of `update_branch`. Only `check_drop` builds a
//! plan, and its `DropKind` names the move, so no other crate can move HEAD through the working-tree exemption.

use crate::commit_signing::Signing;
use crate::copy_commit::CopyCommitError;
use crate::paths::changed_paths;
use crate::ref_transaction::RefTransaction;
use crate::rewrite_plan::{PreparedRewrite, RefusalKind, prepare_drop_commits, refused_in};
use crate::update_branch::{
  DROP, FileMove, HeadRef, MAX_BLOCKED_PATHS, MoveOperation, UNAPPLY, UndoCheck, UpdateUndo, apply_move_undo_with, blocking_paths, check_move_undo, check_update_guards,
  move_branch_and_files_carrying,
};
use anyhow::{Result, anyhow};
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

/// The drop of the commits is refused: a later commit depends on the changes of one of them (a conflict), a later
/// commit would become empty (it holds only changes of the dropped commits), the root would go, or the range holds a
/// merge; nothing was changed
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct DropRefused(pub String);

/// Which move drops the commits: it names the move in the reflog, which its undo and the finish of an interrupted one
/// look for, and in the messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropKind {
  /// The commits of a virtual branch leave HEAD, the branch kept as unapplied (`apply_unapply`)
  Unapply,
  /// Commits leave HEAD for good (`apply_drop`), `brd drop` and the app's Drop Commit
  Drop,
}

impl DropKind {
  const fn operation(self) -> MoveOperation {
    match self {
      Self::Unapply => UNAPPLY,
      Self::Drop => DROP,
    }
  }
}

/// A checked drop: the new history is written (objects on temporary refs) and the files can follow. `apply_unapply` or
/// `apply_drop` (by its kind) applies it, `discard` abandons it
#[derive(Debug)]
pub struct DropPlan {
  kind: DropKind,
  branch: String,
  prepared: PreparedRewrite,
  files_change: bool,
}

impl DropPlan {
  /// The checked out branch
  pub fn branch(&self) -> &str {
    &self.branch
  }

  /// HEAD before the drop
  pub fn head(&self) -> &str {
    self.prepared.old_head()
  }

  /// HEAD after the drop
  pub fn new_head(&self) -> &str {
    self.prepared.new_head()
  }

  /// Whether the files follow the drop: the index and the working tree change
  pub const fn files_change(&self) -> bool {
    self.files_change
  }

  /// The replayed commits that keep the mapping notes of their originals, newest first: a sync keeps their virtual
  /// copies
  pub fn carried_notes(&self) -> impl Iterator<Item = &str> {
    self.prepared.carried_notes()
  }

  /// Write the mapping notes the replayed commits keep, before the lock and the move, so a sync that the move wakes
  /// finds them; best effort, as for every rewrite (`PreparedRewrite::ready`). A dry run writes none
  pub fn write_carried_notes(&self, git_executor: &GitCommandExecutor, repo_path: &str) {
    self.prepared.write_carried_notes(git_executor, repo_path);
  }

  /// Delete the temporary refs of the drop, leaving its objects to `git gc`
  pub fn discard(&self, git_executor: &GitCommandExecutor, repo_path: &str) {
    self.prepared.discard(git_executor, repo_path);
  }
}

/// Check the drop of `commit_ids` from the checked out branch: the guards (no operation in progress, HEAD on a branch,
/// no merge in the range), the drop replayed on objects (a later commit that depends on a dropped one or would become
/// empty refuses it, `DropRefused`), then the dry run of the files on a copy of the index. Local changes on a path the
/// drop changes refuse it with `MoveBlocked`, staged or not; work on other paths stays as it is. Nothing is changed but
/// git objects
#[instrument(skip(git_executor))]
pub fn check_drop(git_executor: &GitCommandExecutor, repo_path: &str, commit_ids: &[String], kind: DropKind, signing: Signing) -> Result<DropPlan> {
  let branch = check_update_guards(git_executor, repo_path)?;
  let prepared = prepare_drop_commits(git_executor, repo_path, commit_ids, signing).map_err(|e| match e {
    CopyCommitError::Other(e) => match refused_in(&e) {
      Some(refused) if refused.kind == RefusalKind::Empty => anyhow::Error::new(DropRefused(emptied_message(kind, &branch))),
      _ => e,
    },
    e @ CopyCommitError::BranchError(_) => anyhow::Error::new(DropRefused(e.to_string())),
  })?;
  let checked = || -> Result<bool> {
    let changed = changed_paths(git_executor, repo_path, prepared.old_head(), prepared.new_head())?;
    if changed.is_empty() {
      return Ok(false);
    }
    let blocked = blocking_paths(git_executor, repo_path, prepared.old_head(), prepared.new_head(), &changed)?;
    if !blocked.is_empty() {
      return Err(anyhow::Error::new(MoveBlocked::new(kind.operation().done, blocked)));
    }
    Ok(true)
  };
  let files_change = checked().inspect_err(|_| prepared.discard(git_executor, repo_path))?;
  debug!(new_head = %prepared.new_head(), files_change, ?kind, "checked the drop");
  Ok(DropPlan {
    kind,
    branch,
    prepared,
    files_change,
  })
}

/// Why a drop that would leave a later commit empty is refused: `git replay` would leave that commit out, its message
/// lost with it
fn emptied_message(kind: DropKind, branch: &str) -> String {
  match kind {
    DropKind::Unapply => {
      format!("A later commit of {branch} holds only changes of the unapplied branch and would become empty: move it into that branch first")
    }
    DropKind::Drop => format!("A later commit of {branch} holds only changes of the dropped commits and would become empty: drop it too"),
  }
}

/// Move HEAD to a checked drop of `DropKind::Drop`, the files following. If the files can't follow (a file changed
/// since the check), HEAD moves back and nothing is changed
#[instrument(skip(git_executor, plan), fields(branch = %plan.branch))]
pub fn apply_drop(git_executor: &GitCommandExecutor, repo_path: &str, plan: &DropPlan) -> Result<()> {
  if plan.kind != DropKind::Drop {
    return Err(anyhow!("A drop of {:?} applies as that move", plan.kind));
  }
  move_files(git_executor, repo_path, plan, RefTransaction::new(), &RefTransaction::new())
}

/// Move HEAD to the checked drop, the files following, with `with` in the same transaction and `rollback_with` in the
/// one that moves HEAD back when the files can't follow: the one move through the working-tree exemption
fn move_files(git_executor: &GitCommandExecutor, repo_path: &str, plan: &DropPlan, mut with: RefTransaction, rollback_with: &RefTransaction) -> Result<()> {
  for written in &plan.prepared.written {
    written.delete_temp_refs(&mut with);
  }
  let moved = move_branch_and_files_carrying(
    git_executor,
    repo_path,
    &FileMove {
      head_ref: &HeadRef::Branch(plan.branch.clone()),
      head: plan.head(),
      new_head: plan.new_head(),
      files_change: plan.files_change,
      refreshed: false,
      operation: plan.kind.operation().done,
      with: &with,
      rollback_with,
    },
  );
  // a move that failed before the transaction leaves the temporary refs; after a rollback they are gone already
  moved.inspect_err(|_| plan.discard(git_executor, repo_path))
}

/// Check the undo of a drop of `kind` that moved the checked out branch from `previous_head` to `new_head`: the last
/// entry of its reflog must be that move, as for the undo of an update (`update_branch::check_update_undo`)
#[instrument(skip(git_executor))]
pub fn check_drop_undo(git_executor: &GitCommandExecutor, repo_path: &str, kind: DropKind, previous_head: &str, new_head: &str) -> Result<UndoCheck> {
  let branch = check_update_guards(git_executor, repo_path)?;
  check_move_undo(git_executor, repo_path, kind.operation(), &HeadRef::Branch(branch), previous_head, new_head)
}

/// Move HEAD back as checked by `check_drop_undo` for `DropKind::Drop`, the files following. Refused while local
/// changes block it; if the files can't follow, HEAD moves forward again and nothing is changed
#[instrument(skip(git_executor, undo))]
pub fn apply_drop_undo(git_executor: &GitCommandExecutor, repo_path: &str, undo: &UpdateUndo) -> Result<()> {
  undo_move(git_executor, repo_path, DropKind::Drop, undo, &RefTransaction::new(), &RefTransaction::new())
}

/// The undo of a drop of `kind` with `with` in its transaction: the one undo through the working-tree exemption
fn undo_move(git_executor: &GitCommandExecutor, repo_path: &str, kind: DropKind, undo: &UpdateUndo, with: &RefTransaction, rollback_with: &RefTransaction) -> Result<()> {
  if !undo.blocked().is_empty() {
    return Err(anyhow::Error::new(MoveBlocked::new(kind.operation().undo, undo.blocked().to_vec())));
  }
  apply_move_undo_with(git_executor, repo_path, undo, with, rollback_with)
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
pub fn apply_unapply(git_executor: &GitCommandExecutor, repo_path: &str, plan: &DropPlan, refs: &UnappliedMove<'_>) -> Result<()> {
  if plan.kind != DropKind::Unapply {
    return Err(anyhow!("A drop of {:?} applies as that move", plan.kind));
  }
  let mut with = RefTransaction::new();
  with.create(refs.unapplied_ref, refs.tip).delete(refs.virtual_ref, Some(refs.tip));
  let mut rollback_with = RefTransaction::new();
  rollback_with.delete(refs.unapplied_ref, Some(refs.tip)).create(refs.virtual_ref, refs.tip);
  move_files(git_executor, repo_path, plan, with, &rollback_with)
}

/// Check the undo of an unapply that moved the checked out branch from `previous_head` to `new_head`: the last entry of
/// its reflog must be that unapply, as for the undo of an update (`update_branch::check_update_undo`)
#[instrument(skip(git_executor))]
pub fn check_unapply_undo(git_executor: &GitCommandExecutor, repo_path: &str, previous_head: &str, new_head: &str) -> Result<UndoCheck> {
  check_drop_undo(git_executor, repo_path, DropKind::Unapply, previous_head, new_head)
}

/// Move HEAD back as checked, the files following, with the unapplied branch moved back to the virtual one in the same
/// transaction (`refs` as the unapply moved them). Refused while local changes block it; if the files can't follow, HEAD
/// and the branches move forward again and nothing is changed
#[instrument(skip(git_executor, undo))]
pub fn apply_unapply_undo(git_executor: &GitCommandExecutor, repo_path: &str, undo: &UpdateUndo, refs: &UnappliedMove<'_>) -> Result<()> {
  let mut with = RefTransaction::new();
  with.delete(refs.unapplied_ref, Some(refs.tip)).create(refs.virtual_ref, refs.tip);
  let mut rollback_with = RefTransaction::new();
  rollback_with.delete(refs.virtual_ref, Some(refs.tip)).create(refs.unapplied_ref, refs.tip);
  undo_move(git_executor, repo_path, DropKind::Unapply, undo, &with, &rollback_with)
}

#[cfg(test)]
#[path = "unapply_test.rs"]
mod unapply_test;
