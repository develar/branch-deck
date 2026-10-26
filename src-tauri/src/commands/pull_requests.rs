use crate::commands::branch_prefix::{PrefixInputs, Prefixes, Settings};
use crate::commands::github::GitHubState;
use crate::commands::session::SessionsState;
use crate::commands::youtrack::YouTrackState;
use crate::operation::{operation, run_blocking};
use crate::repository_state::RepositoryStateCache;
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use domain_types::pull_request::{GitHubUser, NewPullRequest};
use domain_types::session::{Version, ViewAnswer};
use git_executor::git_command_executor::GitCommandExecutor;
use github::service::GitHubService;
use remote_facts::pull_request_create::{GetNewPullRequestParams, SearchGitHubUsersParams, new_pull_request, read_viewer_login, search_github_users as core_search_github_users};
use remote_facts::pull_request_writes::{
  ClosePullRequestParams, GetPullRequestHeadParams, SubmitPullRequestReviewParams, close_pull_request as core_close_pull_request, read_pull_request_head,
  submit_pull_request_review as core_submit_pull_request_review,
};
use remote_facts::pull_requests::{NOT_ON_GITHUB_MESSAGE, failure_message};
use remote_facts::stack_pull_request::{GetNewStackPullRequestParams, read_new_stack_pull_request};
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::session::SessionKey;
use sync_core::github_repository::{GitHubRepository, parse_known_github_repository};
use tauri::State;
use tracing::instrument;

/// Closes a pull request of the virtual branches with a comment: an ancestor a Safe Push of a stacked branch landed
/// with it. The session of the repository has it closed at once; the version of the view after its patch
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github, sessions, settings, prefixes))]
#[specta::specta]
pub async fn close_pull_request(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  sessions: State<'_, SessionsState>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  params: ClosePullRequestParams,
) -> Result<Option<Version>, ApiError> {
  operation(async move {
    let writer = PullRequestWriter::new(&git_executor, &repository_state, &github, &sessions, &settings, &prefixes);
    let (path, number) = (params.repository_path.as_str(), params.pull_request_number);
    let close = async |service: &GitHubService, repository: &GitHubRepository| core_close_pull_request(service, path, repository, number, &params.comment).await;
    Ok(writer.write(path, close, |&(): &()| PullRequestWrite::Closed { number }).await?.version)
  })
  .await
}

/// What a write of the app to a pull request runs with: the GitHub of the repository, and its session, which the result
/// goes into under the prefix Rust resolves
struct PullRequestWriter<'a> {
  git_executor: &'a GitCommandExecutor,
  repository_state: &'a RepositoryStateCache,
  github: &'a GitHubState,
  sessions: &'a SessionsState,
  inputs: PrefixInputs,
}

impl<'a> PullRequestWriter<'a> {
  fn new(
    git_executor: &'a State<'_, GitCommandExecutor>,
    repository_state: &'a State<'_, RepositoryStateCache>,
    github: &'a State<'_, GitHubState>,
    sessions: &'a State<'_, SessionsState>,
    settings: &Settings<'_>,
    prefixes: &Prefixes<'_>,
  ) -> Self {
    Self {
      inputs: PrefixInputs::new(git_executor, settings, prefixes),
      git_executor,
      repository_state,
      github,
      sessions,
    }
  }

  /// Runs `write` on GitHub in the repository at `path`, then puts what it changed (`change` of its outcome) into the
  /// session of the repository, newer than every listing that started before: the outcome with the version of the view
  /// after its patch; none when no session is open (or the prefix is gone), the next listing reads it
  async fn write<T, E: std::fmt::Display>(
    self,
    path: &str,
    write: impl AsyncFnOnce(&GitHubService, &GitHubRepository) -> Result<T, E>,
    change: impl FnOnce(&T) -> PullRequestWrite,
  ) -> Result<ViewAnswer<T>, ApiError> {
    let repository = required_github_repository(self.git_executor, self.repository_state, path, None).await?;
    let outcome = write(&self.github.service, &repository).await.map_err(|e| ApiError::internal(e.to_string()))?;
    let write = change(&outcome);
    let key = match self.inputs.resolve(path).await {
      Ok(prefix) => SessionKey {
        worktree: path.to_owned(),
        prefix,
      },
      Err(_) => return Ok(ViewAnswer { outcome, version: None }),
    };
    let version = self.sessions.sessions().write_pull_request(&key, write).await;
    Ok(ViewAnswer { outcome, version })
  }
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
) -> Result<(), ApiError> {
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
    .map_err(|e| ApiError::internal(e.to_string()))
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
) -> Result<String, ApiError> {
  operation(async move {
    let repository = required_github_repository(&git_executor, &repository_state, &params.repository_path, params.github_repository.as_deref()).await?;
    read_pull_request_head(&github.service, &params.repository_path, &repository, params.pull_request_number)
      .await
      .map_err(|e| ApiError::internal(e.to_string()))
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
  settings: State<'_, SettingsService>,
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
