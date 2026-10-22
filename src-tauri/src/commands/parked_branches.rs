use crate::command_error::CommandError;
use crate::operation::run_blocking;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::parked::{ParkBranchesParams, ParkResult, UnparkBranchesParams, park_branches as park_branches_core, unpark_branches as unpark_branches_core};
use tauri::State;
use tracing::instrument;

/// Parks branches with their notes, or changes the notes of parked ones: kept local, never pushed, until unparked. No
/// rewrite: one commit of the store `refs/branch-deck/parked`, under the repository lock.
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branches = params.branches.len()))]
#[specta::specta]
pub async fn park_branches(git_executor: State<'_, GitCommandExecutor>, params: ParkBranchesParams) -> Result<ParkResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || park_branches_core(&git, &params)).await
}

/// Unparks branches: they can be pushed and landed again.
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branches = params.branch_names.len()))]
#[specta::specta]
pub async fn unpark_branches(git_executor: State<'_, GitCommandExecutor>, params: UnparkBranchesParams) -> Result<ParkResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || unpark_branches_core(&git, &params)).await
}
