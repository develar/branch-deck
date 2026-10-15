use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use std::path::PathBuf;
use sync_core::pull_request_writes::{
  ClosePullRequestParams, RetargetPullRequestParams, close_pull_request as core_close_pull_request, retarget_pull_request as core_retarget_pull_request,
};
use sync_core::pull_requests::{GH_TIMEOUT, GetPullRequestsParams, GitHubRepository, find_gh, get_pull_requests as core_get_pull_requests, resolve_github_repository};
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

/// Changes the base of a pull request of the virtual branches (`gh pr edit --base`): the push of a stacked branch keeps
/// its pull request on the branch it is built on
#[tauri::command]
#[specta::specta]
pub async fn retarget_pull_request(git_executor: State<'_, GitCommandExecutor>, params: RetargetPullRequestParams) -> Result<(), CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || {
    let (gh, repository) = gh_and_repository(&git, &params.repository_path)?;
    core_retarget_pull_request(&gh, &params.repository_path, &repository, params.pull_request_number, &params.base, GH_TIMEOUT).map_err(CommandError::new)
  })
  .await
}

/// Closes a pull request of the virtual branches with a comment (`gh pr close`): an ancestor a Safe Push of a stacked
/// branch landed with it
#[tauri::command]
#[specta::specta]
pub async fn close_pull_request(git_executor: State<'_, GitCommandExecutor>, params: ClosePullRequestParams) -> Result<(), CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || {
    let (gh, repository) = gh_and_repository(&git, &params.repository_path)?;
    core_close_pull_request(&gh, &params.repository_path, &repository, params.pull_request_number, &params.comment, GH_TIMEOUT).map_err(CommandError::new)
  })
  .await
}

/// `gh` and the GitHub repository the pull requests are listed in (`resolve_github_repository`)
fn gh_and_repository(git: &GitCommandExecutor, repository_path: &str) -> Result<(PathBuf, GitHubRepository), CommandError> {
  let repository = resolve_github_repository(git, repository_path)
    .map_err(CommandError::from)?
    .ok_or_else(|| CommandError::new("The pull requests of the branches are not on GitHub"))?;
  let gh = find_gh().ok_or_else(|| CommandError::new("The gh CLI is not installed"))?;
  Ok((gh, repository))
}
