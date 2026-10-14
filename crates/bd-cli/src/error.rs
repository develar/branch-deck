use crate::output::SchemaVersion;
use crate::sync_command::RemoteSummary;
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
  /// The remote branch is not where the lease expects it; bd fetched it, `branches` says what it has now
  LeaseRejected,
  /// Another git process holds a lock; retry
  Locked,
  /// A rebase, merge, cherry-pick or revert is in progress
  OperationInProgress,
  /// HEAD is detached
  DetachedHead,
  /// Staged changes overlap the files the operation rewrites
  StagedOverlap,
  /// A branch can't be pushed: not synced, behind its remote branch without --force, or its parent is not on the remote
  PushRefused,
  /// No remote to push to
  NoRemote,
  /// `bd install` can't write the symlink or a skill, e.g. a file that is not a symlink is in the way
  InstallRefused,
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
      Self::NoBaseline | Self::OperationInProgress | Self::DetachedHead | Self::StagedOverlap | Self::PushRefused | Self::NoRemote | Self::InstallRefused => 4,
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
  /// The branches the error is about, with their remote status, if any
  pub branches: Vec<ErrorBranch>,
}

impl CliError {
  pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
    Self {
      code,
      message: message.into(),
      commits: Vec::new(),
      branches: Vec::new(),
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
  /// The branches whose remote branch has commits they lack, with their remote status: the rejected ones of
  /// `lease_rejected` as bd fetched them, the ones behind of `push_refused`
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub branches: Vec<ErrorBranch>,
  /// Whether the same command may succeed when retried
  pub retryable: bool,
}

/// A branch an error is about
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ErrorBranch {
  pub name: String,
  /// Its remote status against the remote pushed to: as fetched after `lease_rejected`
  pub remote: RemoteSummary,
}
