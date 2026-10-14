use dashmap::DashMap;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, RwLock};
use sync_types::patronus::PatronusRobotsResult;
use sync_types::pull_request::PullRequestsResult;

pub struct AppState {
  pub repositories: DashMap<String, TestRepository>,
  pub path_to_id: DashMap<String, String>, // path -> repository_id
  pub git_executor: GitCommandExecutor,
  pub test_root_dir: tempfile::TempDir,
  pub templates_dir: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelState {
  NotDownloaded, // Model needs to be downloaded
  Downloaded,    // Model is ready, return suggestions
  Downloading,   // Model is currently downloading (for future use)
}

pub struct TestRepository {
  pub id: String,
  pub path: String,
  pub store: DashMap<String, serde_json::Value>,
  pub model_state: Arc<RwLock<ModelState>>,
  pub download_cancelled: Arc<AtomicBool>, // Per-repository download cancellation
  /// What `get_pull_requests` answers (E2E never runs gh); not on GitHub until a test sets it
  pub pull_requests: RwLock<PullRequestsResult>,
  /// How often the app asked for pull requests, so a test can check the refresh policy
  pub pull_request_calls: AtomicU32,
  /// What `get_patronus_robots` answers (E2E never calls Patronus); not on Space until a test sets it
  pub patronus: RwLock<PatronusRobotsResult>,
  /// How often the app listed and polled the robots, so a test can check the request policy
  pub patronus_list_calls: AtomicU32,
  pub patronus_poll_calls: AtomicU32,
}
