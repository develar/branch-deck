use axum::{
  extract::State,
  http::{HeaderMap, StatusCode},
  response::{
    Json,
    sse::{Event, KeepAlive, Sse},
  },
};
use futures::stream::{Stream, StreamExt};
use git_ops::model::{BranchError, BranchSyncStatus};
use model_ai::types::{BranchSuggestion, DownloadProgress, SuggestBranchNameParams, SuggestionProgress};
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use sync_core::branch_prefix::get_branch_prefix_from_git_config_sync;
use sync_core::delete_archived_branch::{DeleteArchivedBranchParams, delete_archived_branch_core};
use sync_core::sync::sync_branches_core_with_cache;
use sync_types::{ProgressReporter, SyncEvent};
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

#[derive(Deserialize)]
pub struct ValidateRepositoryPathRequest {
  path: String,
}

pub async fn validate_repository_path(Json(request): Json<ValidateRepositoryPathRequest>) -> Json<String> {
  // Use production validation logic
  match sync_core::repository_validation::validate_path(&request.path) {
    Ok(_) => Json(String::new()),
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
  // This matches how Tauri commands work: they return Result<String, String> not HTTP status codes
  if request.repository_path.starts_with("NO_REPO_") {
    let error_response = serde_json::json!({
      "status": "error",
      "error": format!("Repository not accessible: {}", request.repository_path)
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
      "error": format!("Repository not found: {}", request.repository_path)
    });
    return Json(error_response);
  }

  // Use the shared git executor from state
  match get_branch_prefix_from_git_config_sync(&state.git_executor, &request.repository_path) {
    Ok(prefix) => Json(serde_json::json!({
      "status": "ok",
      "data": prefix
    })),
    Err(e) => {
      tracing::error!("Failed to get branch prefix: {}", e);
      Json(serde_json::json!({
        "status": "error",
        "error": e.to_string()
      }))
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
  match sync_branches_core_with_cache(git_executor, repository_path, branch_prefix, progress, None).await {
    Ok(_) => {
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

  // Convert the receiver into a stream of SSE events
  let stream = UnboundedReceiverStream::new(rx).map(|event| Ok(Event::default().event("sync").data(serde_json::to_string(&event).unwrap())));

  Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(1)).text("keep-alive")))
}

pub async fn create_branch_from_commits(
  State(state): State<Arc<AppState>>,
  Json(params): Json<sync_core::create_branch::CreateBranchFromCommitsParams>,
) -> Result<Json<sync_core::create_branch::RewordResult>, StatusCode> {
  // Validate that the repository path belongs to a test repository
  ensure_repository_exists(&state, &params.repository_path)?;

  // Use the shared git executor from state
  match sync_core::create_branch::do_create_branch_from_commits(&state.git_executor, params) {
    Ok(result) => Ok(Json(result)),
    Err(e) => {
      tracing::error!("Failed to create branch from commits: {}", e);
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
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

  // Use shared validation logic from branch-sync
  Ok(Json(sync_core::repository_validation::validate_and_create_result(path)))
}

// AI Command Handlers

// check_model_status removed - frontend doesn't call it directly
// Types are now imported from model_ai::types

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
          model_name: "Qwen3-1.7B".to_string(),
          model_size: "1.2 GB".to_string(),
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
          let _ = tx.send(SuggestionProgress::SuggestionReady { suggestion, index: index as u32 });
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

  // Convert to SSE stream
  let stream = UnboundedReceiverStream::new(rx).map(|event| Ok(Event::default().event("suggestion").data(serde_json::to_string(&event).unwrap())));

  Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(1)).text("keep-alive")))
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
        let slow = settings_obj.get("simulateSlowDownload").and_then(|v| v.as_bool()).unwrap_or(false);
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
  let _ = tx.send(DownloadProgress::Started { total_files: 3 });
  tracing::debug!("Sent Started event immediately");

  // Clone repo_id for the spawned task
  let repo_id_clone = repo_id.clone();

  // Spawn download simulation for the rest of the events
  tokio::spawn(async move {
    tracing::debug!(repo_id = repo_id_clone, "Starting download simulation");

    // Small delay after Started event
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Simulate downloading 3 files
    let files = vec![
      ("config.json", 1024),
      ("model.gguf", 1200 * 1024 * 1024), // 1.2 GB
      ("tokenizer.json", 512 * 1024),
    ];

    for (file_name, file_size) in files {
      // Check if cancelled
      if download_cancelled.load(std::sync::atomic::Ordering::SeqCst) {
        // Update model state back to NotDownloaded
        if let Ok(mut model_state) = model_state_lock.write() {
          *model_state = crate::state::ModelState::NotDownloaded;
        }
        let _ = tx.send(DownloadProgress::Cancelled);
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
        total_time_s = (chunks as u64 * delay_ms) / 1000,
        "Download simulation parameters"
      );

      for i in 1..=chunks {
        // Check if cancelled
        if download_cancelled.load(std::sync::atomic::Ordering::SeqCst) {
          // Update model state back to NotDownloaded
          if let Ok(mut model_state) = model_state_lock.write() {
            *model_state = crate::state::ModelState::NotDownloaded;
          }
          let _ = tx.send(DownloadProgress::Cancelled);
          return;
        }

        let downloaded = chunk_size * i;
        let _progress = (i as f32 / chunks as f32 * 100.0) as u32;

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

  // Convert to SSE stream
  tracing::debug!("Creating SSE stream for download_model");
  let stream = UnboundedReceiverStream::new(rx).map(|event| {
    tracing::debug!(?event, "Sending SSE event");
    Ok(Event::default().event("download").data(serde_json::to_string(&event).unwrap()))
  });

  tracing::debug!("Returning SSE response for download_model");
  // Add keep-alive to prevent connection from closing
  Ok(Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new().interval(std::time::Duration::from_secs(1)).text("keep-alive")))
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
