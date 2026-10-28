//! The stable error codes: the `code` of every error object `bd` prints and of every error a command of the app
//! answers (`ApiError`), with the exit code and the retry of its class. Agents and the page branch on the code (and
//! the exit code), never on the message.

use crate::landing::LandingRefusal;
use serde::{Deserialize, Serialize};

/// A stable error code. Agents branch on the code and the exit code, never on the message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
  /// The parked branches (`refs/branch-deck/parked`) changed since the caller read them; read again and retry
  ParkedMoved,
  /// The app's sync of a session whose branch prefix the git config no longer names (a `git config` meanwhile): nothing
  /// was built; resolve the prefix again and sync the session under it. `bd` resolves the prefix once per command and
  /// never answers it
  PrefixChanged,
  /// An operation of the remote was prepared over facts that changed before its first effect (a rewrite of another
  /// session, a pull request, a robot, a remote branch): its plan is another one now, and nothing was done. Read the
  /// plan again (`bd safe-push --check`), then retry
  PlanChanged,
  /// `bd unpark`: the branch is not parked
  NotParked,
  /// `bd unapply`: the virtual branch is not the copies of its commits now (a conflict, no sync since a new commit):
  /// a commit would leave HEAD without a copy. `bd sync` says why; fix the conflict, then unapply. Nothing was changed
  BranchNotBuilt,
  /// `bd unapply`: a worktree has the virtual branch checked out (the message names it); nothing was changed
  BranchCheckedOut,
  /// `bd unapply`: a later commit depends on the changes of a commit of the branch, or the range holds a merge; nothing
  /// was changed. Move that commit into the branch first (`bd group`), or leave the branch applied
  DropConflict,
  /// `bd squash` and `bd fold`: a squashed commit and a commit in between or after it change the same lines
  /// (`commits`, `paths` when a merge named them); nothing was changed. The user decides: squash other commits, or
  /// leave them as they are
  SquashConflict,
  /// `bd squash` and `bd fold`: the squashed commit would have no changes, or a later commit would become empty
  /// (`commits`); nothing was changed
  SquashEmpty,
  /// `bd reapply`: a commit of the unapplied branch conflicts with HEAD (`commits`, `paths`); nothing was changed. The
  /// user decides: resolve it in the branch elsewhere, or leave it unapplied
  ReapplyConflict,
  /// `bd reapply`: HEAD has every commit of the unapplied branch already, or the branch has no common history with the
  /// baseline; nothing was changed
  NothingToReapply,
  /// `bd push` and `bd safe-push` of a branch that is unapplied (`unapplied` of `bd status`): its commits are not in
  /// HEAD, it lands nothing; `bd push --unapplied <name>` keeps a copy on the remote. Nothing was pushed or started
  Unapplied,
  /// `bd push` and `bd safe-push`: the branch is parked, or built on a parked branch, or the whole stack holds one: its
  /// commits stay local until the user unparks it (`bd unpark`, never on an agent's own). Nothing was pushed or started
  Parked,
  /// The remote branch is not where the lease expects it; bd fetched it, `branches` says what it has now. Not retryable:
  /// a push again (with --force) would drop what the remote has now; show `dropped_commits` to the user first
  LeaseRejected,
  /// Another git process holds a lock; retry
  Locked,
  /// An operation of another process (the app, another `bd`) runs what this one needs: the same subject, a branch it
  /// lands or pushes, a robot, a field of a pull request (`holder` names it, from the operations registry of the
  /// repository). Nothing was done; retry when it is done
  Busy,
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
  /// `bd pick --remote`: the remote branch has no commit the branch lacks; nothing was picked
  NothingToPick,
  /// The app's Pick Remote Commits: a commit of the remote branch conflicts with the checked out branch (`details`: the
  /// commit and its files); nothing was picked
  PickConflict,
  /// `bd lease pick`: the checkout the lease was cut from has another branch checked out than at the get; nothing was
  /// changed. Ask the user: they switched it, and the pick waits until the branch is back
  HomeBranchSwitched,
  /// `bd lease pick`: the lease was cut from a detached HEAD, so no branch is named to land on
  NoHomeBranch,
  /// `bd lease pick` in a repository whose `.branchdeck.toml` sets `pick_requires_check`: no green build or test
  /// recorded the tree of the lease's HEAD (`bd-checks.jsonl` of its git dir); nothing landed. Build and test that
  /// tree, then pick again
  UncheckedTree,
  /// `bd update` in a lease of the pool: the lease follows the branch it was cut from, not the baseline; `bd lease
  /// pick` lands its commits there and moves it onto the tip, and the owner of the shared checkout updates that branch
  LeaseFollowsHome,
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
  /// The resolution leaves the round's commit without changes: ask the user, then `--allow-empty` or `--drop-empty`;
  /// `--landed` for a commit whose twin the baseline has
  EmptyResolution,
  /// `bd update --continue --landed` on a round whose commit has no twin in the baseline (no `landed_as`)
  NotLandedRound,
  /// `bd update --continue --landed` without `--user` on a round whose commit changes lines that its twin in the
  /// baseline lacks (`landed_as.local_only`): a drop loses them, so the user decides
  LandedDiffers,
  /// `bd update --continue --landed` on a round whose commit still changes files with `onto` for every conflicting
  /// file: `paths` lists them
  LandedNotEmpty,
  /// The branch of a resolve session moved, but the files did not follow and bd can't finish it: show the user
  FinishPending,
  /// A branch can't be pushed: not synced, or behind its remote branch without --force; or the remote refused the push
  /// with every lease holding (a hook, a protected branch, a pack too large). Nothing was pushed
  PushRefused,
  /// The files the push would add exceed the push size limit (`branchdeck.pushSizeLimit`, 50 MiB unless set): mostly
  /// build output committed by mistake. Nothing was pushed, the remote not asked; the message names the directory that
  /// holds most of it and the largest files, `paths` lists them. Show the user: the files leave the commits, or the
  /// user raises the limit
  PushTooLarge,
  /// No remote to push to
  NoRemote,
  /// `bd install` can't write the symlink or a skill, e.g. a file that is not a symlink is in the way
  InstallRefused,
  /// `bd safe-push`: the code owners haven't approved the branch, and a Safe Push fails without their approvals; nothing
  /// was pushed or started. `review` lists the groups, their reviewers and files, and the merge request; ask the user
  ApprovalsMissing,
  /// `bd safe-push`: the push of new commits into the pull request the robot starts from dismisses its approvals, which
  /// Patronus counts (the rules of the target dismiss stale reviews on push): the robot would start without them and
  /// fail; nothing was pushed or started. Push first (`bd push <branch>`), then ask for the review again
  /// (`bd request-review <branch>`)
  PushDismissesApprovals,
  /// `bd safe-push`: GitHub didn't answer the rules of the target (no token, no access, an error) for a push of new
  /// commits into a pull request someone approved: the push may dismiss the approvals Patronus counts; nothing was
  /// pushed or started. Ask the user; `--allow-unknown-rules` starts anyway
  RulesUnknown,
  /// `bd safe-push`: Patronus couldn't answer the code-owner review of the Safe Push (its pre-check failed): whether the
  /// approvals count is not known, never "approved"; nothing was pushed or started. Ask the user;
  /// `--allow-unknown-review` starts anyway, and the robot checks the approvals when it starts
  ReviewUnknown,
  /// `bd safe-push`: the open pull request of the branch goes into another base than the target (a release branch), and
  /// Patronus merges a pull request into its base: its approvals count for no Safe Push into the target; nothing was
  /// pushed or started. Retarget it on GitHub once the user approves; `--without-approvals` starts through the API
  PullRequestOtherBase,
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
  /// `bd safe-push`: a commit of what the robot lands uses a name that a commit of another branch adds, which no merged
  /// Safe Push landed (`bd check` names it in `missing_declarations`); git merges it, the build would fail. Stack the root
  /// of what lands on that branch, or move the commit; `--allow-missing` starts anyway
  MissingDeclaration,
  /// `bd safe-push`: the repository is not on Space, or the baseline is not a branch of `origin`
  NotOnSpace,
  /// `bd safe-push`: no Space token for Patronus, it expired, or Patronus rejected it; the user runs `./safePush.cmd`
  /// once
  NotLoggedIn,
  /// `bd safe-push`: Patronus can't start the robot: the target is not protected, the pull request is a draft, or the
  /// `/patronus` comment was rejected
  PatronusRefused,
  /// `bd land`: the token may not push into the repository of the target (`viewerPermission` below write): nothing was
  /// pushed; someone with write access lands it
  NoWriteAccess,
  /// `bd land`: the rules of the target take a change only through a pull request, and the user may not bypass them:
  /// nothing was pushed. Land it through its pull request
  BaselineRequiresPullRequest,
  /// `bd land`: the target refuses the push: a rule of it no push of the copy passes (the merge queue, restricted
  /// updates, signed commits), or the server refused it (`[remote rejected]`: a hook, a protected branch; the message
  /// says what it said). Nothing was pushed
  BaselineRefused,
  /// `bd land`: the target moved since the copy was built on it (the server refused a push that is no fast-forward):
  /// nothing was pushed, the baseline was fetched. `bd update` brings the branch onto it, then `bd land` again builds
  /// the copy anew
  BaselineMoved,
  /// `bd land`: CI did not pass on the commit the push lands (no open pull request, one that shows another commit, its
  /// checks failing or running, GitHub listing none), or changes are requested on its pull request: a push into the
  /// target runs no CI. Nothing was pushed; ask the user, `--without-checks` pushes once they agree
  ChecksNotPassed,
  /// `bd land`: the route of the repository (`branchdeck.landing`) lands no branch through `bd land` yet. Nothing was
  /// pushed
  LandingRouteUnsupported,
  /// `bd land`: GitHub can't merge the pull request of the branch: a merge would conflict, the base requires its head to
  /// be up to date (`bd update`, then `bd push`), or GitHub refused the merge (the message says what it said). Nothing
  /// was merged
  NotMergeable,
  /// `bd land`: the rules of the target hold the merge of the pull request back: approvals or required checks are
  /// missing, changes are requested, it is a draft. Nothing was merged; it lands once they come
  MergeBlocked,
  /// `bd request-review`: the branch has no open pull request, or only a draft; `bd safe-push <branch>
  /// --create-pull-request` opens one, a draft is marked ready for review on GitHub
  NoPullRequest,
  /// `bd request-review`: a missing group has no code owner with a GitHub login Patronus names; pass `--reviewer`
  NoReviewer,
  /// `bd request-review`: the branch has work its remote branch lacks, which the reviewers would not see; `--push`
  /// pushes it first
  NeedsPush,
  /// `bd request-review`: GitHub asked nobody, it refused every reviewer (the author, a user without access); the
  /// message names each with the reason
  ReviewRefused,
  /// `bd lease return`, `bd lease pick`: the worktree holds no lease of the pool, or its get still prepares it
  NoLease,
  /// `bd lease return`: a resolve session of the slot is open; finish it (`bd update --continue`) or close it
  /// (`bd update --abort`) first, so the next lease of the slot never finds it
  ResolveInProgress,
  /// `bd lease return --lease-id`: the worktree holds another lease
  LeaseMismatch,
  /// `bd lease adopt`: the directory is not the top level of a linked worktree of this repository
  NotASlot,
  /// `bd lease adopt`: the pool holds the worktree already
  SlotInPool,
  /// `bd lease return` and `adopt`: a process other than bd and its session has its working directory in the worktree;
  /// `processes` names each with its argv and kind. Nothing was changed
  SlotBusy,
  /// `bd lease return`: the slot is one of the host slots that `bd-host` keeps for its runs (`n`); `bd-host slots
  /// --local --shrink <n>` gives it back to the pool. Nothing was changed
  HostSlot,
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
  /// `bd lease get`: the volume of the pool has less free space than `branchdeck.leaseMinFree` or `--min-free` asks
  LowDiskSpace,
  /// No usable git executable: none found, or one older than the minimum (the message says which and how to install
  /// or upgrade Git)
  GitNotFound,
  /// Git cannot form the identity of the user (`git var GIT_COMMITTER_IDENT`: `user.name` or `user.email` unset), which
  /// the notes and the rewritten commits are written with: nothing was written; set both, then run again
  NoIdentity,
  /// A git command failed
  GitError,
  /// A network command (fetch, push) failed
  NetworkError,
  /// A request to Patronus failed; `message` is its answer
  PatronusError,
  /// A failure without a code of its own: `message` is the whole chain of its causes. `bd` prints `git_error` for a
  /// failure of git or IO, as before; the app answers this code
  Internal,
  /// The user stopped it: a sign-in, a review fetch, a symbol search, a CLI tool's action, a lock wait. Nothing more to show
  Canceled,
}

impl ErrorCode {
  /// 0 ok, 1 conflict (never an error: the output of the command, see `bd_core::error::CONFLICT_EXIT`), 2 invalid input, 3 stale
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
      | Self::NotLandedRound
      | Self::LandedDiffers
      | Self::LandedNotEmpty
      | Self::NoLease
      | Self::LeaseMismatch
      | Self::NotASlot
      | Self::SlotInPool
      | Self::NotParked
      | Self::NoIdentity => 2,
      Self::HeadMoved
      | Self::GroupsMoved
      | Self::StacksMoved
      | Self::ParkedMoved
      | Self::PrefixChanged
      | Self::RoundMoved
      | Self::PlanChanged
      | Self::BaselineMoved
      | Self::Locked
      | Self::Busy => 3,
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
      | Self::NothingToPick
      | Self::PickConflict
      | Self::HomeBranchSwitched
      | Self::NoHomeBranch
      | Self::UncheckedTree
      | Self::LeaseFollowsHome
      | Self::ResolveInProgress
      | Self::UncommittedOverlap
      | Self::SessionStale
      | Self::UnsupportedConflict
      | Self::FinishPending
      | Self::PushRefused
      | Self::PushTooLarge
      | Self::NoRemote
      | Self::InstallRefused
      | Self::ApprovalsMissing
      | Self::PushDismissesApprovals
      | Self::RulesUnknown
      | Self::ReviewUnknown
      | Self::PullRequestOtherBase
      | Self::RobotRunning
      | Self::Landed
      | Self::Parked
      | Self::BranchNotBuilt
      | Self::BranchCheckedOut
      | Self::DropConflict
      | Self::SquashConflict
      | Self::SquashEmpty
      | Self::Unapplied
      | Self::ReapplyConflict
      | Self::NothingToReapply
      | Self::DanglingReference
      | Self::MissingDeclaration
      | Self::NotOnSpace
      | Self::NotLoggedIn
      | Self::PatronusRefused
      | Self::NoWriteAccess
      | Self::BaselineRequiresPullRequest
      | Self::BaselineRefused
      | Self::ChecksNotPassed
      | Self::LandingRouteUnsupported
      | Self::NotMergeable
      | Self::MergeBlocked
      | Self::NoPullRequest
      | Self::NoReviewer
      | Self::NeedsPush
      | Self::ReviewRefused
      | Self::SlotBusy
      | Self::HostSlot
      | Self::UnlandedCommits
      | Self::DirtyWorktree
      | Self::BranchExists
      | Self::OtherVolume
      | Self::LowDiskSpace
      | Self::Canceled => 4,
      Self::GitNotFound | Self::GitError | Self::NetworkError | Self::PatronusError | Self::Internal => 5,
    }
  }

  /// What may succeed after this error: nothing (`No`), the same request again (`Same`: the holder of the lock or the
  /// operation of another process that holds what it needs finishes, the network comes back), or the request built again from a new read of the state (`AfterReread`: HEAD or a store
  /// moved, another round of the resolve session is open)
  pub const fn retry(self) -> Retry {
    match self {
      Self::Locked | Self::Busy | Self::NetworkError => Retry::Same,
      code if code.moved_ref() => Retry::AfterReread,
      Self::PrefixChanged | Self::RoundMoved | Self::PlanChanged | Self::BaselineMoved => Retry::AfterReread,
      _ => Retry::No,
    }
  }

  /// Whether a ref the request was made on moved before anything was written (HEAD, the issue groups, the stack
  /// parents, the parked branches): the refusal of the compare-and-swap of a rewrite, which a request built again on
  /// the refs as they are now may pass (`Retry::AfterReread` of `retry`), never a failure that came after a write. The
  /// one list of these codes: the engine runs a rewrite again only after one, and an undo is stale after one
  /// (`in_undo`)
  pub const fn moved_ref(self) -> bool {
    matches!(self, Self::HeadMoved | Self::GroupsMoved | Self::StacksMoved | Self::ParkedMoved)
  }

  /// The code of this error when an undo fails with it: a ref that moved on since the operation (`moved_ref`) makes
  /// the token stale (`UndoStale`: the same undo can never succeed), and a plan the token names is a wrong argument
  /// (`InvalidArguments`); any other code stays. The one rule of `bd undo` and of the undo commands of the app
  #[must_use]
  pub const fn in_undo(self) -> Self {
    match self {
      code if code.moved_ref() => Self::UndoStale,
      Self::InvalidPlan => Self::InvalidArguments,
      code => code,
    }
  }

  /// Whether running the same command again (after re-reading the state for a stale code) may succeed: the `retryable`
  /// of `bd`
  pub const fn retryable(self) -> bool {
    !matches!(self.retry(), Retry::No)
  }

  /// Whether an agent shows the error to the user and asks before it goes on: a decision on the user's files or
  /// commits that only the user makes (`ask_user` of the error object of `bd`)
  pub const fn asks_user(self) -> bool {
    matches!(self, Self::UncommittedOverlap | Self::EmptyResolution | Self::LandedDiffers)
  }
}

/// What may succeed after an error (`ErrorCode::retry`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum Retry {
  /// The same request fails again: the user decides
  No,
  /// The same request again may succeed, as it is
  Same,
  /// A request built again from a new read of the state may succeed; the same one fails again
  AfterReread,
}

/// The error a command of the app answers, the same JSON from Tauri and from the E2E server: `{"code", "message",
/// "details", "retry", "operation_id"}`. The page branches on `code` (and `details`), never on `message`, which is for
/// the user
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[error("{message}")]
pub struct ApiError {
  pub code: ErrorCode,
  pub message: String,
  /// The data of the code, typed by it; null for a code without data
  pub details: Option<ErrorDetails>,
  /// `code.retry()`, so the page needn't know the classes
  pub retry: Retry,
  /// The operation that failed, the `operation_id` of its log lines (the key of its Activity entry); null for an error
  /// raised outside one
  pub operation_id: Option<String>,
}

impl ApiError {
  pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
    Self {
      code,
      message: message.into(),
      details: None,
      retry: code.retry(),
      operation_id: None,
    }
  }

  /// A failure without a code of its own (`ErrorCode::Internal`)
  pub fn internal(message: impl Into<String>) -> Self {
    Self::new(ErrorCode::Internal, message)
  }

  /// This error as an undo answers it: its code `in_undo`, with the retry of that code
  #[must_use]
  pub const fn in_undo(mut self) -> Self {
    self.code = self.code.in_undo();
    self.retry = self.code.retry();
    self
  }

  #[must_use]
  pub fn with_details(mut self, details: ErrorDetails) -> Self {
    self.details = Some(details);
    self
  }
}

/// The data of an error code, tagged by its kind of data: `{"type": "conflict", "commits": [...], "paths": [...]}`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ErrorDetails {
  /// The commit that conflicts and its files: `target_conflict`, `hunk_conflict`, `stack_conflict`,
  /// `reapply_conflict`, `drop_conflict`
  Conflict { commits: Vec<String>, paths: Vec<String> },
  /// The files the error is about, relative to the repository: the local files of `uncommitted_overlap`; the directory
  /// that holds most of a `push_too_large` push (`dir/`) and its largest files
  Paths { paths: Vec<String> },
  /// The full names of the branches the error is about: the rejected ones of `lease_rejected`
  Branches { branches: Vec<String> },
  /// The ids of the blocks the file has now: `block_not_found`
  Blocks { blocks: Vec<String> },
  /// Who holds the repository lock: `locked`; `None` when the holder wrote no record
  Locked { holder: Option<LockHolderInfo> },
  /// The operation of another process that runs what this one needs: `busy`; `None` when the registry is of another
  /// Branch Deck (another schema), whose operations can't be read
  Busy { holder: Option<Box<OperationHolder>> },
  /// What the landing gate refuses of a start, each with its fix: `missing_declaration`, `dangling_reference`
  Landing { refusals: Vec<LandingRefusal> },
  /// The prefix to suggest where none names the branches (`no_branch_prefix` of the app): the local part of the
  /// committer email of git, without a `+tag`, as a branch name; empty when git names no email
  PrefixSuggestion { prefix: String },
}

/// The holder of the repository lock as the page names it: "`bd push auth` (pid 4242) has held the repository for 40 s"
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LockHolderInfo {
  pub tool: HolderTool,
  /// Its command line, the program by its file name: `bd check --fix`
  pub command: String,
  pub pid: u32,
  /// Seconds since it took the lock
  pub held_for: u32,
}

/// An operation of another process as the page and `bd` name it: "`bd` (pid 4242) has run the Safe Push of auth for 40 s"
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct OperationHolder {
  /// Its id, the `operation_id` of its log lines
  pub op: String,
  pub tool: HolderTool,
  pub pid: u32,
  pub kind: OperationKind,
  /// The branch it runs on, `stack:<root>` a whole stack; `None` for an operation of a robot, a pull request or an issue
  pub subject: Option<String>,
  /// Seconds since it was admitted
  pub held_for: u32,
  /// A start whose answer was lost (`OperationStep::StartUnknown`): it holds what it held until a read of Patronus tells
  /// whether its robot started, or the user forgets it (Forget This Start, `bd operation forget <op>`)
  #[serde(default)]
  pub unknown: bool,
}

/// What an operation does, by the family of its intent (`Intent`), for the words of its holder
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
  SafePush,
  DryRun,
  Push,
  RestartRobot,
  CancelRobot,
  Retarget,
  CreatePullRequest,
  RequestReview,
  ClosePullRequest,
  DisableAutoMerge,
  CloseLandedPullRequests,
  SubmitReview,
  SetIssueState,
  ForgetStart,
  Rewrite,
  FetchBaseline,
  UpdateProject,
  DismissLanded,
  FetchReview,
  Pick,
}

/// Which program holds the lock, or runs an operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum HolderTool {
  /// The agent CLI
  Bd,
  /// Branch Deck, the app
  App,
}

#[cfg(test)]
#[path = "error_test.rs"]
mod error_test;
