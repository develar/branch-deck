//! The commands of app-api over HTTP, as the page mock sends them (`tests/e2e/setup/tauri-mocks.ts`): one route
//! `/invoke/<name>` per entry of `app_api::for_each_command!`, which runs the body of the app in the `AppContext` of the
//! test repository the request names (`X-Repository-Id`). A JSON command answers its value, as Tauri resolves the
//! `invoke` of the page with it, or `{ "status": "error", "error": <ApiError> }` (`Rejection`); a raw one its bytes
//! (`application/octet-stream`) or the error.

use crate::state::AppState;
use app_api::context::AppContext;
use app_api::registry::Command;
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::post;
use domain_types::error::ApiError;
use serde::Serialize;
use std::sync::Arc;

/// How a command fails: with its `ApiError`, answered the way Tauri rejects a command (`{ status: "error", error:
/// <ApiError> }` with status 200, which the page mock hands to the bindings: the same JSON as the app,
/// `api_error_contract_test.rs`), or with a failure of the server itself, a panicked command (500). A type of the
/// server only because axum's `IntoResponse` can't be implemented for `ApiError` outside `domain-types`.
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

/// The callback of `for_each_command!`: the route of each command by its kind
macro_rules! invoke_routes {
  ($($(#[doc = $doc:literal])* $class:ident $kind:ident $name:ident($($arg:ident: $type:ty),*) -> $output:ty = $body:path;)*) => {
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

async fn json<C: Command>(State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response
where
  C::Output: Serialize,
{
  match run::<C>(&state, &headers, &body).await {
    Ok(value) => Json(value).into_response(),
    Err(rejection) => rejection.into_response(),
  }
}

async fn raw<C: Command<Output = Vec<u8>>>(State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
  match run::<C>(&state, &headers, &body).await {
    Ok(bytes) => ([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response(),
    Err(rejection) => rejection.into_response(),
  }
}

/// Runs `C` with the arguments of `body` in the context of the repository the request names
async fn run<C: Command>(state: &AppState, headers: &HeaderMap, body: &[u8]) -> Result<C::Output, Rejection> {
  let context = requesting_context(state, headers)?;
  let args = arguments::<C>(body).map_err(|e| failed(C::SPEC.name, format!("Invalid arguments of {}: {e}", C::SPEC.name)))?;
  C::call(&context, args).await.map_err(|error| {
    tracing::debug!("Cannot {}: {}", C::SPEC.name, error.message);
    Rejection::Error(error)
  })
}

/// The arguments of `C` from the body the page mock sends: the object of the arguments by their camelCase names, or
/// the value of `params` alone, which the mock unwraps
fn arguments<C: Command>(body: &[u8]) -> serde_json::Result<C::Args> {
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
