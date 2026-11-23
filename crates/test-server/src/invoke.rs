//! The commands of app-api over HTTP, as the page mock sends them (`tests/e2e/setup/tauri-mocks.ts`): one route
//! `/invoke/<name>` per entry of `app_api::for_each_command!`, which hands the command to the host of the test
//! repository the request names (`X-Repository-Id`) as the Tauri shell hands it (`backend_host::page::Pages`, the
//! connection of the app's own windows: `TestRepository::pages`), for the window the request names (`X-Window-Label`,
//! `main` without one). So the page mock and the IDE's connection of a test repository share one host, one session and
//! one set of events, as the app and the bridge do. A JSON command answers its value, as Tauri resolves the `invoke` of
//! the page with it, or `{ "status": "error", "error": <ApiError> }` (`Rejection`); a raw one its bytes
//! (`application/octet-stream`) or the error; a command that panicked fails the request (500).
//!
//! A command with a channel of the page runs as the app runs it: the page mock names the channel of the command
//! (`X-Progress-Channel`, its key in `window.__progressChannels`) and reads the answer as SSE events, each message of
//! the channel `{"progress": …}` while the command runs, then its answer `{"result": …}` (`{ status, data }`,
//! `{ status, error }`, or `{ panicked }`); the stream ends once nothing can send to the channel any more, so the
//! patches of a subscription come on it until the subscription ends. Without the header (a direct call of a test), the
//! answer alone, and a message to the channel fails: a subscription ends with its first patch.

use crate::state::AppState;
use app_api::registry::CommandSpec;
use app_api::transport::{EventSink, SinkGone};
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::post;
use backend_host::page::{Answer, Failed, PageChannel, Pages, Reply};
use domain_types::error::ApiError;
use serde::Serialize;
use serde_json::json;
use std::convert::Infallible;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::Instrument;

/// The header that names the channel of a command in the page
pub const PROGRESS_CHANNEL_HEADER: &str = "X-Progress-Channel";

/// The header with which the page mock names the window of its page (`__e2eWindowLabel`) on every command
pub const WINDOW_LABEL_HEADER: &str = "X-Window-Label";

/// The header of a command that names the clock of the page (`Date.now()`, ms), whose time a test installs and moves
pub const PAGE_NOW_HEADER: &str = "X-Page-Now";

/// How a command fails: with its `ApiError`, answered the way Tauri rejects a command (`{ status: "error", error:
/// <ApiError> }` with status 200, which the page mock hands to the bindings: the same JSON as the app,
/// `api_error_contract_test.rs`), or with a failure of the server itself, a panicked command (500). A type of
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

/// The SSE stream of a request as the channel of a command of app-api, each event as its JSON
impl EventSink<serde_json::Value> for Progress {
  fn send(&self, event: serde_json::Value) -> Result<(), SinkGone> {
    if self.try_send(&event) { Ok(()) } else { Err(SinkGone) }
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

/// The callback of `for_each_command!`: the route of each command, which runs it by its name (`app_api::dispatch`)
macro_rules! invoke_routes {
  ($($(#[doc = $doc:literal])* $class:ident $(($reason:ident))? $kind:ident $(($($field:tt)*))? $name:ident($($arg:ident: $type:ty),*) $([channel $channel:ident: $event:ty $(, $open:ident)?])? $(-> $output:ty)? = $body:path;)*) => {
    /// `router` with the route of every command of app-api
    pub fn command_routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
      router $(.route(concat!("/invoke/", stringify!($name)), post(|state: State<Arc<AppState>>, headers: HeaderMap, body: Bytes| invoke(stringify!($name), state, headers, body))))*
    }
  };
}

app_api::for_each_command!(invoke_routes);

/// The handler of the command `name`: run through the host of the repository as the app runs it (`Pages`), JSON, on
/// the SSE stream of its channel when it has one and the page named it, or the bytes of a raw command
async fn invoke(name: &'static str, State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
  let spec = app_api::dispatch::spec(name).expect("a route is a command of the registry");
  if spec.channel.is_none() || progress_channel(&headers).is_none() {
    return match run(name, &state, &headers, &body, Arc::new(NoPage)).await {
      Ok(reply) => reply_response(reply),
      Err(rejection) => rejection.into_response(),
    };
  }
  let (sender, receiver) = mpsc::unbounded_channel();
  let page = Arc::new(PageStream(sender.clone()));
  tokio::spawn(
    async move {
      let result = match run(name, &state, &headers, &body, page).await {
        Ok(reply) => reply_result(reply),
        Err(Rejection::Error(error)) => json!({ "status": "error", "error": error }).to_string(),
        Err(Rejection::Panicked) => r#"{"panicked":true}"#.to_owned(),
      };
      let _ = sender.send(format!(r#"{{"result":{result}}}"#));
    }
    .in_current_span(),
  );
  text_stream(receiver)
}

/// The answer of a command without a channel: its JSON, as Tauri resolves the `invoke`, the bytes of a raw command, or
/// its error as the app rejects it (`Rejection`)
pub(crate) fn reply_response(reply: Reply) -> Response {
  match reply {
    Ok(Answer::Json(json)) => ([(header::CONTENT_TYPE, "application/json")], json).into_response(),
    Ok(Answer::Bytes(bytes)) => ([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response(),
    Err(Failed::Error(error)) => ([(header::CONTENT_TYPE, "application/json")], format!(r#"{{"status":"error","error":{error}}}"#)).into_response(),
    Err(Failed::Panicked) => Rejection::Panicked.into_response(),
  }
}

/// The answer of a command on the stream of its channel: `{ status, data }`, `{ status, error }`, or `{ panicked }` of a
/// command that panicked
pub(crate) fn reply_result(reply: Reply) -> String {
  match reply {
    Ok(Answer::Json(json)) => format!(r#"{{"status":"ok","data":{json}}}"#),
    Ok(Answer::Bytes(_)) => json!({ "status": "error", "error": ApiError::internal("A command with a channel answered bytes") }).to_string(),
    Err(Failed::Error(error)) => format!(r#"{{"status":"error","error":{error}}}"#),
    Err(Failed::Panicked) => r#"{"panicked":true}"#.to_owned(),
  }
}

/// The channel of a command whose page named none (a direct call of a test): a message to it fails, so a subscription
/// ends with its first patch
struct NoPage;

impl PageChannel for NoPage {
  fn send(&self, _json: String) -> bool {
    false
  }
}

/// The SSE stream of a request as the channel of a command: each message `{"progress": …}`
struct PageStream(mpsc::UnboundedSender<String>);

impl PageChannel for PageStream {
  fn send(&self, json: String) -> bool {
    self.0.send(format!(r#"{{"progress":{json}}}"#)).is_ok()
  }
}

/// The texts of `receiver` as an SSE answer, each the JSON of one event, with a keep-alive every second; it ends once
/// nothing can send to `receiver` any more
fn text_stream(receiver: mpsc::UnboundedReceiver<String>) -> Response {
  let stream = UnboundedReceiverStream::new(receiver).map(|message| Ok::<_, Infallible>(Event::default().data(message)));
  Sse::new(stream)
    .keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(1)).text("keep-alive"))
    .into_response()
}

/// Runs the command `name` with the arguments of `body` through the host of the repository the request names, for the
/// window it names, its channel `channel`
async fn run(name: &'static str, state: &AppState, headers: &HeaderMap, body: &[u8], channel: Arc<dyn PageChannel>) -> Result<Reply, Rejection> {
  let pages = requesting_pages(state, headers)?;
  note_page_clock(state, headers);
  let spec = app_api::dispatch::spec(name).expect("a route is a command of the registry");
  let args = arguments(spec, body).map_err(|e| failed(name, format!("Invalid arguments of {name}: {e}")))?;
  let reply = pages.invoke(requesting_window(headers), name, &args, spec.channel.as_ref().map(|_| channel)).await;
  if let Err(Failed::Error(error)) = &reply {
    tracing::debug!("Cannot {name}: {error}");
  }
  Ok(reply)
}

/// The window of the page that sends a command (`X-Window-Label`): the main window for a request that names none (a
/// direct call of a test)
fn requesting_window(headers: &HeaderMap) -> &str {
  headers
    .get(WINDOW_LABEL_HEADER)
    .and_then(|value| value.to_str().ok())
    .unwrap_or(app_settings::windows::MAIN_WINDOW)
}

/// The arguments of a command from the body the page mock sends, as the page's `invoke` names them: the object of the
/// arguments by their camelCase names, or the value of `params` alone, which the mock unwraps; nothing for a command
/// without arguments
fn arguments(spec: &CommandSpec, body: &[u8]) -> serde_json::Result<serde_json::Value> {
  if spec.args.is_empty() {
    return Ok(serde_json::json!({}));
  }
  let value: serde_json::Value = if body.is_empty() { serde_json::Value::Null } else { serde_json::from_slice(body)? };
  if spec.args == ["params"] {
    return Ok(serde_json::json!({ "params": value }));
  }
  Ok(value)
}

/// The context of the test repository of the page that sends a command (`X-Repository-Id`)
/// The clock of the page that sends a command (`X-Page-Now`, ms), kept by its repository (`TestRepository::page_clock`)
fn note_page_clock(state: &AppState, headers: &HeaderMap) {
  let page_ms = headers
    .get(PAGE_NOW_HEADER)
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.parse::<u64>().ok());
  let repository = headers
    .get(crate::REPOSITORY_ID_HEADER)
    .and_then(|value| value.to_str().ok())
    .and_then(|id| state.repositories.get(id));
  if let (Some(page_ms), Some(repository)) = (page_ms, repository) {
    *repository.page_clock.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((page_ms, std::time::Instant::now()));
  }
}

fn requesting_pages(state: &AppState, headers: &HeaderMap) -> Result<Arc<Pages>, Rejection> {
  headers
    .get(crate::REPOSITORY_ID_HEADER)
    .and_then(|value| value.to_str().ok())
    .and_then(|id| state.repositories.get(id))
    .map(|repository| repository.pages())
    .ok_or_else(|| failed("find the test repository", "no test repository named by the request"))
}

#[cfg(test)]
#[path = "invoke_test.rs"]
mod invoke_test;
