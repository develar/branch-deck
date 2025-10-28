use axum::Router;
use axum::{
  extract::State,
  http::StatusCode,
  response::Json,
  routing::{get, post},
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub mod state;
pub mod static_files;
pub mod tauri_command_bridge;

#[cfg(test)]
mod sse_test;

use state::{AppState, TestRepository};
use static_files::serve_static_files;
use svix_ksuid::{Ksuid, KsuidLike};
use test_utils::repo_template::templates;

pub async fn create_test_app() -> Router {
  let (app, _state) = create_test_app_with_state().await;
  app
}

pub async fn create_test_app_with_state() -> (Router, Arc<AppState>) {
  // Create a single temp directory for all test repositories
  let test_root_dir = tempfile::tempdir().expect("Failed to create test root directory");
  tracing::info!("Test root directory created at: {:?}", test_root_dir.path());

  // Create templates directory inside the temp directory
  let templates_dir = test_root_dir.path().join("templates");

  // Create test templates in the temp directory (each test instance gets its own templates)
  if let Err(e) = create_test_templates(&templates_dir).await {
    tracing::error!("Failed to create test repositories: {}", e);
  }

  // Create shared application state
  let state = Arc::new(AppState {
    repositories: DashMap::new(),
    path_to_id: DashMap::new(),
    git_executor: git_executor::git_command_executor::GitCommandExecutor::new(),
    test_root_dir,
    templates_dir,
  });

  let app = create_app(state.clone());
  (app, state)
}

pub fn create_app(state: Arc<AppState>) -> Router {
  Router::new()
    // Repository management endpoints
    .route("/repositories", post(create_repository))
    .route("/repositories/{id}", get(get_repository))
    .route("/repositories/{id}", axum::routing::delete(delete_repository))
    // Store endpoints
    .route("/store/{repo_id}/{key}", get(get_store_value))
    .route("/store/{repo_id}/{key}", post(set_store_value))
    .route("/store/{repo_id}/{key}", axum::routing::delete(delete_store_value))
    // Tauri command endpoints
    .route("/invoke/validate_repository_path", post(tauri_command_bridge::validate_repository_path))
    .route("/invoke/get_branch_prefix_from_git_config", post(tauri_command_bridge::get_branch_prefix_from_git_config))
    .route("/invoke/sync_branches", post(tauri_command_bridge::sync_branches))
    .route("/invoke/add_issue_reference_to_commits", post(tauri_command_bridge::add_issue_reference_to_commits))
    .route("/invoke/create_branch_from_commits", post(tauri_command_bridge::create_branch_from_commits))
    .route("/invoke/delete_archived_branch", post(tauri_command_bridge::delete_archived_branch))
    .route("/invoke/amend_uncommitted_to_branch", post(tauri_command_bridge::amend_uncommitted_to_branch))
    .route("/invoke/get_uncommitted_changes", post(tauri_command_bridge::get_uncommitted_changes))
    .route("/invoke/browse_repository/{repo_id}", post(tauri_command_bridge::browse_repository))
    // AI command endpoints
    .route("/invoke/suggest_branch_name_stream", post(tauri_command_bridge::suggest_branch_name_stream))
    .route("/invoke/download_model/{repo_id}", post(tauri_command_bridge::download_model))
    .route("/invoke/cancel_model_download/{repo_id}", post(tauri_command_bridge::cancel_model_download))
    // Menu command endpoints
    .route("/invoke/update_menu_checkbox", post(tauri_command_bridge::update_menu_checkbox))
    // Health check
    .route("/health", get(health_check))
    // Serve static files from .output/public with SPA fallback
    .fallback_service(serve_static_files())
    .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
    .layer(TraceLayer::new_for_http())
    .with_state(state)
}

async fn health_check() -> &'static str {
  "OK"
}

pub async fn ensure_test_repos() -> anyhow::Result<()> {
  let test_repos_dir = get_test_repos_dir();

  // Ensure the directory exists and is empty for fresh templates
  tracing::info!("Ensuring test repositories directory is empty for fresh templates");
  remove_dir_all::ensure_empty_dir(&test_repos_dir)?;

  create_test_templates(&test_repos_dir).await
}

pub async fn create_test_templates(target_dir: &std::path::Path) -> anyhow::Result<()> {
  // Create all templates fresh on every launch
  let templates_to_create = vec![
    ("simple", templates::simple()),
    ("simple_no_prefix", templates::simple_no_prefix()),
    ("unassigned", templates::unassigned()),
    ("conflict_unassigned", templates::conflict_unassigned()),
    ("conflict_branches", templates::conflict_branches()),
    ("single_unassigned", templates::single_unassigned()),
    ("issue_links", templates::issue_links()),
  ];

  // Build all templates in parallel
  let mut futures: Vec<_> = templates_to_create
    .into_iter()
    .map(|(name, template)| {
      let repo_path = target_dir.join(name);
      let name = name.to_string();

      tokio::task::spawn_blocking(move || {
        tracing::info!("Creating test repository template: {}", name);
        template.build(&repo_path)
      })
    })
    .collect();

  // Add the empty-non-git template separately (different type)
  {
    let repo_path = target_dir.join("empty-non-git");
    futures.push(tokio::task::spawn_blocking(move || {
      tracing::info!("Creating test repository template: empty-non-git");
      templates::empty_non_git().build(&repo_path)
    }));
  }
  {
    let repo_path = target_dir.join("archived_branches");
    futures.push(tokio::task::spawn_blocking(move || {
      tracing::info!("Creating test repository template: archived_branches");
      templates::archived_branches().build(&repo_path)
    }));
  }
  {
    let repo_path = target_dir.join("amend_changes");
    futures.push(tokio::task::spawn_blocking(move || {
      tracing::info!("Creating test repository template: amend_changes");
      templates::amend_changes().build(&repo_path)
    }));
  }

  // Wait for all templates to be created
  for result in futures {
    result.await?.map_err(|e| anyhow::anyhow!("Failed to create template: {}", e))?;
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

#[derive(Serialize)]
pub struct CreateRepositoryResponse {
  pub id: String,
  pub path: String,
}

#[derive(Deserialize)]
pub enum RepositoryTemplate {
  #[serde(rename = "simple")]
  Simple,
  #[serde(rename = "simple_no_prefix")]
  SimpleNoPrefix,
  #[serde(rename = "unassigned")]
  Unassigned,
  #[serde(rename = "conflict_unassigned")]
  ConflictUnassigned,
  #[serde(rename = "conflict_branches")]
  ConflictBranches,
  #[serde(rename = "single_unassigned")]
  SingleUnassigned,
  #[serde(rename = "issue_links")]
  IssueLinks,
  #[serde(rename = "archived_branches")]
  ArchivedBranches,
  #[serde(rename = "amend_changes")]
  AmendChanges,
  #[serde(rename = "empty-non-git")]
  EmptyNonGit,
  #[serde(rename = "NO_REPO")]
  NoRepo,
}

impl RepositoryTemplate {
  pub fn as_str(&self) -> &'static str {
    match self {
      RepositoryTemplate::Simple => "simple",
      RepositoryTemplate::SimpleNoPrefix => "simple_no_prefix",
      RepositoryTemplate::Unassigned => "unassigned",
      RepositoryTemplate::ConflictUnassigned => "conflict_unassigned",
      RepositoryTemplate::ConflictBranches => "conflict_branches",
      RepositoryTemplate::SingleUnassigned => "single_unassigned",
      RepositoryTemplate::IssueLinks => "issue_links",
      RepositoryTemplate::ArchivedBranches => "archived_branches",
      RepositoryTemplate::AmendChanges => "amend_changes",
      RepositoryTemplate::EmptyNonGit => "empty-non-git",
      RepositoryTemplate::NoRepo => "NO_REPO",
    }
  }
}

#[derive(Deserialize)]
pub struct CreateRepositoryRequest {
  pub template: RepositoryTemplate,
  #[serde(default = "default_prepopulate_store")]
  pub prepopulate_store: bool,
  #[serde(default)]
  pub model_state: Option<state::ModelState>,
}

fn default_prepopulate_store() -> bool {
  true
}

pub async fn create_repository(State(state): State<Arc<AppState>>, Json(request): Json<CreateRepositoryRequest>) -> Result<Json<CreateRepositoryResponse>, StatusCode> {
  let id = Ksuid::new(None, None).to_string();

  // Determine model state - default to NotDownloaded
  let model_state = std::sync::Arc::new(std::sync::RwLock::new(request.model_state.unwrap_or(state::ModelState::NotDownloaded)));

  // Handle special NO_REPO template
  if matches!(request.template, RepositoryTemplate::NoRepo) {
    // Use a special path that doesn't exist
    let path = format!("NO_REPO_{id}");

    // Create store and populate it if requested
    let store = DashMap::new();
    if request.prepopulate_store {
      // Populate store with the invalid path so frontend tries to load it
      // Omit cachedBranchPrefix to avoid Zod validation issues (it's optional)
      store.insert(
        "recentProjects".to_string(),
        serde_json::json!([{
          "path": path.clone()
        }]),
      );
      tracing::info!("Populated NO_REPO store with invalid path: {}", path);
    }

    let repo = TestRepository {
      id: id.clone(),
      path: path.clone(),
      store,
      model_state,
      download_cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };

    state.repositories.insert(id.clone(), repo);
    // Don't insert into path_to_id for NO_REPO

    tracing::info!("Created NO_REPO repository with id: {}", id);

    return Ok(Json(CreateRepositoryResponse { id, path }));
  }

  // Normal repository creation
  let repo_dir = state.test_root_dir.path().join(&id);
  let path = repo_dir.to_string_lossy().to_string();

  let template_path = state.templates_dir.join(request.template.as_str());

  if !template_path.exists() {
    tracing::error!("Template '{}' not found", request.template.as_str());
    return Err(StatusCode::BAD_REQUEST);
  }

  // Create the repository directory first
  std::fs::create_dir_all(&repo_dir).map_err(|e| {
    tracing::error!("Failed to create repository directory: {}", e);
    StatusCode::INTERNAL_SERVER_ERROR
  })?;

  // Copy template contents to the new unique repository directory
  let mut copy_options = fs_extra::dir::CopyOptions::new();
  copy_options.content_only = true;
  fs_extra::dir::copy(&template_path, &repo_dir, &copy_options).map_err(|e| {
    tracing::error!("Failed to copy template: {}", e);
    StatusCode::INTERNAL_SERVER_ERROR
  })?;

  // Pre-populate store with default values if requested and not empty-non-git template
  let store = DashMap::new();
  if request.prepopulate_store && !matches!(request.template, RepositoryTemplate::EmptyNonGit) {
    store.insert(
      "selectedProjectData".to_string(),
      serde_json::json!({
        "path": path.clone(),
        "cachedBranchPrefix": "user-name"
      }),
    );
    store.insert(
      "recentProjects".to_string(),
      serde_json::json!([{
        "path": path.clone(),
        "cachedBranchPrefix": "user-name"
      }]),
    );
  }

  let repo = TestRepository {
    id: id.clone(),
    path: path.clone(),
    store,
    model_state,
    download_cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
  };

  state.repositories.insert(id.clone(), repo);
  state.path_to_id.insert(path.clone(), id.clone());

  tracing::info!("Created repository with id: {} at path: {}", id, path);

  Ok(Json(CreateRepositoryResponse { id, path }))
}

pub async fn get_repository(axum::extract::Path(id): axum::extract::Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, StatusCode> {
  match state.repositories.get(&id) {
    Some(repo) => Ok(Json(serde_json::json!({
        "id": repo.id,
        "path": repo.path,
    }))),
    None => Err(StatusCode::UNPROCESSABLE_ENTITY),
  }
}

pub async fn delete_repository(axum::extract::Path(id): axum::extract::Path<String>, State(state): State<Arc<AppState>>) -> Result<StatusCode, StatusCode> {
  // Remove the repository from the map
  match state.repositories.remove(&id) {
    Some((_, repo)) => {
      // Cancel any ongoing download before deleting
      repo.download_cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
      tracing::debug!(id, "Repository being deleted, cancelling any ongoing downloads");

      // Also remove from path_to_id map
      state.path_to_id.remove(&repo.path);

      // Manually clean up the repository directory
      if let Err(e) = std::fs::remove_dir_all(&repo.path) {
        tracing::warn!("Failed to remove repository directory {}: {}", repo.path, e);
      }

      tracing::info!("Deleted repository: {}", id);
      Ok(StatusCode::NO_CONTENT)
    }
    None => Err(StatusCode::UNPROCESSABLE_ENTITY),
  }
}

// Store API endpoints
pub async fn get_store_value(
  axum::extract::Path((repo_id, key)): axum::extract::Path<(String, String)>,
  State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
  if let Some(repo) = state.repositories.get(&repo_id) {
    if let Some(value) = repo.store.get(&key) {
      return Ok(Json(value.clone()));
    }
    // Key not found in valid repo - return null (not an error)
    return Ok(Json(serde_json::json!(null)));
  }

  // Repository not found - this is an error
  Err(StatusCode::UNPROCESSABLE_ENTITY)
}

pub async fn set_store_value(
  axum::extract::Path((repo_id, key)): axum::extract::Path<(String, String)>,
  State(state): State<Arc<AppState>>,
  Json(value): Json<serde_json::Value>,
) -> Result<StatusCode, StatusCode> {
  if let Some(repo) = state.repositories.get(&repo_id) {
    repo.store.insert(key, value);
    return Ok(StatusCode::OK);
  }

  Err(StatusCode::UNPROCESSABLE_ENTITY)
}

pub async fn delete_store_value(axum::extract::Path((repo_id, key)): axum::extract::Path<(String, String)>, State(state): State<Arc<AppState>>) -> Result<StatusCode, StatusCode> {
  if let Some(repo) = state.repositories.get(&repo_id) {
    repo.store.remove(&key);
    return Ok(StatusCode::NO_CONTENT);
  }

  Err(StatusCode::UNPROCESSABLE_ENTITY)
}
