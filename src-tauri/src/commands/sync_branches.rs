use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use crate::progress::TauriProgressReporter;
use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Deserialize;
use sync_core::sync::{IssueConfigSource, SyncOptions};
use sync_types::SyncEvent;
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
#[instrument(skip(git_executor, cache, progress), fields(repository_path = %params.repository_path, branch_prefix = %params.branch_prefix))]
#[specta::specta]
pub async fn sync_branches(
  git_executor: State<'_, GitCommandExecutor>,
  cache: State<'_, RepositoryStateCache>,
  params: SyncBranchesParams,
  progress: Channel<SyncEvent>,
) -> Result<(), CommandError> {
  let repository_path = &params.repository_path;
  let branch_prefix = &params.branch_prefix;

  // Git discovery runs a process and the config lookup reads the file system: both block.
  // Discovery rejects a too old git, and runs once per process; it fails here before any progress is reported.
  let issue_config = {
    let git = (*git_executor).clone();
    let cache = (*cache).clone();
    let repository_path = repository_path.clone();
    run_blocking(move || {
      git.get_info()?;
      Ok::<_, anyhow::Error>(cache.issue_config(&repository_path))
    })
    .await
    .inspect_err(|e| error!("Failed to prepare sync: {e}"))?
  };
  let issue_config = IssueConfigSource::Cached((*issue_config).clone());

  // Use the branch-sync implementation with TauriProgressReporter adapter
  let progress_adapter = TauriProgressReporter::new(progress);

  let options = SyncOptions {
    issue_config,
    ..Default::default()
  };
  // the page takes the events; the report is for `bd`
  sync_core::sync::sync_branches(&git_executor, repository_path, branch_prefix, progress_adapter, options)
    .await
    .map(|_report| ())
    .map_err(|e| {
      error!(error = ?e, "Branch synchronization failed");
      CommandError::from(e)
    })
}
