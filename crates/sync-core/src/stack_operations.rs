//! Branch operations of the UI over the regroup cores: rename a branch with its stacked children and its issue groups,
//! and stack a branch on another one or make it a root. Their results have the shape of a move, and the UI undoes
//! every one of them, a move included, with `undo_regroup`. `get_stack_independence` tells, before an unstack, whether
//! the branch builds without its parent.

use crate::check::{StackIndependence, stack_independence};
use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::move_commits::{MoveCommitsResult, move_result};
use history_rewrite::regroup::{RegroupOptions, UndoToken, rename_branch_core, restack_branch_core, undo_regroup_core};
use history_rewrite::repository_lock::DEFAULT_LOCK_TIMEOUT;
use object_cache::cache::ObjectCache;
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
pub struct StackIndependenceParams {
  pub repository_path: String,
  pub branch_prefix: String,
  /// A stacked branch
  pub branch_name: String,
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
/// their prefix, and the issue groups of that name get the new name without a rewrite
#[instrument(skip(git_executor, params), fields(branch = %params.branch_name, new_name = %params.new_name))]
pub fn rename_branch(git_executor: &GitCommandExecutor, params: &RenameBranchParams) -> Result<MoveCommitsResult> {
  let outcome = rename_branch_core(git_executor, &params.repository_path, &params.branch_name, &params.new_name, &RegroupOptions::default())?;
  Ok(move_result(&outcome, &[], Some(params.new_name.clone())))
}

/// Stacks a branch on `parent`, or makes it a root, in one history rewrite of its commits' prefixes
#[instrument(skip(git_executor, params), fields(branch = %params.branch_name, parent = ?params.parent))]
pub fn restack_branch(git_executor: &GitCommandExecutor, params: &RestackBranchParams) -> Result<MoveCommitsResult> {
  let outcome = restack_branch_core(
    git_executor,
    &params.repository_path,
    &params.branch_name,
    params.parent.as_deref(),
    &RegroupOptions::default(),
  )?;
  Ok(move_result(&outcome, &[], Some(params.branch_name.clone())))
}

/// Whether a stacked branch builds on the baseline without its parent, and else which commits of its ancestors it
/// needs: a trial build of the branch as a root (`stack_independence`), objects only, without the repository lock
#[instrument(skip(git_executor, cache, params), fields(branch = %params.branch_name))]
pub fn get_stack_independence(git_executor: &GitCommandExecutor, cache: &ObjectCache, params: &StackIndependenceParams) -> Result<StackIndependence> {
  Ok(stack_independence(
    git_executor,
    &params.repository_path,
    &params.branch_prefix,
    &params.branch_name,
    cache,
  )?)
}

/// Moves back what a move, a rename or a restack wrote, as its `undo_token` names it: HEAD and the issue groups in one
/// ref transaction with compare-and-swap, under the repository lock (`undo_regroup_core`)
#[instrument(skip(git_executor, params), fields(token = %params.token))]
pub fn undo_regroup(git_executor: &GitCommandExecutor, params: &UndoRegroupParams) -> Result<()> {
  let token = UndoToken::parse(&params.token).with_context(|| format!("Not an undo token: {}", params.token))?;
  undo_regroup_core(git_executor, &params.repository_path, &token, DEFAULT_LOCK_TIMEOUT)?;
  Ok(())
}

#[cfg(test)]
#[path = "stack_operations_test.rs"]
mod stack_operations_test;
