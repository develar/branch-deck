use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use sync_core::repository_config::{RepositoryConfig, RepositoryConfigChange, RepositoryConfigKey, read_repository_config, set_repository_config as write_repository_config};
use tauri::{AppHandle, Emitter, State};

/// Sent to the main window after a write of Settings → Repository: the main window syncs, or reloads the branch prefix
pub const REPOSITORY_CONFIG_CHANGED_EVENT: &str = "repository_config_changed";

/// The payload of `repository_config_changed`
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryConfigChanged {
  pub repository_path: String,
  pub key: RepositoryConfigKey,
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GetRepositoryConfigParams {
  pub repository_path: String,
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SetRepositoryConfigParams {
  pub repository_path: String,
  pub change: RepositoryConfigChange,
}

/// The `branchdeck.*` settings of the repository with their scopes, and the detected GitHub repository
#[tauri::command]
#[specta::specta]
pub async fn get_repository_config(git_executor: State<'_, GitCommandExecutor>, params: GetRepositoryConfigParams) -> Result<RepositoryConfig, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || read_repository_config(&git, &params.repository_path)).await
}

/// Writes one key to the local config of the repository and answers the settings read again
#[tauri::command]
#[specta::specta]
pub async fn set_repository_config(app: AppHandle, git_executor: State<'_, GitCommandExecutor>, params: SetRepositoryConfigParams) -> Result<RepositoryConfig, CommandError> {
  let git = (*git_executor).clone();
  let SetRepositoryConfigParams { repository_path, change } = params;
  let key = change.key();
  let path = repository_path.clone();
  let config = run_blocking(move || write_repository_config(&git, &path, &change)).await?;
  app.emit_to("main", REPOSITORY_CONFIG_CHANGED_EVENT, RepositoryConfigChanged { repository_path, key })?;
  Ok(config)
}
