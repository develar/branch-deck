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
pub async fn get_branch_prefix_from_git_config(git_executor: State<'_, GitCommandExecutor>, params: GetBranchPrefixParams) -> Result<String, String> {
  sync_core::branch_prefix::get_branch_prefix_from_git_config_sync(&git_executor, &params.repository_path).map_err(|e| e.to_string())
}
