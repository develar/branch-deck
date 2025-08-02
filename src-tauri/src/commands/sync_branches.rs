use crate::progress::{SyncEvent, TauriProgressReporter};
use branch_sync::sync::sync_branches_core;
use git_ops::git_command::GitCommandExecutor;
use serde::Deserialize;
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
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, progress), fields(repository_path = %params.repository_path, branch_prefix = %params.branch_prefix))]
pub async fn sync_branches(git_executor: State<'_, GitCommandExecutor>, params: SyncBranchesParams, progress: Channel<SyncEvent>) -> Result<(), String> {
  let repository_path = &params.repository_path;
  let branch_prefix = &params.branch_prefix;

  // Use the branch-sync implementation with TauriProgressReporter adapter
  let progress_adapter = TauriProgressReporter::new(&progress);
  sync_branches_core(&git_executor, repository_path, branch_prefix, &progress_adapter).await.map_err(|e| {
    error!("Branch synchronization failed: {e}");
    format!("{e:?}")
  })
}
