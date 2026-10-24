use crate::commands::branch_prefix::PrefixInputs;
use crate::operation::{operation, run_blocking};
use crate::progress::TauriProgressReporter;
use crate::repository_state::RepositoryStateCache;
use app_api::api_error::{api_error, no_branch_prefix};
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::SyncEvent;
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use repository_model::branch_table::BranchTable;
use repository_model::views::RepositoryViews;
use serde::Deserialize;
use sync_core::config_snapshot::ConfigSnapshot;
use sync_core::remote_status::RemoteStatusCollector;
use sync_core::sync::{IssueConfigSource, SyncOptions};
use tauri::State;
use tauri::ipc::Channel;
use tracing::{error, instrument};

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncBranchesParams {
  pub repository_path: String,
}

/// Synchronizes branches by grouping commits by prefix and creating/updating branches, named with the prefix Rust
/// resolves from the git config the sync reads (one `ConfigSnapshot`, which also keeps the prefix for the commands
/// after it) and the app's prefix of the settings; a repository without a prefix is refused before any event
// `instrument` must come before `specta::specta`: specta parses the remaining attributes and rejects `%` in `fields`
#[tauri::command]
#[instrument(skip(git_executor, cache, views, settings, prefixes, progress), fields(repository_path = %params.repository_path, branch_prefix))]
#[specta::specta]
pub async fn sync_branches(
  git_executor: State<'_, GitCommandExecutor>,
  cache: State<'_, RepositoryStateCache>,
  views: State<'_, RepositoryViews>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: SyncBranchesParams,
  progress: Channel<SyncEvent>,
) -> Result<(), ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let repository_path = &params.repository_path;

    // Git discovery runs a process and the config lookup reads the file system: both block.
    // Discovery rejects a too old git, and runs once per process; it fails here before any progress is reported.
    // the object cache, shared with `bd`, holds the conflict views of the sync; the git config, read once, names the
    // prefix and goes on to the sync
    let (issue_config, object_cache, config, branch_prefix) = {
      let cache = (*cache).clone();
      let repository_path = repository_path.clone();
      run_blocking(move || {
        let git = &inputs.git;
        git.get_info()?;
        let config = ConfigSnapshot::read(git, &repository_path)?;
        let branch_prefix = inputs.refreshed(&repository_path, config.branch_prefix()).prefix;
        if branch_prefix.is_empty() {
          return Err(anyhow::Error::new(no_branch_prefix()));
        }
        Ok::<_, anyhow::Error>((cache.issue_config(&repository_path), cache.object_cache(git, &repository_path), config, branch_prefix))
      })
      .await
      .inspect_err(|e| error!("Failed to prepare sync: {e}"))?
    };
    tracing::Span::current().record("branch_prefix", branch_prefix.as_str());
    let branch_prefix = &branch_prefix;
    let issue_config = IssueConfigSource::Cached((*issue_config).clone());

    // the page takes the events; the remote statuses among them are kept for the table
    let progress_adapter = RemoteStatusCollector::new(TauriProgressReporter::new(progress));

    let options = SyncOptions {
      issue_config,
      object_cache,
      config: Some(config),
      ..Default::default()
    };
    // the table the sync left, with the remote statuses it emitted, answers its repository views
    sync_core::sync::sync_branches(&git_executor, repository_path, branch_prefix, progress_adapter.clone(), options)
      .await
      .map(|report| {
        let table = BranchTable::from_report(branch_prefix, &report, &progress_adapter.take_statuses());
        views.keep(repository_path, branch_prefix, table);
      })
      .map_err(|e| {
        error!(error = ?e, "Branch synchronization failed");
        api_error(e)
      })
  })
  .await
}
