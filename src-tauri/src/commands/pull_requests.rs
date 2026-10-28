use crate::api::Settings;
use crate::commands::github::GitHubState;
use crate::commands::youtrack::YouTrackState;
use app_api::context::PrefixInputs;
use app_api::operation::{operation, run_blocking};
use app_api::repository_state::RepositoryStateCache;
use app_settings::branch_prefixes::BranchPrefixes;
use domain_types::error::ApiError;
use domain_types::pull_request::{GitHubUser, NewPullRequest};
use git_executor::git_command_executor::GitCommandExecutor;
use remote_facts::pull_request_create::{GetNewPullRequestParams, SearchGitHubUsersParams, new_pull_request, read_viewer_login, search_github_users as core_search_github_users};
use remote_facts::pull_requests::{NOT_ON_GITHUB_MESSAGE, failure_message};
use remote_facts::stack_pull_request::{GetNewStackPullRequestParams, read_new_stack_pull_request};
use sync_core::github_repository::{GitHubRepository, parse_known_github_repository};
use tauri::State;
use tracing::instrument;

/// The text a new pull request of a virtual branch starts with (`bd describe`, else the commits it will show, with the
/// summaries of their issues from the YouTrack the app is signed in to), and the user of the token it is created with
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, head = %params.head, base = %params.base, stack_ancestors = params.stack_ancestors.len()))]
#[specta::specta]
pub async fn get_new_pull_request(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  youtrack: State<'_, YouTrackState>,
  params: GetNewPullRequestParams,
) -> Result<NewPullRequest, ApiError> {
  operation(async move {
    let pull_request = new_pull_request(&git_executor, Some(&youtrack.service), "origin", &params)
      .await
      .map_err(|e| ApiError::internal(e.to_string()))?;
    with_viewer(&git_executor, &repository_state, &github, &params.repository_path, pull_request).await
  })
  .await
}

/// The text of a new pull request with the user of the token, who is its assignee; none when GitHub doesn't answer
async fn with_viewer(
  git_executor: &State<'_, GitCommandExecutor>,
  repository_state: &State<'_, RepositoryStateCache>,
  github: &State<'_, GitHubState>,
  repository_path: &str,
  mut pull_request: NewPullRequest,
) -> Result<NewPullRequest, ApiError> {
  if let Some(repository) = github_repository(git_executor, repository_state, repository_path, None).await? {
    pull_request.viewer_login = read_viewer_login(&github.service, repository_path, &repository).await.ok();
  }
  Ok(pull_request)
}

/// The text the "Create pull request" dialog of the whole stack starts with (`read_new_stack_pull_request`): the
/// descriptions of the stack branch and its members, else the subjects of their commits; and the user of the token,
/// who is the assignee
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, root = %params.root, members = params.members.len()))]
#[specta::specta]
pub async fn get_new_stack_pull_request(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  settings: Settings<'_>,
  prefixes: State<'_, BranchPrefixes>,
  params: GetNewStackPullRequestParams,
) -> Result<NewPullRequest, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let request = params.clone();
    let pull_request = run_blocking(move || {
      let branch_prefix = inputs.branch_prefix(&request.repository_path)?;
      read_new_stack_pull_request(&inputs.git, &branch_prefix, &request)
    })
    .await?;
    with_viewer(&git_executor, &repository_state, &github, &params.repository_path, pull_request).await
  })
  .await
}

/// The users of the repository a review can be asked from whose login or name matches the query
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github))]
#[specta::specta]
pub async fn search_github_users(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: SearchGitHubUsersParams,
) -> Result<Vec<GitHubUser>, ApiError> {
  operation(async move {
    let repository = required_github_repository(&git_executor, &repository_state, &params.repository_path, None).await?;
    core_search_github_users(&github.service, &params.repository_path, &repository, &params.query)
      .await
      .map_err(|failure| ApiError::internal(failure_message(&failure)))
  })
  .await
}

/// The GitHub repository the pull requests are listed in: a known `owner/name` (the one the review requests were read
/// from), else the resolved one, kept while the repository's config stays as it is (`RepositoryStateCache`)
pub async fn github_repository(
  git_executor: &GitCommandExecutor,
  repository_state: &RepositoryStateCache,
  repository_path: &str,
  known: Option<&str>,
) -> Result<Option<GitHubRepository>, ApiError> {
  if let Some(repository) = parse_known_github_repository(known) {
    return Ok(Some(repository));
  }
  let (git, state, path) = (git_executor.clone(), repository_state.clone(), repository_path.to_string());
  run_blocking(move || state.github_repository(&git, &path)).await
}

async fn required_github_repository(
  git_executor: &GitCommandExecutor,
  repository_state: &RepositoryStateCache,
  repository_path: &str,
  known: Option<&str>,
) -> Result<GitHubRepository, ApiError> {
  github_repository(git_executor, repository_state, repository_path, known)
    .await?
    .ok_or_else(|| ApiError::internal(NOT_ON_GITHUB_MESSAGE))
}
