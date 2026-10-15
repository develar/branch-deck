//! Error type returned by Tauri commands.
//!
//! Serialized to the frontend as `{ "message": string }`.

use serde::Serialize;

/// Error returned by Tauri commands, serialized as `{ message }`.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct CommandError {
  pub message: String,
}

impl CommandError {
  pub fn new(message: impl Into<String>) -> Self {
    Self { message: message.into() }
  }
}

impl std::fmt::Display for CommandError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.message)
  }
}

impl std::error::Error for CommandError {}

impl From<anyhow::Error> for CommandError {
  fn from(err: anyhow::Error) -> Self {
    // Alternate format includes the whole context chain: "outer: inner: root cause"
    Self::new(format!("{err:#}"))
  }
}

impl From<tauri::Error> for CommandError {
  fn from(err: tauri::Error) -> Self {
    Self::new(err.to_string())
  }
}

impl From<String> for CommandError {
  fn from(message: String) -> Self {
    Self::new(message)
  }
}

impl From<&str> for CommandError {
  fn from(message: &str) -> Self {
    Self::new(message)
  }
}

#[cfg(test)]
mod tests {
  use super::CommandError;
  use anyhow::Context;

  #[test]
  fn anyhow_error_keeps_context_chain() {
    let err = std::fs::read("/nonexistent/branch-deck").context("Failed to read config").unwrap_err();
    let command_error = CommandError::from(err);
    assert!(command_error.message.starts_with("Failed to read config: "), "{}", command_error.message);
  }

  #[test]
  fn serializes_as_message_object() {
    let json = serde_json::to_string(&CommandError::new("boom")).unwrap();
    assert_eq!(json, r#"{"message":"boom"}"#);
  }
}
