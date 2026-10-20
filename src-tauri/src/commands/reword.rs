use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::regroup::RegroupOptions;
use history_rewrite::reword::{RewordRequest, RewordResult, reword_request_core};
use tauri::State;
use tracing::instrument;

/// Replaces the messages of commits ahead of the baseline in one regroup (`bd reword`); the commits after the oldest one
/// are recreated with the same trees, and the undo token restores HEAD.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn reword(git_executor: State<'_, GitCommandExecutor>, params: RewordRequest) -> Result<RewordResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || reword_request_core(&git, &params, &RegroupOptions::default())).await
}
