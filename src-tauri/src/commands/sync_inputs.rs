use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Deserialize;
use tauri::State;
use tracing::instrument;

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncInputsParams {
  pub repository_path: String,
  pub branch_prefix: String,
}

/// The fingerprint of what a sync reads (`sync_inputs_fingerprint`): the window syncs on focus only when it changed
/// since the last sync
// `instrument` must come before `specta::specta`: specta parses the remaining attributes and rejects `%` in `fields`
#[tauri::command]
#[instrument(skip(git_executor), fields(repository_path = %params.repository_path))]
#[specta::specta]
pub async fn get_sync_inputs_fingerprint(git_executor: State<'_, GitCommandExecutor>, params: SyncInputsParams) -> Result<String, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || sync_core::sync_inputs::sync_inputs_fingerprint(&git, &params.repository_path, &params.branch_prefix)).await
}
