use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::fmt::Write;
use tracing::{debug, instrument};

/// Reflog messages start with this, so `git reflog` shows which entries Branch Deck wrote
pub const REFLOG_PREFIX: &str = "branch-deck: ";

/// Ref changes applied atomically by one `git update-ref --stdin`: all of them or none.
/// An expected old value makes an update fail if the ref moved meanwhile (e.g. a commit made in a terminal).
#[derive(Debug, Default)]
pub struct RefTransaction {
  commands: String,
}

impl RefTransaction {
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// Point `full_ref` at `new_id`; with `expected_id`, only if it still points there
  pub fn update(&mut self, full_ref: &str, new_id: &str, expected_id: Option<&str>) -> &mut Self {
    let _ = write!(self.commands, "update {full_ref} {new_id}");
    if let Some(expected_id) = expected_id {
      self.commands.push(' ');
      self.commands.push_str(expected_id);
    }
    self.commands.push('\n');
    self
  }

  /// Create `full_ref`; fails if it already exists
  pub fn create(&mut self, full_ref: &str, new_id: &str) -> &mut Self {
    let _ = writeln!(self.commands, "create {full_ref} {new_id}");
    self
  }

  /// Delete `full_ref`; with `expected_id`, only if it still points there
  pub fn delete(&mut self, full_ref: &str, expected_id: Option<&str>) -> &mut Self {
    let _ = write!(self.commands, "delete {full_ref}");
    if let Some(expected_id) = expected_id {
      self.commands.push(' ');
      self.commands.push_str(expected_id);
    }
    self.commands.push('\n');
    self
  }

  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.commands.is_empty()
  }

  /// Apply all changes, recording `branch-deck: <operation>` in the reflog of every updated ref
  #[instrument(skip(self, git_executor))]
  pub fn commit(&self, git_executor: &GitCommandExecutor, repo_path: &str, operation: &str) -> Result<()> {
    if self.is_empty() {
      return Ok(());
    }
    debug!(commands = %self.commands, "updating refs");
    let message = format!("{REFLOG_PREFIX}{operation}");
    git_executor
      .execute_command_with_input(&["update-ref", "-m", &message, "--stdin"], repo_path, &self.commands)
      .map_err(|e| anyhow!("Failed to update refs ({operation}): {e:#}"))?;
    Ok(())
  }
}
