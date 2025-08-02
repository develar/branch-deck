use dashmap::DashMap;
use git_ops::git_command::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

pub struct AppState {
  pub repositories: DashMap<String, TestRepository>,
  pub path_to_id: DashMap<String, String>, // path -> repository_id
  pub git_executor: GitCommandExecutor,
  pub test_root_dir: tempfile::TempDir,
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
}
