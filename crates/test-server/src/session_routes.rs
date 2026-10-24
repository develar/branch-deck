//! The session commands of the app (`src-tauri/src/commands/session.rs`) on the sessions of this server: the same
//! `Sessions` and `GitSessionHost`, on a `TestClock` that stands still, with the prefix resolved as the server's sync
//! family resolves it. `subscribe_view` is a long-lived event stream of its channel (`channel_commands`): its answer
//! `{"result": …}` first, then each patch as `{"progress": <ViewPatch>}` until `unsubscribe_view` or the deletion of the
//! repository (`delete_repository` closes its sessions, so the stream ends and the deletion does not wait for it); a
//! failure answers `{"result": {"status": "error", "error": <ApiError>}}` on the stream, the JSON of the app. The next run
//! of a session takes the overrides a test set for its repository (`sync_overrides`).

use crate::channel_commands::{Progress, answer_with_progress, progress_channel};
use crate::state::AppState;
use crate::sync_overrides::OverridingReporter;
use crate::tauri_command_bridge::{CommandResult, Rejection, branch_prefix_of, failed, repository_by_path};
use app_api::api_error::{api_error, no_branch_prefix};
use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use domain_types::ProgressReporter;
use domain_types::error::ApiError;
use domain_types::session::{SyncAnswer, SyncReason, TableView, ViewPatch, ViewSubscription};
use repository_model::runtime::SessionEnvironment;
use repository_model::session::{SessionKey, Subscriber, SubscriberGone, SubscriptionId};
use serde::Deserialize;
use std::sync::{Arc, Weak};
use sync_core::sync::{IssueConfigSource, SyncOptions};

/// What the syncs of the server's sessions take: the issue links read by the sync, and the settings of the test
/// repository for the focus
pub struct TestSessionEnvironment {
  pub state: Weak<AppState>,
}

impl SessionEnvironment for TestSessionEnvironment {
  fn sync_options(&self, _key: &SessionKey) -> anyhow::Result<SyncOptions> {
    Ok(SyncOptions {
      issue_config: IssueConfigSource::Load,
      ..Default::default()
    })
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    api_error(error)
  }

  /// The overrides a test set for the next sync of the repository, taken by this run (`sync_overrides`)
  fn reporter(&self, key: &SessionKey, reporter: Arc<dyn ProgressReporter>) -> Arc<dyn ProgressReporter> {
    let overrides = self.state.upgrade().and_then(|state| {
      let repository = repository_by_path(&state, &key.worktree).ok()?;
      repository.sync_overrides.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take()
    });
    match overrides {
      Some(overrides) => Arc::new(OverridingReporter { inner: reporter, overrides }),
      None => reporter,
    }
  }

  /// "Sync on focus" of the settings of the test repository; a repository already deleted syncs nothing
  fn syncs_on_focus(&self, key: &SessionKey) -> bool {
    self
      .state
      .upgrade()
      .and_then(|state| repository_by_path(&state, &key.worktree).ok().map(|repository| repository.store.app().auto_sync_on_focus))
      .unwrap_or(false)
  }
}

/// The patches of a session to the event stream of `subscribe_view`
struct StreamSubscriber(Progress);

impl Subscriber for StreamSubscriber {
  fn send(&self, patch: &ViewPatch) -> Result<(), SubscriberGone> {
    if self.0.try_send(patch) { Ok(()) } else { Err(SubscriberGone) }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRequest {
  repository_path: String,
}

/// The session key of the test repository at `repository_path`, as the app's `session_key`: refused without a prefix
async fn session_key(state: &AppState, repository_path: &str) -> Result<SessionKey, Rejection> {
  let prefix = branch_prefix_of(state, repository_path).await?;
  if prefix.is_empty() {
    return Err(failed("open the session", no_branch_prefix()));
  }
  Ok(SessionKey {
    worktree: repository_path.to_owned(),
    prefix,
  })
}

pub async fn open_session(State(state): State<Arc<AppState>>, Json(request): Json<SessionRequest>) -> CommandResult<TableView> {
  let key = session_key(&state, &request.repository_path).await?;
  state.sessions.open(&key);
  let view = state.sessions.get_snapshot(&key).await;
  view
    .map(Json)
    .ok_or_else(|| failed("open the session", ApiError::internal("The session closed while it opened")))
}

pub async fn get_snapshot(State(state): State<Arc<AppState>>, Json(request): Json<SessionRequest>) -> CommandResult<Option<TableView>> {
  let key = session_key(&state, &request.repository_path).await?;
  Ok(Json(state.sessions.get_snapshot(&key).await))
}

/// The stream of a window's subscription: its answer, then the patches. A call without a channel (a test of this crate
/// that calls the route directly) gets the answer, and the subscription ends with its first patch
pub async fn subscribe_view(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(request): Json<SessionRequest>) -> Response {
  answer_with_progress(progress_channel(&headers), move |progress| async move {
    let key = session_key(&state, &request.repository_path).await?;
    let (id, view) = state
      .sessions
      .subscribe(&key, Box::new(StreamSubscriber(progress)))
      .await
      .ok_or_else(|| failed("subscribe to the view", ApiError::internal("The session closed while the window subscribed")))?;
    Ok(Json(ViewSubscription { id: id.0, view }))
  })
  .await
  .into_response()
}

#[derive(Deserialize)]
pub struct UnsubscribeRequest {
  id: u32,
}

pub async fn unsubscribe_view(State(state): State<Arc<AppState>>, Json(request): Json<UnsubscribeRequest>) -> Json<()> {
  state.sessions.unsubscribe(SubscriptionId(request.id));
  Json(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowFocusRequest {
  repository_path: String,
  focused: bool,
}

pub async fn set_window_focus(State(state): State<Arc<AppState>>, Json(request): Json<WindowFocusRequest>) -> CommandResult<Option<SyncAnswer>> {
  let key = session_key(&state, &request.repository_path).await?;
  match state.sessions.set_window_focus(&key, request.focused) {
    Some(ticket) => ticket.wait().await.map(|answer| Json(Some(answer))).map_err(|error| failed("sync on focus", error)),
    None => Ok(Json(None)),
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
  repository_path: String,
  reason: SyncReason,
}

pub async fn sync(State(state): State<Arc<AppState>>, Json(request): Json<SyncRequest>) -> CommandResult<SyncAnswer> {
  let key = session_key(&state, &request.repository_path).await?;
  state.sessions.sync(&key, request.reason).wait().await.map(Json).map_err(|error| failed("sync", error))
}
