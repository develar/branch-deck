//! Branch operations of the UI over the regroup cores: rename a branch with its stacked children and its issue groups,
//! and stack a branch on another one or make it a root. Their results have the shape of a move, and the UI undoes
//! every one of them, a move included, with `undo_regroup`. Whether the branch builds without its parent, before an
//! unstack, is a trial build (`check::get_stack_independence`).

use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::move_commits::{MoveCommitsResult, move_result};
use history_rewrite::regroup::{RegroupOptions, UndoToken, rename_branch_core, restack_branch_core, undo_regroup_core};
use history_rewrite::repository_lock::LockWait;
use serde::Deserialize;
use tracing::instrument;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RenameBranchParams {
  pub repository_path: String,
  /// The branch (a leaf with stacked branches on)
  pub branch_name: String,
  pub new_name: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RestackBranchParams {
  pub repository_path: String,
  pub branch_name: String,
  /// The branch to stack on; `None` makes the branch a root (unstack)
  pub parent: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UndoRegroupParams {
  pub repository_path: String,
  /// What the operation reported in `MoveCommitsResult.undo_token`
  pub token: String,
}

/// Renames a branch in one history rewrite: its commits and those of the branches stacked on it get the new name in
/// their prefix, and the issue groups of that name get the new name without a rewrite; the lock is waited for as
/// `lock_wait` says
#[instrument(skip(git_executor, params), fields(branch = %params.branch_name, new_name = %params.new_name))]
pub fn rename_branch(git_executor: &GitCommandExecutor, params: &RenameBranchParams, lock_wait: LockWait) -> Result<MoveCommitsResult> {
  let options = RegroupOptions {
    lock_wait,
    ..RegroupOptions::default()
  };
  let outcome = rename_branch_core(git_executor, &params.repository_path, &params.branch_name, &params.new_name, &options)?;
  Ok(move_result(&outcome, &[], Some(params.new_name.clone())))
}

/// Stacks a branch on `parent`, or makes it a root, in one history rewrite of its commits' prefixes; the lock is
/// waited for as `lock_wait` says
#[instrument(skip(git_executor, params), fields(branch = %params.branch_name, parent = ?params.parent))]
pub fn restack_branch(git_executor: &GitCommandExecutor, params: &RestackBranchParams, lock_wait: LockWait) -> Result<MoveCommitsResult> {
  let options = RegroupOptions {
    lock_wait,
    ..RegroupOptions::default()
  };
  let outcome = restack_branch_core(git_executor, &params.repository_path, &params.branch_name, params.parent.as_deref(), &options)?;
  Ok(move_result(&outcome, &[], Some(params.branch_name.clone())))
}

/// Moves back what a move, a rename or a restack wrote, as its `undo_token` names it: HEAD and the issue groups in one
/// ref transaction with compare-and-swap, under the repository lock (`undo_regroup_core`)
#[instrument(skip(git_executor, params), fields(token = %params.token))]
pub fn undo_regroup(git_executor: &GitCommandExecutor, params: &UndoRegroupParams, lock_wait: LockWait) -> Result<()> {
  let token = UndoToken::parse(&params.token).with_context(|| format!("Not an undo token: {}", params.token))?;
  undo_regroup_core(git_executor, &params.repository_path, &token, lock_wait)?;
  Ok(())
}

#[cfg(test)]
#[path = "stack_operations_test.rs"]
mod stack_operations_test;
