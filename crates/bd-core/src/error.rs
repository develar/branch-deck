use crate::output::SchemaVersion;
use crate::review_summary::ReviewSummary;
use crate::sync_command::RemoteSummary;
use domain_types::error::ErrorCode;
use history_rewrite::baseline::NoBaseline;
use history_rewrite::regroup::{FixupRefusal, RegroupError};
use schemars::JsonSchema;
use serde::Serialize;
use std::fmt;
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
  /// The processes of `slot_busy`
  pub processes: Vec<ProcessOutput>,
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
        retryable: self.code.retryable(),
      },
    }
  }
}

impl fmt::Display for CliError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}: {}", self.code, self.message)
  }
}

/// A missing baseline (`NoBaseline`, also a repository without commits) is `no_baseline`; any other git failure, which
/// has no more specific code, is `git_error`
impl From<anyhow::Error> for CliError {
  fn from(error: anyhow::Error) -> Self {
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
  /// The processes that work in the worktree of `slot_busy`, with their argv: no `ps` needed to tell them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub processes: Vec<ProcessOutput>,
  /// Whether the same command may succeed when retried
  pub retryable: bool,
}

/// A process that works in a worktree of the lease pool
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
      FixupRefusal::MarkerChanged => "",
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
