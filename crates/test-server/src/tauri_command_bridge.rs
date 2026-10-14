use axum::{
  extract::State,
  http::{HeaderMap, StatusCode},
  response::{
    Json,
    sse::{Event, KeepAlive, Sse},
  },
};
use futures::stream::{Stream, StreamExt};
use git_ops::commit_files::{CommitFiles, GetCommitFileDiffParams, GetCommitFilesParams, GetCommitsFilesParams, read_commit_file_diff, read_commit_files_cached};
use git_ops::conflict_analysis::FileDiff;
use git_ops::model::{BranchError, BranchSyncStatus};
use model_types::{BranchSuggestion, DownloadProgress, SuggestBranchNameParams, SuggestionProgress};
use object_cache::cache::ObjectCache;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};
use sync_core::amend_to_branch::{AmendCommandResult, AmendUncommittedToBranchParams, amend_uncommitted_to_branch_core};
use sync_core::delete_archived_branch::{DeleteArchivedBranchParams, delete_archived_branch_core};
use sync_core::sync::{IssueConfigSource, SyncOptions};
use sync_core::uncommitted_changes::{GetUncommittedChangesParams, UncommittedChangesResult, get_uncommitted_changes as core_get_uncommitted_changes};
use sync_types::{ProgressReporter, SyncEvent};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::state::AppState;

// Helper function to find a repository by its path
fn find_repository_by_path(state: &AppState, path: &str) -> Option<String> {
  state.path_to_id.get(path).map(|entry| entry.value().clone())
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

/// Runs blocking work off the async runtime; a failed task or closure is logged and answered with 500
async fn run_blocking<T, E, F>(what: &'static str, f: F) -> Result<T, StatusCode>
where
  F: FnOnce() -> Result<T, E> + Send + 'static,
  T: Send + 'static,
  E: std::fmt::Display + Send + 'static,
{
  match tokio::task::spawn_blocking(f).await {
    Ok(Ok(result)) => Ok(result),
    Ok(Err(e)) => {
      tracing::error!("{what}: {e}");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
    Err(join_error) => {
      tracing::error!("{what}: task failed: {join_error}");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

// Helper function to ensure a repository exists by path
fn ensure_repository_exists(state: &AppState, path: &str) -> Result<(), StatusCode> {
  let repo_id = find_repository_by_path(state, path);
  if repo_id.is_none() {
    tracing::warn!("Repository not found for path: {}", path);
    return Err(StatusCode::NOT_FOUND);
  }
  Ok(())
}

pub async fn add_issue_reference_to_commits(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::add_issue_reference::AddIssueReferenceParams>,
) -> Result<Json<sync_core::add_issue_reference::AddIssueReferenceResult>, StatusCode> {
  // Validate that the repository path belongs to a test repository
  ensure_repository_exists(&state, &params.repository_path)?;

  // Use the shared git executor from state
  match sync_core::add_issue_reference::add_issue_reference_to_commits_core(&state.git_executor, params) {
    Ok(result) => Ok(Json(result)),
    Err(e) => {
      tracing::error!("Failed to add issue reference: {}", e);
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// Fails with the error message as the body, which the page mock shows like a Tauri `CommandError`
pub async fn reword_commits(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::reword_commit::RewordCommitsRequest>,
) -> Result<Json<sync_core::reword_commit::RewordCommitsResult>, (StatusCode, String)> {
  ensure_repository_exists(&state, &params.repository_path).map_err(|status| (status, String::new()))?;

  sync_core::reword_commit::reword_commits_core(&state.git_executor, &params).map(Json).map_err(|e| {
    tracing::error!("Failed to reword commits: {:#}", e);
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))
  })
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

pub async fn get_branch_prefix_from_git_config(State(state): State<Arc<AppState>>, Json(request): Json<GetBranchPrefixRequest>) -> Json<serde_json::Value> {
  // Handle NO_REPO case - return error result to simulate non-existent directory
  // This matches how Tauri commands work: they return Result<String, CommandError> not HTTP status codes
  if request.repository_path.starts_with("NO_REPO_") {
    let error_response = serde_json::json!({
      "status": "error",
      "error": { "message": format!("Repository not accessible: {}", request.repository_path) }
    });
    return Json(error_response);
  }

  // Handle empty path - return success with empty string (for global config fallback)
  if request.repository_path.is_empty() {
    tracing::debug!("Empty path detected, returning empty branch prefix for global config");
    return Json(serde_json::json!({
      "status": "ok",
      "data": ""
    }));
  }

  // Log all known paths for debugging
  {
    let keys: Vec<_> = state.path_to_id.iter().map(|entry| entry.key().clone()).collect();
    tracing::debug!("Known repository paths: {:?}", keys);
  }

  // Validate that the repository path belongs to a test repository
  if ensure_repository_exists(&state, &request.repository_path).is_err() {
    let error_response = serde_json::json!({
      "status": "error",
      "error": { "message": format!("Repository not found: {}", request.repository_path) }
    });
    return Json(error_response);
  }

  // For deterministic tests, ignore user's global git config.
  // Only read the local repo config for branchdeck.branchPrefix.
  // If not set locally, return empty string instead of falling back to global.
  let repo_path = &request.repository_path;
  match state.git_executor.execute_command_with_status(&["config", "--local", "branchdeck.branchPrefix"], repo_path) {
    Ok((output, exit_code)) => match exit_code {
      0 => Json(serde_json::json!({ "status": "ok", "data": output.trim() })),
      1 => Json(serde_json::json!({ "status": "ok", "data": "" })),
      128 => Json(serde_json::json!({ "status": "error", "error": { "message": format!("Repository not accessible: {}", repo_path) } })),
      code => {
        tracing::warn!(code, repo_path, "Unexpected exit code reading local branch prefix");
        Json(serde_json::json!({ "status": "ok", "data": "" }))
      }
    },
    Err(e) => {
      tracing::error!(error = %e, repo_path, "Failed to execute git config --local");
      Json(serde_json::json!({ "status": "error", "error": { "message": format!("Failed to access repository {}: {}", repo_path, e) } }))
    }
  }
}

// Progress reporter that sends events through a channel
#[derive(Clone)]
struct ChannelProgressReporter {
  sender: mpsc::UnboundedSender<SyncEvent>,
}

impl ProgressReporter for ChannelProgressReporter {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    self.sender.send(event).map_err(|_| anyhow::anyhow!("Channel closed"))?;
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

pub async fn sync_branches(State(state): State<Arc<AppState>>, Json(request): Json<SyncBranchesRequest>) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
  // Validate that the repository path belongs to a test repository
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
  match sync_core::sync::sync_branches(git_executor, repository_path, branch_prefix, progress, options).await {
    Ok(()) => {
      // Core function completed successfully
    }
    Err(e) => {
      tracing::error!("Sync branches failed: {}", e);
      // Send error as a branch status event so the client knows what happened
      let _ = reporter.send(SyncEvent::BranchStatusUpdate {
        branch_name: String::from("sync"),
        status: BranchSyncStatus::Error,
        error: Some(BranchError::Generic(format!("Sync failed: {e}"))),
      });
    }
  }

  Ok(sse_events(rx, "sync"))
}

/// Fails with the error message as the body, which the page mock shows like a Tauri `CommandError`
pub async fn move_commits_to_branch(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::move_commits::MoveCommitsParams>,
) -> Result<Json<sync_core::move_commits::MoveCommitsResult>, (StatusCode, String)> {
  ensure_repository_exists(&state, &params.repository_path).map_err(|status| (status, String::new()))?;

  sync_core::move_commits::move_commits_to_branch_core(&state.git_executor, &params)
    .map(Json)
    .map_err(|e| command_failed("move commits", &e))
}

/// Fails with the error message as the body, see `move_commits_to_branch`
pub async fn rename_branch(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::stack_operations::RenameBranchParams>,
) -> Result<Json<sync_core::move_commits::MoveCommitsResult>, (StatusCode, String)> {
  ensure_repository_exists(&state, &params.repository_path).map_err(|status| (status, String::new()))?;

  sync_core::stack_operations::rename_branch(&state.git_executor, &params)
    .map(Json)
    .map_err(|e| command_failed("rename branch", &e))
}

/// Fails with the error message as the body, see `move_commits_to_branch`
pub async fn undo_issue_groups(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::stack_operations::UndoIssueGroupsParams>,
) -> Result<Json<()>, (StatusCode, String)> {
  ensure_repository_exists(&state, &params.repository_path).map_err(|status| (status, String::new()))?;

  sync_core::stack_operations::undo_issue_groups(&state.git_executor, &params)
    .map(Json)
    .map_err(|e| command_failed("undo issue groups", &e))
}

/// Fails with the error message as the body, see `move_commits_to_branch`
pub async fn restack_branch(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::stack_operations::RestackBranchParams>,
) -> Result<Json<sync_core::move_commits::MoveCommitsResult>, (StatusCode, String)> {
  ensure_repository_exists(&state, &params.repository_path).map_err(|status| (status, String::new()))?;

  sync_core::stack_operations::restack_branch(&state.git_executor, &params)
    .map(Json)
    .map_err(|e| command_failed("restack branch", &e))
}

/// Fails with the error message as the body, see `move_commits_to_branch`
pub async fn squash_commits(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::squash::SquashCommitsParams>,
) -> Result<Json<sync_core::squash::SquashCommitsResult>, (StatusCode, String)> {
  ensure_repository_exists(&state, &params.repository_path).map_err(|status| (status, String::new()))?;
  sync_core::squash::squash_commits_core(&state.git_executor, &params)
    .map(Json)
    .map_err(|e| command_failed("squash commits", &e))
}

pub async fn restore_branch_head(State(state): State<Arc<AppState>>, Json(params): Json<sync_core::squash::RestoreBranchHeadParams>) -> Result<Json<()>, (StatusCode, String)> {
  ensure_repository_exists(&state, &params.repository_path).map_err(|status| (status, String::new()))?;
  sync_core::squash::restore_branch_head_core(&state.git_executor, &params)
    .map(Json)
    .map_err(|e| command_failed("restore the branch head", &e))
}

pub async fn get_autosquash_plan(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::autosquash::GetAutosquashPlanParams>,
) -> Result<Json<sync_core::autosquash::AutosquashPlan>, (StatusCode, String)> {
  ensure_repository_exists(&state, &params.repository_path).map_err(|status| (status, String::new()))?;
  sync_core::autosquash::get_autosquash_plan_core(&state.git_executor, &params.repository_path)
    .map(Json)
    .map_err(|e| command_failed("get the autosquash plan", &e))
}

/// Fails with the error message as the body, see `move_commits_to_branch`
pub async fn squash_fixup_commits(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::autosquash::SquashFixupsParams>,
) -> Result<Json<sync_core::autosquash::SquashFixupsResult>, (StatusCode, String)> {
  ensure_repository_exists(&state, &params.repository_path).map_err(|status| (status, String::new()))?;
  sync_core::autosquash::squash_fixup_commits_core(&state.git_executor, &params)
    .map(Json)
    .map_err(|e| command_failed("squash fixup commits", &e))
}

/// Fails with the error message as the body, see `move_commits_to_branch`. The progress channel is not mocked; the
/// fetch reads the upstream from a directory of the template, so the update works offline.
pub async fn update_project(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::update_project::UpdateProjectParams>,
) -> Result<Json<sync_core::update_project::UpdateProjectResult>, (StatusCode, String)> {
  ensure_repository_exists(&state, &params.repository_path).map_err(|status| (status, String::new()))?;
  sync_core::update_project::update_project_core(&state.git_executor, &params, &|_| {})
    .map(Json)
    .map_err(|e| command_failed("update the project", &e))
}

#[derive(Deserialize)]
pub struct DismissLandedRequest {
  #[serde(rename = "repositoryPath")]
  repository_path: String,
}

/// Clears the report of what landed; see `update_project`
pub async fn dismiss_landed(State(state): State<Arc<AppState>>, Json(request): Json<DismissLandedRequest>) -> Result<Json<()>, (StatusCode, String)> {
  ensure_repository_exists(&state, &request.repository_path).map_err(|status| (status, String::new()))?;
  sync_core::landed::dismiss_landed(&state.git_executor, &request.repository_path)
    .map(Json)
    .map_err(|e| command_failed("dismiss the landed commits", &e))
}

/// Fails with the error message as the body, see `update_project`; the progress channel is not mocked
pub async fn fetch_baseline(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::baseline_fetch::FetchBaselineParams>,
) -> Result<Json<sync_core::baseline_fetch::FetchBaselineResult>, (StatusCode, String)> {
  ensure_repository_exists(&state, &params.repository_path).map_err(|status| (status, String::new()))?;
  sync_core::baseline_fetch::fetch_baseline_core(&state.git_executor, &params, &mut |_| {})
    .map(Json)
    .map_err(|e| command_failed("fetch the baseline", &e))
}

/// The fixture set by `POST /repositories/{id}/pull-requests`; gh never runs in E2E
pub async fn get_pull_requests(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::pull_requests::GetPullRequestsParams>,
) -> Result<Json<sync_types::pull_request::PullRequestsResult>, StatusCode> {
  let id = find_repository_by_path(&state, &params.repository_path).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  repo.pull_request_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
  let mut result = repo.pull_requests.read().expect("pull request fixture lock").clone();
  drop(repo);
  for pull_request in result.pull_requests.values_mut() {
    if pull_request.head_ref_oid.is_empty() {
      let full_ref = format!("refs/heads/{}", pull_request.head_ref_name);
      pull_request.head_ref_oid = state
        .git_executor
        .execute_command(&["rev-parse", "--verify", "--quiet", &full_ref], &params.repository_path)
        .unwrap_or_default();
    }
  }
  Ok(Json(result))
}

/// The fixture set by `POST /repositories/{id}/patronus`; Patronus is never called in E2E
pub async fn get_patronus_robots(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_types::patronus::GetPatronusRobotsParams>,
) -> Result<Json<sync_types::patronus::PatronusRobotsResult>, StatusCode> {
  let id = find_repository_by_path(&state, &params.repository_path).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  repo.patronus_list_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
  let result = repo.patronus.read().expect("patronus fixture lock").clone();
  drop(repo);
  Ok(Json(resolve_patronus_revisions(&state, &params.repository_path, &params.branch_prefix, result)))
}

/// The fixture robots whose status differs from the one the app knows, as Patronus `fetch-updates` answers
pub async fn poll_patronus_robots(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_types::patronus::PollPatronusRobotsParams>,
) -> Result<Json<sync_types::patronus::PatronusRobotsResult>, StatusCode> {
  let id = find_repository_by_path(&state, &params.repository_path).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  repo.patronus_poll_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
  let mut result = repo.patronus.read().expect("patronus fixture lock").clone();
  drop(repo);
  result
    .robots
    .retain(|robot| params.robots.iter().any(|known| known.id == robot.id && known.status != robot.status));
  Ok(Json(resolve_patronus_revisions(&state, &params.repository_path, &params.branch_prefix, result)))
}

/// The fixture robot without checks, problems or profile; a test that needs them routes the command in the page
pub async fn get_patronus_robot_details(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_types::patronus::PatronusRobotParams>,
) -> Result<Json<sync_types::patronus::PatronusRobotDetails>, (StatusCode, String)> {
  let id = find_repository_by_path(&state, &params.repository_path).ok_or((StatusCode::UNPROCESSABLE_ENTITY, String::new()))?;
  let repo = state.repositories.get(&id).ok_or((StatusCode::UNPROCESSABLE_ENTITY, String::new()))?;
  let result = repo.patronus.read().expect("patronus fixture lock").clone();
  drop(repo);
  let result = resolve_patronus_revisions(&state, &params.repository_path, &params.branch_prefix, result);
  let robot = result
    .robots
    .into_iter()
    .find(|robot| robot.id == params.robot_id)
    .ok_or((StatusCode::UNPROCESSABLE_ENTITY, format!("No robot {}", params.robot_id)))?;
  Ok(Json(sync_types::patronus::PatronusRobotDetails {
    robot,
    profile_id: None,
    profile_name: None,
    checks: None,
    problems: Vec::new(),
  }))
}

/// No problems: a test that needs some routes the command in the page
pub async fn get_patronus_robot_problems(Json(_params): Json<sync_types::patronus::PatronusRobotParams>) -> Json<Vec<sync_types::patronus::PatronusProblem>> {
  Json(Vec::new())
}

/// No review needed: a test that needs one routes the command in the page
pub async fn get_patronus_review_requirement(Json(_params): Json<sync_types::patronus::PatronusBranchParams>) -> Json<sync_types::patronus::PatronusReviewRequirement> {
  Json(sync_types::patronus::PatronusReviewRequirement {
    required: false,
    merge_request: None,
    groups: Vec::new(),
    skipped: false,
  })
}

/// An empty `sourceRevision` of a fixture robot is the local tip of its virtual branch
fn resolve_patronus_revisions(
  state: &AppState,
  repository_path: &str,
  branch_prefix: &str,
  mut result: sync_types::patronus::PatronusRobotsResult,
) -> sync_types::patronus::PatronusRobotsResult {
  for robot in &mut result.robots {
    if let (Some(""), Some(branch_name)) = (robot.source_revision.as_deref(), robot.branch_name.as_deref()) {
      let full_ref = format!("refs/heads/{branch_prefix}/virtual/{branch_name}");
      robot.source_revision = state.git_executor.execute_command(&["rev-parse", "--verify", "--quiet", &full_ref], repository_path).ok();
    }
  }
  result
}

fn command_failed(operation: &str, e: &anyhow::Error) -> (StatusCode, String) {
  tracing::error!("Failed to {operation}: {e:#}");
  (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))
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
      path: Some(path.clone()),
      valid: false,
      error: Some("Path does not exist".to_string()),
    }));
  }

  // Use shared validation logic from branch-sync
  Ok(Json(sync_core::repository_validation::validate_and_create_result(path)))
}

// AI Command Handlers

// Types are imported from model_types

/// Mock of `check_model_status` (the AI help popover shows the model name and its Hugging Face link)
pub async fn check_model_status(State(state): State<Arc<AppState>>, axum::extract::Path(repo_id): axum::extract::Path<String>) -> Result<Json<serde_json::Value>, StatusCode> {
  let Some(repo) = state.repositories.get(&repo_id) else {
    tracing::debug!(repo_id, "Repository not found for check_model_status");
    return Err(StatusCode::NOT_FOUND);
  };
  let available = matches!(*repo.model_state.read().unwrap(), crate::state::ModelState::Downloaded);
  Ok(Json(serde_json::json!({
    "available": available,
    "modelName": "Qwen3-4B-Instruct-2507",
    "modelSize": "2.5 GB",
    "modelUrl": "https://huggingface.co/Qwen/Qwen3-4B-Instruct-2507",
    "filesPresent": { "model": available, "tokenizer": available },
  })))
}

pub async fn suggest_branch_name_stream(
  State(state): State<Arc<AppState>>,
  _headers: HeaderMap,
  Json(params): Json<SuggestBranchNameParams>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
  // Single repository lookup
  let repo_id = find_repository_by_path(&state, &params.repository_path).ok_or(StatusCode::NOT_FOUND)?;

  let repo = state.repositories.get(&repo_id).ok_or(StatusCode::NOT_FOUND)?;

  // Get model state from repository
  let model_state_lock = repo.model_state.clone();
  let model_state = model_state_lock.read().unwrap().clone();

  // Create channel for streaming
  let (tx, rx) = mpsc::unbounded_channel();

  // Spawn task to generate suggestions
  tokio::spawn(async move {
    match model_state {
      crate::state::ModelState::NotDownloaded => {
        // Send download required event
        let _ = tx.send(SuggestionProgress::ModelDownloadInProgress {
          model_name: "Qwen3-4B-Instruct-2507".to_string(),
          model_size: "2.5 GB".to_string(),
        });
      }
      crate::state::ModelState::Downloaded => {
        // Send suggestions
        let _ = tx.send(SuggestionProgress::Started { total: 2 });

        // Simulate some processing time
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Generate mock suggestions based on commit messages
        let commit_keywords: Vec<&str> = params.commits.iter().flat_map(|c| c.message.split_whitespace()).filter(|w| w.len() > 3).collect();

        let suggestions = vec![
          BranchSuggestion {
            name: format!("{}fix-{}", params.branch_prefix, commit_keywords.first().unwrap_or(&"issue").to_lowercase()),
            reason: Some("Based on commit message keywords".to_string()),
          },
          BranchSuggestion {
            name: format!("{}update-{}", params.branch_prefix, commit_keywords.get(1).unwrap_or(&"feature").to_lowercase()),
            reason: Some("Alternative suggestion".to_string()),
          },
        ];

        // Send suggestions
        for (index, suggestion) in suggestions.into_iter().enumerate() {
          tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
          let _ = tx.send(SuggestionProgress::SuggestionReady {
            suggestion,
            index: u32::try_from(index).unwrap_or(u32::MAX),
          });
        }

        // Complete
        let _ = tx.send(SuggestionProgress::Completed);
      }
      crate::state::ModelState::Downloading => {
        // Model is currently downloading - return error
        let _ = tx.send(SuggestionProgress::Error {
          message: "Model is currently downloading".to_string(),
        });
      }
    }
  });

  Ok(sse_events(rx, "suggestion"))
}

pub async fn download_model(
  State(state): State<Arc<AppState>>,
  axum::extract::Path(repo_id): axum::extract::Path<String>,
  _headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
  // Get repository and update model state to Downloading
  let (simulate_slow_download, model_state_lock, download_cancelled) = if let Some(repo_entry) = state.repositories.get(&repo_id) {
    // Reset cancellation flag for new download
    repo_entry.download_cancelled.store(false, std::sync::atomic::Ordering::SeqCst);

    // Update model state to Downloading
    if let Ok(mut model_state) = repo_entry.model_state.write() {
      *model_state = crate::state::ModelState::Downloading;
    }

    let slow = if let Some(settings_value) = repo_entry.store.get("ai") {
      if let Some(settings_obj) = settings_value.as_object() {
        let slow = settings_obj.get("simulateSlowDownload").and_then(serde_json::Value::as_bool).unwrap_or(false);
        tracing::debug!(slow, "simulateSlowDownload setting found");
        slow
      } else {
        tracing::debug!("ai is not an object");
        false
      }
    } else {
      tracing::debug!("ai not found in store");
      false
    };

    (slow, repo_entry.model_state.clone(), repo_entry.download_cancelled.clone())
  } else {
    tracing::debug!(repo_id, "Repository not found");
    return Err(StatusCode::NOT_FOUND);
  };

  // Create channel for streaming
  let (tx, rx) = mpsc::unbounded_channel();

  // Send the Started event immediately to establish the SSE connection
  // This prevents ERR_EMPTY_RESPONSE if the spawned task doesn't run immediately
  let _ = tx.send(DownloadProgress::Started { total_files: 2 });
  tracing::debug!("Sent Started event immediately");

  // Clone repo_id for the spawned task
  let repo_id_clone = repo_id.clone();

  // Spawn download simulation for the rest of the events
  tokio::spawn(async move {
    tracing::debug!(repo_id = repo_id_clone, "Starting download simulation");

    // Small delay after Started event
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Simulate downloading the files of the real model (`ModelConfig::files`)
    let files = vec![("Qwen3-4B-Instruct-2507-Q4_K_M.gguf", 2_497_281_120), ("tokenizer.json", 11_422_654)];

    for (file_name, file_size) in files {
      if is_download_cancelled(&download_cancelled, &model_state_lock, &tx) {
        return;
      }

      // File started
      let _ = tx.send(DownloadProgress::FileStarted {
        file_name: file_name.to_string(),
        file_size: Some(file_size),
      });

      // Simulate download progress
      let chunks = if simulate_slow_download { 300 } else { 10 }; // 300 chunks for slow mode
      let chunk_size = file_size / chunks;
      let delay_ms: u64 = if simulate_slow_download { 1000 } else { 100 }; // 1 second vs 100ms per chunk

      tracing::debug!(
        file_name,
        chunks,
        delay_ms,
        total_time_s = (u64::from(chunks) * delay_ms) / 1000,
        "Download simulation parameters"
      );

      for i in 1..=chunks {
        if is_download_cancelled(&download_cancelled, &model_state_lock, &tx) {
          return;
        }

        let downloaded = chunk_size * i;

        let _ = tx.send(DownloadProgress::Progress {
          file_name: file_name.to_string(),
          downloaded,
          total: file_size,
          bytes_per_second: Some(if simulate_slow_download { 400 * 1024 } else { 10 * 1024 * 1024 }), // 400 KB/s vs 10 MB/s
          seconds_remaining: Some((file_size - downloaded) / if simulate_slow_download { 400 * 1024 } else { 10 * 1024 * 1024 }),
        });

        if i % 50 == 0 {
          tracing::debug!(file_name, chunk = i, total_chunks = chunks, "Download progress");
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
      }

      // File completed
      let _ = tx.send(DownloadProgress::FileCompleted { file_name: file_name.to_string() });
    }

    // All completed - update model state to Downloaded
    if let Ok(mut model_state) = model_state_lock.write() {
      *model_state = crate::state::ModelState::Downloaded;
    }
    let _ = tx.send(DownloadProgress::Completed);
  });

  tracing::debug!("Returning SSE response for download_model");
  Ok(sse_events(rx, "download"))
}

/// When the download was cancelled, resets the model state to `NotDownloaded` and sends the `Cancelled` event
fn is_download_cancelled(download_cancelled: &AtomicBool, model_state_lock: &RwLock<crate::state::ModelState>, tx: &mpsc::UnboundedSender<DownloadProgress>) -> bool {
  if !download_cancelled.load(std::sync::atomic::Ordering::SeqCst) {
    return false;
  }
  if let Ok(mut model_state) = model_state_lock.write() {
    *model_state = crate::state::ModelState::NotDownloaded;
  }
  let _ = tx.send(DownloadProgress::Cancelled);
  true
}

pub async fn cancel_model_download(
  State(state): State<Arc<AppState>>,
  axum::extract::Path(repo_id): axum::extract::Path<String>,
  _headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
  if let Some(repo) = state.repositories.get(&repo_id) {
    repo.download_cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
    tracing::debug!(repo_id, "Download cancelled for repository");
    Ok(StatusCode::OK)
  } else {
    tracing::debug!(repo_id, "Repository not found for cancel");
    Err(StatusCode::NOT_FOUND)
  }
}

pub async fn delete_archived_branch(State(state): State<Arc<AppState>>, Json(params): Json<DeleteArchivedBranchParams>) -> Result<StatusCode, StatusCode> {
  // Validate that the repository path belongs to a test repository
  ensure_repository_exists(&state, &params.repository_path)?;

  // Use the shared git executor from state
  match delete_archived_branch_core(&state.git_executor, params) {
    Ok(()) => Ok(StatusCode::OK),
    Err(e) => {
      tracing::error!("Failed to delete archived branch: {}", e);
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

pub async fn amend_uncommitted_to_branch(State(state): State<Arc<AppState>>, Json(params): Json<AmendUncommittedToBranchParams>) -> Result<Json<AmendCommandResult>, StatusCode> {
  // Validate that the repository path belongs to a test repository
  ensure_repository_exists(&state, &params.repository_path)?;

  let git_executor = state.git_executor.clone();
  run_blocking("Failed to amend uncommitted changes", move || amend_uncommitted_to_branch_core(&git_executor, params))
    .await
    .map(Json)
}

pub async fn get_uncommitted_changes(State(state): State<Arc<AppState>>, Json(params): Json<GetUncommittedChangesParams>) -> Result<Json<UncommittedChangesResult>, StatusCode> {
  // Validate that the repository path belongs to a test repository
  ensure_repository_exists(&state, &params.repository_path)?;

  let git_executor = state.git_executor.clone();
  run_blocking("Failed to get uncommitted changes", move || core_get_uncommitted_changes(&git_executor, params))
    .await
    .map(Json)
}

// The commit files commands run real git on the fixture repository, without the object cache: an LMDB environment
// per test repository would stay open (and keep its files) for the life of the server

pub async fn get_commit_files(State(state): State<Arc<AppState>>, Json(params): Json<GetCommitFilesParams>) -> Result<Json<CommitFiles>, StatusCode> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_blocking("Failed to get commit files", move || {
    read_commit_files_cached(&git_executor, &params.repository_path, &ObjectCache::disabled(), std::slice::from_ref(&params.commit_id))?
      .pop()
      .ok_or_else(|| anyhow::anyhow!("No files read for {}", params.commit_id))
  })
  .await
  .map(Json)
}

pub async fn get_commits_files(State(state): State<Arc<AppState>>, Json(params): Json<GetCommitsFilesParams>) -> Result<Json<Vec<CommitFiles>>, StatusCode> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_blocking("Failed to get the files of commits", move || {
    read_commit_files_cached(&git_executor, &params.repository_path, &ObjectCache::disabled(), &params.commit_ids)
  })
  .await
  .map(Json)
}

pub async fn get_commit_file_diff(State(state): State<Arc<AppState>>, Json(params): Json<GetCommitFileDiffParams>) -> Result<Json<FileDiff>, StatusCode> {
  ensure_repository_exists(&state, &params.repository_path)?;
  let git_executor = state.git_executor.clone();
  run_blocking("Failed to get the diff of a commit file", move || {
    read_commit_file_diff(&git_executor, &params.repository_path, &params.commit_id, &params.path, params.old_path.as_deref())
  })
  .await
  .map(Json)
}

#[derive(Deserialize)]
pub struct UpdateMenuCheckboxRequest {
  #[serde(rename = "menuId")]
  menu_id: String,
  checked: bool,
}

pub async fn update_menu_checkbox(Json(request): Json<UpdateMenuCheckboxRequest>) -> StatusCode {
  // No-op for tests - just log and return success
  tracing::debug!("Menu checkbox updated: {} = {}", request.menu_id, request.checked);
  StatusCode::OK
}
