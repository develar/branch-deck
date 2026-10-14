use git_executor::git_command_executor::GitCommandExecutor;
use model_tauri::blocking::run_blocking;
use model_tauri::command_error::CommandError;
use sync_core::pull_requests::{GetPullRequestsParams, get_pull_requests as core_get_pull_requests};
use sync_types::pull_request::PullRequestsResult;
use tauri::State;

/// The GitHub pull requests of the virtual branches, read through the `gh` CLI; when `gh` can't answer, the
/// availability says why
#[tauri::command]
#[specta::specta]
pub async fn get_pull_requests(git_executor: State<'_, GitCommandExecutor>, params: GetPullRequestsParams) -> Result<PullRequestsResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || core_get_pull_requests(&git, &params)).await
}
