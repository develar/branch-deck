use dashmap::DashMap;
use git_ops::git_command::GitCommandExecutor;

pub struct AppState {
  pub repositories: DashMap<String, TestRepository>,
  pub path_to_id: DashMap<String, String>, // path -> repository_id
  pub git_executor: GitCommandExecutor,
  pub test_root_dir: tempfile::TempDir,
}

pub struct TestRepository {
  pub id: String,
  pub path: String,
  pub store: DashMap<String, serde_json::Value>,
}
