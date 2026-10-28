use crate::output::SchemaVersion;
use crate::review_summary::ReviewSummary;
use crate::sync_command::RemoteSummary;
use domain_types::error::{ApiError, ErrorCode, ErrorDetails, OperationHolder};
use history_rewrite::baseline::NoBaseline;
use history_rewrite::regroup::{FixupRefusal, RegroupError};
use schemars::JsonSchema;
use serde::Serialize;
use std::fmt;
use sync_core::push_size::{PUSH_SIZE_LIMIT_CONFIG_KEY, PushTooLarge};
use sync_core::sync::SyncDeferred;

/// The exit code of a conflict: the output of the command (`state: "conflict"`, a branch not built, `check.ok` false),
/// never an error object
pub const CONFLICT_EXIT: i32 = 1;

/// A failed command, printed as [`ErrorOutput`]
#[derive(Debug)]
pub struct CliError {
  pub code: ErrorCode,
  pub message: String,
  /// The commits the error is about, if any
  pub commits: Vec<String>,
  /// The branches the error is about, with their remote status, if any
  pub branches: Vec<ErrorBranch>,
  /// The files the error is about, if any
  pub paths: Vec<String>,
  /// The lists of a few codes (`block_not_found`, `slot_busy`); boxed, as every command returns this error
  pub extra: Option<Box<ErrorExtra>>,
  /// The review of `approvals_missing`; boxed, as every command returns this error
  pub review: Option<Box<ReviewSummary>>,
}

/// The lists of `CliError` that only a few codes fill
#[derive(Debug, Default)]
pub struct ErrorExtra {
  /// The block ids of `block_not_found`
  pub blocks: Vec<String>,
  /// The processes of `slot_busy`, and those that may own the index lock of `locked`
  pub processes: Vec<ProcessOutput>,
  /// The commands that act on the refusal, in `ErrorBody::next_steps`; here, as most errors name none
  pub next_steps: Vec<NextStep>,
  /// The operation of another process that makes the command `busy`
  pub holder: Option<OperationHolder>,
}

impl CliError {
  pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
    Self {
      code,
      message: message.into(),
      commits: Vec::new(),
      branches: Vec::new(),
      paths: Vec::new(),
      extra: None,
      review: None,
    }
  }

  /// The error with one more of its `next_steps`: `command` ready to run, which runs only once the user approved it
  /// when `needs_approval`. Never a suppression (`--allow-*`, `--without-approvals`, `--min-free`, a config change):
  /// the message names those with their condition
  #[must_use]
  pub fn with_next_step(mut self, command: impl Into<String>, needs_approval: bool) -> Self {
    self.push_next_step(NextStep::new(command, needs_approval));
    self
  }

  /// Adds `step` to the `next_steps` of the error, once
  pub fn push_next_step(&mut self, step: NextStep) {
    let steps = &mut self.extra.get_or_insert_default().next_steps;
    if !steps.contains(&step) {
      steps.push(step);
    }
  }

  /// The `next_steps` of the error
  pub fn next_steps(&self) -> &[NextStep] {
    self.extra.as_ref().map_or(&[], |extra| extra.next_steps.as_slice())
  }

  pub fn to_output(&self) -> ErrorOutput {
    ErrorOutput {
      schema: SchemaVersion,
      error: ErrorBody {
        code: self.code,
        message: self.message.clone(),
        commits: self.commits.clone(),
        branches: self.branches.clone(),
        paths: self.paths.clone(),
        blocks: self.extra.as_ref().map_or_default(|extra| extra.blocks.clone()),
        review: self.review.as_deref().cloned(),
        processes: self.extra.as_ref().map_or_default(|extra| extra.processes.clone()),
        next_steps: self.next_steps().to_vec(),
        holder: self.extra.as_ref().and_then(|extra| extra.holder.clone()),
        retryable: self.code.retryable(),
        ask_user: self.code.asks_user(),
      },
    }
  }
}

impl fmt::Display for CliError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}: {}", self.code, self.message)
  }
}

/// The error of a core of the app's code (the engine of the operations), with its code and message; the holder of `busy`
/// with it
impl From<ApiError> for CliError {
  fn from(error: ApiError) -> Self {
    let mut cli = Self::new(error.code, error.message);
    if let Some(ErrorDetails::Busy { holder }) = error.details {
      cli.extra.get_or_insert_default().holder = holder.map(|holder| *holder);
    }
    cli
  }
}

/// `push_too_large`, its `paths` the directory that holds most of the push and the largest files; the limit is the
/// user's to raise, never an agent's
impl From<&PushTooLarge> for CliError {
  fn from(too_large: &PushTooLarge) -> Self {
    let message = format!(
      "{} Show it to the user: the files go out of the commits (build output?), or the user raises the limit; never raise \
       {PUSH_SIZE_LIMIT_CONFIG_KEY} yourself",
      too_large.facts()
    );
    let mut error = Self::new(ErrorCode::PushTooLarge, message);
    error.paths = too_large.paths();
    error
  }
}

/// A missing baseline (`NoBaseline`, also a repository without commits) is `no_baseline`, a push refused for its size
/// `push_too_large`; any other git failure, which has no more specific code, is `git_error`
impl From<anyhow::Error> for CliError {
  fn from(error: anyhow::Error) -> Self {
    if let Some(too_large) = error.downcast_ref::<PushTooLarge>() {
      return Self::from(too_large);
    }
    let code = if error.downcast_ref::<NoBaseline>().is_some() {
      ErrorCode::NoBaseline
    } else {
      error.downcast_ref::<SyncDeferred>().map_or(ErrorCode::GitError, SyncDeferred::code)
    };
    Self::new(code, format!("{error:#}"))
  }
}

/// `{"schema": 2, "error": {...}}`, printed on stdout with the exit code of the error code
#[derive(Debug, Serialize, JsonSchema)]
pub struct ErrorOutput {
  pub schema: SchemaVersion,
  pub error: ErrorBody,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ErrorBody {
  pub code: ErrorCode,
  pub message: String,
  /// The commits the error is about; empty when it is about none
  pub commits: Vec<String>,
  /// The branches whose remote branch has commits they lack, with their remote status: the rejected ones of
  /// `lease_rejected` as bd fetched them, the ones behind of `push_refused`
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub branches: Vec<ErrorBranch>,
  /// The files the error is about, relative to the repository: the local files of `uncommitted_overlap`, the
  /// conflicting files of a refused resolution
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub paths: Vec<String>,
  /// The ids of the blocks the file has now, for `block_not_found`: pass one of them to `bd move-hunk --block`
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub blocks: Vec<String>,
  /// The code-owner review of `approvals_missing`: the groups whose approval is missing, their files, and where the
  /// approvals count
  #[serde(skip_serializing_if = "Option::is_none")]
  pub review: Option<ReviewSummary>,
  /// The processes that work in the worktree of `slot_busy`, or that may own the index lock of `locked`, with their
  /// argv: no `ps` needed to tell them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub processes: Vec<ProcessOutput>,
  /// The commands that act on the refusal, ready to run, in the order to run them: a step with `needs_approval` runs
  /// only once the user approved it (a push, a rewrite, a cancel of a robot); one without it only reads (`bd robot`,
  /// `bd sync`, a `--check`). Never a way to start anyway: `--allow-*`, `--without-approvals` and `--min-free` stay in
  /// `message`, with the condition of each
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub next_steps: Vec<NextStep>,
  /// The operation of another process that runs what the command needs, for `busy`: its `op` (the `operation_id` of
  /// its log lines), `tool` (`bd` or `app`), `pid`, `kind` (`safe_push`, `push`, …), `subject` and `held_for` seconds;
  /// omitted when the registry is of another Branch Deck, whose operations can't be read, and for any other code
  #[serde(skip_serializing_if = "Option::is_none")]
  pub holder: Option<OperationHolder>,
  /// Whether the same command may succeed when retried
  pub retryable: bool,
  /// The user decides: show `message` (with `paths`) to the user and ask before you go on (`uncommitted_overlap`,
  /// `empty_resolution`, `landed_differs`); omitted when false
  #[serde(default, skip_serializing_if = "is_false")]
  pub ask_user: bool,
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature of `skip_serializing_if`")]
const fn is_false(value: &bool) -> bool {
  !*value
}

/// A command that acts on a refusal (`ErrorBody::next_steps`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct NextStep {
  /// The command, ready to run in a shell
  pub command: String,
  /// It pushes, rewrites or cancels: run it only once the user approved it, never on your own
  pub needs_approval: bool,
}

impl NextStep {
  pub fn new(command: impl Into<String>, needs_approval: bool) -> Self {
    Self {
      command: command.into(),
      needs_approval,
    }
  }
}

/// A process that works in a worktree of the lease pool, or that may own the index lock of a worktree
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ProcessOutput {
  pub pid: u32,
  /// The command name, as `ps -o comm` shows it
  pub command: String,
  /// The arguments as one line, at most 200 characters
  pub argv: String,
  /// `bazel_server`: it serves only this worktree, and `bd lease return` stops it itself once no other process is in
  /// the worktree; `other`: wait until it is done
  pub kind: ProcessKind,
  /// The `--workspace_directory` of a Bazel server
  #[serde(skip_serializing_if = "Option::is_none")]
  pub workspace: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProcessKind {
  BazelServer,
  Other,
}

/// A branch an error is about
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ErrorBranch {
  pub name: String,
  /// Its remote status against the remote pushed to: as fetched after `lease_rejected`
  pub remote: RemoteSummary,
}

/// A refused or failed regroup with the code of its error (`RegroupError::code`, `git_error` without one) and, for
/// stranded fixups, what `bd` does about them; by value, for `map_err`
#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(regroup_error)`")]
pub fn regroup_error(error: RegroupError) -> CliError {
  let mut message = error.to_string();
  if let RegroupError::FixupsStranded { refusal, .. } = &error {
    message.push_str(match refusal {
      FixupRefusal::MarkerChanged => " (to squash commits the user asked to squash, use bd squash <into> <commit>...)",
      FixupRefusal::Retargeted => " (fold them first with bd fold, or reword them in the same plan to name the new subject)",
      FixupRefusal::NamedById => " (fold them first with bd fold)",
      FixupRefusal::AmendWaits => " (reword the amend! too, or fold first with bd fold)",
    });
  }
  CliError {
    commits: error.commits().to_vec(),
    ..CliError::new(error.code().unwrap_or(ErrorCode::GitError), message)
  }
}
