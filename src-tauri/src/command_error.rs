//! Error type returned by Tauri commands.
//!
//! Serialized to the frontend as `{ "message": string, "kind": null, "operation_id": null }`, `kind` naming a refusal the
//! page tells apart, `operation_id` the operation whose log lines tell what failed.

use serde::Serialize;
use sync_core::sync::SyncDeferred;

/// Error returned by Tauri commands, serialized as `{ message, kind, operation_id }`.
#[derive(Debug, Clone, Serialize, specta::Type, thiserror::Error)]
#[error("{message}")]
pub struct CommandError {
  pub message: String,
  /// What the page acts on without reading the message; null for any other failure
  #[specta(optional)]
  pub kind: Option<CommandErrorKind>,
  /// The operation that failed, the `operation_id` of its log lines: `run_blocking` names it; null for an error raised
  /// outside one
  #[specta(optional)]
  pub operation_id: Option<String>,
}

/// A refusal of a command that is a precondition, not a failure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CommandErrorKind {
  /// git is in the middle of a rebase, merge, cherry-pick or revert (`SyncDeferred`)
  OperationInProgress,
  /// HEAD is not on a branch (`SyncDeferred`)
  DetachedHead,
}

impl CommandError {
  pub fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
      kind: None,
      operation_id: None,
    }
  }
}

impl From<anyhow::Error> for CommandError {
  fn from(err: anyhow::Error) -> Self {
    let kind = err.downcast_ref::<SyncDeferred>().map(|deferred| match deferred {
      SyncDeferred::OperationInProgress(_) => CommandErrorKind::OperationInProgress,
      SyncDeferred::DetachedHead => CommandErrorKind::DetachedHead,
    });
    // Alternate format includes the whole context chain: "outer: inner: root cause"
    Self {
      message: format!("{err:#}"),
      kind,
      operation_id: None,
    }
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
    assert_eq!(json, r#"{"message":"boom","kind":null,"operation_id":null}"#);
  }

  #[test]
  fn names_a_deferred_sync_by_its_kind() {
    let command_error = CommandError::from(anyhow::Error::new(sync_core::sync::SyncDeferred::OperationInProgress("a rebase")));
    let json = serde_json::to_string(&command_error).unwrap();
    assert_eq!(
      json,
      r#"{"message":"Not synced: a rebase is in progress","kind":"operationInProgress","operation_id":null}"#
    );
  }
}
