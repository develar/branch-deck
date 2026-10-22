//! Every Tauri command that can fail is an operation with an id of its own (`sync_utils::operation_id::OperationId`): its
//! body runs inside the span `operation{operation_id}`, a child of the command's `#[instrument]` span, so a log line of it
//! reads `get_pull_requests{…}:operation{operation_id=…}: …`, and an error leaves with the id (`CommandError.operation_id`),
//! the key from the Activity entry of the error to the command's lines in the app's log
//! (docs/design/async-patterns.md#operation-spans).
//!
//! An async command wraps its body in [`operation`], a synchronous one (it runs on the main thread) in [`operation_now`];
//! a blocking step of the body, git or the file system, runs through [`run_blocking`], which alone is the operation of a
//! command that has only that step. `command_scan_test.rs` checks every command.

use crate::command_error::CommandError;
use std::future::Future;
use sync_utils::operation_id::OperationId;
use sync_utils::span::in_current_span;
use tracing::Instrument;

tokio::task_local! {
  /// The operation the current task runs in
  static CURRENT: OperationId;
}

/// Runs the body of an async command as its operation: in the span `operation{operation_id}`, the error named after it
pub async fn operation<T>(body: impl Future<Output = Result<T, CommandError>>) -> Result<T, CommandError> {
  let operation_id = OperationId::new();
  let span = tracing::info_span!("operation", %operation_id);
  CURRENT.scope(operation_id, body.instrument(span)).await.map_err(|error| named(error, operation_id))
}

/// Runs the body of a synchronous command as its operation, on the caller's thread
pub fn operation_now<T>(body: impl FnOnce() -> Result<T, CommandError>) -> Result<T, CommandError> {
  let operation_id = OperationId::new();
  let span = tracing::info_span!("operation", %operation_id);
  span.in_scope(body).map_err(|error| named(error, operation_id))
}

/// Runs `step` on tokio's blocking thread pool, inside the span of the caller, and converts both a panicked task and the
/// step's error into a [`CommandError`]. Inside an [`operation`] it is a step of it; alone, it is the operation of the
/// command, with the id of its own. Use it for git commands, file system access and anything else that would stall the
/// async runtime.
pub async fn run_blocking<T, E, F>(step: F) -> Result<T, CommandError>
where
  F: FnOnce() -> Result<T, E> + Send + 'static,
  T: Send + 'static,
  E: Into<CommandError> + Send + 'static,
{
  let step = async move {
    tokio::task::spawn_blocking(in_current_span(step))
      .await
      .map_err(|e| CommandError::new(format!("Task failed: {e}")))
      .and_then(|result| result.map_err(Into::into))
  };
  if CURRENT.try_with(|_| ()).is_ok() {
    return step.await;
  }
  operation(step).await
}

/// `error` with the operation it came out of; an error that already names one keeps it
fn named(mut error: CommandError, operation_id: OperationId) -> CommandError {
  error.operation_id.get_or_insert_with(|| operation_id.to_string());
  error
}

#[cfg(test)]
mod tests {
  use super::{operation, operation_now, run_blocking};
  use crate::command_error::CommandError;

  #[tokio::test]
  async fn returns_closure_value() {
    let value = run_blocking(|| Ok::<_, anyhow::Error>(42)).await.unwrap();
    assert_eq!(value, 42);
  }

  #[tokio::test]
  async fn converts_anyhow_error_with_context_chain() {
    let err = run_blocking(|| Err::<(), _>(anyhow::anyhow!("root cause").context("outer"))).await.unwrap_err();
    assert_eq!(err.message, "outer: root cause");
  }

  #[tokio::test]
  async fn passes_command_error_through() {
    let err = run_blocking(|| Err::<(), _>(CommandError::new("boom"))).await.unwrap_err();
    assert_eq!(err.message, "boom");
  }

  #[tokio::test]
  async fn converts_string_error() {
    let err = run_blocking(|| Err::<(), _>("plain".to_string())).await.unwrap_err();
    assert_eq!(err.message, "plain");
  }

  #[tokio::test]
  async fn a_blocking_step_alone_is_an_operation_and_an_error_that_names_one_keeps_it() {
    let first = run_blocking(|| Err::<(), _>("plain".to_string())).await.unwrap_err();
    let second = run_blocking(|| Err::<(), _>("plain".to_string())).await.unwrap_err();
    let first_id = first.operation_id.unwrap();
    assert_eq!(first_id.len(), 26, "{first_id}");
    assert_ne!(Some(&first_id), second.operation_id.as_ref());

    let inner = CommandError {
      operation_id: Some(first_id.clone()),
      ..CommandError::new("inner")
    };
    let outer = run_blocking(move || Err::<(), _>(inner)).await.unwrap_err();
    assert_eq!(outer.operation_id, Some(first_id));
  }

  #[tokio::test]
  async fn an_error_of_an_async_body_names_its_operation() {
    let first = operation(async { Err::<(), _>(CommandError::new("the service refused")) }).await.unwrap_err();
    let second = operation(async { Err::<(), _>(CommandError::new("the service refused")) }).await.unwrap_err();
    assert_eq!(first.operation_id.as_ref().map(String::len), Some(26), "{first:?}");
    assert_ne!(first.operation_id, second.operation_id);
  }

  #[tokio::test]
  async fn a_blocking_step_inside_an_operation_leaves_the_error_to_the_operation() {
    // the step's error has no id of its own: the operation names it
    let mut seen = None;
    let error = operation(async {
      let step = run_blocking(|| Err::<(), _>("no widget".to_string())).await.unwrap_err();
      seen = step.operation_id.clone();
      Err::<(), _>(step)
    })
    .await
    .unwrap_err();
    assert_eq!(seen, None);
    assert_eq!(error.operation_id.as_ref().map(String::len), Some(26));
  }

  #[test]
  fn a_synchronous_body_is_an_operation_too() {
    let value = operation_now(|| Ok::<_, CommandError>(7)).unwrap();
    assert_eq!(value, 7);
    let error = operation_now(|| Err::<(), _>(CommandError::new("no menu"))).unwrap_err();
    assert_eq!(error.operation_id.as_ref().map(String::len), Some(26));
  }
}
