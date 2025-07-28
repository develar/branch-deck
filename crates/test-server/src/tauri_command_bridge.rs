use axum::{
  extract::State,
  http::StatusCode,
  response::{
    sse::{Event, Sse},
    Json,
  },
};
use branch_sync::progress::{ProgressReporter, SyncEvent};
use branch_sync::sync::sync_branches_core;
use futures::stream::{Stream, StreamExt};
use git_ops::model::{BranchError, BranchSyncStatus};
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::state::AppState;

// Helper function to find a repository by its path
fn find_repository_by_path(state: &AppState, path: &str) -> Option<String> {
  state.path_to_id.get(path).map(|entry| entry.value().clone())
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
  Json(params): Json<branch_sync::add_issue_reference::AddIssueReferenceParams>,
) -> Result<Json<branch_sync::add_issue_reference::AddIssueReferenceResult>, StatusCode> {
  // Validate that the repository path belongs to a test repository
  ensure_repository_exists(&state, &params.repository_path)?;

  // Use the shared git executor from state
  match branch_sync::add_issue_reference::add_issue_reference_to_commits_core(&state.git_executor, params).await {
    Ok(result) => Ok(Json(result)),
    Err(e) => {
      tracing::error!("Failed to add issue reference: {}", e);
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

#[derive(Deserialize)]
pub struct ValidateRepositoryPathRequest {
  path: String,
}

pub async fn validate_repository_path(Json(request): Json<ValidateRepositoryPathRequest>) -> Json<String> {
  // Use production validation logic
  match branch_sync::repository_validation::validate_path(&request.path) {
    Ok(_) => Json(String::new()),
    Err(e) => Json(e.to_string()),
  }
}

#[derive(Deserialize)]
pub struct GetBranchPrefixRequest {
  #[serde(rename = "repositoryPath")]
  repository_path: String,
}

pub async fn get_branch_prefix_from_git_config(State(state): State<Arc<AppState>>, Json(request): Json<GetBranchPrefixRequest>) -> Result<Json<String>, StatusCode> {
  tracing::debug!("get_branch_prefix_from_git_config called with path: {}", request.repository_path);

  // Handle NO_REPO case or empty path - return empty string
  if request.repository_path.is_empty() || request.repository_path.starts_with("NO_REPO_") {
    tracing::debug!("NO_REPO or empty path detected, returning empty branch prefix");
    return Ok(Json(String::new()));
  }

  // Log all known paths for debugging
  {
    let keys: Vec<_> = state.path_to_id.iter().map(|entry| entry.key().clone()).collect();
    tracing::debug!("Known repository paths: {:?}", keys);
  }

  // Validate that the repository path belongs to a test repository
  ensure_repository_exists(&state, &request.repository_path)?;

  // Use the shared git executor from state
  let prefix = branch_sync::branch_prefix::get_branch_prefix_from_git_config_sync(&state.git_executor, &request.repository_path).map_err(|e| {
    tracing::error!("Failed to get branch prefix: {}", e);
    StatusCode::INTERNAL_SERVER_ERROR
  })?;
  Ok(Json(prefix))
}

// Progress reporter that sends events through a channel
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

  // Clone values for the spawned task
  let repository_path = request.repository_path.clone();
  let branch_prefix = request.branch_prefix.clone();

  // Clone the git executor for the spawned task
  let git_executor = state.git_executor.clone();

  // Spawn a task to run the sync
  tokio::spawn(async move {
    // Create a progress reporter that sends events through the channel
    let reporter = ChannelProgressReporter { sender: tx };

    // Run the sync
    match sync_branches_core(&git_executor, &repository_path, &branch_prefix, &reporter).await {
      Ok(_) => {
        // Send completion event
        let _ = reporter.send(SyncEvent::Completed);
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
  });

  // Convert the receiver into a stream of SSE events
  let stream = UnboundedReceiverStream::new(rx).map(|event| Ok(Event::default().event("sync").data(serde_json::to_string(&event).unwrap())));

  Ok(Sse::new(stream))
}

pub async fn create_branch_from_commits(
  State(state): State<Arc<AppState>>,
  Json(params): Json<branch_sync::create_branch::CreateBranchFromCommitsParams>,
) -> Result<Json<branch_sync::create_branch::RewordResult>, StatusCode> {
  // Validate that the repository path belongs to a test repository
  ensure_repository_exists(&state, &params.repository_path)?;

  // Use the shared git executor from state
  match branch_sync::create_branch::do_create_branch_from_commits(&state.git_executor, params).await {
    Ok(result) => Ok(Json(result)),
    Err(e) => {
      tracing::error!("Failed to create branch from commits: {}", e);
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

#[derive(Deserialize)]
pub struct BrowseRepositoryRequest {
  #[serde(rename = "repoId")]
  repo_id: String,
}

pub async fn browse_repository(
  State(state): State<Arc<AppState>>,
  Json(request): Json<BrowseRepositoryRequest>,
) -> Result<Json<branch_sync::repository_validation::BrowseResult>, StatusCode> {
  // Get the repository from state
  let repo = state.repositories.get(&request.repo_id).ok_or_else(|| {
    tracing::error!("Repository not found: {}", request.repo_id);
    StatusCode::NOT_FOUND
  })?;

  let path = repo.path.clone();

  // Use shared validation logic from branch-sync
  Ok(Json(branch_sync::repository_validation::validate_and_create_result(path)))
}
