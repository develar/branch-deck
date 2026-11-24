//! The pull requests the user creates and the reviewers they ask (openspec/specs/branch/pull-requests/spec.md): the text a new
//! pull request of a branch or of a whole stack starts with, the users a review can be asked from, and whom a review
//! after landing asks by default (openspec/specs/branch/shown-reviews/spec.md), over the GitHub, the YouTrack and the
//! `CodeOwners` service of the session of the repository (`ServiceHost`)

use crate::context::AppContext;
use crate::operation::run_blocking;
use crate::transport::Call;
use codeowners::client::CodeOwnersError;
use domain_types::code_owners::{GetShowReviewersParams, ShowReviewers};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::pull_request::{GitHubUser, NewPullRequest};
use remote_facts::pull_request_create::{
  GetNewPullRequestParams, SearchGitHubUsersParams, new_pull_request as read_new_pull_request, read_viewer_login, search_github_users as search_users,
};
use remote_facts::pull_requests::{NOT_ON_GITHUB_MESSAGE, failure_message};
use remote_facts::show_reviewers::{lookup_candidates, owner_groups};
use remote_facts::stack_pull_request::{GetNewStackPullRequestParams, read_new_stack_pull_request};
use repository_model::session::SessionKey;

/// The text of a new pull request of a virtual branch, with the summaries of its issues from the YouTrack of the app
/// (`new_pull_request`), and the user of the token (`with_viewer`)
pub async fn get_new_pull_request(ctx: &AppContext, _call: &Call, key: SessionKey, params: GetNewPullRequestParams) -> Result<NewPullRequest, ApiError> {
  let youtrack = ctx.services.youtrack(&key)?;
  let pull_request = read_new_pull_request(&ctx.git, Some(&youtrack), "origin", &params)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
  with_viewer(ctx, &key, pull_request).await
}

/// The text of the pull request of a whole stack (`read_new_stack_pull_request`, one blocking step), and the user of the
/// token (`with_viewer`)
pub async fn get_new_stack_pull_request(ctx: &AppContext, _call: &Call, key: SessionKey, params: GetNewStackPullRequestParams) -> Result<NewPullRequest, ApiError> {
  let (git, prefix) = (ctx.git.clone(), key.prefix.clone());
  let pull_request = run_blocking(move || read_new_stack_pull_request(&git, &prefix, &params)).await?;
  with_viewer(ctx, &key, pull_request).await
}

/// The users GitHub finds for the query among those who can review in the repository
pub async fn search_github_users(ctx: &AppContext, _call: &Call, key: SessionKey, params: SearchGitHubUsersParams) -> Result<Vec<GitHubUser>, ApiError> {
  let repository = ctx.services.github_repository(&key).await?;
  let repository = repository.ok_or_else(|| ApiError::internal(NOT_ON_GITHUB_MESSAGE))?;
  let github = ctx.services.github(&key)?;
  search_users(&github, &key.worktree, &repository, &params.query)
    .await
    .map_err(|failure| ApiError::internal(failure_message(&failure)))
}

/// The text of a new pull request with the user of the token, who is its assignee; none when GitHub doesn't answer or
/// nothing names the GitHub repository
async fn with_viewer(ctx: &AppContext, key: &SessionKey, mut pull_request: NewPullRequest) -> Result<NewPullRequest, ApiError> {
  if let Some(repository) = ctx.services.github_repository(key).await? {
    let github = ctx.services.github(key)?;
    pull_request.viewer_login = read_viewer_login(&github, &key.worktree, &repository).await.ok();
  }
  Ok(pull_request)
}

/// Whom the review after landing of `params.branch_name` asks by default (`remote_facts::show_reviewers`, as `brd
/// safe-push --request-review` without `--reviewer`): the owner groups of what the branch changes against the baseline
/// of the session, by the `OWNERSHIP` files there, then one available member of each from the `CodeOwners` service,
/// never the user of the GitHub token. No lookup where no `OWNERSHIP` file names an owner of what it changes
///
/// # Errors
/// The session is not open or its table lacks the branch, no Space token, a failure of git, of the service or of GitHub
pub async fn get_show_reviewers(ctx: &AppContext, _call: &Call, key: SessionKey, params: GetShowReviewersParams) -> Result<ShowReviewers, ApiError> {
  let branch = params.branch_name;
  let read = {
    let branch = branch.clone();
    ctx
      .sessions
      .read(&key, move |state, facts| {
        let table = facts.table_facts(state)?;
        Some((table.table.tip_of(&branch)?.to_owned(), state.header().baseline_branch.clone()))
      })
      .await
      .flatten()
  };
  let (tip, baseline) = read.ok_or_else(|| ApiError::new(ErrorCode::InvalidArguments, format!("The table of the repository has no built branch {branch}")))?;
  let (git, path) = (ctx.git.clone(), key.worktree.clone());
  let target_ref = format!("refs/remotes/{baseline}");
  let groups = run_blocking(move || owner_groups(&git, &path, &target_ref, &tip)).await?;
  if groups.is_empty() {
    return Ok(ShowReviewers::default());
  }
  let access = ctx.services.codeowners(&key)?;
  let repository = ctx.services.github_repository(&key).await?.ok_or_else(|| ApiError::internal(NOT_ON_GITHUB_MESSAGE))?;
  let github = ctx.services.github(&key)?;
  let author = read_viewer_login(&github, &key.worktree, &repository)
    .await
    .map_err(|failure| ApiError::new(ErrorCode::NetworkError, failure_message(&failure)))?;
  let candidates = lookup_candidates(&access.client, &access.token, &groups, &author).await.map_err(|error| {
    let code = if matches!(error, CodeOwnersError::Unauthorized) {
      ErrorCode::NotLoggedIn
    } else {
      ErrorCode::NetworkError
    };
    ApiError::new(code, error.message())
  })?;
  Ok(ShowReviewers {
    groups,
    reviewers: candidates.reviewers,
    without_reviewer: candidates.without_reviewer,
  })
}
