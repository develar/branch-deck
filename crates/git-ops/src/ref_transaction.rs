use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_executor::interactive_command::InteractiveCommand;
use std::fmt::Write;
use std::time::Duration;
use tracing::{debug, instrument};

/// Reflog messages start with this, so `git reflog` shows which entries Branch Deck wrote
pub const REFLOG_PREFIX: &str = "branch-deck: ";

/// Ref changes applied atomically by one `git update-ref --stdin`: all of them or none.
/// An expected old value makes an update fail if the ref moved meanwhile (e.g. a commit made in a terminal).
#[derive(Debug, Default)]
pub struct RefTransaction {
  commands: String,
  /// `--create-reflog`: write a reflog for the updated refs also with `core.logAllRefUpdates=false`
  create_reflog: bool,
}

/// How long `commit_while` waits for each reply of `update-ref`; ref locks are taken without waiting
/// (`core.filesRefLockTimeout`, 100 ms by default), so a reply comes at once
const REPLY_TIMEOUT: Duration = Duration::from_secs(60);

/// Send a command of `update-ref --stdin` that replies, and require its `<command>: ok`
fn request(update_ref: &mut InteractiveCommand, command: &str) -> Result<()> {
  update_ref.send(command)?;
  let reply = update_ref.read_line(REPLY_TIMEOUT)?;
  if reply != format!("{command}: ok") {
    bail!("unexpected reply to {command}: {reply}");
  }
  Ok(())
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

  /// Fail the whole transaction unless `full_ref` still points at `expected_id`; the ref itself does not change
  pub fn verify(&mut self, full_ref: &str, expected_id: &str) -> &mut Self {
    let _ = writeln!(self.commands, "verify {full_ref} {expected_id}");
    self
  }

  /// Write a reflog for the updated refs even where `core.logAllRefUpdates` is off, e.g. for an undo that reads it
  pub fn create_reflog(&mut self) -> &mut Self {
    self.create_reflog = true;
    self
  }

  /// Add the changes of `other`, so that one `update-ref` applies both or neither
  pub fn extend(&mut self, other: &RefTransaction) -> &mut Self {
    self.commands.push_str(&other.commands);
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
      .execute_command_with_input(&self.args(&message), repo_path, &self.commands)
      .map_err(|e| anyhow!("Failed to update refs ({operation}): {e:#}"))?;
    Ok(())
  }

  /// `commit` that runs `check` while git holds the locks of every ref of the transaction, and commits only if it
  /// passes; else the transaction is aborted and the check's error returned. The locks shut out every other writer of
  /// those refs, so what `check` reads can't change before the commit: e.g. which branch HEAD points at, which
  /// `update HEAD` (deref) moves and a `git switch` would change. Runs `update-ref --stdin` interactively: `start`,
  /// the commands, `prepare` (takes the locks and checks every old value), then `commit` or `abort`
  #[instrument(skip(self, git_executor, check))]
  pub fn commit_while(&self, git_executor: &GitCommandExecutor, repo_path: &str, operation: &str, check: impl FnOnce() -> Result<()>) -> Result<()> {
    debug!(commands = %self.commands, "updating refs after a check under their locks");
    let message = format!("{REFLOG_PREFIX}{operation}");
    let failed = |e: anyhow::Error| anyhow!("Failed to update refs ({operation}): {e:#}");
    let mut update_ref = git_executor.spawn_interactive(&self.args(&message), repo_path).map_err(failed)?;
    request(&mut update_ref, "start").map_err(failed)?;
    for command in self.commands.lines() {
      update_ref.send(command).map_err(failed)?;
    }
    request(&mut update_ref, "prepare").map_err(failed)?;
    match check() {
      Ok(()) => {
        request(&mut update_ref, "commit").map_err(failed)?;
        update_ref.finish().map_err(failed)
      }
      Err(refused) => {
        if let Err(e) = request(&mut update_ref, "abort").and_then(|()| update_ref.finish()) {
          tracing::warn!(error = %format!("{e:#}"), "aborting the ref transaction failed; git aborts it when it exits");
        }
        Err(refused)
      }
    }
  }

  fn args<'a>(&self, message: &'a str) -> Vec<&'a str> {
    let mut args = vec!["update-ref", "-m", message];
    if self.create_reflog {
      args.push("--create-reflog");
    }
    args.push("--stdin");
    args
  }
}
