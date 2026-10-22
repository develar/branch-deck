//! Runs blocking work from async Tauri commands, each run an operation with its own id.

use crate::command_error::CommandError;
use sync_utils::operation_id::OperationId;

/// Runs `f` on tokio's blocking thread pool, inside the span `operation{operation_id}`, and converts both a panicked
/// task and the closure's error into a [`CommandError`] that names the operation.
///
/// The span is a child of the current one, the `#[instrument]` span of the command, so a log line of the work reads
/// `get_pull_requests{…}:operation{operation_id=…}: …` (docs/design/async-patterns.md#operation-spans). Use it for
/// git commands, file system access and anything else that would stall the async runtime.
pub async fn run_blocking<T, E, F>(f: F) -> Result<T, CommandError>
where
  F: FnOnce() -> Result<T, E> + Send + 'static,
  T: Send + 'static,
  E: Into<CommandError> + Send + 'static,
{
  let operation_id = OperationId::new();
  let span = tracing::info_span!("operation", %operation_id);
  let result = tokio::task::spawn_blocking(move || span.in_scope(f))
    .await
    .map_err(|e| CommandError::new(format!("Task failed: {e}")))
    .and_then(|result| result.map_err(Into::into));
  result.map_err(|mut error| {
    // an error of an inner run keeps the operation it names
    error.operation_id.get_or_insert_with(|| operation_id.to_string());
    error
  })
}

#[cfg(test)]
mod tests {
  use super::run_blocking;
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
  async fn an_error_names_its_operation_and_an_inner_one_keeps_its_own() {
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
}
