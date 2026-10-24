use crate::commands::branch_prefix::PrefixInputs;
use crate::operation::run_blocking;
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Deserialize;
use tauri::State;
use tracing::instrument;

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncInputsParams {
  pub repository_path: String,
}

/// The fingerprint of what a sync reads (`sync_inputs_fingerprint`) under the prefix Rust resolves for the repository:
/// the window syncs on focus only when it changed since the last sync, a changed prefix too
// `instrument` must come before `specta::specta`: specta parses the remaining attributes and rejects `%` in `fields`
#[tauri::command]
#[instrument(skip(git_executor, settings, prefixes), fields(repository_path = %params.repository_path))]
#[specta::specta]
pub async fn get_sync_inputs_fingerprint(
  git_executor: State<'_, GitCommandExecutor>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: SyncInputsParams,
) -> Result<String, ApiError> {
  let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    sync_core::sync_inputs::sync_inputs_fingerprint(&inputs.git, &params.repository_path, &branch_prefix)
  })
  .await
}
