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
use history_rewrite::baseline::detect_baseline_branch;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use sync_utils::fs_retry;
use sync_utils::span::in_current_span;
use tracing::Instrument;

use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub mod activity_routes;
pub mod channel_commands;
pub mod cli_tools_fixtures;
pub mod remote_fixtures;
pub mod remote_git_fixtures;
pub mod remote_host;
pub mod review_fixtures;
pub mod session_routes;
pub mod state;
pub mod static_files;
pub mod sync_overrides;
pub mod tauri_command_bridge;
pub mod templates;
pub mod updater;
pub mod youtrack_fixtures;

#[cfg(test)]
mod api_error_contract_test;

#[cfg(test)]
mod branch_prefix_test;

#[cfg(test)]
mod command_registry_test;

#[cfg(test)]
mod commit_files_test;

#[cfg(test)]
mod github_host_test;

#[cfg(test)]
mod repository_config_test;

#[cfg(test)]
mod repository_lifecycle_test;

#[cfg(test)]
mod review_readiness_test;

#[cfg(test)]
mod review_requests_test;

#[cfg(test)]
mod remote_fixtures_test;
#[cfg(test)]
mod remote_git_fixtures_test;

#[cfg(test)]
mod reword_test;

#[cfg(test)]
mod squash_test;

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod channel_commands_test;

#[cfg(test)]
mod session_routes_test;

#[cfg(test)]
mod patch_bytes_test;

#[cfg(test)]
mod templates_test;

#[cfg(test)]
mod youtrack_host_test;

use state::{AppState, TestRepository};
use static_files::{serve_static_files, workspace_root};

/// The app of a route test: it builds a template when the test first asks for it (`create_route_test_app_with_state`)
pub fn create_test_app() -> Router {
  let (app, _state) = create_route_test_app_with_state();
  app
}

/// The app of the E2E server: the templates of its build come from their cache, or build in the background while it
/// serves (`templates.rs`)
pub fn create_test_app_with_state() -> (Router, Arc<AppState>) {
  create_app_and_state(true)
}

/// The app of a route test: a template is built when a repository is first made of it, in the temporary directory of the
/// test. A test asks for one or two of the templates, and nextest runs each test in a process of its own, which would
/// build all of them
pub fn create_route_test_app_with_state() -> (Router, Arc<AppState>) {
  create_app_and_state(false)
}

/// Where the app of the E2E server records its operations: the registry of the `$GIT_COMMON_DIR` of each worktree, one
/// `rev-parse` the first time a worktree asks (`git_ops::revs::git_common_dir`)
fn registry_files(git: git_executor::git_command_executor::GitCommandExecutor) -> operations::registry::RegistryFiles {
  let known = std::sync::Mutex::new(std::collections::HashMap::<String, PathBuf>::new());
  operations::registry::RegistryFiles::new(domain_types::error::HolderTool::App, move |key| {
    let cached = known.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&key.worktree).cloned();
    if cached.is_some() {
      return cached;
    }
    let dir = PathBuf::from(git_ops::revs::git_common_dir(&git, &key.worktree).ok()?);
    known.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(key.worktree.clone(), dir.clone());
    Some(dir)
  })
}

/// Unix seconds now for the operations registry of the test repository of `worktree`: the start of the server and the
/// time of the clock of its sessions (`WorktreeClocks`), which stands still until its test moves it, so a start whose
/// answer was lost settles only when the test moves the clock past it (`POST /repositories/{id}/clock/advance`)
pub fn registry_now(clocks: &state::WorktreeClocks, worktree: &str) -> u32 {
  static EPOCH: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
  let epoch = *EPOCH.get_or_init(sync_utils::timestamp::unix_now);
  let elapsed = u32::try_from(sync_utils::clock::Clock::now(&clocks.of_worktree(worktree)).as_secs()).unwrap_or(u32::MAX);
  epoch.saturating_add(elapsed)
}

fn create_app_and_state(all_templates: bool) -> (Router, Arc<AppState>) {
  // Create a single temp directory for all test repositories
  let test_root_dir = tempfile::tempdir().expect("Failed to create test root directory");
  tracing::info!("Test root directory created at: {:?}", test_root_dir.path());

  let git_executor = git_executor::git_command_executor::GitCommandExecutor::new();
  let fallback_templates = test_root_dir.path().join("templates");
  let templates = if all_templates {
    let git_version = git_executor.get_info().map_or_default(|info| info.version.clone());
    let templates = Arc::new(templates::Templates::cached(&git_version, fallback_templates));
    templates.build_all_in_background();
    templates
  } else {
    Arc::new(templates::Templates::on_demand(fallback_templates))
  };

  // Create shared application state
  let session_clocks = Arc::new(state::WorktreeClocks::default());
  let state = Arc::new_cyclic(|state| {
    let environment = session_routes::TestSessionEnvironment {
      state: std::sync::Weak::clone(state),
    };
    let host = repository_model::runtime::GitSessionHost::new(git_executor.clone(), environment);
    let sessions = Arc::new(repository_model::session::Sessions::new(
      tokio::runtime::Handle::current(),
      Arc::new(host),
      Arc::clone(&session_clocks) as Arc<dyn repository_model::session::SessionClocks>,
    ));
    // the operations of the app are recorded in the registry of each test repository, as the app records them, and a
    // start whose answer was lost is settled by the fakes of its repository
    let clocks = Arc::clone(&session_clocks);
    let files = registry_files(git_executor.clone())
      .settled_with(session_routes::settle_host(state, git_executor.clone()))
      .at(move |key| registry_now(&clocks, &key.worktree));
    let registry = operations::registry::Registry::new(Arc::clone(&sessions), files);
    AppState {
      repositories: DashMap::new(),
      path_to_id: DashMap::new(),
      git_executor,
      test_root_dir,
      templates,
      branch_prefixes: app_settings::branch_prefixes::BranchPrefixes::default(),
      lock_waits: history_rewrite::lock_wait::LockWaits::default(),
      registry,
      sessions,
      session_clocks,
    }
  });
  // the facts of the sessions, read on the same clocks from the fakes of each test repository
  if let Some(events) = state.sessions.events() {
    let scheduler = remote_facts::scheduler::Scheduler {
      // the registry applies the facts: a list of the robots settles the starts whose answer was lost
      port: Arc::new(state.registry.clone()) as Arc<dyn remote_facts::scheduler::FactsPort>,
      sources: Arc::new(session_routes::TestFactSources::new(&state)),
      clocks: Arc::clone(&state.session_clocks) as Arc<dyn repository_model::session::SessionClocks>,
    };
    scheduler.start(&tokio::runtime::Handle::current(), events);
  }

  let app = create_app(Arc::clone(&state));
  (app, state)
}

pub fn create_app(state: Arc<AppState>) -> Router {
  Router::new()
    // Repository management endpoints
    .route("/repositories", post(create_repository))
    .route("/repositories/{id}", get(get_repository))
    .route("/repositories/{id}", axum::routing::delete(delete_repository))
    .route("/repositories/{id}/reword", post(reword_commit))
    .route("/repositories/{id}/commit", post(commit_on_head))
    .route("/repositories/{id}/repository-lock", post(hold_repository_lock))
    .route("/repositories/{id}/foreign-operation", post(remote_fixtures::set_foreign_operation))
    .route("/repositories/{id}/stamp", post(remote_fixtures::touch_repository_stamp))
    .route("/repositories/{id}/agent-operation", post(remote_fixtures::journal_agent_operation))
    .route("/repositories/{id}/journal-gone", post(remote_fixtures::lose_journal))
    .route("/repositories/{id}/sync-overrides", post(sync_overrides::set_sync_overrides))
    .route("/repositories/{id}/revision", get(sync_overrides::get_revision))
    .route("/repositories/{id}/rebase-in-progress", post(sync_overrides::set_rebase_in_progress))
    .route("/repositories/{id}/branch-prefix", post(sync_overrides::set_branch_prefix_in_terminal))
    .route(
      "/repositories/{id}/pull-requests",
      post(remote_fixtures::set_pull_requests).get(remote_fixtures::get_github_log),
    )
    .route(
      "/repositories/{id}/github-sign-in",
      post(remote_fixtures::set_github_sign_in).get(remote_fixtures::get_github_sign_in),
    )
    .route("/repositories/{id}/youtrack", post(youtrack_fixtures::set_youtrack).get(youtrack_fixtures::get_youtrack))
    .route(
      "/repositories/{id}/branch-name-suggestions",
      post(set_branch_name_suggestions).get(get_branch_name_suggestion_fixture),
    )
    .route(
      "/repositories/{id}/patronus",
      post(remote_fixtures::set_patronus).get(remote_fixtures::get_patronus_fixture),
    )
    .route("/repositories/{id}/patronus-details", post(remote_fixtures::set_patronus_details))
    .route("/repositories/{id}/patronus-history", post(remote_fixtures::set_patronus_history))
    .route("/repositories/{id}/review-requests", post(set_review_requests).get(get_review_request_fixture))
    .route("/repositories/{id}/patronus-progress", post(remote_fixtures::set_patronus_progress))
    .route("/repositories/{id}/patronus-profiles", post(remote_fixtures::set_patronus_profiles))
    .route("/repositories/{id}/patronus-start-refusal", post(remote_fixtures::set_patronus_start_refusal))
    .route(
      "/repositories/{id}/remote-gates",
      post(remote_fixtures::set_remote_gates).get(remote_fixtures::get_remote_gates),
    )
    .route(
      "/repositories/{id}/review-answers",
      post(review_fixtures::set_review_answers).get(review_fixtures::get_review_answers),
    )
    .route("/repositories/{id}/patronus-replies", post(review_fixtures::set_patronus_replies))
    .route("/repositories/{id}/remote", get(remote_git_fixtures::get_remote))
    .route("/repositories/{id}/remote/push", post(remote_git_fixtures::push_in_terminal_route))
    .route("/repositories/{id}/remote/commit", post(remote_git_fixtures::commit_on_remote_route))
    .route("/repositories/{id}/remote/fetch", post(remote_git_fixtures::fetch_in_terminal_route))
    .route("/repositories/{id}/remote/refusal", post(remote_git_fixtures::set_remote_refusal))
    .route("/repositories/{id}/github-answer", post(review_fixtures::set_github_answer))
    .route("/repositories/{id}/update", post(updater::set_update))
    .route(
      "/repositories/{id}/cli-tools",
      post(cli_tools_fixtures::set_cli_tools).get(cli_tools_fixtures::get_cli_tools),
    )
    .route("/repositories/{id}/sessions/close", post(session_routes::close_sessions))
    .route("/repositories/{id}/clock/advance", post(session_routes::advance_clock))
    // Store endpoints
    .route("/store/{repo_id}", get(get_store_snapshot))
    .route("/store/{repo_id}/{key}", get(get_store_value))
    .route("/store/{repo_id}/{key}", post(set_store_value))
    .route("/store/{repo_id}/{key}", axum::routing::delete(delete_store_value))
    // Tauri command endpoints
    .route("/invoke/validate_repository_path", post(tauri_command_bridge::validate_repository_path))
    .route("/invoke/resolve_branch_prefix", post(tauri_command_bridge::resolve_branch_prefix))
    .route("/invoke/check_git_setup", post(tauri_command_bridge::check_git_setup))
    .route("/invoke/get_repository_config", post(tauri_command_bridge::get_repository_config))
    .route("/invoke/set_repository_config", post(tauri_command_bridge::set_repository_config))
    .route("/invoke/read_activity", post(activity_routes::read_activity))
    .route("/invoke/record_activity", post(activity_routes::record_activity))
    .route("/invoke/clear_activity", post(activity_routes::clear_activity))
    .route("/invoke/get_snapshot", post(session_routes::get_snapshot))
    .route("/invoke/prepare", post(session_routes::prepare))
    .route("/invoke/execute", post(session_routes::execute))
    .route("/invoke/cancel_operation", post(session_routes::cancel_operation))
    .route("/invoke/subscribe_view", post(session_routes::subscribe_view))
    .route("/invoke/unsubscribe_view", post(session_routes::unsubscribe_view))
    .route("/invoke/set_window_focus", post(session_routes::set_window_focus))
    .route("/invoke/set_demand", post(session_routes::set_demand))
    .route("/invoke/refresh_facts", post(session_routes::refresh_facts))
    .route("/invoke/link_pull_request", post(session_routes::link_pull_request))
    .route("/invoke/unlink_pull_request", post(session_routes::unlink_pull_request))
    .route("/invoke/sync", post(session_routes::sync))
    .route("/invoke/add_issue_reference_to_commits", post(tauri_command_bridge::add_issue_reference_to_commits))
    .route("/invoke/reword", post(tauri_command_bridge::reword))
    .route("/invoke/preview_move", post(tauri_command_bridge::preview_move))
    .route("/invoke/unapply_branch", post(tauri_command_bridge::unapply_branch))
    .route("/invoke/pick_remote_commits", post(tauri_command_bridge::pick_remote_commits))
    .route("/invoke/undo_pick_remote_commits", post(tauri_command_bridge::undo_pick_remote_commits))
    .route("/invoke/undo_unapply_branch", post(tauri_command_bridge::undo_unapply_branch))
    .route("/invoke/reapply_branch", post(tauri_command_bridge::reapply_branch))
    .route("/invoke/undo_reapply_branch", post(tauri_command_bridge::undo_reapply_branch))
    .route("/invoke/get_stack_independence", post(tauri_command_bridge::get_stack_independence))
    .route("/invoke/find_conflict_fix", post(tauri_command_bridge::find_conflict_fix))
    .route("/invoke/cancel_conflict_fix", post(tauri_command_bridge::cancel_conflict_fix))
    .route("/invoke/apply_conflict_fix", post(tauri_command_bridge::apply_conflict_fix))
    .route("/invoke/squash_commits", post(tauri_command_bridge::squash_commits))
    .route("/invoke/restore_branch_head", post(tauri_command_bridge::restore_branch_head))
    .route("/invoke/get_autosquash_plan", post(tauri_command_bridge::get_autosquash_plan))
    .route("/invoke/squash_fixup_commits", post(tauri_command_bridge::squash_fixup_commits))
    .route("/invoke/update_project", post(tauri_command_bridge::update_project))
    .route("/invoke/dismiss_landed", post(tauri_command_bridge::dismiss_landed))
    .route("/invoke/fetch_baseline", post(tauri_command_bridge::fetch_baseline))
    .route("/invoke/cancel_lock_wait", post(tauri_command_bridge::cancel_lock_wait))
    .route("/invoke/get_github_auth_status", post(tauri_command_bridge::get_github_auth_status))
    .route("/invoke/start_github_sign_in", post(tauri_command_bridge::start_github_sign_in))
    .route("/invoke/wait_github_sign_in", post(tauri_command_bridge::wait_github_sign_in))
    .route("/invoke/cancel_github_sign_in", post(tauri_command_bridge::cancel_github_sign_in))
    .route("/invoke/sign_out_of_github", post(tauri_command_bridge::sign_out_of_github))
    .route("/invoke/get_youtrack_auth_status", post(tauri_command_bridge::get_youtrack_auth_status))
    .route("/invoke/start_youtrack_sign_in", post(tauri_command_bridge::start_youtrack_sign_in))
    .route("/invoke/wait_youtrack_sign_in", post(tauri_command_bridge::wait_youtrack_sign_in))
    .route("/invoke/cancel_youtrack_sign_in", post(tauri_command_bridge::cancel_youtrack_sign_in))
    .route("/invoke/save_youtrack_token", post(tauri_command_bridge::save_youtrack_token))
    .route("/invoke/sign_out_of_youtrack", post(tauri_command_bridge::sign_out_of_youtrack))
    .route("/invoke/get_youtrack_issue", post(tauri_command_bridge::get_youtrack_issue))
    .route("/invoke/get_new_pull_request", post(tauri_command_bridge::get_new_pull_request))
    .route("/invoke/search_github_users", post(tauri_command_bridge::search_github_users))
    .route("/invoke/fetch_review_head", post(tauri_command_bridge::fetch_review_head))
    .route("/invoke/cancel_review_fetch", post(tauri_command_bridge::cancel_review_fetch))
    .route("/invoke/prune_review_refs", post(tauri_command_bridge::prune_review_refs))
    .route("/invoke/get_range_files", post(tauri_command_bridge::get_range_files))
    .route("/invoke/get_branch_review", post(tauri_command_bridge::get_branch_review))
    .route("/invoke/get_new_stack_pull_request", post(tauri_command_bridge::get_new_stack_pull_request))
    .route("/invoke/prefetch_branch_review", post(tauri_command_bridge::prefetch_branch_review))
    .route("/invoke/get_file_owners", post(tauri_command_bridge::get_file_owners))
    .route("/invoke/get_robot_stats", post(tauri_command_bridge::get_robot_stats))
    .route("/invoke/get_robot_failure", post(tauri_command_bridge::get_robot_failure))
    .route("/invoke/get_robot_commit_map", post(tauri_command_bridge::get_robot_commit_map))
    .route("/invoke/delete_archived_branch", post(tauri_command_bridge::delete_archived_branch))
    .route("/invoke/get_archived_branch_commits", post(tauri_command_bridge::get_archived_branch_commits))
    .route("/invoke/get_unapplied_branch_commits", post(tauri_command_bridge::get_unapplied_branch_commits))
    .route("/invoke/delete_unapplied_branch", post(tauri_command_bridge::delete_unapplied_branch))
    .route("/invoke/amend_uncommitted_to_branch", post(tauri_command_bridge::amend_uncommitted_to_branch))
    .route("/invoke/get_uncommitted_changes", post(tauri_command_bridge::get_uncommitted_changes))
    .route("/invoke/get_commit_files", post(tauri_command_bridge::get_commit_files))
    .route("/invoke/get_commits_files", post(tauri_command_bridge::get_commits_files))
    .route("/invoke/get_file_diffs", post(tauri_command_bridge::get_file_diffs))
    .route("/invoke/get_file_merges", post(tauri_command_bridge::get_file_merges))
    .route("/invoke/get_blob_texts", post(tauri_command_bridge::get_blob_texts))
    .route("/invoke/get_blob_syntax", post(tauri_command_bridge::get_blob_syntax))
    .route("/invoke/find_declaration", post(tauri_command_bridge::find_declaration))
    .route("/invoke/find_usages", post(tauri_command_bridge::find_usages))
    .route("/invoke/cancel_symbol_search", post(tauri_command_bridge::cancel_symbol_search))
    .route("/invoke/browse_repository/{repo_id}", post(tauri_command_bridge::browse_repository))
    .route("/invoke/suggest_branch_names", post(tauri_command_bridge::suggest_branch_names))
    // the native menu: a test counts the requests of the page
    .route("/invoke/set_auto_sync_menu_checked", post(tauri_command_bridge::set_auto_sync_menu_checked))
    .route("/invoke/set_setting", post(tauri_command_bridge::set_setting))
    .route("/invoke/get_settings_changes", post(tauri_command_bridge::get_settings_changes))
    .route("/invoke/sub_window_loaded", post(tauri_command_bridge::sub_window_loaded))
    .route("/invoke/sub_window_drawn", post(tauri_command_bridge::sub_window_drawn))
    .route("/invoke/agent_cli_command", post(tauri_command_bridge::agent_cli_command))
    .route("/invoke/install_agent_cli", post(tauri_command_bridge::install_agent_cli))
    .route("/invoke/agent_cli_status", post(tauri_command_bridge::agent_cli_status))
    .route("/invoke/cli_tools_status", post(cli_tools_fixtures::cli_tools_status))
    .route("/invoke/run_cli_tool_action", post(cli_tools_fixtures::run_cli_tool_action))
    .route("/invoke/cancel_cli_tool_action", post(cli_tools_fixtures::cancel_cli_tool_action))
    .route("/invoke/get_setup_items", post(cli_tools_fixtures::get_setup_items))
    .route("/invoke/start_youtrack_cli_sign_in", post(cli_tools_fixtures::start_youtrack_cli_sign_in))
    .route("/invoke/wait_youtrack_cli_sign_in", post(cli_tools_fixtures::wait_youtrack_cli_sign_in))
    // the updater, by the update of the fixture; the install and the restart of the app fail
    .route("/invoke/check_for_updates", post(updater::check_for_updates))
    .route("/invoke/get_update_status", post(updater::get_update_status))
    .route("/invoke/install_update", post(updater::install_update))
    .route("/invoke/restart_app", post(updater::restart_app))
    // Health check
    .route("/health", get(health_check))
    // Serve static files from dist with SPA fallback
    .fallback_service(serve_static_files())
    .layer(axum::middleware::from_fn_with_state(Arc::clone(&state), hold_repository))
    .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
    .layer(TraceLayer::new_for_http())
    .with_state(state)
}

/// The header with which the page mock names the test repository of its page (`?repoId=`) on every request
pub const REPOSITORY_ID_HEADER: &str = "x-repository-id";

/// Holds the repository named by `X-Repository-Id` while a command of the page runs (`TestRepository::in_flight`), so
/// the deletion (`remove_repository`) waits for the commands still running for it instead of deleting their files under
/// them.
///
/// The command runs in a task of its own that holds the lock: a test that ends closes its page, the browser aborts the
/// requests, and the server drops their handlers, but the git work of a command (`spawn_blocking`) cannot be canceled
/// and runs on. Most commands do all their work before they answer (a sync streams events it has already sent), so the
/// handler is the whole command; a command that answers while it runs (`channel_commands`: SSE events of its channel,
/// then its answer) holds the lock until its event stream ends, read to its end also when the page went away. A
/// request for a deleted repository is a page that outlived its test.
async fn hold_repository(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
  let Some(id) = request.headers().get(REPOSITORY_ID_HEADER).and_then(|value| value.to_str().ok()).map(str::to_owned) else {
    return next.run(request).await;
  };
  let Some(in_flight) = state.repositories.get(&id).map(|repo| Arc::clone(&repo.in_flight)) else {
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
    let response = next.run(request).await;
    let streams = response
      .headers()
      .get(axum::http::header::CONTENT_TYPE)
      .is_some_and(|value| value.as_bytes().starts_with(b"text/event-stream"));
    if !streams {
      drop(command);
      return response;
    }
    // the events go on to the page while the command runs; the lock is held until the last one
    let (parts, body) = response.into_parts();
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(
      async move {
        let _command = command;
        let mut events = body.into_data_stream();
        while let Some(chunk) = tokio_stream::StreamExt::next(&mut events).await {
          let _ = sender.send(chunk);
        }
      }
      .in_current_span(),
    );
    Response::from_parts(parts, axum::body::Body::from_stream(tokio_stream::wrappers::UnboundedReceiverStream::new(receiver)))
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
  #[serde(rename = "commit_deleted")]
  CommitDeleted,
  #[serde(rename = "commit_many_files")]
  CommitManyFiles,
  #[serde(rename = "commit_whitespace")]
  CommitWhitespace,
  #[serde(rename = "symbol_navigation")]
  SymbolNavigation,
  #[serde(rename = "script_colors")]
  ScriptColors,
  #[serde(rename = "stacked_branches")]
  StackedBranches,
  #[serde(rename = "stack_island")]
  StackIsland,
  #[serde(rename = "readme_showcase")]
  ReadmeShowcase,
  #[serde(rename = "branch_table_large")]
  BranchTableLarge,
  #[serde(rename = "stack_side_conflict")]
  StackSideConflict,
  #[serde(rename = "stacked_issues")]
  StackedIssues,
  #[serde(rename = "stacked_conflict")]
  StackedConflict,
  #[serde(rename = "conflict_fix")]
  ConflictFix,
  #[serde(rename = "conflict_words")]
  ConflictWords,
  #[serde(rename = "resolved_words")]
  ResolvedWords,
  #[serde(rename = "stacked_ownership")]
  StackedOwnership,
  #[serde(rename = "stack_push_alone")]
  StackPushAlone,
  #[serde(rename = "stack_uses_parent")]
  StackUsesParent,
  #[serde(rename = "landing_needs")]
  LandingNeeds,
  #[serde(rename = "landing_needs_fork")]
  LandingNeedsFork,
  #[serde(rename = "stack_problems")]
  StackProblems,
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
  #[serde(rename = "update_words")]
  UpdateWords,
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
  pub const fn as_str(&self) -> &'static str {
    match self {
      Self::Simple => "simple",
      Self::SimpleNoPrefix => "simple_no_prefix",
      Self::Unassigned => "unassigned",
      Self::ConflictUnassigned => "conflict_unassigned",
      Self::ConflictBranches => "conflict_branches",
      Self::SingleUnassigned => "single_unassigned",
      Self::IssueLinks => "issue_links",
      Self::IssueSplit => "issue_split",
      Self::IssueGroups => "issue_groups",
      Self::IssueCombine => "issue_combine",
      Self::Fixups => "fixups",
      Self::LongSubjects => "long_subjects",
      Self::CommitTree => "commit_tree",
      Self::CommitDeleted => "commit_deleted",
      Self::CommitManyFiles => "commit_many_files",
      Self::CommitWhitespace => "commit_whitespace",
      Self::SymbolNavigation => "symbol_navigation",
      Self::ScriptColors => "script_colors",
      Self::StackedBranches => "stacked_branches",
      Self::StackIsland => "stack_island",
      Self::ReadmeShowcase => "readme_showcase",
      Self::BranchTableLarge => "branch_table_large",
      Self::StackSideConflict => "stack_side_conflict",
      Self::StackedIssues => "stacked_issues",
      Self::StackedConflict => "stacked_conflict",
      Self::ConflictFix => "conflict_fix",
      Self::ConflictWords => "conflict_words",
      Self::ResolvedWords => "resolved_words",
      Self::StackedOwnership => "stacked_ownership",
      Self::StackPushAlone => "stack_push_alone",
      Self::StackUsesParent => "stack_uses_parent",
      Self::LandingNeeds => "landing_needs",
      Self::LandingNeedsFork => "landing_needs_fork",
      Self::StackProblems => "stack_problems",
      Self::BranchFilter => "branch_filter",
      Self::ArchivedBranches => "archived_branches",
      Self::AmendChanges => "amend_changes",
      Self::BehindUpstream => "behind_upstream",
      Self::UpdateConflict => "update_conflict",
      Self::UpdateWords => "update_words",
      Self::UpdateConflictWide => "update_conflict_wide",
      Self::UpdateBlocked => "update_blocked",
      Self::LandedUpstream => "landed_upstream",
      Self::EmptyNonGit => "empty-non-git",
      Self::EmptyGit => "empty_git",
      Self::Bare => "bare",
      Self::NoRepo => "NO_REPO",
    }
  }
}

#[derive(Deserialize)]
pub struct CreateRepositoryRequest {
  pub template: RepositoryTemplate,
  #[serde(default = "default_prepopulate_store")]
  pub prepopulate_store: bool,
}

const fn default_prepopulate_store() -> bool {
  true
}

/// The start of the id of a test repository, which `REPOSITORY_ID_DIGITS` digits follow: 27 characters, as the KSUIDs
/// before, so the repository selector shortens every path to `…/<id>` on every platform. The directory of the
/// repository is named by it
pub(crate) const REPOSITORY_ID_PREFIX: &str = "repo-";
pub(crate) const REPOSITORY_ID_DIGITS: usize = 22;
/// The path of a `NO_REPO` repository, which does not exist: this, then its id
pub(crate) const NO_REPO_PREFIX: &str = "NO_REPO_";

pub async fn create_repository(State(state): State<Arc<AppState>>, Json(request): Json<CreateRepositoryRequest>) -> Result<Json<CreateRepositoryResponse>, StatusCode> {
  // unique for the life of the server: the repositories live in its own temporary directory
  static NEXT_ID: AtomicU64 = AtomicU64::new(1);
  let id = format!("{REPOSITORY_ID_PREFIX}{:0width$}", NEXT_ID.fetch_add(1, Ordering::Relaxed), width = REPOSITORY_ID_DIGITS);

  // Handle special NO_REPO template
  if matches!(request.template, RepositoryTemplate::NoRepo) {
    // Use a special path that doesn't exist
    let path = format!("{NO_REPO_PREFIX}{id}");

    let mut store = Vec::new();
    if request.prepopulate_store {
      // the invalid path, so the page tries to load it; no cachedBranchPrefix (it's optional)
      store.push(("recentProjects", serde_json::json!([{ "path": path }])));
      tracing::info!("Populated NO_REPO store with invalid path: {}", path);
    }

    insert_repository(&state, &id, &path, &store)?;
    // Don't insert into path_to_id for NO_REPO

    tracing::info!("Created NO_REPO repository with id: {}", id);

    return Ok(Json(CreateRepositoryResponse { id, path }));
  }

  // Normal repository creation
  let repo_dir = state.test_root_dir.path().join(&id);
  let path = repo_dir.to_string_lossy().to_string();

  let template_path = state.templates.path(request.template.as_str()).await.map_err(|e| {
    tracing::error!("Template '{}' not found: {e:#}", request.template.as_str());
    StatusCode::BAD_REQUEST
  })?;
  let target = repo_dir.clone();
  tokio::task::spawn_blocking(in_current_span(move || copy_template(&template_path, &target)))
    .await
    .map_err(|e| anyhow::anyhow!(e))
    .and_then(|copied| copied)
    .map_err(|e| {
      tracing::error!("Failed to copy template: {e:#}");
      StatusCode::INTERNAL_SERVER_ERROR
    })?;

  // Pre-populate store with default values if requested and not empty-non-git template
  let mut store = Vec::new();
  if request.prepopulate_store && !matches!(request.template, RepositoryTemplate::EmptyNonGit | RepositoryTemplate::Bare) {
    store.push((
      "recentProjects",
      serde_json::json!([{
        "path": path,
        "cachedBranchPrefix": { "prefix": "user-name", "source": "repository_config" }
      }]),
    ));
  }

  insert_repository(&state, &id, &path, &store)?;
  state.path_to_id.insert(path.clone(), id.clone());

  tracing::info!("Created repository with id: {} at path: {}", id, path);

  Ok(Json(CreateRepositoryResponse { id, path }))
}

/// The repository of a test from its template at `template`: one APFS clone of the whole tree on macOS, which shares the
/// blocks of the template until the test writes (a copy took a tenth of a second under a full run), else a copy
fn copy_template(template: &std::path::Path, repo_dir: &std::path::Path) -> anyhow::Result<()> {
  match lease_pool::clone_dir::clone_directory(template, repo_dir, &lease_pool::clone_dir::CloneSkips::default()) {
    Ok(()) => return Ok(()),
    Err(e) if e.kind() == std::io::ErrorKind::Unsupported => {}
    Err(e) => {
      // another volume, or no APFS: the same for every repository of the run
      static WARNED: std::sync::Once = std::sync::Once::new();
      WARNED.call_once(|| tracing::warn!(error = %e, "cannot clone the templates: copied instead"));
      let _ = fs_retry::remove_dir_all(repo_dir);
    }
  }
  std::fs::create_dir_all(repo_dir)?;
  let mut copy_options = fs_extra::dir::CopyOptions::new();
  copy_options.content_only = true;
  fs_extra::dir::copy(template, repo_dir, &copy_options)?;
  Ok(())
}

/// The origin of the changes of the settings the test server makes (`/store` routes, the prepopulated values)
const FIXTURE_ORIGIN: &str = "test";

/// A change of the settings by the fixture of a test: the whole value of `key`, `null` removes it
fn set_fixture_value(store: &app_settings::service::SettingsService, key: &str, value: serde_json::Value) -> Result<(), StatusCode> {
  let patch = domain_types::settings::SettingsPatch::Value { key: key.to_owned(), value };
  store.set(patch, FIXTURE_ORIGIN).map(drop).map_err(|e| {
    tracing::error!(key, error = %e, "the fixture cannot change the settings");
    StatusCode::UNPROCESSABLE_ENTITY
  })
}

fn insert_repository(state: &AppState, id: &str, path: &str, values: &[(&str, serde_json::Value)]) -> Result<(), StatusCode> {
  let app_data = state.app_data_of(id);
  let store = app_settings::service::SettingsService::open(app_data.clone()).map_err(|e| {
    tracing::error!(error = %e, "cannot start the writer of the settings");
    StatusCode::INTERNAL_SERVER_ERROR
  })?;
  for (key, value) in values {
    set_fixture_value(&store, key, value.clone())?;
  }
  let repo = TestRepository {
    id: id.to_string(),
    path: path.to_string(),
    app_data,
    in_flight: Arc::default(),
    store,
    pull_request_calls: std::sync::atomic::AtomicU32::new(0),
    review_requests: std::sync::RwLock::default(),
    review_request_calls: std::sync::atomic::AtomicU32::new(0),
    reviews_calls: std::sync::atomic::AtomicU32::new(0),
    review_fetch_calls: std::sync::atomic::AtomicU32::new(0),
    review_prune_keeps: std::sync::Mutex::default(),
    remote: std::sync::OnceLock::new(),
    patronus_list_calls: std::sync::atomic::AtomicU32::new(0),
    patronus_poll_calls: std::sync::atomic::AtomicU32::new(0),
    patronus_answered: std::sync::Mutex::new(std::collections::BTreeMap::new()),
    patronus_detail_calls: std::sync::atomic::AtomicU32::new(0),
    review_readiness_calls: std::sync::Mutex::default(),
    branch_name_suggestions: std::sync::RwLock::new(state::BranchNameSuggestionFixture::default()),
    branch_name_suggestion_calls: std::sync::atomic::AtomicU32::new(0),
    agent_cli_installed: std::sync::atomic::AtomicBool::new(false),
    available_update: std::sync::RwLock::default(),
    cli_tools: Arc::new(cli_tools_fixtures::CliToolsHost::new(&state.app_data_of(id))),
    held_lock: std::sync::Mutex::default(),
    foreign_operation: Arc::default(),
    sync_overrides: std::sync::Mutex::default(),
  };
  state.repositories.insert(id.to_string(), repo);
  Ok(())
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
  let baseline_branch = detect_baseline_branch(&state.git_executor, &path).map_err(|e| {
    tracing::warn!(error = %e, "Cannot detect the baseline branch of the repository");
    StatusCode::UNPROCESSABLE_ENTITY
  })?;
  match reword_commits_batch(&state.git_executor, &path, &baseline_branch, rewrites) {
    Ok(mapping) => Ok(Json(serde_json::json!({ "newCommitId": mapping.get(&request.commit_id) }))),
    Err(e) => {
      tracing::error!("Failed to reword commit {}: {}", request.commit_id, e);
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

#[derive(Deserialize)]
pub struct CommitRequest {
  message: String,
  /// Path -> content of the files the commit writes
  files: Vec<(String, String)>,
  /// Unix seconds of the author and committer dates
  timestamp: i64,
}

/// Commits files onto HEAD as a commit in a terminal would: lets an E2E test add work behind the app's back (a new commit
/// of a branch). Returns the new commit id.
pub async fn commit_on_head(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
  Json(request): Json<CommitRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
  let path = state.repositories.get(&id).map(|repo| repo.path.clone()).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let files: Vec<(&str, &str)> = request.files.iter().map(|(name, content)| (name.as_str(), content.as_str())).collect();
  match test_utils::git_test_utils::commit_files_on_head(std::path::Path::new(&path), &request.message, &files, request.timestamp) {
    Ok(commit) => Ok(Json(serde_json::json!({ "commitId": commit }))),
    Err(e) => {
      tracing::error!("Failed to commit onto HEAD: {e}");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// `POST /repositories/{id}/repository-lock`: whether the server holds the repository lock
#[derive(Deserialize)]
pub struct RepositoryLockRequest {
  held: bool,
}

/// Takes the repository lock of the repository (`$GIT_COMMON_DIR/branch-deck.lock`, with its holder record) or releases
/// it, as another process holds it (`bd pick --follow` of an agent, `lock_repository_as_another_process`: outside the
/// line of the server's process, in which the app's operations wait for each other): its record names `bd pick
/// --follow` and the server's pid, alive while it runs
pub async fn hold_repository_lock(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
  Json(request): Json<RepositoryLockRequest>,
) -> Result<StatusCode, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let mut held = repo.held_lock.lock().expect("held lock");
  *held = None;
  if request.held {
    let common_dir = std::path::Path::new(&repo.path).join(".git");
    let lock = history_rewrite::repository_lock::lock_repository_as_another_process(&common_dir, "bd pick --follow").map_err(|e| {
      tracing::error!("Cannot take the repository lock: {e:#}");
      StatusCode::CONFLICT
    })?;
    *held = Some(lock);
  }
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
  Ok(Json(serde_json::json!({ "calls": repo.branch_name_suggestion_calls.load(Ordering::SeqCst) })))
}

/// Sets what the session of the repository reads of its review requests and how `fetch_review_head` fails (`fetchError`)
pub async fn set_review_requests(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
  Json(fixture): Json<state::ReviewRequestsFixture>,
) -> Result<StatusCode, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  // GitHub has the pull requests the user is asked to review: the reviews and the comments go there
  if let domain_types::pull_request::PullRequestAvailability::Available { owner, name } = &fixture.result.availability {
    let (pulls, nodes) = review_fixtures::fixture_pulls(&fixture);
    let remote = repo.remote(&state.git_executor);
    remote.add_review_pulls(owner, name, pulls);
    remote.set_pull_request_nodes(nodes);
  }
  *repo.review_requests.write().expect("review request fixture lock") = fixture;
  Ok(StatusCode::OK)
}

/// How often the app asked for review requests and fetched a review, and the `keep` of every prune
pub async fn get_review_request_fixture(axum::extract::Path(id): axum::extract::Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let ordering = Ordering::SeqCst;
  Ok(Json(serde_json::json!({
    "calls": repo.review_request_calls.load(ordering),
    "reviewsCalls": repo.reviews_calls.load(ordering),
    "fetchCalls": repo.review_fetch_calls.load(ordering),
    "prunes": repo.review_prune_keeps.lock().expect("review prune lock").clone(),
  })))
}

/// How long a deletion waits for the commands still running for the repository: a sync of a loaded machine takes tens of
/// seconds after its test failed; a command that takes longer is stuck
const DELETE_WAIT: std::time::Duration = std::time::Duration::from_secs(120);

/// Forgets the repository, so no new command starts for it, and answers: its files go in the background
/// (`remove_repository`), once the commands still running for it ended, which may outlast its test; `NO_REPO` has none
pub async fn delete_repository(axum::extract::Path(id): axum::extract::Path<String>, State(state): State<Arc<AppState>>) -> Result<StatusCode, StatusCode> {
  let Some((_, repo)) = state.repositories.remove(&id) else {
    return Err(StatusCode::UNPROCESSABLE_ENTITY);
  };
  state.path_to_id.remove(&repo.path);
  // a window's subscription streams until its session closes, and holds the repository while it does
  state.sessions.close_worktree(&repo.path);
  state.session_clocks.forget(&repo.path);
  // a sign-in the page left waiting on GitHub would hold the repository until its code expires
  if let Some(host) = repo.existing_remote() {
    host.end_sign_ins();
    // and an answer the test held at a fake until it is released
    host.release_all_answers();
  }
  tokio::spawn(remove_repository(state, id, repo).in_current_span());
  Ok(StatusCode::ACCEPTED)
}

/// Releases the repository lock the test of a deleted repository left held, then waits for the commands still running
/// for it (`hold_repository`) and for the computation of its landing needs (its session holds the repository while it
/// runs, `TestSessionEnvironment::landing_needs`), then deletes its files (`fs_retry`: on Windows a process whose
/// current directory is in them, or a scanner with a file open, blocks the removal until it lets go)
async fn remove_repository(state: Arc<AppState>, id: String, repo: TestRepository) {
  // the lock first (`hold_repository_lock`), its holder record with it: a rewrite of the page that waits for it, as
  // long as its holder (this server) lives, runs and ends instead of keeping the deletion waiting
  drop(repo.held_lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take());
  // the process of the agent's operation ends with its record
  let operation = repo.foreign_operation.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
  if let Some(operation) = operation {
    operation.end();
  }
  let no_commands = tokio::time::timeout(DELETE_WAIT, Arc::clone(&repo.in_flight).write_owned()).await;
  if no_commands.is_err() {
    tracing::warn!(id, "Commands for the test repository still run after {DELETE_WAIT:?}; deleting it anyway");
  }
  // the writer saves what is pending now, not when the repository is dropped after its app data is gone
  if let Err(e) = repo.store.flush() {
    tracing::warn!("Failed to save the settings of {id}: {e}");
  }
  if repo.app_data.exists()
    && let Err(e) = fs_retry::remove_dir_all(&repo.app_data)
  {
    tracing::warn!("Failed to remove the app data {}: {}", repo.app_data.display(), e);
  }
  let global_config = state.global_config_of(&id);
  if let Err(e) = fs_retry::remove_file(&global_config)
    && e.kind() != std::io::ErrorKind::NotFound
  {
    tracing::warn!("Failed to remove the global config {}: {}", global_config.display(), e);
  }

  let directory = PathBuf::from(&repo.path);
  if directory.starts_with(state.test_root_dir.path()) {
    let removed = tokio::task::spawn_blocking(move || fs_retry::remove_dir_all(&directory)).await;
    match removed {
      Ok(Ok(())) => {}
      Ok(Err(e)) => tracing::warn!("Failed to remove repository directory {}: {}", repo.path, e),
      Err(e) => tracing::error!("Failed to remove repository directory {}: {}", repo.path, e),
    }
  }

  tracing::info!("Deleted repository: {}", id);
}

// Store API endpoints: the fixture of a test reads and writes the settings through the writer of the app

/// The settings a window of the app starts from, with their version (`__TAURI_STORE__`, `__TAURI_STORE_VERSION__`)
pub async fn get_store_snapshot(axum::extract::Path(repo_id): axum::extract::Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, StatusCode> {
  let repo = state.repositories.get(&repo_id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let (version, values) = app_settings::windows::window_snapshot(&repo.store, app_settings::windows::MAIN_WINDOW);
  Ok(Json(serde_json::json!({ "version": version, "values": values })))
}

pub async fn get_store_value(
  axum::extract::Path((repo_id, key)): axum::extract::Path<(String, String)>,
  State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
  if let Some(repo) = state.repositories.get(&repo_id) {
    // a key the settings lack is null, not an error
    let (_, values) = repo.store.snapshot();
    return Ok(Json(values.get(&key).cloned().unwrap_or(serde_json::Value::Null)));
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
    set_fixture_value(&repo.store, &key, value)?;
    return Ok(StatusCode::OK);
  }

  Err(StatusCode::UNPROCESSABLE_ENTITY)
}

pub async fn delete_store_value(axum::extract::Path((repo_id, key)): axum::extract::Path<(String, String)>, State(state): State<Arc<AppState>>) -> Result<StatusCode, StatusCode> {
  if let Some(repo) = state.repositories.get(&repo_id) {
    set_fixture_value(&repo.store, &key, serde_json::Value::Null)?;
    return Ok(StatusCode::NO_CONTENT);
  }

  Err(StatusCode::UNPROCESSABLE_ENTITY)
}
