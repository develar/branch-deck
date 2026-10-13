use git_executor::git_command_executor::GitCommandExecutor;
use model_tauri::blocking::run_blocking;
use model_tauri::command_error::CommandError;
use sync_core::unapply_branch::{UnapplyBranchParams, UnapplyBranchResult, unapply_branch_core};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn unapply_branch(git_executor: State<'_, GitCommandExecutor>, params: UnapplyBranchParams) -> Result<UnapplyBranchResult, CommandError> {
  let git = (*git_executor).clone();

  run_blocking(move || unapply_branch_core(&git, params)).await
}
