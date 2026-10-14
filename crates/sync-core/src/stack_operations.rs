//! Branch operations of the UI over the regroup cores: rename a branch with its stacked children and its issue groups,
//! and stack a branch on another one or make it a root. Their results have the shape of a move, so the UI undoes them
//! the same way, and moves the issue groups back with `undo_issue_groups`.

use crate::move_commits::{IssueGroupsMove, MoveCommitsResult, MovedCommit};
use crate::regroup::{RegroupOptions, RegroupOutcome, UndoToken, rename_branch_core, restack_branch_core, undo_regroup_core};
use crate::repository_lock::DEFAULT_LOCK_TIMEOUT;
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

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UndoIssueGroupsParams {
  pub repository_path: String,
  /// What the operation reported in `MoveCommitsResult.issue_groups`
  pub issue_groups: IssueGroupsMove,
}

/// Renames a branch in one history rewrite: its commits and those of the branches stacked on it get the new name in
/// their prefix, and the issue groups of that name get the new name without a rewrite
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

/// Moves the issue groups back from `new` to `previous` with compare-and-swap, under the repository lock: the undo of
/// the issue groups part of a rename
#[instrument(skip(git_executor, params))]
pub fn undo_issue_groups(git_executor: &GitCommandExecutor, params: &UndoIssueGroupsParams) -> Result<()> {
  let token = UndoToken {
    head: None,
    issue_groups: Some((params.issue_groups.previous.clone(), params.issue_groups.new.clone())),
  };
  undo_regroup_core(git_executor, &params.repository_path, &token, DEFAULT_LOCK_TIMEOUT)?;
  Ok(())
}

/// The rewritten commits with their previous messages, oldest first, and the change of the issue groups, for an undo
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
  let issue_groups = outcome.issue_groups.and_then(|change| {
    change.new_commit.map(|new| IssueGroupsMove {
      previous: change.previous_commit,
      new,
    })
  });
  MoveCommitsResult { moved, issue_groups }
}

#[cfg(test)]
#[path = "stack_operations_test.rs"]
mod stack_operations_test;
