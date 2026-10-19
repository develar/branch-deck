use axum::{
  extract::State,
  http::StatusCode,
  response::{
    IntoResponse, Json, Response,
    sse::{Event, KeepAlive, Sse},
  },
};
use file_diffs::model::{BlobText, FileDiffEvent, FileMergeEvent, GetBlobTextsParams, GetFileDiffsParams, GetFileMergesParams};
use file_diffs::service::FileDiffs;
use git_ops::commit_files::{CommitFiles, DiffSubject, GetCommitFilesParams, GetCommitsFilesParams, read_commit_files_cached, read_diff_files};
use object_cache::cache::ObjectCache;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use sync_core::amend_to_branch::{AmendCommandResult, AmendUncommittedToBranchParams, amend_uncommitted_to_branch_core};
use sync_core::branch_name_suggestion::clean_names;
use sync_core::branch_prefix::GitBranchPrefix;
use sync_core::delete_archived_branch::{DeleteArchivedBranchParams, delete_archived_branch_core};
use sync_core::repository_config::{
  ConfigScope, RepositoryConfig, RepositoryConfigChange, detect_github_repository, read_config_entries, repository_config_from_entries, with_stacks_off_clashes,
  write_repository_config,
};
use sync_core::repository_lock::RewriteOptions;
use sync_core::sync::{IssueConfigSource, SyncOptions};
use sync_core::uncommitted_changes::{GetUncommittedChangesParams, UncommittedChangesResult, get_uncommitted_changes as core_get_uncommitted_changes};
use sync_types::branch_name_suggestion::{BranchNameSuggestionOutcome, SuggestBranchNamesParams};
use sync_types::{ProgressReporter, SyncEvent};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::{Stream, StreamExt};

use crate::state::AppState;

use crate::state::TestRepository;

/// How a command fails: with its error, answered the way Tauri rejects a command with a `CommandError`
/// (`{ status: "error", error: { message } }` with status 200, which the page mock hands to the bindings), or with a
/// failure of the server itself (500). An error is the expected answer of an error-path test, not a failure of the
/// server, so it is logged at debug; a panicked command is logged as an error.
pub enum CommandFailure {
  Error(String),
  /// A refusal the page tells apart by its kind, as the app's `CommandErrorKind`
  Refused {
    message: String,
    kind: &'static str,
  },
  Panicked,
}

impl CommandFailure {
  fn error(operation: &str, error: impl std::fmt::Display) -> Self {
    let message = format!("{error:#}");
    tracing::debug!("Cannot {operation}: {message}");
    Self::Error(message)
  }
}

impl IntoResponse for CommandFailure {
  fn into_response(self) -> Response {
    match self {
      Self::Error(message) => Json(serde_json::json!({ "status": "error", "error": { "message": message } })).into_response(),
      Self::Refused { message, kind } => Json(serde_json::json!({ "status": "error", "error": { "message": message, "kind": kind } })).into_response(),
      Self::Panicked => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
  }
}

type CommandResult<T> = Result<Json<T>, CommandFailure>;

/// Runs a command off the async runtime, as the app runs its git work (`spawn_blocking`)
async fn run_command<T, E, F>(operation: &'static str, command: F) -> CommandResult<T>
where
  F: FnOnce() -> Result<T, E> + Send + 'static,
  T: Send + 'static,
  E: std::fmt::Display + Send + 'static,
{
  match tokio::task::spawn_blocking(command).await {
    Ok(Ok(value)) => Ok(Json(value)),
    Ok(Err(error)) => Err(CommandFailure::error(operation, error)),
    Err(join_error) => {
      tracing::error!("Cannot {operation}: the command panicked: {join_error}");
      Err(CommandFailure::Panicked)
    }
  }
}

/// A path that is no test repository (the deliberately missing directory of `NO_REPO`) fails the command, as the app
/// fails it for a directory that is gone
fn ensure_repository_exists(state: &AppState, path: &str) -> Result<(), CommandFailure> {
  repository_by_path(state, path).map(drop)
}

fn repository_by_path<'a>(state: &'a AppState, path: &str) -> Result<dashmap::mapref::one::Ref<'a, String, TestRepository>, CommandFailure> {
  state
    .path_to_id
    .get(path)
    .and_then(|id| state.repositories.get(id.value()))
    .ok_or_else(|| CommandFailure::error("find the test repository", format!("Repository not accessible: {path}")))
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

pub async fn add_issue_reference_to_commits(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::add_issue_reference::AddIssueReferenceParams>,
) -> CommandResult<sync_core::add_issue_reference::AddIssueReferenceResult> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("add the issue reference", move || {
    sync_core::add_issue_reference::add_issue_reference_to_commits_core(&git_executor, params, &RewriteOptions::default())
  })
  .await
}

pub async fn reword_commits(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::reword_commit::RewordCommitsRequest>,
) -> CommandResult<sync_core::reword_commit::RewordCommitsResult> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("reword commits", move || {
    sync_core::reword_commit::reword_commits_core(&git_executor, &params, &RewriteOptions::default())
  })
  .await
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

/// The local `branchdeck.branchPrefix` only, never the user's global config, so the tests are deterministic (a local
/// prefix is the repository's own: `repository_scoped`); an empty path (the global config of the settings) is an empty
/// prefix
pub async fn get_branch_prefix_from_git_config(State(state): State<Arc<AppState>>, Json(request): Json<GetBranchPrefixRequest>) -> CommandResult<GitBranchPrefix> {
  if request.repository_path.is_empty() {
    return Ok(Json(GitBranchPrefix::default()));
  }
  ensure_repository_exists(&state, &request.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the branch prefix", move || {
    let repo_path = &request.repository_path;
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
  })
  .await
}

/// The production read with the local entries only: the user's global config never shows, so the tests are
/// deterministic
fn read_local_repository_config(git_executor: &git_executor::git_command_executor::GitCommandExecutor, repository_path: &str) -> anyhow::Result<RepositoryConfig> {
  let entries: Vec<_> = read_config_entries(git_executor, repository_path)?
    .into_iter()
    .filter(|entry| entry.scope == ConfigScope::Local)
    .collect();
  let config = repository_config_from_entries(&entries, detect_github_repository(git_executor, repository_path)?);
  Ok(with_stacks_off_clashes(git_executor, repository_path, config))
}

pub async fn get_repository_config(State(state): State<Arc<AppState>>, Json(request): Json<GetBranchPrefixRequest>) -> CommandResult<RepositoryConfig> {
  ensure_repository_exists(&state, &request.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the repository settings", move || {
    read_local_repository_config(&git_executor, &request.repository_path)
  })
  .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRepositoryConfigRequest {
  repository_path: String,
  change: RepositoryConfigChange,
}

/// The production write; the answer is the local read of `get_repository_config`
pub async fn set_repository_config(State(state): State<Arc<AppState>>, Json(request): Json<SetRepositoryConfigRequest>) -> CommandResult<RepositoryConfig> {
  ensure_repository_exists(&state, &request.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("change the repository settings", move || {
    write_repository_config(&git_executor, &request.repository_path, &request.change)?;
    read_local_repository_config(&git_executor, &request.repository_path)
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

// Progress reporter that sends events through a channel
#[derive(Clone)]
struct ChannelProgressReporter {
  sender: mpsc::UnboundedSender<SyncEvent>,
}

impl ProgressReporter for ChannelProgressReporter {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    self.sender.send(event).map_err(|e| anyhow::anyhow!("Channel closed: {e}"))?;
    Ok(())
  }
}

#[derive(Deserialize)]
pub struct SyncBranchesRequest {
  #[serde(rename = "repositoryPath")]
  repository_path: String,
  #[serde(rename = "branchPrefix")]
  branch_prefix: String,
}

/// The fingerprint of what a sync reads, as the app's command: a focus syncs only when it changed
pub async fn get_sync_inputs_fingerprint(State(state): State<Arc<AppState>>, Json(request): Json<SyncBranchesRequest>) -> CommandResult<String> {
  ensure_repository_exists(&state, &request.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the sync inputs", move || {
    sync_core::sync_inputs::sync_inputs_fingerprint(&git_executor, &request.repository_path, &request.branch_prefix)
  })
  .await
}

/// Streams the events of the sync once it is done; a failed sync answers with its error instead, as the app's command
/// rejects (the page alert), without the events sent before the failure
pub async fn sync_branches(
  State(state): State<Arc<AppState>>,
  Json(request): Json<SyncBranchesRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, CommandFailure> {
  ensure_repository_exists(&state, &request.repository_path)?;

  // Create a channel for streaming events
  let (tx, rx) = mpsc::unbounded_channel();

  // Create a progress reporter that sends events through the channel
  let reporter = ChannelProgressReporter { sender: tx };

  // Run the sync directly (no need for tokio::spawn since this is already async)
  let git_executor = &state.git_executor;
  let repository_path = &request.repository_path;
  let branch_prefix = &request.branch_prefix;
  let progress = reporter.clone();
  let options = SyncOptions {
    issue_config: IssueConfigSource::Load,
    ..Default::default()
  };
  if let Err(e) = sync_core::sync::sync_branches(git_executor, repository_path, branch_prefix, progress, options).await {
    // a deferred sync, as the app's `CommandError` names it
    let kind = e.downcast_ref::<sync_core::sync::SyncDeferred>().map(|deferred| match deferred {
      sync_core::sync::SyncDeferred::OperationInProgress(_) => "operationInProgress",
      sync_core::sync::SyncDeferred::DetachedHead => "detachedHead",
    });
    return Err(match kind {
      Some(kind) => CommandFailure::Refused { message: e.to_string(), kind },
      None => CommandFailure::error("sync branches", e),
    });
  }

  Ok(sse_events(rx, "sync"))
}

pub async fn move_commits_to_branch(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::move_commits::MoveCommitsParams>,
) -> CommandResult<sync_core::move_commits::MoveCommitsResult> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("move commits", move || sync_core::move_commits::move_commits_to_branch_core(&git_executor, &params)).await
}

pub async fn rename_branch(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::stack_operations::RenameBranchParams>,
) -> CommandResult<sync_core::move_commits::MoveCommitsResult> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("rename branch", move || sync_core::stack_operations::rename_branch(&git_executor, &params)).await
}

pub async fn undo_regroup(State(state): State<Arc<AppState>>, Json(params): Json<sync_core::stack_operations::UndoRegroupParams>) -> CommandResult<()> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("undo regroup", move || sync_core::stack_operations::undo_regroup(&git_executor, &params)).await
}

pub async fn restack_branch(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::stack_operations::RestackBranchParams>,
) -> CommandResult<sync_core::move_commits::MoveCommitsResult> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("restack branch", move || sync_core::stack_operations::restack_branch(&git_executor, &params)).await
}

/// Without the object cache, as every command of the mock server
pub async fn get_stack_independence(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::stack_operations::StackIndependenceParams>,
) -> CommandResult<sync_core::check::StackIndependence> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("check the stack independence", move || {
    sync_core::stack_operations::get_stack_independence(&git_executor, &ObjectCache::disabled(), &params)
  })
  .await
}

/// What the merged robots of the request landed, by the real core on the test repository: a fixture robot's commit is
/// the tip its virtual branch had when the test set it (an empty `sourceRevision`)
pub async fn get_robot_landings(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_types::robot_landing::GetRobotLandingsParams>,
) -> CommandResult<Vec<sync_types::robot_landing::RobotLanding>> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read what the merged robots landed", move || {
    sync_core::robot_landing::robot_landings(&git_executor, &params.repository_path, &params.branch_prefix, &params.baseline_branch, &params.robots)
  })
  .await
}

/// Without the object cache, as every command of the mock server; nothing cancels it
pub async fn find_conflict_fix(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::conflict_fix::FindConflictFixParams>,
) -> CommandResult<sync_core::conflict_fix::ConflictFix> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("find the conflict fix", move || {
    sync_core::conflict_fix::find_conflict_fix_core(&git_executor, &ObjectCache::disabled(), &params, &std::sync::atomic::AtomicBool::new(false))
  })
  .await
}

/// A search of the mock server runs to its end
pub async fn cancel_conflict_fix() -> Json<()> {
  Json(())
}

pub async fn apply_conflict_fix(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::conflict_fix::ApplyConflictFixParams>,
) -> CommandResult<sync_core::conflict_fix::AppliedConflictFix> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("apply the conflict fix", move || sync_core::conflict_fix::apply_conflict_fix_core(&git_executor, &params)).await
}

pub async fn squash_commits(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::squash::SquashCommitsParams>,
) -> CommandResult<sync_core::squash::SquashCommitsResult> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("squash commits", move || {
    sync_core::squash::squash_commits_core(&git_executor, &params, &RewriteOptions::default())
  })
  .await
}

pub async fn restore_branch_head(State(state): State<Arc<AppState>>, Json(params): Json<sync_core::squash::RestoreBranchHeadParams>) -> CommandResult<()> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("restore the branch head", move || sync_core::squash::restore_branch_head_core(&git_executor, &params)).await
}

pub async fn get_autosquash_plan(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::autosquash::GetAutosquashPlanParams>,
) -> CommandResult<sync_core::autosquash::AutosquashPlan> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("get the autosquash plan", move || {
    sync_core::autosquash::get_autosquash_plan_core(&git_executor, &params.repository_path)
  })
  .await
}

pub async fn squash_fixup_commits(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::autosquash::SquashFixupsParams>,
) -> CommandResult<sync_core::autosquash::SquashFixupsResult> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("squash fixup commits", move || {
    sync_core::autosquash::squash_fixup_commits_core(&git_executor, &params, &RewriteOptions::default())
  })
  .await
}

/// The progress channel is not mocked; the fetch reads the upstream from a directory of the template, so the update
/// works offline
pub async fn update_project(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::update_project::UpdateProjectParams>,
) -> CommandResult<sync_core::update_project::UpdateProjectResult> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("update the project", move || {
    sync_core::update_project::update_project_core(&git_executor, &params, &|_| {})
  })
  .await
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

/// The app's stack push, on the same core. Most templates name the repository itself as `origin` (`.`), so a test of a
/// push answers it in the page (`routePushBranches`); the progress stays there too
pub async fn push_branches(State(state): State<Arc<AppState>>, Json(params): Json<sync_core::push::PushBranchesParams>) -> CommandResult<sync_core::push::PushBranchesOutcome> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("push the branches", move || sync_core::push::push_branches(&git_executor, &params, &mut |_| {})).await
}

/// The fixture set by `POST /repositories/{id}/pull-requests`; GitHub is never asked in E2E
pub async fn get_pull_requests(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::pull_requests::GetPullRequestsParams>,
) -> CommandResult<sync_types::pull_request::PullRequestsResult> {
  let mut result = {
    let repo = repository_by_path(&state, &params.repository_path)?;
    repo.pull_request_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    repo.pull_requests.read().expect("pull request fixture lock").clone()
  };
  let git_executor = state.git_executor.clone();
  run_command("list pull requests", move || {
    // the pull request of a stack branch too: its head is the stack branch the sync built
    for pull_request in result.pull_requests.values_mut().chain(result.stack_pull_requests.values_mut()) {
      if pull_request.head_ref_oid.is_empty() {
        let full_ref = format!("refs/heads/{}", pull_request.head_ref_name);
        pull_request.head_ref_oid = git_executor
          .execute_command(&["rev-parse", "--verify", "--quiet", &full_ref], &params.repository_path)
          .unwrap_or_default();
      }
    }
    anyhow::Ok(result)
  })
  .await
}

/// No rule dismisses stale reviews: GitHub is never asked in E2E, a test of the warning answers it in the page
/// (`routeCommand`)
pub async fn get_branch_rules() -> CommandResult<sync_core::branch_rules::BranchRules> {
  Ok(Json(sync_core::branch_rules::BranchRules {
    dismiss_stale_reviews_on_push: false,
  }))
}

/// Not signed in, no token, no sign-in in the build: a test of the sign-in answers it in the page (`routeCommand`)
pub async fn get_github_auth_status() -> CommandResult<sync_types::pull_request::GitHubAuthStatus> {
  Ok(Json(sync_types::pull_request::GitHubAuthStatus {
    sign_in_available: false,
    signed_in_as: None,
    token_source: None,
    manage_url: None,
  }))
}

/// The fixture set by `POST /repositories/{id}/review-requests`; GitHub is never asked in E2E
pub async fn get_review_requests(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_types::review_request::GetReviewRequestsParams>,
) -> CommandResult<sync_types::review_request::ReviewRequestsResult> {
  let repo = repository_by_path(&state, &params.repository_path)?;
  repo.review_request_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
  Ok(Json(repo.review_requests.read().expect("review request fixture lock").result.clone()))
}

/// The range of the test repository without a network: an empty head is the tip of the local branch `headRefName`
/// (else of `HEAD`), an empty base the baseline
/// (`HEAD~2` when the baseline is `HEAD`, or less in a short template), so both commits are local and the real `fetch_review_head` only reads them.
/// The progress channel is not mocked; the fixture's `fetchError` fails it
pub async fn fetch_review_head(
  State(state): State<Arc<AppState>>,
  Json(mut params): Json<sync_types::review_request::FetchReviewHeadParams>,
) -> CommandResult<sync_types::review_request::ReviewRange> {
  let fetch_error = {
    let repo = repository_by_path(&state, &params.repository_path)?;
    repo.review_fetch_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    repo.review_requests.read().expect("review request fixture lock").fetch_error.clone()
  };
  if let Some(message) = fetch_error {
    return Err(CommandFailure::error("fetch the pull request", message));
  }
  let git_executor = state.git_executor.clone();
  run_command("fetch the pull request", move || {
    let path = params.repository_path.clone();
    let commit = |rev: &str| git_executor.execute_command(&["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{rev}^{{commit}}")], &path);
    if params.head_ref_oid.is_empty() {
      params.head_ref_oid = commit(&format!("refs/heads/{}", params.head_ref_name)).or_else(|_| commit("HEAD"))?;
    }
    if params.base_ref_oid.is_empty() {
      let baseline = sync_core::baseline::detect_baseline_branch(&git_executor, &path).and_then(|baseline| commit(&baseline));
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

pub async fn prune_review_refs(State(state): State<Arc<AppState>>, Json(params): Json<sync_types::review_request::PruneReviewRefsParams>) -> CommandResult<()> {
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

pub async fn get_range_files(State(state): State<Arc<AppState>>, Json(params): Json<sync_types::review_request::GetRangeParams>) -> CommandResult<CommitFiles> {
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

/// The text of the pull request of the whole stack, from the real core (`read_new_stack_pull_request`); the viewer is
/// GitHub's, which the test server has not
pub async fn get_new_stack_pull_request(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::stack_pull_request::GetNewStackPullRequestParams>,
) -> CommandResult<sync_types::pull_request::NewPullRequest> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the text of the stack's pull request", move || {
    sync_core::stack_pull_request::read_new_stack_pull_request(&git_executor, &params)
  })
  .await
}

pub async fn get_branch_review(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::branch_review::GetBranchReviewParams>,
) -> CommandResult<sync_core::branch_review::BranchReview> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the review of a branch", move || sync_core::branch_review::get_branch_review(&git_executor, &params)).await
}

pub async fn get_file_owners(State(state): State<Arc<AppState>>, Json(params): Json<git_ops::ownership::GetFileOwnersParams>) -> CommandResult<Vec<git_ops::ownership::FileOwner>> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the owners of files", move || {
    git_ops::ownership::read_file_owners(&git_executor, &params.repository_path, &params.baseline_branch, &params.paths)
  })
  .await
}

/// The fixture set by `POST /repositories/{id}/patronus`; Patronus is never called in E2E
pub async fn get_patronus_robots(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_types::patronus::GetPatronusRobotsParams>,
) -> CommandResult<sync_types::patronus::PatronusRobotsResult> {
  let result = {
    let repo = repository_by_path(&state, &params.repository_path)?;
    repo.patronus_list_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    repo.patronus.read().expect("patronus fixture lock").clone()
  };
  let git_executor = state.git_executor.clone();
  run_command("list Patronus robots", move || {
    anyhow::Ok(resolve_patronus_revisions(&git_executor, &params.repository_path, &params.branch_prefix, result))
  })
  .await
}

/// The fixture robots whose status differs from the one the app knows, as Patronus `fetch-updates` answers
pub async fn poll_patronus_robots(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_types::patronus::PollPatronusRobotsParams>,
) -> CommandResult<sync_types::patronus::PatronusRobotsResult> {
  let mut result = {
    let repo = repository_by_path(&state, &params.repository_path)?;
    repo.patronus_poll_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    repo.patronus.read().expect("patronus fixture lock").clone()
  };
  result
    .robots
    .retain(|robot| params.robots.iter().any(|known| known.id == robot.id && known.status != robot.status));
  let git_executor = state.git_executor.clone();
  run_command("poll Patronus robots", move || {
    anyhow::Ok(resolve_patronus_revisions(&git_executor, &params.repository_path, &params.branch_prefix, result))
  })
  .await
}

/// The robot of the Patronus fixture with its revisions resolved in the test repository
fn find_fixture_robot(
  git_executor: &git_executor::git_command_executor::GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  fixture: sync_types::patronus::PatronusRobotsResult,
  robot_id: &str,
) -> anyhow::Result<sync_types::patronus::PatronusRobot> {
  resolve_patronus_revisions(git_executor, repository_path, branch_prefix, fixture)
    .robots
    .into_iter()
    .find(|robot| robot.id == robot_id)
    .ok_or_else(|| anyhow::anyhow!("No robot {robot_id}"))
}

/// The failure of a fixture robot: one failed check with a TeamCity build, an error problem, and in the full version a
/// failed build with a failed test; the commit map is the real one of the test repository. A test that needs another
/// failure routes the command in the page.
pub async fn get_robot_failure(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_types::robot_failure::GetRobotFailureParams>,
) -> CommandResult<sync_types::robot_failure::RobotFailureReport> {
  use sync_types::robot_failure::{FailedBuild, FailedCheck, FailedTest, OmittedFailures, RobotFailure, RobotFailureKind, RobotFailureProblem};
  let result = {
    let repo = repository_by_path(&state, &params.repository_path)?;
    repo.patronus.read().expect("patronus fixture lock").clone()
  };
  let git_executor = state.git_executor.clone();
  run_command("read the failure of the Patronus robot", move || {
    let robot = find_fixture_robot(&git_executor, &params.repository_path, &params.branch_prefix, result, &params.robot_id)?;
    let running = robot.status == sync_types::patronus::PatronusRobotStatus::Running;
    let build_id = "1095220100".to_string();
    let failure = RobotFailure {
      running,
      test_revision: None,
      target_revision: None,
      kind: RobotFailureKind::Checks,
      checks: vec![FailedCheck {
        name: "Fast Tests".to_string(),
        build_configuration_id: Some("Fast_Aggregator".to_string()),
        attempt: 1,
        attempt_limit: Some(2),
        build_id: build_id.clone(),
        build_url: Some(format!("https://ci.example/build/{build_id}")),
        failed_tests: Some(1),
        failed_builds: Some(1),
        failed_to_start: None,
        running,
      }],
      problems: vec![RobotFailureProblem {
        title: "Fast Tests failed".to_string(),
        subsystem: Some("TEAMCITY_CHECKS".to_string()),
        details: None,
      }],
      builds: params.full.then(|| {
        vec![FailedBuild {
          name: "Unit Tests".to_string(),
          project: Some("Example".to_string()),
          build_id: "1095220101".to_string(),
          build_url: Some("https://ci.example/build/1095220101".to_string()),
          failed_to_start: false,
          problems: Vec::new(),
          failed_tests: vec![FailedTest {
            name: "com.example.AuthTest.testLogin".to_string(),
            url: None,
            message: Some("expected: <true> but was: <false>".to_string()),
            first_failed_before: Some(false),
          }],
          log_tail: None,
        }]
      }),
      omitted: OmittedFailures::default(),
      robot,
    };
    sync_core::robot_commits::robot_failure_report(
      &git_executor,
      &params.repository_path,
      &params.branch_prefix,
      &params.ancestors,
      &params.baseline_branch,
      failure,
    )
  })
  .await
}

/// The fixture robot without checks, problems or profile; a test that needs them routes the command in the page
pub async fn get_patronus_robot_details(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_types::patronus::PatronusRobotParams>,
) -> CommandResult<sync_types::patronus::PatronusRobotDetails> {
  // the repository is not held across the await
  let (result, progress) = {
    let repo = repository_by_path(&state, &params.repository_path)?;
    repo.patronus_detail_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let progress = repo.patronus_progress.read().expect("patronus progress lock").get(&params.robot_id).cloned();
    (repo.patronus.read().expect("patronus fixture lock").clone(), progress)
  };
  let git_executor = state.git_executor.clone();
  run_command("read the Patronus robot", move || {
    let robot = find_fixture_robot(&git_executor, &params.repository_path, &params.branch_prefix, result, &params.robot_id)?;
    // the profile of the fixture robot, as the options of `GET /robots/{id}` name it
    let profile = robot.profile.clone();
    anyhow::Ok(sync_types::patronus::PatronusRobotDetails {
      robot,
      profile_id: profile.as_ref().and_then(|profile| profile.id.clone()),
      profile_name: profile.and_then(|profile| profile.name),
      checks: None,
      problems: Vec::new(),
      failing: false,
      progress,
    })
  })
  .await
}

/// No problems: a test that needs some routes the command in the page
pub async fn get_patronus_robot_problems(Json(_params): Json<sync_types::patronus::PatronusRobotParams>) -> Json<Vec<sync_types::patronus::PatronusProblem>> {
  Json(Vec::new())
}

/// No robot will fail: a test that needs one routes the command in the page
pub async fn get_patronus_failing_robots(Json(_params): Json<sync_types::patronus::PatronusFailingRobotsParams>) -> Json<Vec<String>> {
  Json(Vec::new())
}

/// The readiness of each branch from the answer of the fixture (`POST /repositories/{id}/review-answers`), computed as
/// the app computes it. Without an answer for a branch: no review needed, unless the baseline has `OWNERSHIP` files;
/// then the owners of the files the branch changes (its commits at the tip the page sent, ahead of the baseline) review,
/// one group per owner, none approved, as a Patronus that doesn't evaluate pull requests answers
pub async fn get_branch_review_readiness(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_types::patronus::GetBranchReviewReadinessParams>,
) -> CommandResult<Vec<sync_types::patronus::BranchReviewReadiness>> {
  let fixture = {
    let repo = repository_by_path(&state, &params.repository_path)?;
    let branches: Vec<&str> = params.branches.iter().map(|branch| branch.branch_name.as_str()).collect();
    repo
      .review_readiness_calls
      .lock()
      .expect("review readiness calls lock")
      .push(serde_json::json!({"branches": branches, "recheck": params.recheck}));
    repo.review_answers.read().expect("review answers fixture lock").clone()
  };
  let git_executor = state.git_executor.clone();
  run_command("read the code-owner review", move || {
    let tips: Vec<String> = params.branches.iter().map(|branch| branch.tip.clone()).collect();
    let inputs = sync_core::patronus_review::read_review_inputs(&git_executor, &params.repository_path, &ObjectCache::disabled(), &params.baseline_branch, &tips)?;
    params
      .branches
      .into_iter()
      .zip(inputs)
      .map(|(branch, input)| {
        let requirement = match fixture.answers.get(&branch.branch_name) {
          _ if fixture.unavailable.is_some() => None,
          Some(answer) => Some(answer.clone()),
          None => Some(ownership_requirement(
            &git_executor,
            &params.repository_path,
            &params.baseline_branch,
            &input.map_or_default(|input| input.paths),
          )?),
        };
        let readiness = match &requirement {
          Some(requirement) => {
            let reviewers = matches!(branch.start, sync_types::patronus::ReviewStart::PullRequest { .. }).then_some(branch.reviewers.as_slice());
            patronus::review_readiness::review_readiness(requirement, reviewers, branch.start)
          }
          None => sync_types::patronus::ReviewReadiness::Unavailable {
            reason: sync_types::patronus::UnavailableReadinessReason::Error,
            message: fixture.unavailable.clone().unwrap_or_default(),
          },
        };
        anyhow::Ok(sync_types::patronus::BranchReviewReadiness {
          branch_name: branch.branch_name,
          tip: branch.tip,
          requirement,
          readiness,
        })
      })
      .collect::<anyhow::Result<Vec<_>>>()
  })
  .await
}

/// The owners of `paths` by the `OWNERSHIP` files of the baseline, one group per owner, none approved
fn ownership_requirement(
  git_executor: &git_executor::git_command_executor::GitCommandExecutor,
  repository_path: &str,
  baseline_branch: &str,
  paths: &[String],
) -> anyhow::Result<sync_types::patronus::PatronusReviewRequirement> {
  let owners = git_ops::ownership::read_file_owners(git_executor, repository_path, baseline_branch, paths)?;
  let mut groups: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
  for owner in owners {
    groups.entry(owner.owner).or_default().push(owner.path);
  }
  Ok(sync_types::patronus::PatronusReviewRequirement {
    required: !groups.is_empty(),
    merge_request: None,
    groups: groups
      .into_iter()
      .map(|(name, paths)| sync_types::patronus::PatronusReviewerGroup {
        name,
        reviewers: Vec::new(),
        paths,
        branch_wide: false,
        paths_source: sync_types::patronus::ReviewPathsSource::Ownership,
        required_approvals: None,
        collected_approvals: None,
        approved: None,
        self_approval_allowed: None,
      })
      .collect(),
    skipped: false,
  })
}

/// The target branch the app finds for the baseline, supported, without profiles: a test that needs profiles routes the
/// command in the page
pub async fn get_patronus_start_options(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_types::patronus::GetPatronusStartOptionsParams>,
) -> CommandResult<sync_types::patronus::PatronusStartOptions> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the Safe Push options", move || {
    let baseline = &params.baseline_branch;
    let target_branch = sync_core::patronus::find_target_branch(&git_executor, &params.repository_path, baseline)?
      .ok_or_else(|| anyhow::anyhow!("The baseline {baseline} is not a branch of origin: Patronus pushes into a branch of origin"))?;
    anyhow::Ok(sync_types::patronus::PatronusStartOptions {
      target_branch,
      supported: true,
      profiles: Vec::new(),
      emergency_profiles: Vec::new(),
      default_profile_id: None,
    })
  })
  .await
}

/// An empty `sourceRevision` of a fixture robot is the local tip of its virtual branch, or of the stack branch of a
/// robot of the whole stack (`stackRoot`)
fn resolve_patronus_revisions(
  git_executor: &git_executor::git_command_executor::GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  mut result: sync_types::patronus::PatronusRobotsResult,
) -> sync_types::patronus::PatronusRobotsResult {
  for robot in &mut result.robots {
    if robot.source_revision.as_deref() != Some("") {
      continue;
    }
    let full_ref = match (robot.stack_root.as_deref(), robot.branch_name.as_deref()) {
      (Some(root), _) => format!("refs/heads/{branch_prefix}/stack/{root}"),
      (None, Some(branch_name)) => format!("refs/heads/{branch_prefix}/virtual/{branch_name}"),
      (None, None) => continue,
    };
    robot.source_revision = git_executor.execute_command(&["rev-parse", "--verify", "--quiet", &full_ref], repository_path).ok();
  }
  result
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
  if path.starts_with("NO_REPO_") {
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
    repo.branch_name_suggestion_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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

pub async fn delete_archived_branch(State(state): State<Arc<AppState>>, Json(params): Json<DeleteArchivedBranchParams>) -> CommandResult<()> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("delete the archived branch", move || delete_archived_branch_core(&git_executor, params)).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetArchivedBranchCommitsRequest {
  repository_path: String,
  branch_prefix: String,
  branch_name: String,
}

/// The commits of an archived branch, as the app reads them
pub async fn get_archived_branch_commits(
  State(state): State<Arc<AppState>>,
  Json(request): Json<GetArchivedBranchCommitsRequest>,
) -> CommandResult<Vec<git_ops::commit_list::Commit>> {
  ensure_repository_exists(&state, &request.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("read the commits of the archived branch", move || {
    let baseline_branch = sync_core::baseline::detect_baseline_branch(&git_executor, &request.repository_path)?;
    branch_integration::archive::get_archived_branch_commits(&git_executor, &request.repository_path, &request.branch_prefix, &request.branch_name, &baseline_branch)
  })
  .await
}

pub async fn amend_uncommitted_to_branch(State(state): State<Arc<AppState>>, Json(params): Json<AmendUncommittedToBranchParams>) -> CommandResult<AmendCommandResult> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_command("amend the uncommitted changes", move || {
    amend_uncommitted_to_branch_core(&git_executor, params, &RewriteOptions::default())
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
pub async fn get_file_diffs(
  State(state): State<Arc<AppState>>,
  Json(params): Json<GetFileDiffsParams>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, CommandFailure> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  stream_events("read the file diffs", "fileDiff", move |send: &mut dyn FnMut(FileDiffEvent)| {
    FileDiffs::default().stream(&git_executor, &ObjectCache::disabled(), &params, send)
  })
  .await
}

/// The events of the app's channel as SSE events named `fileMerge`, as `get_file_diffs`
pub async fn get_file_merges(
  State(state): State<Arc<AppState>>,
  Json(params): Json<GetFileMergesParams>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, CommandFailure> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  stream_events("read the file merges", "fileMerge", move |send: &mut dyn FnMut(FileMergeEvent)| {
    FileDiffs::default().stream_merges(&git_executor, &ObjectCache::disabled(), &params, send)
  })
  .await
}

/// Runs `produce` off the async runtime and answers with the events it sent, as SSE events named `event_name`
async fn stream_events<E, F>(operation: &'static str, event_name: &'static str, produce: F) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, CommandFailure>
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

/// The page has no native menu: the check is the request, which a test reads (`checked` of its body)
pub async fn set_auto_sync_menu_checked(Json(request): Json<SetAutoSyncMenuCheckedRequest>) -> Json<serde_json::Value> {
  tracing::debug!(checked = request.checked, "Auto-sync on Focus checked");
  Json(serde_json::Value::Null)
}
