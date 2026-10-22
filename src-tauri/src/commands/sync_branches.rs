use crate::command_error::CommandError;
use crate::operation::{operation, run_blocking};
use crate::progress::TauriProgressReporter;
use crate::repository_state::RepositoryStateCache;
use domain_types::SyncEvent;
use git_executor::git_command_executor::GitCommandExecutor;
use repository_view::branch_table::BranchTable;
use repository_view::views::RepositoryViews;
use serde::Deserialize;
use sync_core::remote_status::RemoteStatusCollector;
use sync_core::sync::{IssueConfigSource, SyncOptions};
use tauri::State;
use tauri::ipc::Channel;
use tracing::{error, instrument};

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncBranchesParams {
  pub repository_path: String,
  pub branch_prefix: String,
}

/// Synchronizes branches by grouping commits by prefix and creating/updating branches
// `instrument` must come before `specta::specta`: specta parses the remaining attributes and rejects `%` in `fields`
#[tauri::command]
#[instrument(skip(git_executor, cache, views, progress), fields(repository_path = %params.repository_path, branch_prefix = %params.branch_prefix))]
#[specta::specta]
pub async fn sync_branches(
  git_executor: State<'_, GitCommandExecutor>,
  cache: State<'_, RepositoryStateCache>,
  views: State<'_, RepositoryViews>,
  params: SyncBranchesParams,
  progress: Channel<SyncEvent>,
) -> Result<(), CommandError> {
  operation(async move {
    let repository_path = &params.repository_path;
    let branch_prefix = &params.branch_prefix;

    // Git discovery runs a process and the config lookup reads the file system: both block.
    // Discovery rejects a too old git, and runs once per process; it fails here before any progress is reported.
    // the object cache, shared with `bd`, holds the conflict views of the sync
    let (issue_config, object_cache) = {
      let git = (*git_executor).clone();
      let cache = (*cache).clone();
      let repository_path = repository_path.clone();
      run_blocking(move || {
        git.get_info()?;
        Ok::<_, anyhow::Error>((cache.issue_config(&repository_path), cache.object_cache(&git, &repository_path)))
      })
      .await
      .inspect_err(|e| error!("Failed to prepare sync: {e}"))?
    };
    let issue_config = IssueConfigSource::Cached((*issue_config).clone());

    // the page takes the events; the remote statuses among them are kept for the table
    let progress_adapter = RemoteStatusCollector::new(TauriProgressReporter::new(progress));

    let options = SyncOptions {
      issue_config,
      object_cache,
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
        CommandError::from(e)
      })
  })
  .await
}
