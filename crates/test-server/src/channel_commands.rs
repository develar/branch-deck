//! The commands with a channel of the page, as the app runs them: the page mock names the channel of a command
//! (`X-Progress-Channel`, its key in `window.__progressChannels`) and reads the answer as SSE events, each message of
//! the channel `{"progress": …}` while the command runs, then its answer `{"result": …}` (`{ status, data }` or
//! `{ status, error }`, as `run_command` answers). A rewrite waits for a live holder of the repository lock as the app
//! does (`LockWait::WhileHeld`), telling the page whom for (`LockWaitEvent`), until `cancel_lock_wait` of its channel;
//! a push and an update tell it on their progress. Without the header (a direct call of a test), the answer alone.

use crate::state::AppState;
use crate::tauri_command_bridge::{CommandResult, Rejection};
use app_api::api_error::IntoApiError;
use axum::Json;
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use domain_types::error::{ApiError, ErrorCode};
use history_rewrite::lock_wait::{LockWaitEvent, LockWaits, WaitTicket};
use history_rewrite::repository_lock::{LockHolder, LockWait, LockWaiter};
use serde::Serialize;
use std::convert::Infallible;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::Instrument;

/// The header that names the channel of a command in the page
pub const PROGRESS_CHANNEL_HEADER: &str = "X-Progress-Channel";

/// The channel a command of the page reports on, by the header the mock sends; None for a direct call
pub fn progress_channel(headers: &HeaderMap) -> Option<u32> {
  headers.get(PROGRESS_CHANNEL_HEADER)?.to_str().ok()?.parse().ok()
}

/// The ids of the waits of direct calls, which no page can stop: above the ids the mock gives its channels
static DIRECT_WAITS: AtomicU32 = AtomicU32::new(1 << 30);

/// What a command sends to its channel in the page
#[derive(Clone)]
pub struct Progress {
  sender: Option<mpsc::UnboundedSender<serde_json::Value>>,
}

impl Progress {
  /// Sends `message` to the channel; nothing without one
  pub fn send(&self, message: &impl Serialize) {
    if let Some(sender) = &self.sender {
      let _ = sender.send(serde_json::json!({ "progress": message }));
    }
  }

  /// Sends `message` to the channel; `false` without one or when its stream is gone, so a subscriber knows its window
  /// went away
  pub fn try_send(&self, message: &impl Serialize) -> bool {
    self.sender.as_ref().is_some_and(|sender| sender.send(serde_json::json!({ "progress": message })).is_ok())
  }
}

/// The answer of `command`, which reports on `progress`: SSE events of its messages and its answer when the page named
/// a channel, else its answer alone
pub async fn answer_with_progress<T, Fut>(channel: Option<u32>, command: impl FnOnce(Progress) -> Fut) -> Response
where
  T: Serialize + Send + 'static,
  Fut: Future<Output = CommandResult<T>> + Send + 'static,
{
  if channel.is_none() {
    return command(Progress { sender: None }).await.into_response();
  }
  let (sender, receiver) = mpsc::unbounded_channel();
  let answer = command(Progress { sender: Some(sender.clone()) });
  tokio::spawn(
    async move {
      let result = match answer.await {
        Ok(Json(data)) => serde_json::json!({ "status": "ok", "data": data }),
        Err(Rejection::Error(error)) => serde_json::json!({ "status": "error", "error": error }),
        Err(Rejection::Panicked) => serde_json::json!({ "panicked": true }),
      };
      let _ = sender.send(serde_json::json!({ "result": result }));
    }
    .in_current_span(),
  );
  let stream = UnboundedReceiverStream::new(receiver).map(|message| Ok::<_, Infallible>(Event::default().data(message.to_string())));
  Sse::new(stream)
    .keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(1)).text("keep-alive"))
    .into_response()
}

/// The wait of the command on `channel` in the registry of the server, telling `tell`; a direct call gets an id no
/// page names
pub fn register_wait(lock_waits: &LockWaits, channel: Option<u32>, tell: impl Fn(Option<&LockHolder>) + Send + Sync + 'static) -> Arc<WaitTicket> {
  let id = channel.unwrap_or_else(|| DIRECT_WAITS.fetch_add(1, Ordering::Relaxed));
  Arc::new(lock_waits.register(id, tell))
}

/// Whether the page stopped the wait of `ticket`, for a core that takes the end of its wait as a function (the push)
pub fn canceled_by(ticket: &Arc<WaitTicket>) -> impl Fn() -> bool + Send + 'static {
  let ticket = Arc::clone(ticket);
  move || ticket.canceled()
}

/// `result` of a command whose wait `ticket` is: an error after the page stopped the wait is `canceled`, as the app's
/// `canceled_code` answers it
pub fn canceled_code<T>(result: CommandResult<T>, ticket: &WaitTicket) -> CommandResult<T> {
  result.map_err(|rejection| match rejection {
    Rejection::Error(error) if ticket.ended_by_cancel() && error.code != ErrorCode::Canceled => Rejection::Error(ApiError {
      operation_id: error.operation_id,
      ..ApiError::new(ErrorCode::Canceled, error.message)
    }),
    rejection => rejection,
  })
}

/// A rewrite of the app, run off the async runtime with its lock waited for as long as the holder lives, as the app's
/// `run_waiting`: the page hears whom it waits for on its channel and stops the wait with `cancel_lock_wait`
pub async fn run_waiting<T, E, F>(state: &AppState, headers: &HeaderMap, operation: &'static str, step: F) -> Response
where
  F: FnOnce(LockWait<'_>) -> Result<T, E> + Send + 'static,
  T: Serialize + Send + 'static,
  E: IntoApiError + Send + 'static,
{
  let channel = progress_channel(headers);
  let lock_waits = state.lock_waits.clone();
  answer_with_progress(channel, move |progress| async move {
    let ticket = register_wait(&lock_waits, channel, move |holder| progress.send(&LockWaitEvent::of(holder)));
    let step_ticket = Arc::clone(&ticket);
    let result = crate::tauri_command_bridge::run_command(operation, move || step(LockWait::WhileHeld(&*step_ticket))).await;
    canceled_code(result, &ticket)
  })
  .await
}
