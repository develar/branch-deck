use crate::command_error::CommandError;
use crate::operation::run_blocking;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::regroup::RegroupOptions;
use history_rewrite::reword::{RewordRequest, RewordResult, reword_request_core};
use tauri::State;
use tracing::instrument;

/// Replaces the messages of commits ahead of the baseline in one regroup (`bd reword`); the commits after the oldest one
/// are recreated with the same trees, and the undo token restores HEAD.
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, rewords = params.rewords.len()))]
#[specta::specta]
pub async fn reword(git_executor: State<'_, GitCommandExecutor>, params: RewordRequest) -> Result<RewordResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || reword_request_core(&git, &params, &RegroupOptions::default())).await
}
