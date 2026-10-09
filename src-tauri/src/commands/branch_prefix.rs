use git_executor::git_command_executor::GitCommandExecutor;
use model_tauri::command_error::CommandError;
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
  tokio::task::spawn_blocking(move || sync_core::branch_prefix::get_branch_prefix_from_git_config_sync(&git, &params.repository_path))
    .await
    .map_err(|e| CommandError::new(format!("Task failed: {e}")))?
    .map_err(CommandError::from)
}
