use crate::operation::{operation, run_blocking, tauri_error};
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use domain_types::settings::GitBranchPrefix;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use sync_core::repository_config::{RepositoryConfig, RepositoryConfigChange, RepositoryConfigKey, WriteScope, read_repository_config, set_config};
use tauri::{AppHandle, Emitter, State};
use tracing::instrument;

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
  /// The local config when omitted
  #[serde(default)]
  #[specta(optional)]
  pub scope: WriteScope,
}

/// The `branchdeck.*` settings of the repository with their scopes, and the detected GitHub repository
#[tauri::command]
#[instrument(skip(git_executor))]
#[specta::specta]
pub async fn get_repository_config(git_executor: State<'_, GitCommandExecutor>, params: GetRepositoryConfigParams) -> Result<RepositoryConfig, ApiError> {
  let git = (*git_executor).clone();
  run_blocking(move || read_repository_config(&git, &params.repository_path)).await
}

/// Writes one key to the local config of the repository, or to the global config of the user, and answers the settings
/// read again. A write of the branch prefix keeps the git prefix the read found for the repository's later commands
/// (`BranchPrefixes`); a global one forgets the prefix of every other repository, which may take it
#[tauri::command]
#[instrument(skip(app, git_executor, settings, prefixes))]
#[specta::specta]
pub async fn set_repository_config(
  app: AppHandle,
  git_executor: State<'_, GitCommandExecutor>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: SetRepositoryConfigParams,
) -> Result<RepositoryConfig, ApiError> {
  operation(async move {
    let (settings, prefixes) = (settings.app(), (*prefixes).clone());
    let git = (*git_executor).clone();
    let SetRepositoryConfigParams { repository_path, change, scope } = params;
    let key = change.key();
    let path = repository_path.clone();
    let config = run_blocking(move || set_config(&git, &path, &change, scope)).await?;
    if key == RepositoryConfigKey::BranchPrefix {
      if scope == WriteScope::Global {
        prefixes.clear();
      }
      let git_prefix = GitBranchPrefix {
        prefix: config.branch_prefix.value.clone().unwrap_or_default(),
        repository_scoped: config.branch_prefix.scope.is_some_and(sync_core::repository_config::ConfigScope::is_repository_scoped),
      };
      prefixes.refresh(&settings, &repository_path, git_prefix);
    }
    app
      .emit_to("main", REPOSITORY_CONFIG_CHANGED_EVENT, RepositoryConfigChanged { repository_path, key })
      .map_err(tauri_error)?;
    Ok(config)
  })
  .await
}
