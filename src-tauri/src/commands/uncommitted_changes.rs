use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::conflict_analysis::FileDiff;
use sync_core::uncommitted_changes::{
  GetFileContentForDiffParams, GetUncommittedChangesParams, UncommittedChangesResult, get_file_content_for_diff as core_get_file_content_for_diff,
  get_uncommitted_changes as core_get_uncommitted_changes,
};
use tauri::State;
use tracing::instrument;

/// Get uncommitted changes with only file metadata (no content or diffs)
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn get_uncommitted_changes(git_executor: State<'_, GitCommandExecutor>, params: GetUncommittedChangesParams) -> Result<UncommittedChangesResult, String> {
  let git = (*git_executor).clone();

  tokio::task::spawn_blocking(move || core_get_uncommitted_changes(&git, params))
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

/// Get file content for diff display when user expands a file in the UI
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn get_file_content_for_diff(git_executor: State<'_, GitCommandExecutor>, params: GetFileContentForDiffParams) -> Result<FileDiff, String> {
  let git = (*git_executor).clone();

  tokio::task::spawn_blocking(move || core_get_file_content_for_diff(&git, params))
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}
