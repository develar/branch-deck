//! The E2E mock server (docs/design/e2e-testing.md): a host of app-api for the page in a browser, so a Playwright test
//! runs the commands of the app on the cores the app runs, against fakes of the remote services.
//! - `invoke`: one route `/invoke/<name>` per entry of the registry (`app_api::commands`), SSE for a channel
//! - `host`, `session_routes`, `state`: what the app gives the commands (the desktop, the event streams of the pages,
//!   the sessions of each test repository)
//! - `remote_host`, `remote_fixtures`, `remote_git_fixtures`, `review_fixtures`, `youtrack_fixtures`,
//!   `cli_tools_fixtures`: the fakes of each test repository and the fixture routes that set and read them
//! - `disk_fixtures`: the lease slots of the fake home of a test repository and the hosts of `bd-host` of the Disk
//!   window
//! - `templates`, `sync_overrides`, `updater`, `static_files`: the repositories a test copies, what it changes of the
//!   next sync, the updater of its app, the built page

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

pub mod cli_tools_fixtures;
pub mod companion_fixtures;
pub mod disk_fixtures;
pub mod host;
pub mod invoke;
pub mod mcp_socket;
pub mod remote_fixtures;
pub mod remote_git_fixtures;
pub mod remote_host;
pub mod review_fixtures;
pub mod session_routes;
pub mod state;
pub mod static_files;
pub mod sync_overrides;
pub mod templates;
pub mod updater;
pub mod youtrack_fixtures;

#[cfg(test)]
mod api_error_contract_test;

#[cfg(test)]
mod app_events_test;

#[cfg(test)]
mod branch_prefix_test;

#[cfg(test)]
mod command_registry_test;

#[cfg(test)]
mod commit_files_test;

#[cfg(test)]
mod mcp_corpus_test;

#[cfg(test)]
mod github_host_test;

#[cfg(test)]
mod repository_config_test;

#[cfg(test)]
mod repository_lifecycle_test;

#[cfg(test)]
mod review_plan_test;

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
mod session_routes_test;

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
  create_app_and_state(true, git_executor::git_command_executor::GitCommandExecutor::new(), DELETE_WAIT)
}

/// The app of a route test: a template is built when a repository is first made of it, in the temporary directory of the
/// test. A test asks for one or two of the templates, and nextest runs each test in a process of its own, which would
/// build all of them
pub fn create_route_test_app_with_state() -> (Router, Arc<AppState>) {
  create_app_and_state(false, git_executor::git_command_executor::GitCommandExecutor::new(), DELETE_WAIT)
}

/// The app of a route test whose every git, its sessions' and its commands', is `git`'s (`WrappedGit`), and whose
/// deletion of a repository waits `delete_wait` for what still runs for it (`AppState::delete_wait`)
#[cfg(test)]
pub(crate) fn create_route_test_app_with_git(git: git_executor::git_command_executor::GitCommandExecutor, delete_wait: std::time::Duration) -> (Router, Arc<AppState>) {
  create_app_and_state(false, git, delete_wait)
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
  .unflushed()
}

/// The journal of a repository changed: the pages of each test repository whose session watches it hear
/// `activity-changed`, as the app's windows hear it (`RegistryFiles::on_journal`)
fn journal_changed(state: &std::sync::Weak<AppState>, watching: &[repository_model::session::SessionKey]) {
  let Some(state) = state.upgrade() else {
    return;
  };
  let ids: std::collections::BTreeSet<String> = watching.iter().filter_map(|key| state.path_to_id.get(&key.worktree).map(|id| id.clone())).collect();
  for id in ids {
    if let Some(repository) = state.repositories.get(&id) {
      app_api::events::activity_changed(&*repository.events);
    }
  }
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

fn create_app_and_state(all_templates: bool, git_executor: git_executor::git_command_executor::GitCommandExecutor, delete_wait: std::time::Duration) -> (Router, Arc<AppState>) {
  // Create a single temp directory for all test repositories
  let test_root_dir = tempfile::tempdir().expect("Failed to create test root directory");
  tracing::info!("Test root directory created at: {:?}", test_root_dir.path());

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
    // the operations of the app are recorded in the registry of each test repository, as the app records them, a
    // start whose answer was lost is settled by the fakes of its repository, and a change of its journal tells its pages
    let clocks = Arc::clone(&session_clocks);
    let journaled = std::sync::Weak::clone(state);
    let files = registry_files(git_executor.clone())
      .settled_with(session_routes::settle_host(state, git_executor.clone()))
      .at(move |key| registry_now(&clocks, &key.worktree))
      .on_journal(move |watching| journal_changed(&journaled, watching));
    let registry = operations::registry::Registry::new(Arc::clone(&sessions), files);
    AppState {
      repositories: DashMap::new(),
      path_to_id: DashMap::new(),
      git_executor,
      test_root_dir,
      templates,
      branch_prefixes: app_settings::branch_prefixes::BranchPrefixes::default(),
      registry,
      sessions,
      session_clocks,
      repository_state: app_api::repository_state::RepositoryStateCache::without_object_caches(),
      made: DashMap::new(),
      delete_wait,
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
  // the commands of app-api, one route each (`invoke.rs`)
  invoke::command_routes(Router::new())
    // Repository management endpoints
    .route("/repositories", post(create_repository))
    .route("/repositories/{id}", get(get_repository))
    .route("/repositories/{id}", axum::routing::delete(delete_repository))
    // `brd mcp` on a WebSocket, for the fake host of the page in Air (`mcp_socket.rs`)
    .route("/repositories/{id}/mcp", get(mcp_socket::mcp_socket))
    .route("/repositories/{id}/reword", post(reword_commit))
    .route("/repositories/{id}/commit", post(commit_on_head))
    .route("/repositories/{id}/update-outside", post(update_outside_the_app))
    .route("/repositories/{id}/sync-outside", post(sync_outside_the_app))
    .route("/repositories/{id}/locks/{lock}", post(hold_lock))
    .route("/repositories/{id}/foreign-operation", post(remote_fixtures::set_foreign_operation))
    .route("/repositories/{id}/stamp", post(remote_fixtures::touch_repository_stamp))
    .route("/repositories/{id}/agent-operation", post(remote_fixtures::journal_agent_operation))
    .route("/repositories/{id}/journal-gone", post(remote_fixtures::lose_journal))
    .route("/repositories/{id}/sync-overrides", post(sync_overrides::set_sync_overrides))
    .route("/repositories/{id}/revision", get(sync_overrides::get_revision))
    .route("/repositories/{id}/rebase-in-progress", post(sync_overrides::set_rebase_in_progress))
    .route("/repositories/{id}/branch-prefix", post(sync_overrides::set_branch_prefix_in_terminal))
    .route("/repositories/{id}/user-name", post(sync_overrides::set_user_name_in_terminal))
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
    .route("/repositories/{id}/remote-gates/{answer}/arrival", get(remote_fixtures::get_remote_gate_arrival))
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
    .route("/repositories/{id}/landing", post(remote_git_fixtures::set_landing).get(remote_git_fixtures::get_landing))
    .route("/repositories/{id}/github-answer", post(review_fixtures::set_github_answer))
    .route("/repositories/{id}/update", post(updater::set_update))
    .route("/repositories/{id}/host-disks", post(disk_fixtures::set_host_disks))
    .route("/repositories/{id}/lease-slots", post(disk_fixtures::add_lease_slots))
    .route("/repositories/{id}/lease-slot", post(disk_fixtures::lease_slot))
    .route(
      "/repositories/{id}/cli-tools",
      post(cli_tools_fixtures::set_cli_tools).get(cli_tools_fixtures::get_cli_tools),
    )
    .route("/repositories/{id}/sessions/close", post(session_routes::close_sessions))
    .route("/repositories/{id}/warm-start", get(session_routes::warm_start))
    .route("/repositories/{id}/warm-start/build", post(session_routes::build_warm_start))
    .route("/repositories/{id}/clock/advance", post(session_routes::advance_clock))
    // Store endpoints
    .route("/store/{repo_id}", get(get_store_snapshot))
    .route("/store/{repo_id}/{key}", get(get_store_value))
    .route("/store/{repo_id}/{key}", post(set_store_value))
    .route("/store/{repo_id}/{key}", axum::routing::delete(delete_store_value))
    // the events of the app to the pages of a repository, and the events of its pages to each other (`host.rs`)
    .route("/repositories/{id}/events", get(host::subscribe_events).post(host::emit_page_event))
    // the desktop of the app: what it opened, the build of a page a test opens, the folder its dialog picks (`host.rs`)
    .route("/repositories/{id}/windows", get(host::get_windows))
    .route("/repositories/{id}/windows/built", post(host::built_window))
    .route("/repositories/{id}/dialog-pick", post(host::set_dialog_pick))
    .route("/repositories/{id}/companion", post(companion_fixtures::add_companion))
    .route("/repositories/{id}/companion-link", post(companion_fixtures::set_companion_link))
    // Health check
    .route("/health", get(health_check))
    .route("/debug/counts", get(debug_counts))
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
/// handler is the whole command; a command that answers while it runs (`invoke::answer_with_progress`: SSE events of
/// its channel, then its answer) holds the lock until its event stream ends, read to its end also when the page went
/// away. A request for a deleted repository is a page that outlived its test.
async fn hold_repository(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
  let Some(id) = request.headers().get(REPOSITORY_ID_HEADER).and_then(|value| value.to_str().ok()).map(str::to_owned) else {
    return next.run(request).await;
  };
  let Some(in_flight) = state.repositories.get(&id).map(|repo| Arc::clone(&repo.in_flight)) else {
    tracing::warn!(id, uri = %request.uri(), "Request for a deleted test repository");
    return StatusCode::GONE.into_response();
  };
  // none once a deletion began; a deletion that came first has removed it and waits out the commands that hold it
  let command = in_flight.hold(request.uri().path());
  if command.is_none() || !state.repositories.contains_key(&id) {
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

/// What the server holds now: its test repositories with the test of each, the deletions still in progress, the apps of
/// deleted repositories still held, the sessions open, the watches of the registries with their looks and the waits the
/// clocks of deleted repositories served, which tell what a test left behind once it ended (a repository, a session, a
/// watch that looks every 2 s, a loop on a clock that ran out). An orphan is named by the repository its worktree is of
/// and the test that made it, with what keeps it: a window (`subscribers`), an operation (`operations`), or nothing, an
/// idle session no sweep removes, as the clock of its worktree stands still
async fn debug_counts(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
  let sessions = state.sessions.open_sessions();
  // the sessions of no repository of a test that lives (nor a slot of its pool): what a deleted test left open
  let orphans: Vec<serde_json::Value> = sessions
    .iter()
    .filter(|open| !state.path_to_id.contains_key(&open.key.worktree))
    .map(|open| {
      let repository = repository_id_in(&open.key.worktree);
      serde_json::json!({
        "worktree": open.key.worktree,
        "prefix": open.key.prefix,
        "repository": repository,
        "test": repository.map(|id| state.test_of(id)),
        "subscribers": open.subscribers,
        "operations": open.operations,
      })
    })
    .collect();
  let mut tests: Vec<serde_json::Value> = state
    .repositories
    .iter()
    .map(|repository| serde_json::json!({ "repository": repository.key(), "test": state.test_of(repository.key()) }))
    .collect();
  tests.sort_by(|a, b| a["repository"].as_str().cmp(&b["repository"].as_str()));
  // the deletions that wait for what still runs for their repository, and the apps of deleted repositories that
  // something they started still holds once their deletion ended, with the memory of their diffs
  let named = |made: &dashmap::mapref::multiple::RefMulti<'_, String, state::Made>| serde_json::json!({ "repository": made.key(), "test": made.test });
  let deleted = state.made.iter().filter(|made| !state.repositories.contains_key(made.key()));
  let (mut deleting, mut kept) = (Vec::new(), Vec::new());
  for made in deleted {
    if !made.removed.load(Ordering::SeqCst) {
      deleting.push(named(&made));
    } else if made.context.strong_count() > 0 {
      kept.push(named(&made));
    }
  }
  deleting.sort_by(|a, b| a["repository"].as_str().cmp(&b["repository"].as_str()));
  kept.sort_by(|a, b| a["repository"].as_str().cmp(&b["repository"].as_str()));
  // the waits the clocks of deleted repositories served once they ran out: a few for each wait with a deadline, ever
  // more for a loop without one, which the busiest tell
  let mut run_out = state.session_clocks.run_out_waits();
  run_out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
  let busiest: Vec<serde_json::Value> = run_out
    .iter()
    .take(RUN_OUT_BUSIEST)
    .map(|(worktree, waits)| {
      let repository = repository_id_in(worktree);
      serde_json::json!({ "worktree": worktree, "repository": repository, "test": repository.map(|id| state.test_of(id)), "waits": waits })
    })
    .collect();
  Json(serde_json::json!({
    "repositories": state.repositories.len(),
    "tests": tests,
    "deleting": deleting,
    "keptContexts": kept,
    "runOutWaits": { "total": run_out.iter().map(|(_, waits)| waits).sum::<u64>(), "busiest": busiest },
    "sessions": sessions.len(),
    "orphanSessions": orphans,
    "watches": state.registry.watch_counts(),
  }))
}

/// How many of the clocks that served the most waits once they ran out `/debug/counts` names
const RUN_OUT_BUSIEST: usize = 5;

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
  #[serde(rename = "fixup_order")]
  FixupOrder,
  #[serde(rename = "fixup_cancels")]
  FixupCancels,
  #[serde(rename = "drop_commits")]
  DropCommits,
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
  #[serde(rename = "commit_marks")]
  CommitMarks,
  #[serde(rename = "declaration_moves")]
  DeclarationMoves,
  #[serde(rename = "free_reorder")]
  FreeReorder,
  #[serde(rename = "review_whitespace")]
  ReviewWhitespace,
  #[serde(rename = "symbol_navigation")]
  SymbolNavigation,
  #[serde(rename = "repeated_edit")]
  RepeatedEdit,
  #[serde(rename = "generated_files")]
  GeneratedFiles,
  #[serde(rename = "review_concerns")]
  ReviewConcerns,
  #[serde(rename = "review_tour")]
  ReviewTour,
  #[serde(rename = "review_prose")]
  ReviewProse,
  #[serde(rename = "review_long_titles")]
  ReviewLongTitles,
  #[serde(rename = "review_modules")]
  ReviewModules,
  #[serde(rename = "review_parts")]
  ReviewParts,
  #[serde(rename = "script_colors")]
  ScriptColors,
  #[serde(rename = "csharp_colors")]
  CSharpColors,
  #[serde(rename = "stacked_branches")]
  StackedBranches,
  #[serde(rename = "stacked_chain")]
  StackedChain,
  #[serde(rename = "stacked_reading")]
  StackedReading,
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
  #[serde(rename = "resolved_words_generated")]
  ResolvedWordsGenerated,
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
  #[serde(rename = "update_lost_bindings")]
  UpdateLostBindings,
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
      Self::FixupOrder => "fixup_order",
      Self::FixupCancels => "fixup_cancels",
      Self::DropCommits => "drop_commits",
      Self::LongSubjects => "long_subjects",
      Self::CommitTree => "commit_tree",
      Self::CommitDeleted => "commit_deleted",
      Self::CommitManyFiles => "commit_many_files",
      Self::CommitWhitespace => "commit_whitespace",
      Self::CommitMarks => "commit_marks",
      Self::DeclarationMoves => "declaration_moves",
      Self::FreeReorder => "free_reorder",
      Self::ReviewWhitespace => "review_whitespace",
      Self::SymbolNavigation => "symbol_navigation",
      Self::RepeatedEdit => "repeated_edit",
      Self::GeneratedFiles => "generated_files",
      Self::ReviewConcerns => "review_concerns",
      Self::ReviewTour => "review_tour",
      Self::ReviewProse => "review_prose",
      Self::ReviewLongTitles => "review_long_titles",
      Self::ReviewModules => "review_modules",
      Self::ReviewParts => "review_parts",
      Self::ScriptColors => "script_colors",
      Self::CSharpColors => "csharp_colors",
      Self::StackedBranches => "stacked_branches",
      Self::StackedChain => "stacked_chain",
      Self::StackedReading => "stacked_reading",
      Self::StackIsland => "stack_island",
      Self::ReadmeShowcase => "readme_showcase",
      Self::BranchTableLarge => "branch_table_large",
      Self::StackSideConflict => "stack_side_conflict",
      Self::StackedIssues => "stacked_issues",
      Self::StackedConflict => "stacked_conflict",
      Self::ConflictFix => "conflict_fix",
      Self::ConflictWords => "conflict_words",
      Self::ResolvedWords => "resolved_words",
      Self::ResolvedWordsGenerated => "resolved_words_generated",
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
      Self::UpdateLostBindings => "update_lost_bindings",
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
  /// The test that makes the repository (its file, line and titles, `TestRepositoryBuilder.init` of the E2E fixture):
  /// what it leaves behind is named by it (`AppState::made`)
  #[serde(default)]
  pub test: Option<String>,
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

/// The id of the test repository whose files `path` is in: the deepest component of the path named by it (its
/// directory, a slot of its pool, a companion beside it: `<id>` or `<id>.<what>`), or the id of a `NO_REPO` path after
/// `NO_REPO_`; none for a path of no test repository
pub(crate) fn repository_id_in(path: &str) -> Option<&str> {
  std::path::Path::new(path).components().rev().find_map(|component| {
    let name = component.as_os_str().to_str()?;
    let name = name.strip_prefix(NO_REPO_PREFIX).unwrap_or(name);
    let id = name.split_once('.').map_or(name, |(id, _what)| id);
    let digits = id.strip_prefix(REPOSITORY_ID_PREFIX)?;
    (digits.len() == REPOSITORY_ID_DIGITS && digits.bytes().all(|byte| byte.is_ascii_digit())).then_some(id)
  })
}

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
      // the invalid path, so the page subscribes to its session, which fails
      store.push(("recentProjects", serde_json::json!([{ "path": path }])));
      tracing::info!("Populated NO_REPO store with invalid path: {}", path);
    }

    insert_repository(&state, &id, &path, &store, request.test)?;
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
    store.push(("recentProjects", serde_json::json!([{ "path": path }])));
  }

  insert_repository(&state, &id, &path, &store, request.test)?;
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
pub(crate) fn set_fixture_value(store: &app_settings::service::SettingsService, key: &str, value: serde_json::Value) -> Result<(), StatusCode> {
  let patch = domain_types::settings::SettingsPatch::Value { key: key.to_owned(), value };
  store.set(patch, FIXTURE_ORIGIN).map(drop).map_err(|e| {
    tracing::error!(key, error = %e, "the fixture cannot change the settings");
    StatusCode::UNPROCESSABLE_ENTITY
  })
}

fn insert_repository(state: &Arc<AppState>, id: &str, path: &str, values: &[(&str, serde_json::Value)], test: Option<String>) -> Result<(), StatusCode> {
  let app_data = state.app_data_of(id);
  let store = app_settings::service::SettingsService::open(app_data.clone()).map_err(|e| {
    tracing::error!(error = %e, "cannot start the writer of the settings");
    StatusCode::INTERNAL_SERVER_ERROR
  })?;
  app_api::app::settings::give_worktree_epochs(&store);
  for (key, value) in values {
    set_fixture_value(&store, key, value.clone())?;
  }
  write_test_identity(&state.global_config_of(id))?;
  let store = Arc::new(store);
  let events = Arc::new(host::TestEvents::default());
  // a change the writer makes of itself reaches the pages as the app's does (`src-tauri/src/settings.rs`)
  {
    let events = Arc::clone(&events);
    store.on_external_change(move |change| app_api::events::settings_changed(&*events, change));
  }
  let hosts = state::TestHosts {
    desktop: Arc::new(host::TestDesktop::new(&store, path)),
    settings: store,
    events,
    updater: Arc::default(),
    cli_tools: Arc::new(cli_tools_fixtures::CliToolsHost::new(&state.app_data_of(id))),
  };
  let repo = TestRepository {
    id: id.to_string(),
    path: path.to_string(),
    app_data,
    in_flight: Arc::default(),
    context: Arc::new(state.context_of(id, &hosts)),
    events: hosts.events,
    store: hosts.settings,
    pull_request_calls: std::sync::atomic::AtomicU32::new(0),
    review_requests: std::sync::RwLock::default(),
    review_request_calls: std::sync::atomic::AtomicU32::new(0),
    reviews_calls: std::sync::atomic::AtomicU32::new(0),
    review_fetch_calls: std::sync::atomic::AtomicU32::new(0),
    review_prune_keeps: std::sync::Mutex::default(),
    page_clock: std::sync::Mutex::default(),
    remote: std::sync::OnceLock::new(),
    sessions: Arc::downgrade(&state.sessions),
    patronus_list_calls: std::sync::atomic::AtomicU32::new(0),
    patronus_poll_calls: std::sync::atomic::AtomicU32::new(0),
    patronus_answered: std::sync::Mutex::new(std::collections::BTreeMap::new()),
    patronus_detail_calls: std::sync::atomic::AtomicU32::new(0),
    review_readiness_calls: std::sync::Mutex::default(),
    branch_name_suggestions: std::sync::RwLock::new(state::BranchNameSuggestionFixture::default()),
    host_disks: std::sync::RwLock::default(),
    branch_name_suggestion_calls: std::sync::atomic::AtomicU32::new(0),
    desktop: hosts.desktop,
    updater: hosts.updater,
    cli_tools: hosts.cli_tools,
    held_lock: std::sync::Mutex::default(),
    fetch_held: std::sync::Mutex::default(),
    foreign_operation: Arc::default(),
    sync_overrides: std::sync::Mutex::default(),
    frame: std::sync::Mutex::default(),
  };
  let made = state::Made {
    test: test.unwrap_or_default(),
    context: Arc::downgrade(&repo.context),
    removed: std::sync::atomic::AtomicBool::new(false),
  };
  state.made.insert(id.to_string(), made);
  state.repositories.insert(id.to_string(), repo);
  Ok(())
}

/// The identity of the test machine, in the global config of a test repository: what `git var` finds outside a
/// repository (the first run of the welcome card, a `NO_REPO` path), as the developer's global config gives the app one.
/// A repository's own identity (the templates set `user.name` and `user.email`) wins.
/// The review plan, on when unset, is off here: the review windows of the other tests look as they did before it, and a
/// test of the plan turns it on in the repository's `.git/config`, which wins (openspec/specs/branch/review-plan/spec.md)
const TEST_IDENTITY: &str = "[user]\n\tname = Test User\n\temail = test-user@example.com\n[branchdeck]\n\treviewExperimental = false\n";

fn write_test_identity(global_config: &std::path::Path) -> Result<(), StatusCode> {
  std::fs::write(global_config, TEST_IDENTITY).map_err(|e| {
    tracing::error!(path = %global_config.display(), error = %e, "cannot write the global config of the test repository");
    StatusCode::INTERNAL_SERVER_ERROR
  })
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

/// The worktree of the test repository `sync_outside_the_app` syncs: a slot of its lease pool it leased
/// (`disk_fixtures::lease_slot`), else the repository itself
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncOutsideFixture {
  #[serde(default)]
  path: Option<String>,
}

/// Syncs the repository behind the app's back, as `brd sync` in a terminal does: the core of the sync
/// (`sync_branches`) under the prefix the session of the repository is keyed by, in no session, so it writes the
/// virtual branches, the notes and the mark of a sync in the worktree (`synced_mark`), and no window learns of it. The
/// worktree is the repository, or the slot of its lease pool the fixture names (`path`)
pub async fn sync_outside_the_app(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
  Json(fixture): Json<SyncOutsideFixture>,
) -> Result<StatusCode, (StatusCode, String)> {
  let own = remote_fixtures::path_of(&state, &id)?;
  let path = match fixture.path {
    None => own,
    Some(path) if path == own || state.path_to_id.get(&path).is_some_and(|entry| *entry.value() == id) => path,
    Some(path) => return Err((StatusCode::UNPROCESSABLE_ENTITY, format!("{path} is no worktree of the repository {id}"))),
  };
  let key = remote_fixtures::session_of(&state, &path).await?;
  // as `brd -C <slot> sync`: under the lease the slot holds as it starts
  let options = sync_core::sync::SyncOptions {
    lease: lease_pool::epoch::lease_guard(std::path::Path::new(&path)),
    lost_bindings: Some(&trial_check::lost_bindings::REPLAY_RULE),
    ..sync_core::sync::SyncOptions::default()
  };
  let synced = sync_core::sync::sync_branches(&state.git_of(&id), &path, &key.prefix, UnreadEvents, options).await;
  synced.map(|_report| StatusCode::OK).map_err(|error| {
    tracing::error!("Failed to sync outside the app: {error:#}");
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{error:#}"))
  })
}

/// The events of a sync no window shows (`sync_outside_the_app`): a terminal prints them, nothing reads them here
#[derive(Clone)]
struct UnreadEvents;

impl domain_types::ProgressReporter for UnreadEvents {
  fn send(&self, _event: domain_types::SyncEvent) -> anyhow::Result<()> {
    Ok(())
  }
}

/// Updates the checked out branch onto the fetched baseline behind the app's back, as `brd update` beside the app does:
/// the core of Update Project without a fetch, in no session, so only the next sync of the page learns what it landed
pub async fn update_outside_the_app(axum::extract::Path(id): axum::extract::Path<String>, State(state): State<Arc<AppState>>) -> Result<StatusCode, StatusCode> {
  let path = state.repositories.get(&id).map(|repo| repo.path.clone()).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let git = state.git_executor.clone();
  let params = update_core::update_project::UpdateProjectParams {
    lease: lease_pool::epoch::lease_guard(std::path::Path::new(&path)),
    repository_path: path,
    fetch: false,
    keep: Vec::new(),
  };
  let update = move || update_core::update_project::update_project_core(&git, &params, &|_| {}, history_rewrite::repository_lock::LockWait::default());
  let updated = tokio::task::spawn_blocking(in_current_span(update)).await;
  match updated {
    Ok(Ok(_)) => Ok(StatusCode::OK),
    Ok(Err(e)) => {
      tracing::error!("Failed to update outside the app: {e:#}");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
    Err(e) => {
      tracing::error!("The update outside the app stopped: {e}");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// `POST /repositories/{id}/locks/{lock}`: whether the server holds the lock
#[derive(Deserialize)]
pub struct RepositoryLockRequest {
  held: bool,
}

/// A lock the server holds for a test as another process holds it, by its name in the route
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum HeldLock {
  /// `$GIT_COMMON_DIR/branch-deck.lock` with its holder record, as an agent's `brd pick --follow` holds it
  /// (`lock_repository_as_another_process`: outside the line of the server's process, in which the app's operations
  /// wait for each other): its record names `brd pick --follow` and the server's pid, alive while it runs
  Repository,
  /// The fetch lock of the baseline, its record naming `brd update` and the tracking ref of `origin/master`, as an agent's
  /// `brd update` that fetches the baseline holds it (`sync_core::fetch_lock::hold_fetch_as_another_process`): the fetch
  /// of an update or of a Fetch of the page waits for it, then fetches itself, the ref unmoved
  Fetch,
}

/// Takes the lock `lock` of the repository or releases it (a lock held before is released either way)
pub async fn hold_lock(
  axum::extract::Path((id, lock)): axum::extract::Path<(String, HeldLock)>,
  State(state): State<Arc<AppState>>,
  Json(request): Json<RepositoryLockRequest>,
) -> Result<StatusCode, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let common_dir = std::path::Path::new(&repo.path).join(".git");
  let slot = match lock {
    HeldLock::Repository => &repo.held_lock,
    HeldLock::Fetch => &repo.fetch_held,
  };
  let mut slot = slot.lock().expect("held lock");
  *slot = None;
  if request.held {
    let taken = match lock {
      HeldLock::Repository => history_rewrite::repository_lock::lock_repository_as_another_process(&common_dir, "brd pick --follow"),
      HeldLock::Fetch => sync_core::fetch_lock::hold_fetch_as_another_process(&common_dir, "refs/remotes/origin/master", "brd update"),
    };
    *slot = Some(taken.map_err(|e| {
      tracing::error!("Cannot take the lock: {e:#}");
      StatusCode::CONFLICT
    })?);
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
  let fixture = fixture.with_title_leads();
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

/// How long a deletion waits for the commands and the sessions still running for the repository
/// (`AppState::delete_wait` of the E2E server): a sync of a loaded machine takes tens of seconds after its test failed;
/// a command or a run that takes longer is stuck
pub(crate) const DELETE_WAIT: std::time::Duration = std::time::Duration::from_secs(120);

/// Forgets the repository, so no new command starts for it, and answers: its files go in the background
/// (`remove_repository`), once the commands still running for it ended, which may outlast its test; `NO_REPO` has none
pub async fn delete_repository(axum::extract::Path(id): axum::extract::Path<String>, State(state): State<Arc<AppState>>) -> Result<StatusCode, StatusCode> {
  let Some((_, repo)) = state.repositories.remove(&id) else {
    return Err(StatusCode::UNPROCESSABLE_ENTITY);
  };
  state.path_to_id.remove(&repo.path);
  // the slots of its pool a test opened as the repository (`lease_slot`)
  let slots: Vec<String> = state.path_to_id.iter().filter(|entry| entry.value() == &id).map(|entry| entry.key().clone()).collect();
  state.path_to_id.retain(|_, of| of != &id);
  // its windows close: their subscriptions end, their review fetches stop; a window's subscription streams until its
  // session closes, and holds the repository while it does
  repo.context.forget_windows();
  // the event stream of each page ends: a page that outlived its test hears nothing more
  repo.events.close();
  // its sessions and those of its slots close now; the files go once each ended, its syncs' git with it. The clock of
  // each runs out: a command whose operation waits on it (the reply to a start comment) ends by its deadline
  let sessions_ended = close_worktrees(&state, std::iter::once(repo.path.clone()).chain(slots));
  // a sign-in the page left waiting on GitHub would hold the repository until its code expires
  if let Some(host) = repo.existing_remote() {
    host.end_sign_ins();
    // and an answer the test held at a fake until it is released
    host.release_all_answers();
  }
  tokio::spawn(remove_repository(state, id, repo, sessions_ended).in_current_span());
  Ok(StatusCode::ACCEPTED)
}

/// Closes the sessions of each of `worktrees` now and runs its clock out (`WorktreeClocks::run_out`): what resolves
/// once the sessions of each ended, by its worktree
fn close_worktrees(state: &AppState, worktrees: impl IntoIterator<Item = String>) -> Vec<(String, impl Future<Output = ()> + Send + 'static)> {
  worktrees
    .into_iter()
    .map(|worktree| {
      let ended = state.sessions.close_worktree_ended(&worktree);
      state.session_clocks.run_out(&worktree);
      (worktree, ended)
    })
    .collect()
}

/// Releases the repository lock the test of a deleted repository left held, then waits for the commands still running
/// for it (`hold_repository`), for the reads of its facts (`TestFactSources`: the computation of its landing needs,
/// the fetch of its moved remotes) and for its closed sessions (`sessions_ended`: a sync that a Fetch or an update that
/// applied nothing asked and let go, or the one of an open or a focus, runs git in its worktree until it ends), closes
/// again the sessions its commands opened anew after the first close and waits for them too, all within
/// `AppState::delete_wait` (past it, a warning names what still holds the repository, `Stuck`), then deletes its files
/// (`fs_retry`: on Windows a process whose current directory is in them, or a scanner with a file open, blocks the
/// removal until it lets go). A git that outlived the files paid a removed working directory, which on macOS reads the
/// entries of the parent for every `getcwd` and seconds of a large `$TMPDIR`. Once the server let go of the repository,
/// its record says its deletion ended (`Made::removed`)
async fn remove_repository(state: Arc<AppState>, id: String, repo: TestRepository, sessions_ended: Vec<(String, impl Future<Output = ()> + Send + 'static)>) {
  // the lock first (`hold_lock`), its holder record with it: a rewrite of the page that waits for it, as
  // long as its holder (this server) lives, runs and ends instead of keeping the deletion waiting
  drop(repo.held_lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take());
  drop(repo.fetch_held.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take());
  // the process of the agent's operation ends with its record
  let operation = repo.foreign_operation.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
  if let Some(operation) = operation {
    operation.end();
  }
  let worktrees: Vec<String> = sessions_ended.iter().map(|(worktree, _)| worktree.clone()).collect();
  let delete_wait = state.delete_wait;
  let deadline = tokio::time::Instant::now() + delete_wait;
  let (_, mut closing) = tokio::join!(tokio::time::timeout_at(deadline, repo.in_flight.close()), sessions_until(sessions_ended, deadline));
  // what holds the repository once the wait for its commands ended, before the second close ends what they opened
  let mut stuck = (!closing.is_empty() || !repo.in_flight.running().is_empty()).then(|| Stuck::of(&state, &id, &repo, &worktrees));
  // a command that ran past the close asked a session of the repository again (the sync after a rewrite or an update,
  // the subscription of a window that loaded as its test ended), which opened a new one that no window holds and no
  // sweep removes, as no test moves its clock: closed again once no command runs for the repository
  let opened_again = sessions_open_in(&state, &worktrees);
  if !opened_again.is_empty() {
    tracing::debug!(id, ?opened_again, "The sessions its commands opened again after the deletion close");
  }
  closing.extend(sessions_until(close_worktrees(&state, worktrees.iter().cloned()), deadline).await);
  if !closing.is_empty() {
    stuck.get_or_insert_with(|| Stuck::of(&state, &id, &repo, &worktrees));
  }
  if let Some(mut stuck) = stuck {
    // the worktrees whose closed sessions still ran at the end of either wait
    stuck.closing = closing.into_iter().collect();
    tracing::warn!(
      id,
      test = stuck.test,
      commands = ?stuck.commands,
      closing = ?stuck.closing,
      reopened = ?stuck.reopened,
      "Commands or sessions of the test repository still run after {delete_wait:?}; deleting it anyway"
    );
  }
  // the git directories and the GitHub repository the commands kept of it
  state.repository_state.forget(&repo.path);
  // the writer saves what is pending now, not when the repository is dropped after its app data is gone
  if let Err(e) = repo.store.flush() {
    tracing::warn!("Failed to save the settings of {id}: {e}");
  }
  if repo.app_data.exists()
    && let Err(e) = fs_retry::remove_dir_all(&repo.app_data)
  {
    tracing::warn!("Failed to remove the app data {}: {}", repo.app_data.display(), e);
  }
  let companions = state.companions_of(&id);
  if companions.exists()
    && let Err(e) = fs_retry::remove_dir_all(&companions)
  {
    tracing::warn!("Failed to remove the companions {}: {}", companions.display(), e);
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
  // the server lets go of it: an app still held from now on is held by something its commands started
  drop(repo);
  if let Some(made) = state.made.get(&id) {
    made.removed.store(true, Ordering::SeqCst);
  }
}

/// Waits until `deadline` for the closed sessions of each worktree to end (`Sessions::close_worktree_ended`): the
/// worktrees whose closed sessions still run then, a run in flight keeping the actor
async fn sessions_until(sessions_ended: Vec<(String, impl Future<Output = ()> + Send + 'static)>, deadline: tokio::time::Instant) -> std::collections::BTreeSet<String> {
  let closing = Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new()));
  let mut ending = tokio::task::JoinSet::new();
  for (worktree, ended) in sessions_ended {
    closing.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(worktree.clone());
    let closing = Arc::clone(&closing);
    ending.spawn(
      async move {
        ended.await;
        closing.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&worktree);
      }
      .in_current_span(),
    );
  }
  let _ = tokio::time::timeout_at(deadline, async { while ending.join_next().await.is_some() {} }).await;
  std::mem::take(&mut *closing.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
}

/// What still holds a test repository whose deletion waited `AppState::delete_wait`, which its warning names: the test
/// that made it, the commands of its page and the reads of its facts that still run (`InFlight::running`), the
/// worktrees whose closed sessions still ran at the end of either wait (`closing`: a run in flight keeps the actor),
/// and the sessions of its worktrees open again since the deletion closed them, with what keeps each (`reopened`)
struct Stuck {
  test: String,
  commands: Vec<String>,
  closing: Vec<String>,
  reopened: Vec<String>,
}

impl Stuck {
  /// What holds the repository now; the worktrees whose sessions still close come after the second wait (`closing`)
  fn of(state: &AppState, id: &str, repo: &TestRepository, worktrees: &[String]) -> Self {
    Self {
      test: state.test_of(id),
      commands: repo.in_flight.running(),
      closing: Vec::new(),
      reopened: sessions_open_in(state, worktrees),
    }
  }
}

/// The sessions of `worktrees` open now, each with what keeps it
fn sessions_open_in(state: &AppState, worktrees: &[String]) -> Vec<String> {
  state
    .sessions
    .open_sessions()
    .into_iter()
    .filter(|open| worktrees.contains(&open.key.worktree))
    .map(|open| {
      format!(
        "{} under {} ({} windows, {} operations)",
        open.key.worktree, open.key.prefix, open.subscribers, open.operations
      )
    })
    .collect()
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
