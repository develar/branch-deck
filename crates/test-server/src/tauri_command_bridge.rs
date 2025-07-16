use axum::{
  extract::State,
  http::StatusCode,
  response::{
    sse::{Event, Sse},
    Json,
  },
};
use branch_sync::progress::ProgressReporter;
use branch_sync::{sync_branches_core, SyncEvent};
use futures::stream::{Stream, StreamExt};
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::state::AppState;

// Helper function to find a repository by its path
async fn find_repository_by_path(state: &AppState, path: &str) -> Option<String> {
  let path_map = state.path_to_id.read().await;
  path_map.get(path).cloned()
}

// Helper function to ensure a repository exists by path
async fn ensure_repository_exists(state: &AppState, path: &str) -> Result<(), StatusCode> {
  let repo_id = find_repository_by_path(state, path).await;
  if repo_id.is_none() {
    tracing::warn!("Repository not found for path: {}", path);
    return Err(StatusCode::NOT_FOUND);
  }
  Ok(())
}

pub async fn add_issue_reference_to_commits(
  State(state): State<Arc<AppState>>,
  Json(params): Json<branch_sync::AddIssueReferenceParams>,
) -> Result<Json<branch_sync::AddIssueReferenceResult>, StatusCode> {
  // Validate that the repository path belongs to a test repository
  ensure_repository_exists(&state, &params.repository_path).await?;

  // Use the shared git executor from state
  match branch_sync::add_issue_reference_to_commits_core(&state.git_executor, params).await {
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
  // For test purposes, always return empty string (valid path) for any non-empty path
  if request.path.is_empty() {
    Json("Path cannot be empty".to_string())
  } else {
    Json(String::new())
  }
}

#[derive(Deserialize)]
pub struct GetBranchPrefixRequest {
  #[serde(rename = "repositoryPath")]
  repository_path: String,
}

pub async fn get_branch_prefix_from_git_config(State(state): State<Arc<AppState>>, Json(request): Json<GetBranchPrefixRequest>) -> Result<Json<String>, StatusCode> {
  tracing::debug!("get_branch_prefix_from_git_config called with path: {}", request.repository_path);

  // Log all known paths for debugging
  {
    let path_map = state.path_to_id.read().await;
    tracing::debug!("Known repository paths: {:?}", path_map.keys().collect::<Vec<_>>());
  }

  // Validate that the repository path belongs to a test repository
  ensure_repository_exists(&state, &request.repository_path).await?;

  // Use the shared git executor from state
  let prefix = branch_sync::get_branch_prefix_from_git_config_sync(&state.git_executor, &request.repository_path).map_err(|e| {
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
  ensure_repository_exists(&state, &request.repository_path).await?;

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
    if let Err(e) = sync_branches_core(&git_executor, &repository_path, &branch_prefix, &reporter).await {
      tracing::error!("Sync branches failed: {}", e);
    }
  });

  // Convert the receiver into a stream of SSE events
  let stream = UnboundedReceiverStream::new(rx).map(|event| Ok(Event::default().event("sync").data(serde_json::to_string(&event).unwrap())));

  Ok(Sse::new(stream))
}
