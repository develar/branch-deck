use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use crate::commands::github::GitHubState;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::pull_request_writes::{
  ClosePullRequestParams, GetPullRequestHeadParams, RetargetPullRequestParams, SubmitPullRequestReviewParams, close_pull_request as core_close_pull_request,
  read_pull_request_head, retarget_pull_request as core_retarget_pull_request, submit_pull_request_review as core_submit_pull_request_review,
};
use sync_core::pull_requests::{GetPullRequestsParams, GitHubRepository, known_or_resolved_github_repository, list_pull_requests};
use sync_types::pull_request::{PullRequestAvailability, PullRequestsResult};
use tauri::State;

/// The GitHub pull requests of the virtual branches, one GraphQL search; when GitHub can't answer, the availability says
/// why
#[tauri::command]
#[specta::specta]
pub async fn get_pull_requests(
  git_executor: State<'_, GitCommandExecutor>,
  github: State<'_, GitHubState>,
  params: GetPullRequestsParams,
) -> Result<PullRequestsResult, CommandError> {
  let Some(repository) = github_repository(&git_executor, &params.repository_path, None).await? else {
    return Ok(PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub));
  };
  Ok(list_pull_requests(&github.service, &params.repository_path, &repository, &params.branch_prefix).await)
}

/// Changes the base of a pull request of the virtual branches: the push of a stacked branch keeps its pull request on
/// the branch it is built on
#[tauri::command]
#[specta::specta]
pub async fn retarget_pull_request(git_executor: State<'_, GitCommandExecutor>, github: State<'_, GitHubState>, params: RetargetPullRequestParams) -> Result<(), CommandError> {
  let repository = required_github_repository(&git_executor, &params.repository_path, None).await?;
  core_retarget_pull_request(&github.service, &params.repository_path, &repository, params.pull_request_number, &params.base)
    .await
    .map_err(|e| CommandError::new(e.to_string()))
}

/// Closes a pull request of the virtual branches with a comment: an ancestor a Safe Push of a stacked branch landed
/// with it
#[tauri::command]
#[specta::specta]
pub async fn close_pull_request(git_executor: State<'_, GitCommandExecutor>, github: State<'_, GitHubState>, params: ClosePullRequestParams) -> Result<(), CommandError> {
  let repository = required_github_repository(&git_executor, &params.repository_path, None).await?;
  core_close_pull_request(&github.service, &params.repository_path, &repository, params.pull_request_number, &params.comment)
    .await
    .map_err(|e| CommandError::new(e.to_string()))
}

/// Approves, or requests changes on, a pull request that waits for the user's review, on the commit the user reviewed
#[tauri::command]
#[specta::specta]
pub async fn submit_pull_request_review(
  git_executor: State<'_, GitCommandExecutor>,
  github: State<'_, GitHubState>,
  params: SubmitPullRequestReviewParams,
) -> Result<(), CommandError> {
  let repository = required_github_repository(&git_executor, &params.repository_path, params.github_repository.as_deref()).await?;
  core_submit_pull_request_review(
    &github.service,
    &params.repository_path,
    &repository,
    params.pull_request_number,
    params.event,
    &params.commit_id,
    params.body.as_deref(),
  )
  .await
  .map_err(|e| CommandError::new(e.to_string()))
}

/// The commit a pull request shows now on GitHub: a Safe Push of a reviewed pull request merges only the approved one
#[tauri::command]
#[specta::specta]
pub async fn get_pull_request_head(git_executor: State<'_, GitCommandExecutor>, github: State<'_, GitHubState>, params: GetPullRequestHeadParams) -> Result<String, CommandError> {
  let repository = required_github_repository(&git_executor, &params.repository_path, params.github_repository.as_deref()).await?;
  read_pull_request_head(&github.service, &params.repository_path, &repository, params.pull_request_number)
    .await
    .map_err(|e| CommandError::new(e.to_string()))
}

/// The GitHub repository the pull requests are listed in: a known `owner/name` (the one the review requests were read
/// from), else `resolve_github_repository` (up to 3 git processes)
pub async fn github_repository(git_executor: &GitCommandExecutor, repository_path: &str, known: Option<&str>) -> Result<Option<GitHubRepository>, CommandError> {
  let git = git_executor.clone();
  let path = repository_path.to_string();
  let known = known.map(str::to_string);
  run_blocking(move || known_or_resolved_github_repository(&git, &path, known.as_deref())).await
}

async fn required_github_repository(git_executor: &GitCommandExecutor, repository_path: &str, known: Option<&str>) -> Result<GitHubRepository, CommandError> {
  github_repository(git_executor, repository_path, known)
    .await?
    .ok_or_else(|| CommandError::new("The pull requests of the branches are not on GitHub"))
}
