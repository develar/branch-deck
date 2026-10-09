use crate::progress::TauriProgressReporter;
use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use model_tauri::command_error::CommandError;
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

  // Get or create cached repository state (includes Git version validation)
  let issue_config = match cache.get_or_create(repository_path, &git_executor).await {
    Ok(state) => IssueConfigSource::Cached(state.issue_config.clone()),
    Err(e) => {
      error!("Failed to initialize repository cache: {e:#}");
      return Err(CommandError::from(e));
    }
  };

  // Use the branch-sync implementation with TauriProgressReporter adapter
  let progress_adapter = TauriProgressReporter::new(progress);

  let options = SyncOptions {
    issue_config,
    ..Default::default()
  };
  sync_core::sync::sync_branches(&git_executor, repository_path, branch_prefix, progress_adapter, options)
    .await
    .map_err(|e| {
      error!(error = ?e, "Branch synchronization failed");
      CommandError::from(e)
    })
}
