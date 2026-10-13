use git_executor::git_command_executor::GitCommandExecutor;
use model_tauri::blocking::run_blocking;
use model_tauri::command_error::CommandError;
use sync_core::reword_commit::{RewordCommitRequest, RewordCommitResult, reword_commit_core};
use tauri::State;
use tracing::instrument;

/// Replaces the message of a commit on the current branch; the commits after it are recreated with the same trees.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn reword_commit(git_executor: State<'_, GitCommandExecutor>, params: RewordCommitRequest) -> Result<RewordCommitResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || reword_commit_core(&git, &params)).await
}
