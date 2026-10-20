use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::parked::{ParkBranchParams, ParkResult, UnparkBranchParams, park_branch as park_branch_core, unpark_branch as unpark_branch_core};
use tauri::State;
use tracing::instrument;

/// Parks a branch with a note, or changes the note of a parked one: kept local, never pushed, until unparked. No
/// rewrite: the store `refs/branch-deck/parked`, under the repository lock.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn park_branch(git_executor: State<'_, GitCommandExecutor>, params: ParkBranchParams) -> Result<ParkResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || park_branch_core(&git, &params)).await
}

/// Unparks a branch: it can be pushed and landed again.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn unpark_branch(git_executor: State<'_, GitCommandExecutor>, params: UnparkBranchParams) -> Result<ParkResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || unpark_branch_core(&git, &params)).await
}
