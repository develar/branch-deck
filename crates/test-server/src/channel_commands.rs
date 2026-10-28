//! The commands with a channel of the page, as the app runs them: the page mock names the channel of a command
//! (`X-Progress-Channel`, its key in `window.__progressChannels`) and reads the answer as SSE events, each message of
//! the channel `{"progress": …}` while the command runs, then its answer `{"result": …}` (`{ status, data }` or
//! `{ status, error }`, as `run_command` answers). Without the header (a direct call of a test), the answer alone.

use crate::tauri_command_bridge::{CommandResult, Rejection};
use axum::Json;
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use std::convert::Infallible;
use std::future::Future;
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
