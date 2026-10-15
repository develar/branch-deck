use crate::output::SchemaVersion;
use crate::sync_command::RemoteSummary;
use schemars::JsonSchema;
use serde::Serialize;
use std::fmt;
use sync_core::baseline::NoBaseline;

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
  /// The issue groups (`refs/branch-deck/issue-groups`) changed since the caller read them; read again and retry
  GroupsMoved,
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
  /// `bd undo update:` of an update the branch moved on from (a commit, another update, a switch): it can't be undone
  UndoStale,
  /// Uncommitted, staged or untracked files sit on paths `bd update` (or its undo) writes; `paths` lists them. Nothing
  /// was changed; never stash them, ask the user
  UncommittedOverlap,
  /// `bd update --continue` or `--abort` without a resolve session of this worktree
  NoSession,
  /// `--continue --round <n>` names another round than the open one; read `bd update --status` again
  RoundMoved,
  /// The resolve session can't continue: the branch was rewritten, HEAD left the branch, or the session belongs to
  /// another worktree; `bd update --abort` removes it
  SessionStale,
  /// The conflict of a round is of a type bd can't resolve (binary, file/directory, rename, mode, submodule, symlink);
  /// `paths` lists the files
  UnsupportedConflict,
  /// Conflict markers are left in the `edit` copies of `paths`
  UnresolvedMarkers,
  /// The `edit` copies of `paths` changed outside their conflict blocks
  ResolutionOutsideConflict,
  /// The files of `paths` have no resolution: no edited `edit` copy, no `--take`, no `--delete`
  MissingResolution,
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
      Self::InvalidArguments
      | Self::NotARepository
      | Self::NoBranchPrefix
      | Self::UnknownBranch
      | Self::InvalidPlan
      | Self::NotAheadOfBaseline
      | Self::NoSession
      | Self::UnresolvedMarkers
      | Self::ResolutionOutsideConflict
      | Self::MissingResolution => 2,
      Self::HeadMoved | Self::GroupsMoved | Self::LeaseRejected | Self::Locked | Self::RoundMoved => 3,
      Self::NoBaseline
      | Self::OperationInProgress
      | Self::DetachedHead
      | Self::StagedOverlap
      | Self::UndoStale
      | Self::UncommittedOverlap
      | Self::SessionStale
      | Self::UnsupportedConflict
      | Self::PushRefused
      | Self::NoRemote
      | Self::InstallRefused => 4,
      Self::GitNotFound | Self::GitError | Self::NetworkError => 5,
    }
  }

  /// Whether running the same command again (after re-reading the state for a stale code) may succeed
  pub fn retryable(self) -> bool {
    matches!(
      self,
      Self::HeadMoved | Self::GroupsMoved | Self::LeaseRejected | Self::Locked | Self::RoundMoved | Self::NetworkError
    )
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
  /// The files the error is about, if any
  pub paths: Vec<String>,
}

impl CliError {
  pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
    Self {
      code,
      message: message.into(),
      commits: Vec::new(),
      branches: Vec::new(),
      paths: Vec::new(),
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
      ErrorCode::GitError
    };
    Self::new(code, format!("{error:#}"))
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
  /// The files the error is about, relative to the repository: the local files of `uncommitted_overlap`, the
  /// conflicting files of a refused resolution
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub paths: Vec<String>,
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
