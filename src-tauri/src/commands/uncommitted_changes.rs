use crate::operation::run_blocking;
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::uncommitted_changes::{GetUncommittedChangesParams, UncommittedChangesResult, get_uncommitted_changes as core_get_uncommitted_changes};
use tauri::State;
use tracing::instrument;

/// Get uncommitted changes with only file metadata (no content or diffs)
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn get_uncommitted_changes(git_executor: State<'_, GitCommandExecutor>, params: GetUncommittedChangesParams) -> Result<UncommittedChangesResult, ApiError> {
  let git = (*git_executor).clone();
  run_blocking(move || core_get_uncommitted_changes(&git, params)).await
}
