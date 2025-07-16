use crate::progress::{SyncEvent, TauriProgressReporter};
use branch_sync::sync_branches_core;
use git_ops::git_command::GitCommandExecutor;
use tauri::State;
use tauri::ipc::Channel;
use tracing::{error, info, instrument};

/// Synchronizes branches by grouping commits by prefix and creating/updating branches
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, progress))]
pub async fn sync_branches(git_executor: State<'_, GitCommandExecutor>, repository_path: &str, branch_prefix: &str, progress: Channel<SyncEvent>) -> Result<(), String> {
  info!("Starting branch synchronization for repository: {repository_path}, prefix: {branch_prefix}");
  do_sync_branches(git_executor, repository_path, branch_prefix, progress).await.map_err(|e| {
    error!("Branch synchronization failed: {e}");
    format!("{e:?}")
  })
}

async fn do_sync_branches(git_executor: State<'_, GitCommandExecutor>, repository_path: &str, branch_prefix: &str, progress: Channel<SyncEvent>) -> anyhow::Result<()> {
  // Use the branch-sync implementation with TauriProgressReporter adapter
  let progress_adapter = TauriProgressReporter::new(&progress);
  sync_branches_core(&git_executor, repository_path, branch_prefix, &progress_adapter).await
}
