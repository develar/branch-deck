//! What the app of a test repository gives the commands of app-api beside the services of its sessions
//! (`app_api::transport::AppHost`): the accounts of the repository's fakes (`RemoteHost`, started on the first need),
//! no `teamcity` CLI, the branch names of its fixture (E2E never runs claude), and the page's clock in UTC, so the weeks
//! and the hours of the Safe Push statistics of a test are fixed. A read of a status the test holds waits at the gate
//! of its answer (`held_or_failed`). The events of the app go to the pages of the repository on a stream of each
//! (`TestEvents`)

use crate::invoke::{Rejection, event_stream, failed};
use crate::remote_host::{HeldAnswer, PAGE_CLOCK_START, RemoteHost};
use crate::state::AppState;
use crate::tauri_command_bridge::rejected;
use app_api::transport::{AppEvents, AppHost, WindowId};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
// jscpd:ignore-start: the implementation of `AppHost` imports the types of its signature, as `transport.rs` does
use domain_types::branch_name_suggestion::{BranchNameSuggestionOutcome, SuggestBranchNamesParams};
use domain_types::calendar::Calendar;
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use patronus::teamcity_cli::TeamCityCli;
use remote_facts::fact_sources::Read;
use remote_facts::sign_in::SignIns;
// jscpd:ignore-end
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use sync_core::branch_name_suggestion::clean_names;
use tokio::sync::mpsc;
use youtrack::service::YouTrackService;

/// The host of the app of the test repository `repository` (`AppState::context_of`)
pub struct TestHost {
  pub state: Weak<AppState>,
  /// The id of the test repository
  pub repository: String,
}

impl TestHost {
  /// The fakes of the test repository, started now if not yet
  fn remote(&self) -> Result<Arc<RemoteHost>, ApiError> {
    let state = self.state.upgrade().ok_or_else(|| ApiError::internal("The test server stopped"))?;
    let repository = state.repositories.get(&self.repository);
    let repository = repository.ok_or_else(|| ApiError::internal(format!("The test repository {} is gone", self.repository)))?;
    Ok(repository.remote(&state.git_executor))
  }
}

impl AppHost for TestHost {
  fn sign_ins(&self) -> Result<SignIns, ApiError> {
    Ok(self.remote()?.sign_ins().clone())
  }

  fn youtrack(&self) -> Result<Arc<YouTrackService>, ApiError> {
    Ok(self.remote()?.shared_youtrack_service())
  }

  /// Held and failed as the test says (`HeldAnswer::AuthStatus`)
  fn before_auth_status(&self) -> Read<'_, Result<(), ApiError>> {
    Box::pin(async move {
      let remote = self.remote()?;
      held_or_failed(&remote, HeldAnswer::AuthStatus, "read the sign-ins").await.map_err(rejected)
    })
  }

  /// Nothing of the developer's machine: no `teamcity` CLI
  fn teamcity(&self) -> Read<'_, Option<TeamCityCli>> {
    Box::pin(async { None })
  }

  /// The fixture of the repository after its delay (`POST /repositories/{id}/branch-name-suggestions`), counted, by
  /// default two names from the words of the commit messages; blocking, as the app's `claude` is
  fn suggest_branch_names(&self, _git: &GitCommandExecutor, params: &SuggestBranchNamesParams, _is_superseded: &dyn Fn() -> bool) -> BranchNameSuggestionOutcome {
    let fixture = self.state.upgrade().and_then(|state| {
      let repository = state.repositories.get(&self.repository)?;
      repository.branch_name_suggestion_calls.fetch_add(1, Ordering::SeqCst);
      let fixture = repository.branch_name_suggestions.read().expect("branch name suggestion fixture lock").clone();
      Some(fixture)
    });
    let Some(fixture) = fixture else {
      return BranchNameSuggestionOutcome::Error {
        message: format!("The test repository {} is gone", self.repository),
      };
    };
    std::thread::sleep(std::time::Duration::from_millis(fixture.delay_ms));
    if let Some(outcome) = fixture.outcome {
      return outcome;
    }
    let words: Vec<&str> = params
      .commits
      .iter()
      .flat_map(|commit| commit.message.split_whitespace())
      .filter(|word| word.len() > 3)
      .collect();
    let names = clean_names(&[
      format!("fix-{}", words.first().unwrap_or(&"issue")),
      format!("update-{}", words.get(1).unwrap_or(&"feature")),
    ]);
    BranchNameSuggestionOutcome::Suggested { names }
  }

  /// The start of the page's clock (`PAGE_CLOCK_START`)
  fn now(&self) -> u32 {
    PAGE_CLOCK_START
  }

  fn calendar(&self) -> Calendar {
    Calendar::utc()
  }
}

/// Waits while a test holds `answer` (`/remote-gates`), then fails it while the test fails it: a read of a status the
/// server gives itself
pub(crate) async fn held_or_failed(host: &RemoteHost, answer: HeldAnswer, operation: &str) -> Result<(), Rejection> {
  host.pass_held(answer).await;
  if host.fails(answer) {
    return Err(failed(operation, "the fixture fails this read"));
  }
  Ok(())
}

/// The events of the app of a test repository to its pages (`AppEvents`), as the app sends them to its windows: each
/// page of the repository subscribes when it starts (`GET /repositories/{id}/events?window=<label>`,
/// `subscribe_events`), and an event goes to every page, or to the pages of one window; what a page emits for the other
/// windows (`emit` of `@tauri-apps/api/event`) goes the same way (`POST /repositories/{id}/events`, `emit_page_event`),
/// as the event plugin of Tauri sends it, the page itself included. The streams end with the repository (`close`)
pub struct TestEvents {
  /// The stream of each page that listens, by its window; none once the repository went
  pages: Mutex<Option<Vec<PageStream>>>,
}

struct PageStream {
  window: String,
  sender: mpsc::UnboundedSender<Value>,
}

impl Default for TestEvents {
  fn default() -> Self {
    Self {
      pages: Mutex::new(Some(Vec::new())),
    }
  }
}

impl TestEvents {
  /// The stream of a new page of `window`, its first message `{"subscribed": true}`; none once the repository went
  pub fn subscribe(&self, window: String) -> Option<mpsc::UnboundedReceiver<Value>> {
    let mut pages = self.pages.lock().unwrap_or_else(PoisonError::into_inner);
    let pages = pages.as_mut()?;
    let (sender, receiver) = mpsc::unbounded_channel();
    sender.send(json!({ "subscribed": true })).ok()?;
    pages.push(PageStream { window, sender });
    Some(receiver)
  }

  /// Sends `event` to the pages of `window`, or of every window; `source`: the window of the page that emitted it, none
  /// for an event of the app. A page that went away leaves
  fn send(&self, window: Option<&str>, event: &str, payload: Value, source: Option<&str>) {
    let mut message = json!({ "event": event, "window": window, "source": source });
    message["payload"] = payload;
    let mut pages = self.pages.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(pages) = pages.as_mut() {
      pages.retain(|page| window.is_some_and(|window| window != page.window) || page.sender.send(message.clone()).is_ok());
    }
  }

  /// The repository went: the stream of each page ends
  pub fn close(&self) {
    self.pages.lock().unwrap_or_else(PoisonError::into_inner).take();
  }
}

impl AppEvents for TestEvents {
  fn emit(&self, event: &str, payload: Value) {
    self.send(None, event, payload, None);
  }

  fn emit_to(&self, window: &WindowId, event: &str, payload: Value) {
    self.send(Some(window.as_str()), event, payload, None);
  }
}

#[derive(Deserialize)]
pub struct EventsQuery {
  /// The label of the window of the page (`main`, `settings`, …)
  window: String,
}

/// The event stream of a page of the test repository `id`, as SSE: `{"subscribed": true}`, then each event the page
/// hears, `{ event, payload, window, source }`, until the repository goes. Outside `hold_repository`: the stream lasts
/// as long as the page, which the deletion of the repository does not wait for
pub async fn subscribe_events(Path(id): Path<String>, Query(query): Query<EventsQuery>, State(state): State<Arc<AppState>>) -> Response {
  let receiver = state.repositories.get(&id).and_then(|repository| repository.events.subscribe(query.window));
  let Some(receiver) = receiver else {
    return StatusCode::GONE.into_response();
  };
  event_stream(receiver)
}

/// An event a page of the repository emits (`emit`, `emit_to` of `@tauri-apps/api/event`)
#[derive(Deserialize)]
pub struct PageEvent {
  event: String,
  #[serde(default)]
  payload: Value,
  /// The window it goes to; none: every window
  #[serde(default)]
  window: Option<String>,
  /// The window of the page that emits it
  source: String,
}

/// Sends the event of a page to the pages of the repository `id` it names, as the event plugin of the app does
pub async fn emit_page_event(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(event): Json<PageEvent>) -> StatusCode {
  let Some(repository) = state.repositories.get(&id) else {
    return StatusCode::GONE;
  };
  repository.events.send(event.window.as_deref(), &event.event, event.payload, Some(&event.source));
  StatusCode::OK
}
