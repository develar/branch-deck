use crate::channel_commands::{answer_with_progress, canceled_code, progress_channel, register_wait, run_waiting};
use crate::remote_host::PAGE_CLOCK_START;
use app_api::api_error::{IntoApiError, patronus_error, undo_error};
use axum::{
  extract::State,
  http::{HeaderMap, StatusCode, header},
  response::{
    IntoResponse, Json, Response,
    sse::{Event, KeepAlive, Sse},
  },
};
use domain_types::branch_name_suggestion::{BranchNameSuggestionOutcome, SuggestBranchNamesParams};
use domain_types::calendar::Calendar;
use domain_types::error::ApiError;
use domain_types::patronus::{PatronusAvailability, PatronusRobotsResult};
use domain_types::pull_request::{GitHubAuthStatus, GitHubDeviceCode, GitHubUser, NewPullRequest};
use domain_types::review_request::ReviewRequestsResult;
use domain_types::robot_failure::{GetRobotFailureParams, RobotFailureReport};
use domain_types::robot_stats::{GetRobotStatsParams, RobotStatsResult};
use domain_types::session::Version;
use domain_types::settings::{GitBranchPrefix, ResolvedBranchPrefix};
use domain_types::youtrack::{YouTrackAuthStatus, YouTrackIssueAnswer, YouTrackSignIn, YouTrackUser};
use file_diffs::model::{
  BlobText, FileDiffEvent, FileMergeEvent, FindDeclarationParams, FoundDeclarations, FoundUsages, GetBlobSyntaxParams, GetBlobTextsParams, GetFileDiffsParams, GetFileMergesParams,
};
use file_diffs::service::FileDiffs;
use git_ops::commit_files::{CommitFiles, DiffSubject, GetCommitFilesParams, GetCommitsFilesParams, read_commit_files_cached, read_diff_files};
use history_rewrite::repository_lock::{LockWait, RewriteOptions};
use object_cache::cache::ObjectCache;
use remote_facts::pull_request_create::{GetNewPullRequestParams, SearchGitHubUsersParams, new_pull_request, read_viewer_login, search_github_users as core_search_github_users};
use remote_facts::pull_requests::{NOT_ON_GITHUB_MESSAGE, failure_message};
use remote_facts::robot_stats::{RobotStatsSource, robot_stats_core};
use repository_model::session::SessionKey;
use repository_model::table::TableState;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use sync_core::branch_name_suggestion::clean_names;
use sync_core::delete_archived_branch::{DeleteArchivedBranchParams, delete_archived_branch_core};
use sync_core::github_repository::{GitHubRepository, parse_known_github_repository, resolve_github_repository};
use sync_core::patronus::{SpaceRepository, required_target_branch};
use sync_core::repository_config::{
  ConfigScope, RepositoryConfig, RepositoryConfigChange, RepositoryConfigKey, WriteScope, detect_github_repository, read_config_entries, repository_config_from_entries,
  with_stacks_off_clashes, write_global_config, write_repository_config,
};
use sync_core::robot_commits::robot_failure_report;
use sync_core::uncommitted_changes::{GetUncommittedChangesParams, UncommittedChangesResult, get_uncommitted_changes as core_get_uncommitted_changes};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::{Stream, StreamExt};

use crate::remote_host::HeldAnswer;
use crate::remote_host::RemoteHost;
use crate::state::AppState;

use crate::state::TestRepository;

/// How a command fails: with its `ApiError`, answered the way Tauri rejects a command (`{ status: "error", error:
/// <ApiError> }` with status 200, which the page mock hands to the bindings: the same JSON as the app,
/// `api_error_contract_test.rs`), or with a failure of the server itself, a panicked command (500). A type of the
/// server only because axum's `IntoResponse` can't be implemented for `ApiError` outside `domain-types`.
pub enum Rejection {
  Error(ApiError),
  Panicked,
}

impl From<ApiError> for Rejection {
  fn from(error: ApiError) -> Self {
    Self::Error(error)
  }
}

impl IntoResponse for Rejection {
  fn into_response(self) -> Response {
    match self {
      Self::Error(error) => Json(serde_json::json!({ "status": "error", "error": error })).into_response(),
      Self::Panicked => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
  }
}

/// The rejection of `operation` with `error` (a core's error through `api_error`, a message as `internal`). An error is
/// the expected answer of an error-path test, not a failure of the server, so it is logged at debug
pub(crate) fn failed(operation: &str, error: impl IntoApiError) -> Rejection {
  let error = error.into_api_error();
  tracing::debug!("Cannot {operation}: {}", error.message);
  Rejection::Error(error)
}

pub(crate) type CommandResult<T> = Result<Json<T>, Rejection>;

/// Runs a command off the async runtime, as the app runs its git work (`spawn_blocking`)
pub(crate) async fn run_command<T, E, F>(operation: &'static str, command: F) -> CommandResult<T>
where
  F: FnOnce() -> Result<T, E> + Send + 'static,
  T: Send + 'static,
  E: IntoApiError + Send + 'static,
{
  match tokio::task::spawn_blocking(command).await {
    Ok(Ok(value)) => Ok(Json(value)),
    Ok(Err(error)) => Err(failed(operation, error)),
    Err(join_error) => {
      tracing::error!("Cannot {operation}: the command panicked: {join_error}");
      Err(Rejection::Panicked)
    }
  }
}

/// A path that is no test repository (the deliberately missing directory of `NO_REPO`) fails the command, as the app
/// fails it for a directory that is gone
fn ensure_repository_exists(state: &AppState, path: &str) -> Result<(), Rejection> {
  repository_by_path(state, path).map(drop)
}

pub(crate) fn repository_by_path<'a>(state: &'a AppState, path: &str) -> Result<dashmap::mapref::one::Ref<'a, String, TestRepository>, Rejection> {
  state
    .path_to_id
    .get(path)
    .and_then(|id| state.repositories.get(id.value()))
    .ok_or_else(|| failed("find the test repository", format!("Repository not accessible: {path}")))
}

/// Streams the events sent to `rx` as SSE events named `event_name`, with a keep-alive every second
fn sse_events<T>(rx: mpsc::UnboundedReceiver<T>, event_name: &'static str) -> Sse<impl Stream<Item = Result<Event, Infallible>>>
where
  T: serde::Serialize + std::fmt::Debug + Send + 'static,
{
  let stream = UnboundedReceiverStream::new(rx).map(move |event| {
    tracing::debug!(?event, event_name, "Sending SSE event");
    Ok(Event::default().event(event_name).data(serde_json::to_string(&event).unwrap()))
  });
  Sse::new(stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(1)).text("keep-alive"))
}

#[derive(Deserialize)]
pub struct ValidateRepositoryPathRequest {
  path: String,
}

pub async fn validate_repository_path(Json(request): Json<ValidateRepositoryPathRequest>) -> Json<String> {
  // Use production validation logic
  match sync_core::repository_validation::validate_path(&request.path) {
    Ok(()) => Json(String::new()),
    Err(e) => Json(e.to_string()),
  }
}

#[derive(Deserialize)]
pub struct GetBranchPrefixRequest {
  #[serde(rename = "repositoryPath")]
  repository_path: String,
}

/// `resolve_branch_prefix` of the app with the local `branchdeck.branchPrefix` only, never the user's global config, so
/// the tests are deterministic (a local prefix is the repository's own), and the app's prefix of the test's settings;
/// an empty path (the settings without a repository) has no git prefix
pub async fn resolve_branch_prefix(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(request): Json<GetBranchPrefixRequest>) -> CommandResult<ResolvedBranchPrefix> {
  if request.repository_path.is_empty() {
    let settings = requesting_repository(&state, &headers).map_or_default(|repository| repository.store.app());
    return Ok(Json(app_settings::resolve::resolve_branch_prefix(
      &GitBranchPrefix::default(),
      settings.global_user_branch_prefix.as_deref(),
    )));
  }
  let inputs = prefix_inputs(&state, &request.repository_path)?;
  run_command("read the branch prefix", move || inputs.fresh(&request.repository_path)).await
}

/// What a command of the sync family resolves the prefix of a test repository with, as the app's `PrefixInputs`: the
/// settings of the test, the git prefixes of the server, the local git prefix as the git read
struct TestPrefixInputs {
  git: git_executor::git_command_executor::GitCommandExecutor,
  settings: domain_types::settings::AppSettings,
  prefixes: app_settings::branch_prefixes::BranchPrefixes,
}

impl TestPrefixInputs {
  /// The prefix of `repository_path` by the git prefix the server keeps, read once; blocking
  fn branch_prefix(&self, repository_path: &str) -> anyhow::Result<String> {
    let read_git = || read_local_branch_prefix(&self.git, repository_path);
    Ok(self.prefixes.resolve(&self.settings, repository_path, read_git)?.prefix)
  }

  /// The prefix of `repository_path` by its git config read now, kept for the commands after it; blocking
  fn fresh(&self, repository_path: &str) -> anyhow::Result<ResolvedBranchPrefix> {
    Ok(
      self
        .prefixes
        .refresh(&self.settings, repository_path, read_local_branch_prefix(&self.git, repository_path)?),
    )
  }
}

/// The inputs of the prefix of the test repository at `repository_path`; an error for an unknown one
fn prefix_inputs(state: &AppState, repository_path: &str) -> Result<TestPrefixInputs, Rejection> {
  let repository = repository_by_path(state, repository_path)?;
  Ok(TestPrefixInputs {
    git: state.git_executor.clone(),
    settings: repository.store.app(),
    prefixes: state.branch_prefixes.clone(),
  })
}

/// The prefix a command of the repository at `repository_path` names its branches with, as the app's commands resolve it
/// (`TestPrefixInputs::branch_prefix`, in a blocking step); an error for an unknown repository
pub(crate) async fn branch_prefix_of(state: &AppState, repository_path: &str) -> Result<String, Rejection> {
  let inputs = prefix_inputs(state, repository_path)?;
  let path = repository_path.to_owned();
  Ok(run_command("read the branch prefix", move || inputs.branch_prefix(&path)).await?.0)
}

/// The prefix of the test repository at `repository_path` by its local config read now, kept for the commands after it:
/// what the run of its session checks its key against, as the app's run resolves it from the config it reads; blocking
pub(crate) fn branch_prefix_now(state: &AppState, repository_path: &str) -> anyhow::Result<String> {
  let inputs = prefix_inputs(state, repository_path).map_err(|_unknown| anyhow::anyhow!("Repository not accessible: {repository_path}"))?;
  Ok(inputs.fresh(repository_path)?.prefix)
}

/// Settings → Patronus, the new API, of the settings of the test repository at `repository_path`, as the app's commands
/// read it from the writer (a test sets it with `initialStoreValues` or through the page's `set_setting`)
pub(crate) fn patronus_new_api_of(state: &AppState, repository_path: &str) -> Result<bool, Rejection> {
  Ok(repository_by_path(state, repository_path)?.store.app().patronus_new_api)
}

/// `branchdeck.branchPrefix` of the repository's local config (the git read of `resolve_branch_prefix`)
pub fn read_local_branch_prefix(git_executor: &git_executor::git_command_executor::GitCommandExecutor, repo_path: &str) -> anyhow::Result<GitBranchPrefix> {
  let (output, exit_code) = git_executor.execute_command_with_status(&["config", "--local", "branchdeck.branchPrefix"], repo_path)?;
  match exit_code {
    0 => Ok(GitBranchPrefix {
      prefix: output.trim().to_string(),
      repository_scoped: true,
    }),
    1 => Ok(GitBranchPrefix::default()),
    128 => Err(anyhow::anyhow!("Repository not accessible: {repo_path}")),
    code => {
      tracing::warn!(code, repo_path, "Unexpected exit code reading local branch prefix");
      Ok(GitBranchPrefix::default())
    }
  }
}

/// The git of the config commands of a test repository: its global config is the repository's own file
/// (`AppState::global_config_of`), never the one of the developer, for a read and for a write
pub(crate) fn config_git(state: &AppState, repository_path: &str) -> Result<git_executor::git_command_executor::GitCommandExecutor, Rejection> {
  let repository = repository_by_path(state, repository_path)?;
  let global_config = state.global_config_of(&repository.id);
  Ok(state.git_executor.with_env("GIT_CONFIG_GLOBAL", &global_config.to_string_lossy()))
}

/// The production read without the system config, its global config the repository's own file (`config_git`), so the
/// tests are deterministic
fn read_test_repository_config(git_executor: &git_executor::git_command_executor::GitCommandExecutor, repository_path: &str) -> anyhow::Result<RepositoryConfig> {
  let entries: Vec<_> = read_config_entries(git_executor, repository_path)?
    .into_iter()
    .filter(|entry| matches!(entry.scope, ConfigScope::Local | ConfigScope::Global))
    .collect();
  let config = repository_config_from_entries(&entries, detect_github_repository(git_executor, repository_path)?);
  Ok(with_stacks_off_clashes(git_executor, repository_path, config))
}

pub async fn get_repository_config(State(state): State<Arc<AppState>>, Json(request): Json<GetBranchPrefixRequest>) -> CommandResult<RepositoryConfig> {
  let git_executor = config_git(&state, &request.repository_path)?;
  run_command("read the repository settings", move || read_test_repository_config(&git_executor, &request.repository_path)).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRepositoryConfigRequest {
  repository_path: String,
  change: RepositoryConfigChange,
  #[serde(default)]
  scope: WriteScope,
}

/// The production write; a write of the global config goes to the repository's own file (`config_git`), as the
/// developer's must not change. The answer is the read of `get_repository_config`
pub async fn set_repository_config(State(state): State<Arc<AppState>>, Json(request): Json<SetRepositoryConfigRequest>) -> CommandResult<RepositoryConfig> {
  let git_executor = config_git(&state, &request.repository_path)?;
  let inputs = prefix_inputs(&state, &request.repository_path)?;
  run_command("change the repository settings", move || {
    match request.scope {
      WriteScope::Local => write_repository_config(&git_executor, &request.repository_path, &request.change)?,
      WriteScope::Global => write_global_config(&git_executor, &request.change)?,
    }
    // the git prefix the later commands resolve with, as the app keeps it after the write
    if request.change.key() == RepositoryConfigKey::BranchPrefix {
      inputs.fresh(&request.repository_path)?;
    }
    read_test_repository_config(&git_executor, &request.repository_path)
  })
  .await
}

/// The production check for a test repository (the template sets user.name and user.email); a fixed suggestion without
/// one, so the first run does not depend on the identity of the machine
pub async fn check_git_setup(State(state): State<Arc<AppState>>, Json(request): Json<GetBranchPrefixRequest>) -> CommandResult<sync_core::git_setup::GitSetup> {
  if request.repository_path.is_empty() || ensure_repository_exists(&state, &request.repository_path).is_err() {
    return Ok(Json(sync_core::git_setup::GitSetup {
      git_error: None,
      identity_error: None,
      suggested_branch_prefix: "test-user".to_string(),
    }));
  }
  let git_executor = state.git_executor.clone();
  run_command("check the git setup", move || {
    anyhow::Ok(sync_core::git_setup::check_git_setup(&git_executor, &request.repository_path))
  })
  .await
}

/// The preview of a move of the "Move to…" form: the core's dry run, nothing written (`preview_move` of the app)
pub async fn preview_move(
  State(state): State<Arc<AppState>>,
  Json(params): Json<history_rewrite::move_commits::MovePreviewParams>,
) -> CommandResult<domain_types::rewrite::MoveCommitsResult> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("preview a move", move || history_rewrite::move_commits::preview_move_core(&git_executor, &params)).await
}

pub async fn unapply_branch(
  State(state): State<Arc<AppState>>,
  headers: HeaderMap,
  Json(params): Json<sync_core::unapply_branch::UnapplyBranchParams>,
) -> Result<Response, Rejection> {
  let repository_path = params.repository_path.clone();
  run_waiting_with_prefix(&state, &headers, &repository_path, "unapply the branch", move |git, branch_prefix, lock_wait| {
    sync_core::unapply_branch::unapply_branch_core(git, params, branch_prefix, &RewriteOptions::waiting(lock_wait))
  })
  .await
}

/// Pick Remote Commits, as the app's command runs it (`pick_remote_commits_core`), without the object cache
pub async fn pick_remote_commits(
  State(state): State<Arc<AppState>>,
  headers: HeaderMap,
  Json(params): Json<update_core::pick::PickRemoteCommitsParams>,
) -> Result<Response, Rejection> {
  let repository_path = params.repository_path.clone();
  run_waiting_with_prefix(&state, &headers, &repository_path, "pick the remote commits", move |git, branch_prefix, lock_wait| {
    update_core::pick::pick_remote_commits_core(
      git,
      &params,
      branch_prefix,
      &RewriteOptions::waiting(lock_wait),
      &ObjectCache::disabled(),
      &FileDiffs::default(),
    )
  })
  .await
}

/// The undo of Pick Remote Commits, as the app's command runs it
pub async fn undo_pick_remote_commits(
  State(state): State<Arc<AppState>>,
  headers: HeaderMap,
  Json(params): Json<update_core::pick::UndoPickRemoteCommitsParams>,
) -> Result<Response, Rejection> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  Ok(
    run_waiting(&state, &headers, "undo the pick of the remote commits", move |_lock_wait| {
      update_core::pick::undo_pick_remote_commits_core(&git_executor, &params, history_rewrite::repository_lock::DEFAULT_LOCK_TIMEOUT).map_err(undo_error)
    })
    .await,
  )
}

/// `step` of a rewrite of the test repository at `repository_path` under `run_waiting`, with git and the prefix the
/// app's commands name its branches with (`branch_prefix_of`); an error for an unknown repository
async fn run_waiting_with_prefix<T, F>(state: &AppState, headers: &HeaderMap, repository_path: &str, operation: &'static str, step: F) -> Result<Response, Rejection>
where
  F: FnOnce(&git_executor::git_command_executor::GitCommandExecutor, &str, LockWait<'_>) -> anyhow::Result<T> + Send + 'static,
  T: serde::Serialize + Send + 'static,
{
  let branch_prefix = branch_prefix_of(state, repository_path).await?;
  let git_executor = state.git_executor.clone();
  Ok(run_waiting(state, headers, operation, move |lock_wait| step(&git_executor, &branch_prefix, lock_wait)).await)
}

pub async fn undo_unapply_branch(
  State(state): State<Arc<AppState>>,
  headers: HeaderMap,
  Json(params): Json<sync_core::unapply_branch::UndoUnapplyParams>,
) -> Result<Response, Rejection> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  Ok(
    run_waiting(&state, &headers, "undo the unapply", move |lock_wait| {
      let token = sync_core::unapply_branch::UnapplyUndoToken::parse(&params.undo_token)
        .ok_or_else(|| anyhow::anyhow!("Not an undo token of an unapply: {}", params.undo_token))
        .map_err(undo_error)?;
      sync_core::unapply_branch::undo_unapply_core(&git_executor, &params.repository_path, &token, lock_wait).map_err(undo_error)
    })
    .await,
  )
}

pub async fn reapply_branch(
  State(state): State<Arc<AppState>>,
  headers: HeaderMap,
  Json(params): Json<sync_core::reapply_branch::ReapplyBranchParams>,
) -> Result<Response, Rejection> {
  let repository_path = params.repository_path.clone();
  run_waiting_with_prefix(&state, &headers, &repository_path, "reapply the branch", move |git, branch_prefix, lock_wait| {
    sync_core::reapply_branch::reapply_branch_core(git, &params, branch_prefix, &RewriteOptions::waiting(lock_wait))
  })
  .await
}

pub async fn undo_reapply_branch(
  State(state): State<Arc<AppState>>,
  headers: HeaderMap,
  Json(params): Json<sync_core::reapply_branch::UndoReapplyParams>,
) -> Result<Response, Rejection> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  Ok(
    run_waiting(&state, &headers, "undo the reapply", move |lock_wait| {
      let token = sync_core::reapply_branch::ReapplyUndoToken::parse(&params.undo_token)
        .ok_or_else(|| anyhow::anyhow!("Not an undo token of a reapply: {}", params.undo_token))
        .map_err(undo_error)?;
      sync_core::reapply_branch::undo_reapply_core(&git_executor, &params.repository_path, &token, lock_wait).map_err(undo_error)
    })
    .await,
  )
}

/// Without the object cache, as every command of the mock server
pub async fn get_stack_independence(
  State(state): State<Arc<AppState>>,
  Json(params): Json<trial_check::check::StackIndependenceParams>,
) -> CommandResult<trial_check::check::StackIndependence> {
  let branch_prefix = branch_prefix_of(&state, &params.repository_path).await?;
  let git_executor = state.git_executor.clone();
  run_command("check the stack independence", move || {
    trial_check::check::get_stack_independence(&git_executor, &ObjectCache::disabled(), &branch_prefix, &params)
  })
  .await
}

/// The commit map of a running robot, by the real core on the test repository
pub async fn get_robot_commit_map(
  State(state): State<Arc<AppState>>,
  Json(params): Json<domain_types::robot_failure::GetRobotCommitMapParams>,
) -> CommandResult<domain_types::robot_failure::RobotCommitMap> {
  let branch_prefix = branch_prefix_of(&state, &params.repository_path).await?;
  let git_executor = state.git_executor.clone();
  run_command("read the commit map of the robot", move || {
    let subject = sync_core::robot_commits::RobotSubject {
      branch_name: params.branch_name.as_deref(),
      stack_root: params.stack_root.as_deref(),
    };
    sync_core::robot_commits::robot_subject_commit_map(
      &git_executor,
      &params.repository_path,
      &branch_prefix,
      subject,
      &params.source_revision,
      &params.baseline_branch,
      &params.ancestors,
    )
    .map(Option::unwrap_or_default)
  })
  .await
}

/// Without the object cache, as every command of the mock server; nothing cancels it
pub async fn find_conflict_fix(
  State(state): State<Arc<AppState>>,
  Json(params): Json<trial_check::conflict_fix::FindConflictFixParams>,
) -> CommandResult<trial_check::conflict_fix::ConflictFix> {
  let branch_prefix = branch_prefix_of(&state, &params.repository_path).await?;
  let git_executor = state.git_executor.clone();
  run_command("find the conflict fix", move || {
    trial_check::conflict_fix::find_conflict_fix_core(&git_executor, &ObjectCache::disabled(), &params, &branch_prefix, &std::sync::atomic::AtomicBool::new(false))
  })
  .await
}

/// A search of the mock server runs to its end
pub async fn cancel_conflict_fix() -> Json<()> {
  Json(())
}

pub async fn get_autosquash_plan(
  State(state): State<Arc<AppState>>,
  Json(params): Json<history_rewrite::autosquash::GetAutosquashPlanParams>,
) -> CommandResult<history_rewrite::autosquash::AutosquashPlan> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("get the autosquash plan", move || {
    history_rewrite::autosquash::get_autosquash_plan_core(&git_executor, &params.repository_path)
  })
  .await
}

/// The progress goes to the channel of the page as the app sends it (`channel_commands`), the wait for a live holder of
/// the repository lock too; the fetch reads the upstream from a directory of the template, so the update works offline
pub async fn update_project(
  State(state): State<Arc<AppState>>,
  headers: HeaderMap,
  Json(params): Json<update_core::update_project::UpdateProjectParams>,
) -> Result<Response, Rejection> {
  use update_core::update_project::{UpdatePhase, UpdateProgress, update_project_core};
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  let lock_waits = state.lock_waits.clone();
  let channel = progress_channel(&headers);
  Ok(
    answer_with_progress(channel, move |progress| async move {
      let tell = progress.clone();
      let ticket = register_wait(&lock_waits, channel, move |holder| {
        if let Some(holder) = holder {
          tell.send(&UpdateProgress {
            phase: UpdatePhase::Waiting,
            fetch: None,
            waiting_for: Some(history_rewrite::lock_wait::waiting_for(holder)),
          });
        }
      });
      let step_ticket = Arc::clone(&ticket);
      let updated = run_command("update the project", move || {
        update_project_core(
          &git_executor,
          &params,
          &|update_progress| progress.send(&update_progress),
          LockWait::WhileHeld(&*step_ticket),
        )
      })
      .await;
      canceled_code(updated, &ticket)
    })
    .await,
  )
}

#[derive(Deserialize)]
pub struct DismissLandedRequest {
  #[serde(rename = "repositoryPath")]
  repository_path: String,
}

/// Clears the report of what landed
pub async fn dismiss_landed(State(state): State<Arc<AppState>>, Json(request): Json<DismissLandedRequest>) -> CommandResult<()> {
  ensure_repository_exists(&state, &request.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("dismiss the landed commits", move || {
    sync_core::landed::dismiss_landed(&git_executor, &request.repository_path)
  })
  .await
}

/// The progress channel is not mocked
pub async fn fetch_baseline(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::baseline_fetch::FetchBaselineParams>,
) -> CommandResult<sync_core::baseline_fetch::FetchBaselineResult> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("fetch the baseline", move || {
    sync_core::baseline_fetch::fetch_baseline_core(&git_executor, &params, &mut |_| {})
  })
  .await
}

#[derive(Deserialize)]
pub struct CancelLockWaitRequest {
  channel: u32,
}

/// `cancel_lock_wait` of the app: stops the wait of the command whose channel the page mock named
pub async fn cancel_lock_wait(State(state): State<Arc<AppState>>, Json(request): Json<CancelLockWaitRequest>) -> Json<bool> {
  Json(state.lock_waits.cancel(request.channel))
}

/// The GitHub repository of the repository at `path` as the app resolves it (`commands/pull_requests.rs`): a known
/// `owner/name`, else `branchdeck.githubRepository` (a fixture writes it), else none; with the fake services
pub(crate) async fn github_of(state: &AppState, path: &str, known: Option<&str>) -> Result<(Arc<RemoteHost>, Option<GitHubRepository>), Rejection> {
  let host = remote_of(state, path)?;
  if let Some(repository) = parse_known_github_repository(known) {
    return Ok((host, Some(repository)));
  }
  let (git, path) = (config_git(state, path)?, path.to_string());
  let Json(repository) = run_command("resolve the GitHub repository", move || resolve_github_repository(&git, &path)).await?;
  Ok((host, repository))
}

/// `github_of` for a request that needs the repository: none fails as the app fails it
async fn required_github_of(state: &AppState, path: &str, known: Option<&str>, operation: &str) -> Result<(Arc<RemoteHost>, GitHubRepository), Rejection> {
  match github_of(state, path, known).await? {
    (host, Some(repository)) => Ok((host, repository)),
    (_, None) => Err(failed(operation, NOT_ON_GITHUB_MESSAGE)),
  }
}

/// The repository of a session, held as a command holds it while a read of its facts runs: its deletion waits
pub(crate) async fn holding(state: &AppState, key: &SessionKey) -> Result<tokio::sync::OwnedRwLockReadGuard<()>, ApiError> {
  let in_flight = Arc::clone(&repository_by_path(state, &key.worktree).map_err(rejected)?.in_flight);
  Ok(in_flight.read_owned().await)
}

/// The sign-in of the requesting page's repository, as the app reads it, with the OAuth app of a fixture (none by
/// default)
pub async fn get_github_auth_status(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<GitHubAuthStatus> {
  let host = requesting_repository(&state, &headers)?.remote(&state.git_executor);
  held_or_failed(&host, HeldAnswer::GitHubAuth, "read the GitHub sign-in").await?;
  let client_id = host.github_client_id();
  Ok(Json(host.github_sign_in().status(&host.github_service(), client_id.as_deref()).await))
}

/// Starts the app's sign-in against the device flow of the fake GitHub, as the app does
pub async fn start_github_sign_in(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<GitHubDeviceCode> {
  let host = requesting_repository(&state, &headers)?.remote(&state.git_executor);
  let client_id = host.github_client_id();
  let code = host.github_sign_in().start(&host.github_service(), client_id.as_deref()).await;
  code.map(Json).map_err(|e| failed("start the GitHub sign-in", e))
}

/// Waits for the code of the sign-in to be entered (a fixture authorizes it) and keeps the token, as the app does; the
/// page mock tells the windows (`github-auth-changed`)
pub async fn wait_github_sign_in(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<String> {
  let host = requesting_repository(&state, &headers)?.remote(&state.git_executor);
  let client_id = host.github_client_id();
  let login = host.github_sign_in().wait(&host.github_service(), client_id.as_deref()).await;
  login.map(Json).map_err(|e| failed("sign in to GitHub", e))
}

/// Ends the wait of the sign-in in progress, as the app does
pub async fn cancel_github_sign_in(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<()> {
  requesting_repository(&state, &headers)?.remote(&state.git_executor).github_sign_in().cancel();
  Ok(Json(()))
}

/// Removes the token of the sign-in, as the app does; the page mock tells the windows (`github-auth-changed`)
pub async fn sign_out_of_github(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<()> {
  let host = requesting_repository(&state, &headers)?.remote(&state.git_executor);
  let signed_out = host.github_sign_in().sign_out(&host.github_service()).await;
  signed_out.map(Json).map_err(|e| failed("sign out of GitHub", e))
}

/// The text a new pull request starts with and the user of the token, as the app reads them: the issue summaries
/// from the YouTrack of the repository, when the issue links name it and the app is signed in
pub async fn get_new_pull_request(State(state): State<Arc<AppState>>, Json(params): Json<GetNewPullRequestParams>) -> CommandResult<NewPullRequest> {
  let host = remote_of(&state, &params.repository_path)?;
  let pull_request = new_pull_request(&state.git_executor, Some(host.youtrack_service()), "origin", &params).await;
  let pull_request = pull_request.map_err(|e| failed("read the text of the pull request", e))?;
  with_viewer(&state, &params.repository_path, pull_request).await
}

/// The text of a new pull request with the user of the token, who is its assignee; none when GitHub doesn't answer
async fn with_viewer(state: &AppState, path: &str, mut pull_request: NewPullRequest) -> CommandResult<NewPullRequest> {
  if let (host, Some(repository)) = github_of(state, path, None).await? {
    pull_request.viewer_login = read_viewer_login(&host.github_service(), path, &repository).await.ok();
  }
  Ok(Json(pull_request))
}

/// The users a review can be asked from, as the app searches them
pub async fn search_github_users(State(state): State<Arc<AppState>>, Json(params): Json<SearchGitHubUsersParams>) -> CommandResult<Vec<GitHubUser>> {
  let operation = "search the GitHub users";
  let (host, repository) = required_github_of(&state, &params.repository_path, None, operation).await?;
  let users = core_search_github_users(&host.github_service(), &params.repository_path, &repository, &params.query).await;
  users.map(Json).map_err(|failure| failed(operation, failure_message(&failure)))
}

/// The YouTrack sign-in of the requesting page's repository, as the app reads it
pub async fn get_youtrack_auth_status(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<YouTrackAuthStatus> {
  let host = requesting_repository(&state, &headers)?.remote(&state.git_executor);
  held_or_failed(&host, HeldAnswer::YouTrackAuth, "read the YouTrack sign-in").await?;
  Ok(Json(host.youtrack_service().status()))
}

#[derive(Deserialize)]
pub struct YouTrackUrlParams {
  url: String,
}

/// Starts the browser sign-in to the YouTrack at `url` (the fake's), as the app does: the Hub page a test fetches
pub async fn start_youtrack_sign_in(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(params): Json<YouTrackUrlParams>) -> CommandResult<YouTrackSignIn> {
  let host = requesting_repository(&state, &headers)?.remote(&state.git_executor);
  let started = host.youtrack_sign_in().start(host.youtrack_service(), &params.url).await;
  started.map(Json).map_err(|e| failed("start the YouTrack sign-in", e))
}

/// Waits for the browser to come back from the Hub page and keeps the tokens, as the app does; the page mock tells the
/// windows (`youtrack-auth-changed`)
pub async fn wait_youtrack_sign_in(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<YouTrackUser> {
  let host = requesting_repository(&state, &headers)?.remote(&state.git_executor);
  let user = host.youtrack_sign_in().wait(host.youtrack_service()).await;
  user.map(Json).map_err(|e| failed("sign in to YouTrack", e))
}

/// Ends the wait of the sign-in in progress, as the app does
pub async fn cancel_youtrack_sign_in(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<()> {
  requesting_repository(&state, &headers)?.remote(&state.git_executor).youtrack_sign_in().cancel();
  Ok(Json(()))
}

#[derive(Deserialize)]
pub struct SaveYouTrackTokenParams {
  url: String,
  token: String,
}

/// Keeps a pasted token once the fake YouTrack accepted it, as the app does; the page mock tells the windows
pub async fn save_youtrack_token(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(params): Json<SaveYouTrackTokenParams>) -> CommandResult<YouTrackUser> {
  let host = requesting_repository(&state, &headers)?.remote(&state.git_executor);
  let user = host.youtrack_sign_in().save_token(host.youtrack_service(), &params.url, &params.token).await;
  user.map(Json).map_err(|e| failed("keep the YouTrack token", e))
}

/// Removes the token of the sign-in, as the app does; the page mock tells the windows
pub async fn sign_out_of_youtrack(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<()> {
  let host = requesting_repository(&state, &headers)?.remote(&state.git_executor);
  let signed_out = host.youtrack_sign_in().sign_out(host.youtrack_service()).await;
  signed_out.map(Json).map_err(|e| failed("sign out of YouTrack", e))
}

#[derive(Deserialize)]
pub struct YouTrackIssueParams {
  key: String,
}

/// The issue of `key` from the fake YouTrack, as the app reads it
pub async fn get_youtrack_issue(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(params): Json<YouTrackIssueParams>) -> CommandResult<YouTrackIssueAnswer> {
  let host = requesting_repository(&state, &headers)?.remote(&state.git_executor);
  Ok(Json(host.youtrack_service().issue(&params.key).await))
}

/// The review requests of the fixture set by `POST /repositories/{id}/review-requests`, as the session of a test
/// repository reads them (`TestFactSources`): GitHub is never asked in E2E; counted
pub(crate) async fn review_requests_of(state: &AppState, key: &SessionKey, reviews: bool) -> Result<ReviewRequestsResult, ApiError> {
  let mut result = {
    let repo = repository_by_path(state, &key.worktree).map_err(rejected)?;
    let fixture = repo.review_requests.read().expect("review request fixture lock").clone();
    let mut result = fixture.result.clone();
    // the rest of Reviews only while the view shows, its links as the app keeps them; a link that went is forgotten
    result.others = None;
    if reviews && let domain_types::pull_request::PullRequestAvailability::Available { owner, name } = &result.availability {
      let full_name = format!("{owner}/{name}");
      let others = crate::review_fixtures::fixture_reviews(&fixture, &app_settings::linked_pull_requests::linked(&repo.store, &full_name));
      let gone = domain_policy::reviews::gone_links(&others);
      if !gone.is_empty() {
        let _ = app_settings::linked_pull_requests::forget(&repo.store, &full_name, &gone);
      }
      result.others = Some(others);
    }
    result
  };
  // an empty head is the tip of the branch the request names, as GitHub would show it (the fetch reviews that commit);
  // resolved before the call is counted, so a test that sees the count sees an answer that only has to be applied
  resolve_review_heads(&key.worktree, &mut result);
  let host = {
    let repo = repository_by_path(state, &key.worktree).map_err(rejected)?;
    repo.review_request_calls.fetch_add(1, Ordering::SeqCst);
    if reviews {
      repo.reviews_calls.fetch_add(1, Ordering::SeqCst);
    }
    repo.existing_remote()
  };
  // a test that holds the answer (`/remote-gates`) sees the page before it; no entry of the map is held meanwhile, so the
  // deletion of the repository, which releases it, can remove the repository
  if let Some(host) = host {
    host.pass_held(HeldAnswer::ReviewRequests).await;
  }
  Ok(result)
}

/// The heads of the fixture's pull requests that are empty, as the tips of the branches they name (else `HEAD`), read
/// from the refs of the repository without a git process: the answer of a listing stays as quick as the fixture
fn resolve_review_heads(path: &str, result: &mut ReviewRequestsResult) {
  let git_dir = std::path::Path::new(path).join(".git");
  if !git_dir.is_dir() {
    return;
  }
  let packed = std::fs::read_to_string(git_dir.join("packed-refs")).unwrap_or_default();
  let tip = |reference: &str| -> Option<String> {
    let loose = std::fs::read_to_string(git_dir.join(reference)).ok().map(|id| id.trim().to_owned());
    loose.filter(|id| id.len() == 40).or_else(|| {
      packed
        .lines()
        .find_map(|line| line.strip_suffix(reference).and_then(|id| id.strip_suffix(' ')).map(str::to_owned))
    })
  };
  let head = std::fs::read_to_string(git_dir.join("HEAD"))
    .ok()
    .and_then(|head| head.trim().strip_prefix("ref: ").map(str::to_owned))
    .and_then(|reference| tip(&reference))
    .unwrap_or_default();
  let mut resolve = |request: &mut domain_types::review_request::ReviewRequest| {
    if request.head_ref_oid.is_empty() {
      request.head_ref_oid = tip(&format!("refs/heads/{}", request.head_ref_name)).unwrap_or_else(|| head.clone());
    }
  };
  result.requests.iter_mut().for_each(&mut resolve);
  result.answered.iter_mut().for_each(|answered| resolve(&mut answered.request));
  if let Some(others) = &mut result.others {
    let linked = others.linked.iter_mut().filter_map(|linked| match linked {
      domain_types::review_request::LinkedAnswer::Open { entry } => Some(entry.as_mut()),
      _ => None,
    });
    for entry in others
      .team
      .entries
      .iter_mut()
      .chain(&mut others.reviewed.entries)
      .chain(&mut others.authored.entries)
      .chain(linked)
    {
      resolve(&mut entry.request);
    }
  }
}

/// The range of the test repository without a network: an empty head is the tip of the local branch `headRefName`
/// (else of `HEAD`), an empty base the baseline
/// (`HEAD~2` when the baseline is `HEAD`, or less in a short template), so both commits are local and the real `fetch_review_head` only reads them.
/// The progress channel is not mocked; the fixture's `fetchError` fails it
pub async fn fetch_review_head(
  State(state): State<Arc<AppState>>,
  Json(mut params): Json<domain_types::review_request::FetchReviewHeadParams>,
) -> CommandResult<domain_types::review_request::ReviewRange> {
  let fetch_error = {
    let repo = repository_by_path(&state, &params.repository_path)?;
    repo.review_fetch_calls.fetch_add(1, Ordering::SeqCst);
    repo.review_requests.read().expect("review request fixture lock").fetch_error.clone()
  };
  if let Some(message) = fetch_error {
    return Err(failed("fetch the pull request", message));
  }
  let git_executor = state.git_executor.clone();
  run_command("fetch the pull request", move || {
    let path = params.repository_path.clone();
    let commit = |rev: &str| git_executor.execute_command(&["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{rev}^{{commit}}")], &path);
    if params.head_ref_oid.is_empty() {
      params.head_ref_oid = commit(&format!("refs/heads/{}", params.head_ref_name)).or_else(|_| commit("HEAD"))?;
    }
    if params.base_ref_oid.is_empty() {
      let baseline = history_rewrite::baseline::detect_baseline_branch(&git_executor, &path).and_then(|baseline| commit(&baseline));
      params.base_ref_oid = match baseline {
        Ok(baseline) if baseline != params.head_ref_oid => baseline,
        // a template with fewer commits has a shorter range
        _ => ["HEAD~2", "HEAD~1", "HEAD"].iter().find_map(|rev| commit(rev).ok()).unwrap_or_default(),
      };
    }
    sync_core::review_fetch::fetch_review_head(&git_executor, &params, &mut |_| {}, &|| false)
  })
  .await
}

/// Nothing to kill: the mocked fetch never waits for a network
pub async fn cancel_review_fetch() -> Json<()> {
  Json(())
}

pub async fn prune_review_refs(State(state): State<Arc<AppState>>, Json(params): Json<domain_types::review_request::PruneReviewRefsParams>) -> CommandResult<()> {
  repository_by_path(&state, &params.repository_path)?
    .review_prune_keeps
    .lock()
    .expect("review prune lock")
    .push(params.keep.clone());
  let git_executor = state.git_executor.clone();
  run_command("prune the review refs", move || {
    sync_core::review_fetch::prune_review_refs(&git_executor, &params.repository_path, &params.keep)
  })
  .await
}

pub async fn get_range_files(State(state): State<Arc<AppState>>, Json(params): Json<domain_types::review_request::GetRangeParams>) -> CommandResult<CommitFiles> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the files of a range", move || {
    read_diff_files(
      &git_executor,
      &params.repository_path,
      &DiffSubject::Range {
        base: params.base,
        head: params.head,
      },
    )
  })
  .await
}

/// Nothing to read ahead: the bridge keeps no caches, every read of a review runs git (the E2E stay deterministic)
pub async fn prefetch_branch_review() -> CommandResult<()> {
  Ok(Json(()))
}

/// The text of the pull request of the whole stack and the user of the token, as the app reads them
pub async fn get_new_stack_pull_request(
  State(state): State<Arc<AppState>>,
  Json(params): Json<remote_facts::stack_pull_request::GetNewStackPullRequestParams>,
) -> CommandResult<NewPullRequest> {
  let branch_prefix = branch_prefix_of(&state, &params.repository_path).await?;
  let (git_executor, path, request) = (state.git_executor.clone(), params.repository_path.clone(), params);
  let Json(pull_request) = run_command("read the text of the stack's pull request", move || {
    remote_facts::stack_pull_request::read_new_stack_pull_request(&git_executor, &branch_prefix, &request)
  })
  .await?;
  with_viewer(&state, &path, pull_request).await
}

pub async fn get_branch_review(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::branch_review::GetBranchReviewParams>,
) -> CommandResult<sync_core::branch_review::BranchReview> {
  let branch_prefix = branch_prefix_of(&state, &params.repository_path).await?;
  let git_executor = state.git_executor.clone();
  run_command("read the review of a branch", move || {
    sync_core::branch_review::get_branch_review(&git_executor, &branch_prefix, &params)
  })
  .await
}

pub async fn get_file_owners(State(state): State<Arc<AppState>>, Json(params): Json<git_ops::ownership::GetFileOwnersParams>) -> CommandResult<Vec<git_ops::ownership::FileOwner>> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the owners of files", move || {
    git_ops::ownership::read_file_owners(&git_executor, &params.repository_path, &params.baseline_branch, &params.paths)
  })
  .await
}

/// The fake services of the repository at `path` (`remote_host.rs`), started now if not yet; the repository is not held
pub(crate) fn remote_of(state: &AppState, path: &str) -> Result<Arc<RemoteHost>, Rejection> {
  Ok(repository_by_path(state, path)?.remote(&state.git_executor))
}

/// The Space repository of the repository at `path`, None while no Patronus fixture put it on Space
pub(crate) fn space_of(state: &AppState, path: &str) -> Result<Option<(Arc<RemoteHost>, SpaceRepository)>, Rejection> {
  let host = repository_by_path(state, path)?.existing_remote();
  Ok(host.and_then(|host| host.space_repository().map(|space| (host, space))))
}

/// Keeps the robots of an answer by id, for the tests that pin a revision the fake resolved
pub(crate) fn keep_answered(state: &AppState, path: &str, result: &PatronusRobotsResult) {
  if let Ok(repository) = repository_by_path(state, path) {
    let mut answered = repository.patronus_answered.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    answered.extend(result.robots.iter().map(|robot| (robot.id.clone(), robot.clone())));
  }
}

/// The Safe Push statistics as the app reads them (`get_robot_stats`): the real core against the fake Patronus of the
/// repository, at the page's clock (`PAGE_CLOCK_START`) and in UTC, so the weeks and the hours of a test are fixed; no
/// object cache, so every read asks the fake. The page gets no events: only the result
pub async fn get_robot_stats(State(state): State<Arc<AppState>>, Json(params): Json<GetRobotStatsParams>) -> CommandResult<RobotStatsResult> {
  let Some((host, space)) = space_of(&state, &params.repository_path)? else {
    return Ok(Json(RobotStatsResult {
      availability: PatronusAvailability::NotPatronus,
      stale: false,
      fetched_at: None,
      stats: None,
    }));
  };
  let branch_prefix = branch_prefix_of(&state, &params.repository_path).await?;
  let git = state.git_executor.clone();
  let (path, baseline) = (params.repository_path.clone(), params.baseline_branch.clone());
  let Json(target) = run_command("read the target branch", move || required_target_branch(&git, &path, &baseline)).await?;
  let service = host.patronus_service();
  let cache = ObjectCache::disabled();
  let source = RobotStatsSource {
    service: &service,
    cache: &cache,
    repository: &space.name,
    target: &target,
    branch_prefix: &branch_prefix,
    reads: None,
  };
  Ok(Json(
    robot_stats_core(&source, &Calendar::utc(), PAGE_CLOCK_START, params.period, params.refresh, &|_| {}).await,
  ))
}

/// Why the robot failed, with the commit map of its branch, as the app reads it; there is no `teamcity` CLI
pub async fn get_robot_failure(State(state): State<Arc<AppState>>, Json(params): Json<GetRobotFailureParams>) -> CommandResult<RobotFailureReport> {
  let service = remote_of(&state, &params.repository_path)?.patronus_service();
  let branch_prefix = branch_prefix_of(&state, &params.repository_path).await?;
  let failure = if params.full {
    service.robot_failure_full(&branch_prefix, &params.robot_id, None).await
  } else {
    service.robot_failure(&branch_prefix, &params.robot_id).await
  }
  .map_err(|e| failed("read the failure of the Patronus robot", patronus_error(e)))?;
  let git_executor = state.git_executor.clone();
  run_command("read the failure of the Patronus robot", move || {
    robot_failure_report(&git_executor, &params.repository_path, &branch_prefix, &params.ancestors, &params.baseline_branch, failure)
  })
  .await
}

/// The error of a rejection, a read of the session's facts answers it
pub(crate) fn rejected(rejection: Rejection) -> ApiError {
  match rejection {
    Rejection::Error(error) => error,
    Rejection::Panicked => ApiError::internal("The test server failed"),
  }
}

pub async fn browse_repository(
  State(state): State<Arc<AppState>>,
  axum::extract::Path(repo_id): axum::extract::Path<String>,
) -> Result<Json<sync_core::repository_validation::BrowseResult>, StatusCode> {
  // Get the repository from state
  let repo = state.repositories.get(&repo_id).ok_or_else(|| {
    tracing::error!("Repository not found: {}", repo_id);
    StatusCode::NOT_FOUND
  })?;

  let path = repo.path.clone();

  // Handle NO_REPO case specially - simulate user selecting a non-existent path
  // Return an error result as if the path exists but is not accessible
  if path.starts_with(crate::NO_REPO_PREFIX) {
    return Ok(Json(sync_core::repository_validation::BrowseResult {
      path: Some(path),
      valid: false,
      error: Some("Path does not exist".to_string()),
    }));
  }

  // Use shared validation logic from branch-sync
  Ok(Json(sync_core::repository_validation::validate_and_create_result(path)))
}

/// Mock of `suggest_branch_names` (E2E never runs claude): the repository's fixture after its delay, by default two
/// names from the words of the commit messages
pub async fn suggest_branch_names(State(state): State<Arc<AppState>>, Json(params): Json<SuggestBranchNamesParams>) -> CommandResult<BranchNameSuggestionOutcome> {
  let fixture = {
    let repo = repository_by_path(&state, &params.repository_path)?;
    repo.branch_name_suggestion_calls.fetch_add(1, Ordering::SeqCst);
    repo.branch_name_suggestions.read().expect("branch name suggestion fixture lock").clone()
  };
  tokio::time::sleep(std::time::Duration::from_millis(fixture.delay_ms)).await;
  if let Some(outcome) = fixture.outcome {
    return Ok(Json(outcome));
  }
  let words: Vec<&str> = params
    .commits
    .iter()
    .flat_map(|commit| commit.message.split_whitespace())
    .filter(|word| word.len() > 3)
    .collect();
  let names = clean_names(&[
    format!("fix-{}", words.first().unwrap_or(&"issue")),
    format!("update-{}", words.get(1).unwrap_or(&"feature")),
  ]);
  Ok(Json(BranchNameSuggestionOutcome::Suggested { names }))
}

/// The app's deletion of an archived branch: the core, then the patch of the session of the repository's key
pub async fn delete_archived_branch(State(state): State<Arc<AppState>>, Json(params): Json<DeleteArchivedBranchParams>) -> CommandResult<Option<Version>> {
  delete_kept(&state, params, "delete the archived branch", delete_archived_branch_core, TableState::remove_archived).await
}

/// The deletion of a kept branch by `delete`, then its removal from the table of the session by `remove`, as the app's
/// command does it (`src-tauri/src/commands/archived_branches.rs`)
async fn delete_kept(
  state: &AppState,
  params: DeleteArchivedBranchParams,
  operation: &'static str,
  delete: fn(&git_executor::git_command_executor::GitCommandExecutor, DeleteArchivedBranchParams, &str) -> anyhow::Result<()>,
  remove: fn(&mut TableState, &str),
) -> CommandResult<Option<Version>> {
  let prefix = branch_prefix_of(state, &params.repository_path).await?;
  let key = SessionKey {
    worktree: params.repository_path.clone(),
    prefix: prefix.clone(),
  };
  let (git, name) = (state.git_executor.clone(), params.branch_name.clone());
  let Json(()) = run_command(operation, move || delete(&git, params, &prefix)).await?;
  Ok(Json(state.sessions.apply(&key, move |table| remove(table, &name)).version().await))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetArchivedBranchCommitsRequest {
  repository_path: String,
  branch_name: String,
}

/// The commits of an archived branch, as the app reads them
pub async fn get_archived_branch_commits(
  State(state): State<Arc<AppState>>,
  Json(request): Json<GetArchivedBranchCommitsRequest>,
) -> CommandResult<Vec<domain_types::commit::Commit>> {
  own_commits(
    &state,
    request,
    "read the commits of the archived branch",
    branch_integration::archive::get_archived_branch_commits,
  )
  .await
}

/// The app's deletion of an unapplied branch, as `delete_archived_branch`
pub async fn delete_unapplied_branch(State(state): State<Arc<AppState>>, Json(params): Json<DeleteArchivedBranchParams>) -> CommandResult<Option<Version>> {
  let delete = sync_core::delete_archived_branch::delete_unapplied_branch_core;
  delete_kept(&state, params, "delete the unapplied branch", delete, TableState::remove_unapplied).await
}

/// The commits of an unapplied branch, as the app reads them
pub async fn get_unapplied_branch_commits(
  State(state): State<Arc<AppState>>,
  Json(request): Json<GetArchivedBranchCommitsRequest>,
) -> CommandResult<Vec<domain_types::commit::Commit>> {
  own_commits(
    &state,
    request,
    "read the commits of the unapplied branch",
    branch_integration::archive::get_unapplied_branch_commits,
  )
  .await
}

/// The own commits of the kept branch of `request` by `read`, from the baseline of the repository
async fn own_commits(
  state: &AppState,
  request: GetArchivedBranchCommitsRequest,
  what: &'static str,
  read: fn(&git_executor::git_command_executor::GitCommandExecutor, &str, &str, &str, &str) -> anyhow::Result<Vec<domain_types::commit::Commit>>,
) -> CommandResult<Vec<domain_types::commit::Commit>> {
  let inputs = prefix_inputs(state, &request.repository_path)?;
  run_command(what, move || {
    let branch_prefix = inputs.branch_prefix(&request.repository_path)?;
    let baseline_branch = history_rewrite::baseline::detect_baseline_branch(&inputs.git, &request.repository_path)?;
    read(&inputs.git, &request.repository_path, &branch_prefix, &request.branch_name, &baseline_branch)
  })
  .await
}

pub async fn get_uncommitted_changes(State(state): State<Arc<AppState>>, Json(params): Json<GetUncommittedChangesParams>) -> CommandResult<UncommittedChangesResult> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the uncommitted changes", move || core_get_uncommitted_changes(&git_executor, params)).await
}

// The commit files commands run real git on the fixture repository, without the object cache: an LMDB environment
// per test repository would stay open (and keep its files) for the life of the server

pub async fn get_commit_files(State(state): State<Arc<AppState>>, Json(params): Json<GetCommitFilesParams>) -> CommandResult<CommitFiles> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the files of a commit", move || {
    read_commit_files_cached(&git_executor, &params.repository_path, &ObjectCache::disabled(), std::slice::from_ref(&params.commit_id))?
      .pop()
      .ok_or_else(|| anyhow::anyhow!("No files read for {}", params.commit_id))
  })
  .await
}

pub async fn get_commits_files(State(state): State<Arc<AppState>>, Json(params): Json<GetCommitsFilesParams>) -> CommandResult<Vec<CommitFiles>> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the files of commits", move || {
    read_commit_files_cached(&git_executor, &params.repository_path, &ObjectCache::disabled(), &params.commit_ids)
  })
  .await
}

/// The events of the app's channel as SSE events named `fileDiff`, once all are known; a failed read answers with its
/// error instead. Without the object cache and with fresh memory caches: each read of a test runs git
pub async fn get_file_diffs(State(state): State<Arc<AppState>>, Json(params): Json<GetFileDiffsParams>) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Rejection> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  stream_events("read the file diffs", "fileDiff", move |send: &mut dyn FnMut(FileDiffEvent)| {
    FileDiffs::default().stream(&git_executor, &ObjectCache::disabled(), &params, send)
  })
  .await
}

/// The events of the app's channel as SSE events named `fileMerge`, as `get_file_diffs`
pub async fn get_file_merges(State(state): State<Arc<AppState>>, Json(params): Json<GetFileMergesParams>) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Rejection> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  stream_events("read the file merges", "fileMerge", move |send: &mut dyn FnMut(FileMergeEvent)| {
    FileDiffs::default().stream_merges(&git_executor, &ObjectCache::disabled(), &params, send)
  })
  .await
}

/// Runs `produce` off the async runtime and answers with the events it sent, as SSE events named `event_name`
async fn stream_events<E, F>(operation: &'static str, event_name: &'static str, produce: F) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Rejection>
where
  E: serde::Serialize + std::fmt::Debug + Send + 'static,
  F: FnOnce(&mut dyn FnMut(E)) -> anyhow::Result<()> + Send + 'static,
{
  let Json(events) = run_command(operation, move || {
    let mut events: Vec<E> = Vec::new();
    produce(&mut |event| events.push(event)).map(|()| events)
  })
  .await?;
  let (sender, receiver) = mpsc::unbounded_channel();
  for event in events {
    let _ = sender.send(event);
  }
  Ok(sse_events(receiver, event_name))
}

pub async fn get_blob_texts(State(state): State<Arc<AppState>>, Json(params): Json<GetBlobTextsParams>) -> CommandResult<Vec<BlobText>> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the texts of blobs", move || {
    FileDiffs::default().blob_texts(&git_executor, &params.repository_path, &params.blobs)
  })
  .await
}

/// The highlighting of one blob as the app answers it: raw bytes (`application/octet-stream`), or the error of a command
pub async fn get_blob_syntax(State(state): State<Arc<AppState>>, Json(params): Json<GetBlobSyntaxParams>) -> Result<Response, Rejection> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  let Json(bytes) = run_command("highlight a blob", move || FileDiffs::default().blob_syntax(&git_executor, &params)).await?;
  Ok(([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response())
}

pub async fn find_declaration(State(state): State<Arc<AppState>>, Json(params): Json<FindDeclarationParams>) -> CommandResult<FoundDeclarations> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  // the index of a test repository is its fixture (`.git/branch-deck-accio.json`), as accio answers
  run_command("find a declaration", move || {
    FileDiffs::default().find_declaration(&git_executor, &ObjectCache::disabled(), Some(&file_diffs::accio::FixtureIndex), &params)
  })
  .await
}

pub async fn find_usages(State(state): State<Arc<AppState>>, Json(params): Json<FindDeclarationParams>) -> CommandResult<FoundUsages> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("find the usages", move || {
    FileDiffs::default().find_usages(&git_executor, Some(&file_diffs::accio::FixtureIndex), &params)
  })
  .await
}

/// Nothing to kill: the index of a test repository is its fixture, read at once
pub async fn cancel_symbol_search() -> Json<()> {
  Json(())
}

#[derive(Deserialize)]
pub struct SetAutoSyncMenuCheckedRequest {
  checked: bool,
}

/// The bd the copied prompts name: a fixed path, the app's `agent_cli_command` reads the running binary
pub const AGENT_CLI: &str = "/Users/test/.local/bin/bd";

#[derive(Deserialize)]
pub struct AgentCliCommandRequest {
  #[serde(rename = "repositoryPath")]
  repository_path: String,
}

/// `AgentCliCommand` of the app for a POSIX shell, with the repository path as it is (test paths are plain)
pub async fn agent_cli_command(Json(request): Json<AgentCliCommandRequest>) -> Json<serde_json::Value> {
  Json(serde_json::json!({ "command": format!("{AGENT_CLI} -C {}", request.repository_path), "powershell": false }))
}

/// The skill files the fake install names
const AGENT_SKILLS: [&str; 2] = ["/Users/test/.claude/skills/branch-deck/SKILL.md", "/Users/test/.agents/skills/branch-deck/SKILL.md"];

/// Waits while a test holds `answer` (`/remote-gates`), then fails it while the test fails it: a read of a status the
/// bridge gives itself
pub(crate) async fn held_or_failed(host: &RemoteHost, answer: HeldAnswer, operation: &str) -> Result<(), Rejection> {
  host.pass_held(answer).await;
  if host.fails(answer) {
    return Err(failed(operation, "the fixture fails this read"));
  }
  Ok(())
}

/// The repository of the page that sends a command (`X-Repository-Id`)
pub(crate) fn requesting_repository<'a>(state: &'a AppState, headers: &HeaderMap) -> Result<dashmap::mapref::one::Ref<'a, String, TestRepository>, Rejection> {
  headers
    .get(crate::REPOSITORY_ID_HEADER)
    .and_then(|value| value.to_str().ok())
    .and_then(|id| state.repositories.get(id))
    .ok_or_else(|| failed("find the test repository", "no test repository named by the request"))
}

/// `bd install` without its files: the installed state of the test repository, and the agent rules of its own global
/// config, as the app reads the developer's
pub async fn install_agent_cli(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<serde_json::Value> {
  let path = {
    let repository = requesting_repository(&state, &headers)?;
    repository.agent_cli_installed.store(true, Ordering::SeqCst);
    repository.path.clone()
  };
  let git_executor = config_git(&state, &path)?;
  run_command("install the Agent CLI", move || {
    // the global config alone, as `bd install` reads it
    read_test_repository_config(&git_executor, &path).map(|config| {
      serde_json::json!({
        "bd": AGENT_CLI,
        "on_path": true,
        "skills": AGENT_SKILLS,
        "agents": { "lease": config.agent_lease.fallback_value, "subject_branch": config.agent_subject_branch.fallback_value },
      })
    })
  })
  .await
}

/// The skills once `install_agent_cli` ran for the test repository, else none
pub async fn agent_cli_status(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<serde_json::Value> {
  let installed = requesting_repository(&state, &headers)?.agent_cli_installed.load(Ordering::SeqCst);
  Ok(Json(if installed {
    serde_json::json!({ "bd": AGENT_CLI, "skills": AGENT_SKILLS })
  } else {
    serde_json::json!({ "bd": null, "skills": [] })
  }))
}

/// The page has no native menu: the check is the request, which a test reads (`checked` of its body)
pub async fn set_auto_sync_menu_checked(Json(request): Json<SetAutoSyncMenuCheckedRequest>) -> Json<serde_json::Value> {
  tracing::debug!(checked = request.checked, "Auto-sync on Focus checked");
  Json(serde_json::Value::Null)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSettingRequest {
  pub patch: domain_types::settings::SettingsPatch,
  /// The label of the page's window (the page mock sends it; the app takes the webview's)
  pub origin: String,
}

/// `set_setting` of the app on the settings of the requesting page's repository: the change, which the page mock sends
/// to its windows (`settings_changed`)
pub async fn set_setting(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(request): Json<SetSettingRequest>) -> CommandResult<domain_types::settings::SettingsChange> {
  let repository = requesting_repository(&state, &headers)?;
  let change = repository.store.set(request.patch, &request.origin);
  change
    .map(Json)
    .map_err(|e| failed("change the settings", anyhow::Error::new(e).context("Cannot change the settings")))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSettingsChangesRequest {
  pub since: u32,
  /// The label of the page's window
  pub origin: String,
}

/// `get_settings_changes` of the app: the changes after the version the page started from that its window gets
pub async fn get_settings_changes(
  State(state): State<Arc<AppState>>,
  headers: HeaderMap,
  Json(request): Json<GetSettingsChangesRequest>,
) -> CommandResult<Vec<domain_types::settings::SettingsChange>> {
  let repository = requesting_repository(&state, &headers)?;
  Ok(Json(app_settings::windows::window_changes_since(&repository.store, &request.origin, request.since)))
}

/// `sub_window_loaded` of the app: the changes of the settings after the snapshot the window was built with, which the
/// page mock names (`since`, the version of the snapshot the test opened the page with), as the app answers them from
/// the version its build remembered; none for the main window, which asks for them itself (`get_settings_changes`); no
/// open of the window came while it loaded, a test opens the page itself
pub async fn sub_window_loaded(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(request): Json<GetSettingsChangesRequest>) -> CommandResult<serde_json::Value> {
  let repository = requesting_repository(&state, &headers)?;
  let settings = if request.origin == app_settings::windows::MAIN_WINDOW {
    Vec::new()
  } else {
    app_settings::windows::window_changes_since(&repository.store, &request.origin, request.since)
  };
  Ok(Json(serde_json::json!({ "update": null, "settings": settings })))
}

/// `sub_window_drawn` of the app, which counts the content budget of the window from the shell: this server shows no
/// window; the page mock records what the page had when it said it
pub async fn sub_window_drawn() -> Json<serde_json::Value> {
  Json(serde_json::Value::Null)
}
