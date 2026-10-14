//! Branch operations of the UI over the regroup cores: rename a branch with its stacked children, and stack a branch
//! on another one or make it a root. Their results have the shape of a move, so the UI undoes them the same way.

use crate::move_commits::{MoveCommitsResult, MovedCommit};
use crate::regroup::{RegroupOptions, RegroupOutcome, rename_branch_core, restack_branch_core};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
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

/// Renames a branch in one history rewrite: its commits and those of the branches stacked on it get the new name in
/// their prefix
#[instrument(skip(git_executor, params), fields(branch = %params.branch_name, new_name = %params.new_name))]
pub fn rename_branch(git_executor: &GitCommandExecutor, params: &RenameBranchParams) -> Result<MoveCommitsResult> {
  let outcome = rename_branch_core(git_executor, &params.repository_path, &params.branch_name, &params.new_name, &RegroupOptions::default())?;
  Ok(moved_commits(outcome))
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
  Ok(moved_commits(outcome))
}

/// The rewritten commits with their previous messages, oldest first, for an undo
fn moved_commits(outcome: RegroupOutcome) -> MoveCommitsResult {
  let moved = outcome
    .commits
    .into_iter()
    .filter(|commit| commit.changed)
    .map(|commit| MovedCommit {
      old_id: commit.id,
      new_id: commit.new_id,
      previous_message: commit.previous_message,
    })
    .collect();
  MoveCommitsResult { moved }
}

#[cfg(test)]
#[path = "stack_operations_test.rs"]
mod stack_operations_test;
