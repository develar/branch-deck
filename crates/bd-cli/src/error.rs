use crate::output::SchemaVersion;
use schemars::JsonSchema;
use serde::Serialize;
use std::fmt;

/// A stable error code. Agents branch on the code and the exit code, never on the message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
  /// The arguments don't parse
  InvalidArguments,
  /// `-C` (or the current directory) is not inside a git working tree
  NotARepository,
  /// No branch prefix from `--prefix`, the app setting or `branchdeck.branchPrefix`
  NoBranchPrefix,
  /// `--branch` names no current group
  UnknownBranch,
  /// No `@{u}`, `<remote>/master`, `<remote>/main` or local `master`/`main` to compare against
  NoBaseline,
  /// A plan doesn't apply to the commits ahead of the baseline
  InvalidPlan,
  /// A commit of the request is not ahead of the baseline
  NotAheadOfBaseline,
  /// A cherry-pick conflicts
  Conflict,
  /// HEAD moved since the caller read it; read again and retry
  HeadMoved,
  /// The remote ref moved since the last push; fetch, check and retry
  LeaseRejected,
  /// Another git process holds a lock; retry
  Locked,
  /// A rebase, merge, cherry-pick or revert is in progress
  OperationInProgress,
  /// HEAD is detached
  DetachedHead,
  /// Staged changes overlap the files the operation rewrites
  StagedOverlap,
  /// No usable git executable
  GitNotFound,
  /// A git command failed
  GitError,
  /// A network command (fetch, push) failed
  NetworkError,
}

impl ErrorCode {
  /// 0 ok, 1 conflict, 2 invalid input, 3 stale (retry), 4 refused, 5 git or network error
  pub fn exit_code(self) -> i32 {
    match self {
      Self::Conflict => 1,
      Self::InvalidArguments | Self::NotARepository | Self::NoBranchPrefix | Self::UnknownBranch | Self::InvalidPlan | Self::NotAheadOfBaseline => 2,
      Self::HeadMoved | Self::LeaseRejected | Self::Locked => 3,
      Self::NoBaseline | Self::OperationInProgress | Self::DetachedHead | Self::StagedOverlap => 4,
      Self::GitNotFound | Self::GitError | Self::NetworkError => 5,
    }
  }

  /// Whether running the same command again (after re-reading the state for a stale code) may succeed
  pub fn retryable(self) -> bool {
    matches!(self, Self::HeadMoved | Self::LeaseRejected | Self::Locked | Self::NetworkError)
  }
}

/// A failed command, printed as [`ErrorOutput`]
#[derive(Debug)]
pub struct CliError {
  pub code: ErrorCode,
  pub message: String,
  /// The commits the error is about, if any
  pub commits: Vec<String>,
}

impl CliError {
  pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
    Self {
      code,
      message: message.into(),
      commits: Vec::new(),
    }
  }

  pub fn to_output(&self) -> ErrorOutput {
    ErrorOutput {
      schema: SchemaVersion,
      error: ErrorBody {
        code: self.code,
        message: self.message.clone(),
        commits: self.commits.clone(),
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

/// A git failure that has no more specific code
impl From<anyhow::Error> for CliError {
  fn from(error: anyhow::Error) -> Self {
    Self::new(ErrorCode::GitError, format!("{error:#}"))
  }
}

/// `{"schema": 1, "error": {...}}`, printed on stdout with the exit code of the error code
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
  /// Whether the same command may succeed when retried
  pub retryable: bool,
}
