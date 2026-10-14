use git_executor::git_command_executor::GitCommandExecutor;
use model_tauri::blocking::run_blocking;
use model_tauri::command_error::CommandError;
use sync_core::move_commits::MoveCommitsResult;
use sync_core::stack_operations::{RenameBranchParams, RestackBranchParams, rename_branch as rename_branch_core, restack_branch as restack_branch_core};
use tauri::State;
use tracing::instrument;

/// Renames a branch, and its name in the prefixes of the branches stacked on it, in one history rewrite.
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
