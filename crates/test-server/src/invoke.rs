//! The commands of app-api over HTTP, as the page mock sends them (`tests/e2e/setup/tauri-mocks.ts`): one route
//! `/invoke/<name>` per entry of `app_api::for_each_command!`, which runs the body of the app in the `AppContext` of the
//! test repository the request names (`X-Repository-Id`), for the window the request names (`X-Window-Label`, `main`
//! without one). A JSON command answers its value, as Tauri resolves the `invoke` of the page with it, or
//! `{ "status": "error", "error": <ApiError> }` (`Rejection`); a raw one its bytes (`application/octet-stream`) or the
//! error.
//!
//! A command with a channel of the page runs as the app runs it: the page mock names the channel of the command
//! (`X-Progress-Channel`, its key in `window.__progressChannels`) and reads the answer as SSE events, each message of
//! the channel `{"progress": …}` while the command runs, then its answer `{"result": …}` (`{ status, data }` or
//! `{ status, error }`); the stream ends once nothing can send to the channel any more, so the patches of a
//! subscription come on it until the subscription ends. Without the header (a direct call of a test), the answer alone,
//! and a message to the channel fails (`SinkGone`): a subscription ends with its first patch.

use crate::state::AppState;
use app_api::context::AppContext;
use app_api::registry::Command;
use app_api::transport::{EventSink, Sink, SinkGone};
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::post;
use domain_types::error::ApiError;
use serde::Serialize;
use std::any::Any;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::Instrument;

/// The header that names the channel of a command in the page
pub const PROGRESS_CHANNEL_HEADER: &str = "X-Progress-Channel";

/// The header with which the page mock names the window of its page (`__e2eWindowLabel`) on every command
pub const WINDOW_LABEL_HEADER: &str = "X-Window-Label";

/// How a command fails: with its `ApiError`, answered the way Tauri rejects a command (`{ status: "error", error:
/// <ApiError> }` with status 200, which the page mock hands to the bindings: the same JSON as the app,
/// `api_error_contract_test.rs`), or with a failure of the server itself, a panicked command (500, `caught`). A type of
/// the server only because axum's `IntoResponse` can't be implemented for `ApiError` outside `domain-types`.
pub enum Rejection {
  Error(ApiError),
  Panicked,
}

impl From<ApiError> for Rejection {
  fn from(error: ApiError) -> Self {
    Self::Error(error)
  }
}

impl IntoResponse for Rejection {
  fn into_response(self) -> Response {
    match self {
      Self::Error(error) => Json(serde_json::json!({ "status": "error", "error": error })).into_response(),
      Self::Panicked => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
  }
}

/// The rejection of `operation` with `error` (a core's error through `api_error`, a message as `internal`). An error is
/// the expected answer of an error-path test, not a failure of the server, so it is logged at debug
pub(crate) fn failed(operation: &str, error: impl app_api::api_error::IntoApiError) -> Rejection {
  let error = error.into_api_error();
  tracing::debug!("Cannot {operation}: {}", error.message);
  Rejection::Error(error)
}

pub(crate) type CommandResult<T> = Result<Json<T>, Rejection>;

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
    let _ = self.try_send(message);
  }

  /// Sends `message` to the channel; `false` without one or when its stream is gone, so a subscriber knows its window
  /// went away
  pub fn try_send(&self, message: &impl Serialize) -> bool {
    self.sender.as_ref().is_some_and(|sender| sender.send(serde_json::json!({ "progress": message })).is_ok())
  }
}

/// The SSE stream of a request as the channel of a command of app-api
impl<T: Serialize + Send + Sync> EventSink<T> for Progress {
  fn send(&self, event: T) -> Result<(), SinkGone> {
    if self.try_send(&event) { Ok(()) } else { Err(SinkGone) }
  }
}

/// The channel argument of a command, made of the stream of its request: none for a command without one
pub trait ChannelOf {
  fn of(progress: Progress) -> Self;
}

impl ChannelOf for () {
  fn of(_progress: Progress) -> Self {}
}

impl<T: Serialize + Send + Sync + 'static> ChannelOf for Sink<T> {
  fn of(progress: Progress) -> Self {
    Arc::new(progress)
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
  event_stream(receiver)
}

/// The messages of `receiver` as an SSE answer, each the JSON of one event, with a keep-alive every second; it ends once
/// nothing can send to `receiver` any more
pub fn event_stream(receiver: mpsc::UnboundedReceiver<serde_json::Value>) -> Response {
  let stream = UnboundedReceiverStream::new(receiver).map(|message| Ok::<_, Infallible>(Event::default().data(message.to_string())));
  Sse::new(stream)
    .keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(1)).text("keep-alive"))
    .into_response()
}

/// The callback of `for_each_command!`: the route of each command by its kind
macro_rules! invoke_routes {
  ($($(#[doc = $doc:literal])* $class:ident $(($reason:ident))? $kind:ident $name:ident($($arg:ident: $type:ty),*) $([channel $channel:ident: $event:ty $(, $open:ident)?])? $(-> $output:ty)? = $body:path;)*) => {
    /// `router` with the route of every command of app-api
    pub fn command_routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
      router $(.route(concat!("/invoke/", stringify!($name)), invoke_route!($kind $name)))*
    }
  };
}

/// The handler of one command: `json` and `plain` answer JSON, `raw` its bytes
macro_rules! invoke_route {
  (raw $name:ident) => {
    post(raw::<app_api::commands::$name::Command>)
  };
  ($kind:ident $name:ident) => {
    post(json::<app_api::commands::$name::Command>)
  };
}

app_api::for_each_command!(invoke_routes);

/// A command that answers JSON: on the SSE stream of its channel when it has one and the page named it
async fn json<C: Command<Args: 'static, Output: Serialize + 'static, Channel: ChannelOf + 'static> + 'static>(
  State(state): State<Arc<AppState>>,
  headers: HeaderMap,
  body: Bytes,
) -> Response {
  let channel = progress_channel(&headers).filter(|_| C::SPEC.channel.is_some());
  answer_with_progress(channel, move |progress| async move { run::<C>(&state, &headers, &body, progress).await.map(Json) }).await
}

async fn raw<C: Command<Args: 'static, Output = Vec<u8>, Channel = ()> + 'static>(State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
  match run::<C>(&state, &headers, &body, Progress { sender: None }).await {
    Ok(bytes) => ([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response(),
    Err(rejection) => rejection.into_response(),
  }
}

/// Runs `C` with the arguments of `body` in the context of the repository the request names, for the call of the
/// window it names, its channel on `progress`
async fn run<C: Command<Args: 'static, Output: 'static, Channel: ChannelOf + 'static> + 'static>(
  state: &AppState,
  headers: &HeaderMap,
  body: &[u8],
  progress: Progress,
) -> Result<C::Output, Rejection> {
  let context = requesting_context(state, headers)?;
  let call = context.call(requesting_window(headers));
  let args = arguments::<C>(body).map_err(|e| failed(C::SPEC.name, format!("Invalid arguments of {}: {e}", C::SPEC.name)))?;
  let channel = C::Channel::of(progress);
  let output = caught(erase(async move { C::call(&context, &call, args, channel).await })).await.inspect_err(|rejection| {
    if let Rejection::Error(error) = rejection {
      tracing::debug!("Cannot {}: {}", C::SPEC.name, error.message);
    }
  })?;
  Ok(*output.downcast::<C::Output>().expect("a command answers its output"))
}

/// The answer of a command with its output erased, so every command runs in the same kind of task (`caught`)
pub(crate) type ErasedAnswer = Pin<Box<dyn Future<Output = Result<Box<dyn Any + Send>, ApiError>> + Send>>;

/// `answer` on the heap, its output erased: the futures of the sessions and the engine are deep, and the router proves
/// each handler `Send`; one task type for every command keeps the server small (a task type per command took its test
/// binary past the 16 MB of unwind tables the linker encodes compactly)
pub(crate) fn erase<T: Send + 'static>(answer: impl Future<Output = Result<T, ApiError>> + Send + 'static) -> ErasedAnswer {
  Box::pin(async move { answer.await.map(|output| Box::new(output) as Box<dyn Any + Send>) })
}

/// The answer of a command, run as a task of its own, as Tauri runs a command: its error, or `Panicked` for a command
/// that panicked, which fails the request
pub(crate) async fn caught(answer: ErasedAnswer) -> Result<Box<dyn Any + Send>, Rejection> {
  match tokio::spawn(answer.in_current_span()).await {
    Ok(answer) => answer.map_err(Rejection::Error),
    Err(failure) => {
      tracing::error!("A command panicked: {failure}");
      Err(Rejection::Panicked)
    }
  }
}

/// The window of the page that sends a command (`X-Window-Label`): the main window for a request that names none (a
/// direct call of a test)
fn requesting_window(headers: &HeaderMap) -> &str {
  headers
    .get(WINDOW_LABEL_HEADER)
    .and_then(|value| value.to_str().ok())
    .unwrap_or(app_settings::windows::MAIN_WINDOW)
}

/// The arguments of `C` from the body the page mock sends: the object of the arguments by their camelCase names, or
/// the value of `params` alone, which the mock unwraps; nothing for a command without arguments
fn arguments<C: Command>(body: &[u8]) -> serde_json::Result<C::Args> {
  if C::SPEC.args.is_empty() {
    return serde_json::from_value(serde_json::json!({}));
  }
  let value: serde_json::Value = if body.is_empty() { serde_json::Value::Null } else { serde_json::from_slice(body)? };
  if C::SPEC.args == ["params"] {
    return serde_json::from_value(serde_json::json!({ "params": value }));
  }
  serde_json::from_value(value)
}

/// The context of the test repository of the page that sends a command (`X-Repository-Id`)
fn requesting_context(state: &AppState, headers: &HeaderMap) -> Result<Arc<AppContext>, Rejection> {
  headers
    .get(crate::REPOSITORY_ID_HEADER)
    .and_then(|value| value.to_str().ok())
    .and_then(|id| state.repositories.get(id))
    .map(|repository| Arc::clone(&repository.context))
    .ok_or_else(|| failed("find the test repository", "no test repository named by the request"))
}

#[cfg(test)]
#[path = "invoke_test.rs"]
mod invoke_test;
