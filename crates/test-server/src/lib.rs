// Re-export what tests need
pub use axum::Router;
use axum::{
  extract::State,
  http::StatusCode,
  response::Json,
  routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

pub mod state;
pub mod tauri_command_bridge;

use state::{AppState, TestRepository};
use test_utils::templates;

pub async fn create_test_app() -> Router {
  // Ensure test repositories exist
  if let Err(e) = ensure_test_repos() {
    tracing::error!("Failed to create test repositories: {}", e);
  }

  // Create shared application state
  let state = Arc::new(AppState {
    repositories: RwLock::new(HashMap::new()),
    path_to_id: RwLock::new(HashMap::new()),
    git_executor: git_ops::git_command::GitCommandExecutor::new(),
  });

  create_app(state)
}

pub fn create_app(state: Arc<AppState>) -> Router {
  Router::new()
    // Repository management endpoints
    .route("/repositories", post(create_repository))
    .route("/repositories/{id}", get(get_repository))
    .route("/repositories/{id}", axum::routing::delete(delete_repository))
    .route("/repositories/{id}/setup", post(setup_repository))
    // Tauri command endpoints
    .route("/invoke/validate_repository_path", post(tauri_command_bridge::validate_repository_path))
    .route("/invoke/get_branch_prefix_from_git_config", post(tauri_command_bridge::get_branch_prefix_from_git_config))
    .route("/invoke/sync_branches", post(tauri_command_bridge::sync_branches))
    .route("/invoke/add_issue_reference_to_commits", post(tauri_command_bridge::add_issue_reference_to_commits))
    // Health check
    .route("/health", get(health_check))
    .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
    .with_state(state)
}

async fn health_check() -> &'static str {
  "OK"
}

pub fn ensure_test_repos() -> anyhow::Result<()> {
  let test_repos_dir = get_test_repos_dir();
  std::fs::create_dir_all(&test_repos_dir)?;

  // Create templates if they don't exist
  let templates_to_create = vec![("simple", templates::simple())];

  for (name, template) in templates_to_create {
    let repo_path = test_repos_dir.join(name);
    if !repo_path.join(".git").exists() {
      tracing::info!("Creating test repository template: {}", name);
      template.build(&repo_path)?;
    } else {
      tracing::debug!("Test repository template '{}' already exists", name);
    }
  }

  Ok(())
}

pub fn get_test_repos_dir() -> PathBuf {
  // Get the path relative to the project root
  let current_exe = std::env::current_exe().expect("Failed to get current executable path");
  let project_root = current_exe
    .ancestors()
    .find(|p| p.join("Cargo.toml").exists() && p.join("tests").exists())
    .expect("Failed to find project root");

  project_root.join("tests").join("test-repos")
}

// Recursively copy directory
pub fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
  use std::fs;

  fs::create_dir_all(dst)?;

  for entry in fs::read_dir(src)? {
    let entry = entry?;
    let src_path = entry.path();
    let dst_path = dst.join(entry.file_name());

    if src_path.is_dir() {
      copy_dir_recursive(&src_path, &dst_path)?;
    } else {
      fs::copy(&src_path, &dst_path)?;
    }
  }

  Ok(())
}

#[derive(Serialize)]
pub struct CreateRepositoryResponse {
  pub id: String,
  pub path: String,
}

#[derive(Deserialize)]
pub struct CreateRepositoryRequest {
  pub template: Option<String>,
}

pub async fn create_repository(State(state): State<Arc<AppState>>, Json(request): Json<CreateRepositoryRequest>) -> Result<Json<CreateRepositoryResponse>, StatusCode> {
  let id = uuid::Uuid::new_v4().to_string();
  let temp_dir = tempfile::tempdir().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
  let path = temp_dir.path().to_string_lossy().to_string();

  // If a template is specified, clone it
  if let Some(template_name) = request.template {
    let test_repos_dir = get_test_repos_dir();
    let template_path = test_repos_dir.join(&template_name);

    if !template_path.exists() {
      tracing::error!("Template '{}' not found", template_name);
      return Err(StatusCode::BAD_REQUEST);
    }

    // Copy the template directory recursively
    copy_dir_recursive(&template_path, &PathBuf::from(&path)).map_err(|e| {
      tracing::error!("Failed to copy template: {}", e);
      StatusCode::INTERNAL_SERVER_ERROR
    })?;
  } else {
    // Create empty repository (backward compatibility)
    let output = std::process::Command::new("git")
      .args(["init", "--initial-branch=master"])
      .current_dir(&path)
      .output()
      .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !output.status.success() {
      return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Set up git user config for the test repository
    std::process::Command::new("git")
      .args(["config", "user.name", "Test User"])
      .current_dir(&path)
      .output()
      .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    std::process::Command::new("git")
      .args(["config", "user.email", "test@example.com"])
      .current_dir(&path)
      .output()
      .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Set default branch prefix for tests
    std::process::Command::new("git")
      .args(["config", "branchdeck.branchPrefix", "user-name"])
      .current_dir(&path)
      .output()
      .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
  }

  let repo = TestRepository {
    id: id.clone(),
    path: path.clone(),
    _temp_dir: temp_dir,
  };

  state.repositories.write().await.insert(id.clone(), repo);
  state.path_to_id.write().await.insert(path.clone(), id.clone());

  tracing::info!("Created repository with id: {} at path: {}", id, path);

  Ok(Json(CreateRepositoryResponse { id, path }))
}

pub async fn get_repository(axum::extract::Path(id): axum::extract::Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, StatusCode> {
  let repos = state.repositories.read().await;
  match repos.get(&id) {
    Some(repo) => Ok(Json(serde_json::json!({
        "id": repo.id,
        "path": repo.path,
    }))),
    None => Err(StatusCode::NOT_FOUND),
  }
}

#[derive(Deserialize)]
pub struct SetupRepositoryRequest {
  pub branches: Vec<BranchSetup>,
}

#[derive(Deserialize)]
pub struct BranchSetup {
  pub name: String,
  pub commits: Vec<CommitSetup>,
}

#[derive(Deserialize)]
pub struct CommitSetup {
  pub message: String,
  pub files: HashMap<String, String>,
}

pub async fn setup_repository(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
  Json(request): Json<SetupRepositoryRequest>,
) -> Result<StatusCode, StatusCode> {
  let repos = state.repositories.read().await;
  let repo = repos.get(&id).ok_or(StatusCode::NOT_FOUND)?;

  // Create branches and commits as specified
  for branch in request.branches {
    // Create and checkout branch
    std::process::Command::new("git")
      .args(["checkout", "-b", &branch.name])
      .current_dir(&repo.path)
      .output()
      .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create commits
    for commit in branch.commits {
      // Write files
      for (file_path, content) in commit.files {
        let full_path = PathBuf::from(&repo.path).join(&file_path);
        if let Some(parent) = full_path.parent() {
          std::fs::create_dir_all(parent).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
        std::fs::write(&full_path, content).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Stage file
        std::process::Command::new("git")
          .args(["add", &file_path])
          .current_dir(&repo.path)
          .output()
          .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
      }

      // Commit
      std::process::Command::new("git")
        .args(["commit", "-m", &commit.message])
        .current_dir(&repo.path)
        .output()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
  }

  // Return to master branch
  std::process::Command::new("git")
    .args(["checkout", "master"])
    .current_dir(&repo.path)
    .output()
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

  Ok(StatusCode::OK)
}

pub async fn delete_repository(axum::extract::Path(id): axum::extract::Path<String>, State(state): State<Arc<AppState>>) -> Result<StatusCode, StatusCode> {
  let mut repos = state.repositories.write().await;

  // Remove the repository from the map
  // The TempDir will be automatically cleaned up when dropped
  match repos.remove(&id) {
    Some(repo) => {
      // Also remove from path_to_id map
      state.path_to_id.write().await.remove(&repo.path);
      tracing::info!("Deleted repository: {}", id);
      Ok(StatusCode::NO_CONTENT)
    }
    None => Err(StatusCode::NOT_FOUND),
  }
}
