use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::move_commits::MoveCommitsResult;
use sync_core::stack_operations::{
  RenameBranchParams, RestackBranchParams, UndoIssueGroupsParams, rename_branch as rename_branch_core, restack_branch as restack_branch_core,
  undo_issue_groups as undo_issue_groups_core,
};
use tauri::State;
use tracing::instrument;

/// Renames a branch, and its name in the prefixes of the branches stacked on it and in the issue groups, in one history
/// rewrite.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn rename_branch(git_executor: State<'_, GitCommandExecutor>, params: RenameBranchParams) -> Result<MoveCommitsResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || rename_branch_core(&git, &params)).await
}

/// Stacks a branch on another one, or makes it a root (`parent: null`), in one history rewrite.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn restack_branch(git_executor: State<'_, GitCommandExecutor>, params: RestackBranchParams) -> Result<MoveCommitsResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || restack_branch_core(&git, &params)).await
}

/// Moves the issue groups back to where they were before a rename, with compare-and-swap.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn undo_issue_groups(git_executor: State<'_, GitCommandExecutor>, params: UndoIssueGroupsParams) -> Result<(), CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || undo_issue_groups_core(&git, &params)).await
}
