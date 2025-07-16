use git_ops::git_command::GitCommandExecutor;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct AppState {
  pub repositories: RwLock<HashMap<String, TestRepository>>,
  pub path_to_id: RwLock<HashMap<String, String>>, // path -> repository_id
  pub git_executor: GitCommandExecutor,
}

pub struct TestRepository {
  pub id: String,
  pub path: String,
  pub _temp_dir: tempfile::TempDir,
}
