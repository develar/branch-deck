use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::reword_commit::{RewordCommitsRequest, RewordCommitsResult, reword_commits_core};
use tauri::State;
use tracing::instrument;

/// Replaces the messages of commits on the current branch; the commits after the oldest one are recreated with the
/// same trees.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn reword_commits(git_executor: State<'_, GitCommandExecutor>, params: RewordCommitsRequest) -> Result<RewordCommitsResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || reword_commits_core(&git, &params)).await
}
