use crate::lock_wait::{WaitChannel, Waits, run_waiting};
use domain_types::error::ApiError;
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
pub async fn reword(git_executor: State<'_, GitCommandExecutor>, lock_waits: Waits<'_>, params: RewordRequest, wait: WaitChannel) -> Result<RewordResult, ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |lock_wait| reword_request_core(&git, &params, &RegroupOptions::waiting(lock_wait))).await
}
