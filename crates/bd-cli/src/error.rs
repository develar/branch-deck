use crate::output::SchemaVersion;
use crate::safe_push_command::ReviewSummary;
use crate::sync_command::RemoteSummary;
use history_rewrite::baseline::NoBaseline;
use schemars::JsonSchema;
use serde::Serialize;
use std::fmt;
use sync_core::sync::SyncDeferred;

/// The exit code of a conflict: the output of the command (`state: "conflict"`, a branch not built, `check.ok` false),
/// never an error object
pub const CONFLICT_EXIT: i32 = 1;

/// A stable error code. Agents branch on the code and the exit code, never on the message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
  /// The arguments don't parse
  InvalidArguments,
  /// `-C` (or the current directory) is not inside a git working tree
  NotARepository,
  /// No branch prefix from `--prefix`, the repository's `branchdeck.branchPrefix`, the app setting or the global one
  NoBranchPrefix,
  /// `--branch` names no current group
  UnknownBranch,
  /// No `@{u}`, `<remote>/master`, `<remote>/main` or local `master`/`main` to compare against
  NoBaseline,
  /// A plan doesn't apply to the commits ahead of the baseline
  InvalidPlan,
  /// A commit of the request is not ahead of the baseline
  NotAheadOfBaseline,
  /// `bd safe-push --whole-stack`: a commit of the stack branch conflicts with the commits of the stack before it
  /// (`commits`, `paths`), though each branch builds: nothing was pushed or started; land the stack in pieces
  StackConflict,
  /// `bd safe-push --whole-stack`: the stack branch is not the build of the stack's commits: a branch of it is blocked or
  /// failed, or the ref is stale (another process moved it); `bd sync` says why. Nothing was pushed or started
  StackNotBuilt,
  /// `bd safe-push`: Patronus can't rebase what the robot lands onto the newest target: a commit (`commits`) conflicts
  /// in files (`paths`) that the target changed after the base of the branch. Nothing was pushed or started; `bd update`
  /// resolves it locally, then start again
  TargetConflict,
  /// `bd move-hunk`: the change doesn't apply to a commit between the two, which changes the same lines or lacks the
  /// file; `commits` and `paths` name it. Nothing was written: move it by hand, or not at all
  HunkConflict,
  /// `bd move-hunk --block` names a block the file in the commit doesn't have now; `blocks` are the ids it has
  BlockNotFound,
  /// HEAD moved since the caller read it; read again and retry
  HeadMoved,
  /// The issue groups (`refs/branch-deck/issue-groups`) changed since the caller read them; read again and retry
  GroupsMoved,
  /// The stack parents (`refs/branch-deck/stack-parents`) changed since the caller read them; read again and retry
  StacksMoved,
  /// The remote branch is not where the lease expects it; bd fetched it, `branches` says what it has now. Not retryable:
  /// a push again (with --force) would drop what the remote has now; show `dropped_commits` to the user first
  LeaseRejected,
  /// Another git process holds a lock; retry
  Locked,
  /// A rebase, merge, cherry-pick or revert is in progress
  OperationInProgress,
  /// HEAD is detached
  DetachedHead,
  /// `bd undo` of a rewrite, an update or a pick the branch (or the issue groups, the stack parents) moved on from: a
  /// commit, another rewrite, a switch. The token can never succeed again
  UndoStale,
  /// `bd pick --from`: the reflog of the branch has no fork point of the worktree, and their shared history is local:
  /// commits the branch rewrote since would come back. Name the commits: `bd pick <first>^..<last>`
  RangeUnknown,
  /// `bd pick --from`: the worktree stands on a newer baseline than the branch; the branch is updated first
  /// (`bd update`, by the session that owns it)
  SourceOnNewerBaseline,
  /// Uncommitted, staged or untracked files sit on paths `bd update`, `bd pick` (or their undo) writes; `paths` lists
  /// them. Nothing was changed; never stash them, ask the user
  UncommittedOverlap,
  /// `bd update --continue` or `--abort` without a resolve session of this worktree
  NoSession,
  /// `--continue --round <n>` names another round than the open one: `bd update` prints the open round
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
  /// The resolution leaves the round's commit without changes: ask the user, then `--allow-empty` or `--drop-empty`
  EmptyResolution,
  /// The branch of a resolve session moved, but the files did not follow and bd can't finish it: show the user
  FinishPending,
  /// A branch can't be pushed: not synced, or behind its remote branch without --force; or the remote refused the push
  /// with every lease holding (a hook, a protected branch). Nothing was pushed
  PushRefused,
  /// No remote to push to
  NoRemote,
  /// `bd install` can't write the symlink or a skill, e.g. a file that is not a symlink is in the way
  InstallRefused,
  /// `bd safe-push`: the code owners haven't approved the branch, and a Safe Push fails without their approvals; nothing
  /// was pushed or started. `review` lists the groups, their reviewers and files, and the merge request; ask the user
  ApprovalsMissing,
  /// `bd safe-push`: a robot runs on the last commit of the branch, on a branch it is built on, or on a branch built on
  /// it; or a Safe Push of an older commit runs, which the push would fail (`--cancel-running`). `bd push`: a Safe Push
  /// runs on the branch or lands it with a branch built on it: a push won't land (`--restart-running`)
  RobotRunning,
  /// `bd push` and `bd safe-push`: a merged Safe Push landed all of the branch (its own, one of a branch built on it, or
  /// of the whole stack), or every branch of the whole stack: its commits are in the target until the update archives
  /// it, a push or a robot would land nothing. `bd status` names who landed it (`landed_by`); `bd update` archives it
  Landed,
  /// `bd safe-push`: the branch, or a branch it is built on, removes a declaration that its tree still uses, and a
  /// commit of another branch changes the use (`bd check` names it); the build would fail. `--allow-dangling` starts
  /// anyway
  DanglingReference,
  /// `bd safe-push`: the repository is not on Space, or the baseline is not a branch of `origin`
  NotOnSpace,
  /// `bd safe-push`: no Space token for Patronus, it expired, or Patronus rejected it; the user runs `./safePush.cmd`
  /// once
  NotLoggedIn,
  /// `bd safe-push`: Patronus can't start the robot: the target is not protected, the pull request is a draft, or the
  /// `/patronus` comment was rejected
  PatronusRefused,
  /// `bd request-review`: the branch has no open pull request, or only a draft; `bd safe-push <branch>
  /// --create-pull-request` opens one, a draft is marked ready for review on GitHub
  NoPullRequest,
  /// `bd request-review`: a missing group has no code owner with a GitHub login Patronus names; pass `--reviewer`
  NoReviewer,
  /// `bd request-review`: GitHub asked nobody, it refused every reviewer (the author, a user without access); the
  /// message names each with the reason
  ReviewRefused,
  /// `bd lease return`: the worktree holds no lease of the pool, or its get still prepares it
  NoLease,
  /// `bd lease return --lease-id`: the worktree holds another lease
  LeaseMismatch,
  /// `bd lease adopt`: the directory is not the top level of a linked worktree of this repository
  NotASlot,
  /// `bd lease adopt`: the pool holds the worktree already
  SlotInPool,
  /// `bd lease return` and `adopt`: a process other than bd and its ancestors has its working directory in the worktree;
  /// the message names it. Nothing was changed
  SlotBusy,
  /// `bd lease return`: HEAD has commits that no branch, tag or remote branch holds (`commits`): land them first
  /// (`bd pick --from <worktree> --follow` in the shared checkout), or `--confirm-preserved` gives them up
  UnlandedCommits,
  /// `bd lease return` and `adopt`: the worktree has local changes (`paths`); `bd lease return --confirm-preserved`
  /// discards them, once the user agreed
  DirtyWorktree,
  /// `bd lease get --branch`: the branch exists
  BranchExists,
  /// `bd lease adopt --move`: the worktree is on another volume than the pool directory; a move would copy every file
  OtherVolume,
  /// No usable git executable
  GitNotFound,
  /// A git command failed
  GitError,
  /// A network command (fetch, push) failed
  NetworkError,
  /// A request to Patronus failed; `message` is its answer
  PatronusError,
}

impl ErrorCode {
  /// 0 ok, 1 conflict (never an error: the output of the command, see `CONFLICT_EXIT`), 2 invalid input, 3 stale
  /// (retry), 4 refused, 5 git or network error
  pub const fn exit_code(self) -> i32 {
    match self {
      Self::InvalidArguments
      | Self::BlockNotFound
      | Self::NotARepository
      | Self::NoBranchPrefix
      | Self::UnknownBranch
      | Self::InvalidPlan
      | Self::NotAheadOfBaseline
      | Self::NoSession
      | Self::UnresolvedMarkers
      | Self::ResolutionOutsideConflict
      | Self::MissingResolution
      | Self::EmptyResolution
      | Self::NoLease
      | Self::LeaseMismatch
      | Self::NotASlot
      | Self::SlotInPool => 2,
      Self::HeadMoved | Self::GroupsMoved | Self::StacksMoved | Self::RoundMoved | Self::Locked => 3,
      Self::NoBaseline
      | Self::OperationInProgress
      | Self::DetachedHead
      | Self::UndoStale
      | Self::LeaseRejected
      | Self::StackConflict
      | Self::StackNotBuilt
      | Self::TargetConflict
      | Self::HunkConflict
      | Self::RangeUnknown
      | Self::SourceOnNewerBaseline
      | Self::UncommittedOverlap
      | Self::SessionStale
      | Self::UnsupportedConflict
      | Self::FinishPending
      | Self::PushRefused
      | Self::NoRemote
      | Self::InstallRefused
      | Self::ApprovalsMissing
      | Self::RobotRunning
      | Self::Landed
      | Self::DanglingReference
      | Self::NotOnSpace
      | Self::NotLoggedIn
      | Self::PatronusRefused
      | Self::NoPullRequest
      | Self::NoReviewer
      | Self::ReviewRefused
      | Self::SlotBusy
      | Self::UnlandedCommits
      | Self::DirtyWorktree
      | Self::BranchExists
      | Self::OtherVolume => 4,
      Self::GitNotFound | Self::GitError | Self::NetworkError | Self::PatronusError => 5,
    }
  }

  /// Whether running the same command again (after re-reading the state for a stale code) may succeed
  pub const fn retryable(self) -> bool {
    matches!(
      self,
      Self::HeadMoved | Self::GroupsMoved | Self::StacksMoved | Self::RoundMoved | Self::Locked | Self::NetworkError
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
  /// The block ids of `block_not_found`; boxed, as every command returns this error
  pub blocks: Option<Box<Vec<String>>>,
  /// The review of `approvals_missing`; boxed, as every command returns this error
  pub review: Option<Box<ReviewSummary>>,
}

impl CliError {
  pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
    Self {
      code,
      message: message.into(),
      commits: Vec::new(),
      branches: Vec::new(),
      paths: Vec::new(),
      blocks: None,
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
        blocks: self.blocks.as_deref().cloned().unwrap_or_default(),
        review: self.review.as_deref().cloned(),
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
      match error.downcast_ref::<SyncDeferred>() {
        Some(SyncDeferred::OperationInProgress(_)) => ErrorCode::OperationInProgress,
        Some(SyncDeferred::DetachedHead) => ErrorCode::DetachedHead,
        None => ErrorCode::GitError,
      }
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
