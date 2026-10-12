use git_executor::git_command_executor::GitCommandExecutor;
use model_tauri::blocking::run_blocking;
use model_tauri::command_error::CommandError;
use sync_core::push::{PushBranchParams, push_branch as core_push_branch};
use sync_types::RemoteStatusUpdate;
use tauri::State;

/// Pushes a specific branch to the remote repository and returns updated remote status
#[tauri::command]
#[specta::specta]
pub async fn push_branch(git_executor: State<'_, GitCommandExecutor>, params: PushBranchParams) -> Result<RemoteStatusUpdate, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || core_push_branch(&git, &params)).await
}
