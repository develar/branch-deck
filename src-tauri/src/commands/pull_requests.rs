use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use crate::commands::github::GitHubState;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::pull_request_create::{
  CreatePullRequestParams, GetNewPullRequestParams, SearchGitHubUsersParams, create_pull_request as core_create_pull_request, read_new_pull_request, read_viewer_login,
  search_github_users as core_search_github_users,
};
use sync_core::pull_request_writes::{
  ClosePullRequestParams, GetPullRequestHeadParams, RetargetPullRequestParams, SubmitPullRequestReviewParams, close_pull_request as core_close_pull_request,
  read_pull_request_head, retarget_pull_request as core_retarget_pull_request, submit_pull_request_review as core_submit_pull_request_review,
};
use sync_core::pull_requests::{GetPullRequestsParams, GitHubRepository, failure_message, known_or_resolved_github_repository, list_pull_requests};
use sync_types::pull_request::{CreatedPullRequest, GitHubUser, NewPullRequest, PullRequestAvailability, PullRequestsResult};
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

/// The text a new pull request of a pushed virtual branch starts with (`bd describe`, else its pushed commits), the
/// commits the push lacks, and the user of the token it is created with
#[tauri::command]
#[specta::specta]
pub async fn get_new_pull_request(
  git_executor: State<'_, GitCommandExecutor>,
  github: State<'_, GitHubState>,
  params: GetNewPullRequestParams,
) -> Result<NewPullRequest, CommandError> {
  let git = git_executor.inner().clone();
  let (path, head, base) = (params.repository_path.clone(), params.head.clone(), params.base.clone());
  let mut pull_request = run_blocking(move || read_new_pull_request(&git, &path, "origin", &head, &base).map_err(|e| CommandError::new(e.to_string()))).await?;
  if let Some(repository) = github_repository(&git_executor, &params.repository_path, None).await? {
    pull_request.viewer_login = read_viewer_login(&github.service, &params.repository_path, &repository).await.ok();
  }
  Ok(pull_request)
}

/// Opens the pull request of a pushed virtual branch, assigned to the user and with the reviewers asked; one that
/// already exists is found instead
#[tauri::command]
#[specta::specta]
pub async fn create_pull_request(
  git_executor: State<'_, GitCommandExecutor>,
  github: State<'_, GitHubState>,
  params: CreatePullRequestParams,
) -> Result<CreatedPullRequest, CommandError> {
  let repository = required_github_repository(&git_executor, &params.repository_path, None).await?;
  core_create_pull_request(&github.service, &repository, &params)
    .await
    .map_err(|e| CommandError::new(e.to_string()))
}

/// The users of the repository a review can be asked from whose login or name matches the query
#[tauri::command]
#[specta::specta]
pub async fn search_github_users(
  git_executor: State<'_, GitCommandExecutor>,
  github: State<'_, GitHubState>,
  params: SearchGitHubUsersParams,
) -> Result<Vec<GitHubUser>, CommandError> {
  let repository = required_github_repository(&git_executor, &params.repository_path, None).await?;
  core_search_github_users(&github.service, &params.repository_path, &repository, &params.query)
    .await
    .map_err(|failure| CommandError::new(failure_message(&failure)))
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
