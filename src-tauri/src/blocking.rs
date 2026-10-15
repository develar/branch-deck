//! Runs blocking work from async Tauri commands.

use crate::command_error::CommandError;

/// Runs `f` on tokio's blocking thread pool and converts both a panicked task and the closure's error into
/// a [`CommandError`].
///
/// Use it for git commands, file system access and anything else that would stall the async runtime.
pub async fn run_blocking<T, E, F>(f: F) -> Result<T, CommandError>
where
  F: FnOnce() -> Result<T, E> + Send + 'static,
  T: Send + 'static,
  E: Into<CommandError> + Send + 'static,
{
  tokio::task::spawn_blocking(f)
    .await
    .map_err(|e| CommandError::new(format!("Task failed: {e}")))?
    .map_err(Into::into)
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
}
