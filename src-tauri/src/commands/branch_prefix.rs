use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GetBranchPrefixParams {
  pub repository_path: String,
}

#[tauri::command]
#[specta::specta]
pub async fn get_branch_prefix_from_git_config(git_executor: State<'_, GitCommandExecutor>, params: GetBranchPrefixParams) -> Result<String, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || sync_core::branch_prefix::get_branch_prefix_from_git_config_sync(&git, &params.repository_path)).await
}

/// Git, the git identity and a branch prefix suggestion, checked when the window opens and on every repository change
#[tauri::command]
#[specta::specta]
pub async fn check_git_setup(git_executor: State<'_, GitCommandExecutor>, params: GetBranchPrefixParams) -> Result<sync_core::git_setup::GitSetup, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || Ok::<_, anyhow::Error>(sync_core::git_setup::check_git_setup(&git, &params.repository_path))).await
}
