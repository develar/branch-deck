use axum::Router;
use axum::{
  extract::{Request, State},
  http::StatusCode,
  middleware::Next,
  response::{IntoResponse, Json, Response},
  routing::{get, post},
};
use dashmap::DashMap;
use git_ops::reword_commits::{RewordCommitParams, reword_commits_batch};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use sync_core::baseline::detect_baseline_branch;
use sync_types::patronus::{PatronusAvailability, PatronusRobotsResult};
use sync_types::pull_request::{PullRequestAvailability, PullRequestsResult};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub mod state;
pub mod static_files;
pub mod tauri_command_bridge;

#[cfg(test)]
mod commit_files_test;

#[cfg(test)]
mod repository_lifecycle_test;

#[cfg(test)]
mod review_requests_test;

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod sse_test;

use state::{AppState, TestRepository};
use static_files::{serve_static_files, workspace_root};
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
    .route("/repositories/{id}/reword", post(reword_commit))
    .route("/repositories/{id}/pull-requests", post(set_pull_requests).get(get_pull_request_fixture))
    .route(
      "/repositories/{id}/branch-name-suggestions",
      post(set_branch_name_suggestions).get(get_branch_name_suggestion_fixture),
    )
    .route("/repositories/{id}/patronus", post(set_patronus).get(get_patronus_fixture))
    .route("/repositories/{id}/review-requests", post(set_review_requests).get(get_review_request_fixture))
    .route("/repositories/{id}/patronus-progress", post(set_patronus_progress))
    // Store endpoints
    .route("/store/{repo_id}/{key}", get(get_store_value))
    .route("/store/{repo_id}/{key}", post(set_store_value))
    .route("/store/{repo_id}/{key}", axum::routing::delete(delete_store_value))
    // Tauri command endpoints
    .route("/invoke/validate_repository_path", post(tauri_command_bridge::validate_repository_path))
    .route("/invoke/get_branch_prefix_from_git_config", post(tauri_command_bridge::get_branch_prefix_from_git_config))
    .route("/invoke/check_git_setup", post(tauri_command_bridge::check_git_setup))
    .route("/invoke/sync_branches", post(tauri_command_bridge::sync_branches))
    .route("/invoke/get_sync_inputs_fingerprint", post(tauri_command_bridge::get_sync_inputs_fingerprint))
    .route("/invoke/add_issue_reference_to_commits", post(tauri_command_bridge::add_issue_reference_to_commits))
    .route("/invoke/reword_commits", post(tauri_command_bridge::reword_commits))
    .route("/invoke/move_commits_to_branch", post(tauri_command_bridge::move_commits_to_branch))
    .route("/invoke/rename_branch", post(tauri_command_bridge::rename_branch))
    .route("/invoke/restack_branch", post(tauri_command_bridge::restack_branch))
    .route("/invoke/get_stack_independence", post(tauri_command_bridge::get_stack_independence))
    .route("/invoke/undo_regroup", post(tauri_command_bridge::undo_regroup))
    .route("/invoke/squash_commits", post(tauri_command_bridge::squash_commits))
    .route("/invoke/restore_branch_head", post(tauri_command_bridge::restore_branch_head))
    .route("/invoke/get_autosquash_plan", post(tauri_command_bridge::get_autosquash_plan))
    .route("/invoke/squash_fixup_commits", post(tauri_command_bridge::squash_fixup_commits))
    .route("/invoke/update_project", post(tauri_command_bridge::update_project))
    .route("/invoke/dismiss_landed", post(tauri_command_bridge::dismiss_landed))
    .route("/invoke/fetch_baseline", post(tauri_command_bridge::fetch_baseline))
    .route("/invoke/get_pull_requests", post(tauri_command_bridge::get_pull_requests))
    .route("/invoke/get_review_requests", post(tauri_command_bridge::get_review_requests))
    .route("/invoke/get_github_auth_status", post(tauri_command_bridge::get_github_auth_status))
    .route("/invoke/fetch_review_head", post(tauri_command_bridge::fetch_review_head))
    .route("/invoke/cancel_review_fetch", post(tauri_command_bridge::cancel_review_fetch))
    .route("/invoke/prune_review_refs", post(tauri_command_bridge::prune_review_refs))
    .route("/invoke/get_range_files", post(tauri_command_bridge::get_range_files))
    .route("/invoke/get_range_patch", post(tauri_command_bridge::get_range_patch))
    .route("/invoke/get_branch_review", post(tauri_command_bridge::get_branch_review))
    .route("/invoke/get_file_owners", post(tauri_command_bridge::get_file_owners))
    .route("/invoke/get_patronus_robots", post(tauri_command_bridge::get_patronus_robots))
    .route("/invoke/poll_patronus_robots", post(tauri_command_bridge::poll_patronus_robots))
    .route("/invoke/get_patronus_robot_details", post(tauri_command_bridge::get_patronus_robot_details))
    .route("/invoke/get_patronus_robot_problems", post(tauri_command_bridge::get_patronus_robot_problems))
    .route("/invoke/get_robot_failure", post(tauri_command_bridge::get_robot_failure))
    .route("/invoke/get_patronus_failing_robots", post(tauri_command_bridge::get_patronus_failing_robots))
    .route("/invoke/get_patronus_review_requirement", post(tauri_command_bridge::get_patronus_review_requirement))
    .route("/invoke/get_patronus_start_options", post(tauri_command_bridge::get_patronus_start_options))
    .route("/invoke/delete_archived_branch", post(tauri_command_bridge::delete_archived_branch))
    .route("/invoke/get_archived_branch_commits", post(tauri_command_bridge::get_archived_branch_commits))
    .route("/invoke/amend_uncommitted_to_branch", post(tauri_command_bridge::amend_uncommitted_to_branch))
    .route("/invoke/get_uncommitted_changes", post(tauri_command_bridge::get_uncommitted_changes))
    .route("/invoke/get_commit_files", post(tauri_command_bridge::get_commit_files))
    .route("/invoke/get_commits_files", post(tauri_command_bridge::get_commits_files))
    .route("/invoke/get_commit_patch", post(tauri_command_bridge::get_commit_patch))
    .route("/invoke/get_blob_contents", post(tauri_command_bridge::get_blob_contents))
    .route("/invoke/browse_repository/{repo_id}", post(tauri_command_bridge::browse_repository))
    .route("/invoke/suggest_branch_names", post(tauri_command_bridge::suggest_branch_names))
    // Menu command endpoints
    .route("/invoke/update_menu_checkbox", post(tauri_command_bridge::update_menu_checkbox))
    .route("/invoke/agent_cli_command", post(tauri_command_bridge::agent_cli_command))
    // Health check
    .route("/health", get(health_check))
    // Serve static files from dist with SPA fallback
    .fallback_service(serve_static_files())
    .layer(axum::middleware::from_fn_with_state(state.clone(), hold_repository))
    .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
    .layer(TraceLayer::new_for_http())
    .with_state(state)
}

/// The header with which the page mock names the test repository of its page (`?repoId=`) on every request
pub const REPOSITORY_ID_HEADER: &str = "x-repository-id";

/// Holds the repository named by `X-Repository-Id` while a command of the page runs (`TestRepository::in_flight`), so
/// `delete_repository` waits for the commands still running for it instead of deleting their files under them.
///
/// The command runs in a task of its own that holds the lock: a test that ends closes its page, the browser aborts the
/// requests, and the server drops their handlers, but the git work of a command (`spawn_blocking`) cannot be canceled
/// and runs on. The commands do all their work before they answer (a sync streams events it has already sent), so the
/// handler is the whole command. A request for a deleted repository is a page that outlived its test.
async fn hold_repository(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
  let Some(id) = request.headers().get(REPOSITORY_ID_HEADER).and_then(|value| value.to_str().ok()).map(str::to_owned) else {
    return next.run(request).await;
  };
  let Some(in_flight) = state.repositories.get(&id).map(|repo| repo.in_flight.clone()) else {
    tracing::warn!(id, uri = %request.uri(), "Request for a deleted test repository");
    return StatusCode::GONE.into_response();
  };
  let command = in_flight.read_owned().await;
  // a deletion that came first has removed it and waited for the lock
  if !state.repositories.contains_key(&id) {
    tracing::warn!(id, uri = %request.uri(), "Request for a deleted test repository");
    return StatusCode::GONE.into_response();
  }
  let uri = request.uri().clone();
  let handler = tokio::spawn(async move {
    let _command = command;
    next.run(request).await
  });
  handler.await.unwrap_or_else(|e| {
    tracing::error!(%uri, "The command task failed: {e}");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
  })
}

/// The checkout this server serves, so the E2E global setup can refuse a server of another worktree on its port
async fn health_check() -> Json<serde_json::Value> {
  Json(serde_json::json!({ "workspaceRoot": workspace_root() }))
}

pub async fn create_test_templates(target_dir: &std::path::Path) -> anyhow::Result<()> {
  // Build all templates in parallel, fresh on every launch
  let mut builds = tokio::task::JoinSet::new();
  let mut spawn = |name: &'static str, build: fn(&std::path::Path) -> anyhow::Result<()>| {
    let repo_path = target_dir.join(name);
    builds.spawn_blocking(move || {
      tracing::info!("Creating test repository template: {name}");
      build(&repo_path)
    });
  };
  spawn("simple", |path| templates::simple().build(path));
  spawn("simple_no_prefix", |path| templates::simple_no_prefix().build(path));
  spawn("unassigned", |path| templates::unassigned().build(path));
  spawn("conflict_unassigned", |path| templates::conflict_unassigned().build(path));
  spawn("conflict_branches", |path| templates::conflict_branches().build(path));
  spawn("single_unassigned", |path| templates::single_unassigned().build(path));
  spawn("issue_links", |path| templates::issue_links().build(path));
  spawn("issue_split", |path| templates::issue_split().build(path));
  spawn("issue_groups", |path| templates::issue_groups().build(path));
  spawn("issue_combine", |path| templates::issue_combine().build(path));
  spawn("fixups", |path| templates::fixups().build(path));
  spawn("long_subjects", |path| templates::long_subjects().build(path));
  spawn("commit_tree", |path| templates::commit_tree().build(path));
  spawn("stacked_branches", |path| templates::stacked_branches().build(path));
  spawn("stack_island", |path| templates::stack_island().build(path));
  spawn("stacked_issues", |path| templates::stacked_issues().build(path));
  spawn("stacked_conflict", |path| templates::stacked_conflict().build(path));
  spawn("stacked_ownership", |path| templates::stacked_ownership().build(path));
  spawn("stack_push_alone", |path| templates::stack_push_alone().build(path));
  spawn("branch_filter", |path| templates::branch_filter().build(path));
  spawn("empty-non-git", |path| templates::empty_non_git().build(path));
  spawn("empty_git", |path| templates::empty_git().build(path));
  spawn("bare", |path| templates::bare().build(path));
  spawn("archived_branches", |path| templates::archived_branches().build(path));
  spawn("behind_upstream", |path| templates::behind_upstream().build(path));
  spawn("update_conflict", |path| templates::update_conflict().build(path));
  spawn("update_conflict_wide", |path| templates::update_conflict_wide().build(path));
  spawn("update_blocked", |path| templates::update_blocked().build(path));
  spawn("landed_upstream", |path| templates::landed_upstream().build(path));
  spawn("amend_changes", |path| templates::amend_changes().build(path));

  while let Some(result) = builds.join_next().await {
    result?.map_err(|e| anyhow::anyhow!("Failed to create template: {e}"))?;
  }
  Ok(())
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
  #[serde(rename = "issue_split")]
  IssueSplit,
  #[serde(rename = "issue_groups")]
  IssueGroups,
  #[serde(rename = "issue_combine")]
  IssueCombine,
  #[serde(rename = "fixups")]
  Fixups,
  #[serde(rename = "long_subjects")]
  LongSubjects,
  #[serde(rename = "commit_tree")]
  CommitTree,
  #[serde(rename = "stacked_branches")]
  StackedBranches,
  #[serde(rename = "stack_island")]
  StackIsland,
  #[serde(rename = "stacked_issues")]
  StackedIssues,
  #[serde(rename = "stacked_conflict")]
  StackedConflict,
  #[serde(rename = "stacked_ownership")]
  StackedOwnership,
  #[serde(rename = "stack_push_alone")]
  StackPushAlone,
  #[serde(rename = "branch_filter")]
  BranchFilter,
  #[serde(rename = "archived_branches")]
  ArchivedBranches,
  #[serde(rename = "amend_changes")]
  AmendChanges,
  #[serde(rename = "behind_upstream")]
  BehindUpstream,
  #[serde(rename = "update_conflict")]
  UpdateConflict,
  #[serde(rename = "update_conflict_wide")]
  UpdateConflictWide,
  #[serde(rename = "update_blocked")]
  UpdateBlocked,
  #[serde(rename = "landed_upstream")]
  LandedUpstream,
  #[serde(rename = "empty-non-git")]
  EmptyNonGit,
  #[serde(rename = "empty_git")]
  EmptyGit,
  #[serde(rename = "bare")]
  Bare,
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
      RepositoryTemplate::IssueSplit => "issue_split",
      RepositoryTemplate::IssueGroups => "issue_groups",
      RepositoryTemplate::IssueCombine => "issue_combine",
      RepositoryTemplate::Fixups => "fixups",
      RepositoryTemplate::LongSubjects => "long_subjects",
      RepositoryTemplate::CommitTree => "commit_tree",
      RepositoryTemplate::StackedBranches => "stacked_branches",
      RepositoryTemplate::StackIsland => "stack_island",
      RepositoryTemplate::StackedIssues => "stacked_issues",
      RepositoryTemplate::StackedConflict => "stacked_conflict",
      RepositoryTemplate::StackedOwnership => "stacked_ownership",
      RepositoryTemplate::StackPushAlone => "stack_push_alone",
      RepositoryTemplate::BranchFilter => "branch_filter",
      RepositoryTemplate::ArchivedBranches => "archived_branches",
      RepositoryTemplate::AmendChanges => "amend_changes",
      RepositoryTemplate::BehindUpstream => "behind_upstream",
      RepositoryTemplate::UpdateConflict => "update_conflict",
      RepositoryTemplate::UpdateConflictWide => "update_conflict_wide",
      RepositoryTemplate::UpdateBlocked => "update_blocked",
      RepositoryTemplate::LandedUpstream => "landed_upstream",
      RepositoryTemplate::EmptyNonGit => "empty-non-git",
      RepositoryTemplate::EmptyGit => "empty_git",
      RepositoryTemplate::Bare => "bare",
      RepositoryTemplate::NoRepo => "NO_REPO",
    }
  }
}

#[derive(Deserialize)]
pub struct CreateRepositoryRequest {
  pub template: RepositoryTemplate,
  #[serde(default = "default_prepopulate_store")]
  pub prepopulate_store: bool,
}

fn default_prepopulate_store() -> bool {
  true
}

pub async fn create_repository(State(state): State<Arc<AppState>>, Json(request): Json<CreateRepositoryRequest>) -> Result<Json<CreateRepositoryResponse>, StatusCode> {
  // unique for the life of the server: the repositories live in its own temporary directory. 27 characters, as the
  // KSUIDs before, so the repository selector shortens every path to `…/<id>` on every platform
  static NEXT_ID: AtomicU64 = AtomicU64::new(1);
  let id = format!("repo-{:022}", NEXT_ID.fetch_add(1, Ordering::Relaxed));

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

    insert_repository(&state, &id, &path, store);
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
  if request.prepopulate_store && !matches!(request.template, RepositoryTemplate::EmptyNonGit | RepositoryTemplate::Bare) {
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

  insert_repository(&state, &id, &path, store);
  state.path_to_id.insert(path.clone(), id.clone());

  tracing::info!("Created repository with id: {} at path: {}", id, path);

  Ok(Json(CreateRepositoryResponse { id, path }))
}

fn insert_repository(state: &AppState, id: &str, path: &str, store: DashMap<String, serde_json::Value>) {
  let repo = TestRepository {
    id: id.to_string(),
    path: path.to_string(),
    in_flight: Arc::default(),
    store,
    pull_requests: std::sync::RwLock::new(PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub)),
    pull_request_calls: std::sync::atomic::AtomicU32::new(0),
    review_requests: std::sync::RwLock::default(),
    review_request_calls: std::sync::atomic::AtomicU32::new(0),
    review_fetch_calls: std::sync::atomic::AtomicU32::new(0),
    review_prune_keeps: std::sync::Mutex::default(),
    patronus: std::sync::RwLock::new(PatronusRobotsResult::from(PatronusAvailability::NotPatronus)),
    patronus_list_calls: std::sync::atomic::AtomicU32::new(0),
    patronus_poll_calls: std::sync::atomic::AtomicU32::new(0),
    patronus_detail_calls: std::sync::atomic::AtomicU32::new(0),
    patronus_progress: std::sync::RwLock::new(std::collections::HashMap::new()),
    branch_name_suggestions: std::sync::RwLock::new(state::BranchNameSuggestionFixture::default()),
    branch_name_suggestion_calls: std::sync::atomic::AtomicU32::new(0),
  };
  state.repositories.insert(id.to_string(), repo);
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewordCommitRequest {
  commit_id: String,
  message: String,
}

/// Rewrites the message of a commit and recreates its descendants, as `git rebase -i` in a terminal would:
/// lets an E2E test change a repository behind the app's back. Returns the new commit id.
pub async fn reword_commit(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
  Json(request): Json<RewordCommitRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
  let path = state.repositories.get(&id).map(|repo| repo.path.clone()).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let rewrites = vec![RewordCommitParams {
    commit_id: request.commit_id.clone(),
    new_message: request.message,
    // a rebase in a terminal doesn't keep the mapping notes of the commits it rewords
    keeps_virtual_commit: false,
  }];
  let baseline_branch = detect_baseline_branch(&state.git_executor, &path, "master").map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
  match reword_commits_batch(&state.git_executor, &path, &baseline_branch, rewrites) {
    Ok(mapping) => Ok(Json(serde_json::json!({ "newCommitId": mapping.get(&request.commit_id) }))),
    Err(e) => {
      tracing::error!("Failed to reword commit {}: {}", request.commit_id, e);
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// Sets what `get_pull_requests` answers for the repository. A pull request with an empty `headRefOid` shows the
/// local tip of its branch at the time of the request (the test doesn't know the commit ids the sync creates).
pub async fn set_pull_requests(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
  Json(result): Json<PullRequestsResult>,
) -> Result<StatusCode, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  *repo.pull_requests.write().expect("pull request fixture lock") = result;
  Ok(StatusCode::OK)
}

/// Sets what `suggest_branch_names` answers for the repository, and how long it takes
pub async fn set_branch_name_suggestions(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
  Json(fixture): Json<state::BranchNameSuggestionFixture>,
) -> Result<StatusCode, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  *repo.branch_name_suggestions.write().expect("branch name suggestion fixture lock") = fixture;
  Ok(StatusCode::OK)
}

/// How often the app asked for branch names
pub async fn get_branch_name_suggestion_fixture(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  Ok(Json(
    serde_json::json!({ "calls": repo.branch_name_suggestion_calls.load(std::sync::atomic::Ordering::SeqCst) }),
  ))
}

/// Sets what `get_patronus_robots` answers; an empty `sourceRevision` means the local tip of the robot's branch
pub async fn set_patronus(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
  Json(result): Json<PatronusRobotsResult>,
) -> Result<StatusCode, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  *repo.patronus.write().expect("patronus fixture lock") = result;
  Ok(StatusCode::OK)
}

/// Sets the progress the details of running robots answer, by robot id
pub async fn set_patronus_progress(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
  Json(progress): Json<std::collections::HashMap<String, sync_types::patronus::PatronusRobotProgress>>,
) -> Result<StatusCode, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  *repo.patronus_progress.write().expect("patronus progress lock") = progress;
  Ok(StatusCode::OK)
}

/// How often the app listed and polled the robots and asked for their details
pub async fn get_patronus_fixture(axum::extract::Path(id): axum::extract::Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let ordering = std::sync::atomic::Ordering::SeqCst;
  Ok(Json(serde_json::json!({
    "listCalls": repo.patronus_list_calls.load(ordering),
    "pollCalls": repo.patronus_poll_calls.load(ordering),
    "detailCalls": repo.patronus_detail_calls.load(ordering),
  })))
}

/// Sets what `get_review_requests` answers for the repository and how `fetch_review_head` fails (`fetchError`)
pub async fn set_review_requests(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
  Json(fixture): Json<state::ReviewRequestsFixture>,
) -> Result<StatusCode, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  *repo.review_requests.write().expect("review request fixture lock") = fixture;
  Ok(StatusCode::OK)
}

/// How often the app asked for review requests and fetched a review, and the `keep` of every prune
pub async fn get_review_request_fixture(axum::extract::Path(id): axum::extract::Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let ordering = std::sync::atomic::Ordering::SeqCst;
  Ok(Json(serde_json::json!({
    "calls": repo.review_request_calls.load(ordering),
    "fetchCalls": repo.review_fetch_calls.load(ordering),
    "prunes": repo.review_prune_keeps.lock().expect("review prune lock").clone(),
  })))
}

/// How often the app asked for pull requests
pub async fn get_pull_request_fixture(axum::extract::Path(id): axum::extract::Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  Ok(Json(serde_json::json!({ "calls": repo.pull_request_calls.load(std::sync::atomic::Ordering::SeqCst) })))
}

/// How long a deletion waits for the commands still running for the repository; a command that takes longer is stuck
const DELETE_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// Forgets the repository, so no new command starts for it, waits for the running ones (`hold_repository`), then
/// deletes its directory; `NO_REPO` has none
pub async fn delete_repository(axum::extract::Path(id): axum::extract::Path<String>, State(state): State<Arc<AppState>>) -> Result<StatusCode, StatusCode> {
  let Some((_, repo)) = state.repositories.remove(&id) else {
    return Err(StatusCode::UNPROCESSABLE_ENTITY);
  };
  state.path_to_id.remove(&repo.path);

  let no_commands = tokio::time::timeout(DELETE_WAIT, repo.in_flight.clone().write_owned()).await;
  if no_commands.is_err() {
    tracing::warn!(id, "Commands for the test repository still run after {DELETE_WAIT:?}; deleting it anyway");
  }

  let directory = PathBuf::from(&repo.path);
  if directory.starts_with(state.test_root_dir.path()) {
    let removed = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&directory)).await;
    match removed {
      Ok(Ok(())) => {}
      Ok(Err(e)) => tracing::warn!("Failed to remove repository directory {}: {}", repo.path, e),
      Err(e) => tracing::error!("Failed to remove repository directory {}: {}", repo.path, e),
    }
  }

  tracing::info!("Deleted repository: {}", id);
  Ok(StatusCode::NO_CONTENT)
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
