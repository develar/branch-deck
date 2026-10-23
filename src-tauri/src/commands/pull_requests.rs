use crate::command_error::CommandError;
use crate::commands::github::GitHubState;
use crate::commands::youtrack::YouTrackState;
use crate::operation::{operation, run_blocking};
use crate::repository_state::RepositoryStateCache;
use domain_types::pull_request::{CreatedPullRequest, GitHubUser, NewPullRequest, PullRequestAvailability, PullRequestsResult, RequestedReviewers};
use git_executor::git_command_executor::GitCommandExecutor;
use remote_facts::branch_rules::{BranchRules, GetBranchRulesParams, read_branch_rules};
use remote_facts::pull_request_create::{
  CreatePullRequestParams, GetNewPullRequestParams, MIRROR_WAIT, SearchGitHubUsersParams, WaitForGitHubBranchParams, create_pull_request as core_create_pull_request,
  new_pull_request, read_viewer_login, search_github_users as core_search_github_users, wait_for_branch,
};
use remote_facts::pull_request_writes::{
  ClosePullRequestParams, GetPullRequestHeadParams, RequestPullRequestReviewersParams, RetargetPullRequestParams, SubmitPullRequestReviewParams,
  close_pull_request as core_close_pull_request, read_pull_request_head, request_pull_request_reviewers as core_request_pull_request_reviewers,
  retarget_pull_request as core_retarget_pull_request, submit_pull_request_review as core_submit_pull_request_review,
};
use remote_facts::pull_requests::{GetPullRequestsParams, NOT_ON_GITHUB_MESSAGE, failure_message, list_pull_requests};
use remote_facts::stack_pull_request::{GetNewStackPullRequestParams, read_new_stack_pull_request};
use sync_core::github_repository::{GitHubRepository, parse_known_github_repository};
use tauri::State;
use tracing::instrument;

/// The GitHub pull requests of the virtual branches, one GraphQL search; when GitHub can't answer, the availability says
/// why
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github))]
#[specta::specta]
pub async fn get_pull_requests(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: GetPullRequestsParams,
) -> Result<PullRequestsResult, CommandError> {
  operation(async move {
    let Some(repository) = github_repository(&git_executor, &repository_state, &params.repository_path, None).await? else {
      return Ok(PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub));
    };
    Ok(list_pull_requests(&github.service, &params.repository_path, &repository, &params.branch_prefix).await)
  })
  .await
}

/// Changes the base of a pull request of the virtual branches: the push of a stacked branch keeps its pull request on
/// the branch it is built on
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github))]
#[specta::specta]
pub async fn retarget_pull_request(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: RetargetPullRequestParams,
) -> Result<(), CommandError> {
  operation(async move {
    let repository = required_github_repository(&git_executor, &repository_state, &params.repository_path, None).await?;
    core_retarget_pull_request(&github.service, &params.repository_path, &repository, params.pull_request_number, &params.base)
      .await
      .map_err(|e| CommandError::new(e.to_string()))
  })
  .await
}

/// Closes a pull request of the virtual branches with a comment: an ancestor a Safe Push of a stacked branch landed
/// with it
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github))]
#[specta::specta]
pub async fn close_pull_request(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: ClosePullRequestParams,
) -> Result<(), CommandError> {
  operation(async move {
    let repository = required_github_repository(&git_executor, &repository_state, &params.repository_path, None).await?;
    core_close_pull_request(&github.service, &params.repository_path, &repository, params.pull_request_number, &params.comment)
      .await
      .map_err(|e| CommandError::new(e.to_string()))
  })
  .await
}

/// Approves, or requests changes on, a pull request that waits for the user's review, on the commit the user reviewed
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github, params), fields(pull_request = params.pull_request_number, event = ?params.event))]
#[specta::specta]
pub async fn submit_pull_request_review(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: SubmitPullRequestReviewParams,
) -> Result<(), CommandError> {
  operation(async move {
    let repository = required_github_repository(&git_executor, &repository_state, &params.repository_path, params.github_repository.as_deref()).await?;
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
  })
  .await
}

/// Asks users to review an open pull request of the virtual branches (Request Review…): whom GitHub asked, and whom it
/// refused with its reason
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, pull_request_number = params.pull_request_number, reviewers = params.reviewers.len()))]
#[specta::specta]
pub async fn request_pull_request_reviewers(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: RequestPullRequestReviewersParams,
) -> Result<RequestedReviewers, CommandError> {
  operation(async move {
    let repository = required_github_repository(&git_executor, &repository_state, &params.repository_path, None).await?;
    core_request_pull_request_reviewers(&github.service, &params.repository_path, &repository, params.pull_request_number, &params.reviewers)
      .await
      .map_err(|e| CommandError::new(e.to_string()))
  })
  .await
}

/// The commit a pull request shows now on GitHub: a Safe Push of a reviewed pull request merges only the approved one
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github))]
#[specta::specta]
pub async fn get_pull_request_head(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: GetPullRequestHeadParams,
) -> Result<String, CommandError> {
  operation(async move {
    let repository = required_github_repository(&git_executor, &repository_state, &params.repository_path, params.github_repository.as_deref()).await?;
    read_pull_request_head(&github.service, &params.repository_path, &repository, params.pull_request_number)
      .await
      .map_err(|e| CommandError::new(e.to_string()))
  })
  .await
}

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
) -> Result<NewPullRequest, CommandError> {
  operation(async move {
    let pull_request = new_pull_request(&git_executor, Some(&youtrack.service), "origin", &params)
      .await
      .map_err(|e| CommandError::new(e.to_string()))?;
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
) -> Result<NewPullRequest, CommandError> {
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
  params: GetNewStackPullRequestParams,
) -> Result<NewPullRequest, CommandError> {
  operation(async move {
    let git = git_executor.inner().clone();
    let request = params.clone();
    let pull_request = run_blocking(move || read_new_stack_pull_request(&git, &request).map_err(|e| CommandError::new(e.to_string()))).await?;
    with_viewer(&git_executor, &repository_state, &github, &params.repository_path, pull_request).await
  })
  .await
}

/// Opens the pull request of a pushed virtual branch, assigned to the user and with the reviewers asked; one that
/// already exists is found instead
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github, params), fields(branch = %params.branch_name, head = %params.head, base = %params.base))]
#[specta::specta]
pub async fn create_pull_request(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: CreatePullRequestParams,
) -> Result<CreatedPullRequest, CommandError> {
  operation(async move {
    let repository = required_github_repository(&git_executor, &repository_state, &params.repository_path, None).await?;
    core_create_pull_request(&github.service, &repository, &params)
      .await
      .map_err(|e| CommandError::new(e.to_string()))
  })
  .await
}

/// Waits until GitHub has the branch just pushed (a mirror copies it a moment later): false when it still doesn't after
/// `MIRROR_WAIT`
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github))]
#[specta::specta]
pub async fn wait_for_github_branch(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: WaitForGitHubBranchParams,
) -> Result<bool, CommandError> {
  operation(async move {
    let repository = required_github_repository(&git_executor, &repository_state, &params.repository_path, None).await?;
    wait_for_branch(&github.service, &params.repository_path, &repository, &params.branch, MIRROR_WAIT)
      .await
      .map_err(|failure| CommandError::new(failure_message(&failure)))
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
) -> Result<Vec<GitHubUser>, CommandError> {
  operation(async move {
    let repository = required_github_repository(&git_executor, &repository_state, &params.repository_path, None).await?;
    core_search_github_users(&github.service, &params.repository_path, &repository, &params.query)
      .await
      .map_err(|failure| CommandError::new(failure_message(&failure)))
  })
  .await
}

/// What the rules of a branch do to the pull requests into it: the Safe Push popover warns before a push that dismisses
/// the approvals of the pull request
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github))]
#[specta::specta]
pub async fn get_branch_rules(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: GetBranchRulesParams,
) -> Result<BranchRules, CommandError> {
  operation(async move {
    let repository = required_github_repository(&git_executor, &repository_state, &params.repository_path, None).await?;
    read_branch_rules(&github.service, &params.repository_path, &repository, &params.branch)
      .await
      .map_err(|failure| CommandError::new(failure_message(&failure)))
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
) -> Result<Option<GitHubRepository>, CommandError> {
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
) -> Result<GitHubRepository, CommandError> {
  github_repository(git_executor, repository_state, repository_path, known)
    .await?
    .ok_or_else(|| CommandError::new(NOT_ON_GITHUB_MESSAGE))
}
