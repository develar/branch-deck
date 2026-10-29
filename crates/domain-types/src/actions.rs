//! The actions of a subject and whether each can run now, as Rust decides them for every surface of the app
//! (spec/branch/branch-row.spec.md, "Branch menu"; docs/design/rust-domain-authority.md, s8-policy): `domain_policy::actions`
//! answers an `ActionState` for each action a subject offers, by a stable id, with the operations of the session laid
//! over it (`with_operations`: its own intent runs or waits, a rewrite queues behind the one that holds HEAD, a remote
//! action waits for the operation that holds what it needs); the page presents each by its id (its label, key, icon and
//! what it runs, `src/presenters/actions.ts`) and words a blocker and an operation.

use crate::landing::Landed;
use crate::next_step::{StepBlocker, StepFact};
use crate::operation::OperationId;
use crate::reason::Reason;
use crate::repository_view::RobotOf;
use crate::session::StackBranchAlong;
use serde::Serialize;

/// An action of a branch, of a whole stack or of a robot, by the id its menu item has on every surface
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum ActionId {
  // the landing of a branch
  SafePush,
  SafePushWithProfile,
  DryRun,
  CheckApprovals,
  SafePushWithoutReview,
  Push,
  PushWithChildren,
  /// The commits someone else pushed to its remote branch, taken into the branch (`bd pick --remote`)
  PickRemoteCommits,
  /// The copy pushed into the target where the repository lands by a push (`Intent::Land`): of a branch, of a whole
  /// stack
  PushToBaseline,
  /// The merge of its pull request on GitHub where the repository lands by one (`Intent::Land`): of a branch, of a whole
  /// stack; now ("Merge #7"), or once the checks pass ("Auto-Merge #7")
  MergePullRequest,
  /// The auto-merge of its pull request disabled (`Intent::DisableAutoMerge`)
  CancelAutoMerge,
  // its review
  ReviewChanges,
  OpenPullRequest,
  RequestReview,
  RetargetPullRequest,
  CreatePullRequest,
  // its shape and its commits: the rewrites
  Rename,
  MoveTo,
  StackOn,
  Unstack,
  Ungroup,
  AmendChanges,
  AddIssueReference,
  SquashFixups,
  Unapply,
  // what sets it aside without a rewrite
  Park,
  Unpark,
  // the whole stack of a root
  SafePushWholeStack,
  DryRunWholeStack,
  SafePushMainLine,
  OpenStackPullRequest,
  CreateStackPullRequest,
  /// The open pull request of its stack branch closed with a comment (`Intent::ClosePullRequest`): offered while it is
  /// open, shown by the page once it no longer matches the stack
  CloseStackPullRequest,
  ReviewWholeStack,
  // a robot
  RestartRobot,
  PushAndRestartRobot,
  CancelRobot,
  CopyRobotPrompt,
  CopyRobotFullPrompt,
  OpenRobot,
  OpenRobotPullRequest,
  // a pull request to review (Your Turn, Reviews and the review window)
  Review,
  Approve,
  /// Approve and Safe Push, or the Safe Push alone after an approval of the head
  SafePushPullRequest,
  RequestChanges,
  /// The branch of the table an own pull request is of
  ShowBranch,
  /// A pull request the user added by its link leaves Reviews
  RemoveFromReviews,
  // a start whose answer was lost
  /// Opens where its robot would be: Patronus, or the pull request of its comment
  OpenUnknownStart,
  /// Its record leaves the operations registry (`Intent::ForgetStart`)
  ForgetStart,
}

impl ActionId {
  /// Every id, in the order of the enum: the catalog the page's presenter is tested on
  pub const ALL: [Self; 49] = [
    Self::SafePush,
    Self::SafePushWithProfile,
    Self::DryRun,
    Self::CheckApprovals,
    Self::SafePushWithoutReview,
    Self::Push,
    Self::PushWithChildren,
    Self::PickRemoteCommits,
    Self::PushToBaseline,
    Self::MergePullRequest,
    Self::CancelAutoMerge,
    Self::ReviewChanges,
    Self::OpenPullRequest,
    Self::RequestReview,
    Self::RetargetPullRequest,
    Self::CreatePullRequest,
    Self::Rename,
    Self::MoveTo,
    Self::StackOn,
    Self::Unstack,
    Self::Ungroup,
    Self::AmendChanges,
    Self::AddIssueReference,
    Self::SquashFixups,
    Self::Unapply,
    Self::Park,
    Self::Unpark,
    Self::SafePushWholeStack,
    Self::DryRunWholeStack,
    Self::SafePushMainLine,
    Self::OpenStackPullRequest,
    Self::CreateStackPullRequest,
    Self::CloseStackPullRequest,
    Self::ReviewWholeStack,
    Self::RestartRobot,
    Self::PushAndRestartRobot,
    Self::CancelRobot,
    Self::CopyRobotPrompt,
    Self::CopyRobotFullPrompt,
    Self::OpenRobot,
    Self::OpenRobotPullRequest,
    Self::Review,
    Self::Approve,
    Self::SafePushPullRequest,
    Self::RequestChanges,
    Self::ShowBranch,
    Self::RemoveFromReviews,
    Self::OpenUnknownStart,
    Self::ForgetStart,
  ];
}

/// An action a subject offers now, and whether it can run
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ActionState {
  pub id: ActionId,
  pub availability: Availability,
}

/// Whether an action can run now: disabled with the reason, never hidden for it (UX Writing, "Actions"), or what an
/// operation of the session does to it
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Availability {
  Enabled,
  Disabled {
    reason: ActionBlocker,
  },
  /// Its intent runs in this process as the operation `op`: a start of it joins it (a spinner in place of its icon)
  Running {
    op: OperationId,
  },
  /// Its intent waits in the queue of the session as the operation `op`, behind `behind`: a start of it joins it
  Queued {
    op: OperationId,
    behind: OperationId,
  },
  /// It can start, and its start waits in the queue of the session behind `op`, the newest operation of this process that
  /// holds HEAD (a rewrite, the check and the move of Update Project): a rewrite behind a rewrite
  Waits {
    op: OperationId,
  },
}

/// Why an action can't run now, as a code with the facts its words need
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ActionBlocker {
  /// A reason of the rules: its build blocked, parked, a robot that holds or lands it, a stack branch not built
  Rule {
    reason: Reason,
  },
  /// What the next step of the subject says against its start (`NextStep::safe_push`, `dry_run`, `without_review`): the
  /// step words it
  Step {
    blocker: StepBlocker,
  },
  /// A fact of the remote it needs has no answer yet
  Pending {
    fact: StepFact,
  },
  /// Patronus didn't tell why the robot failed: a restart and a prompt would act as if it hadn't failed for approvals
  ProblemsFailed {
    message: String,
  },
  /// A sync rebuilds the branch, whose push waits for its build; of Push with Children, the remote statuses a running
  /// sync computes are to come. No rewrite waits for a sync: the session admits it at once, named on the table its window
  /// showed
  Syncing,
  /// A remote action on a frame nobody checked yet: the table is the frame an earlier run of the app saved
  /// (`RepositoryView::cached`), or a fact the action reads is a value of it (`FactState::Cached`). It waits for the first
  /// sync and the fresh answers of this process; a rewrite does not, the session runs it after that sync
  Cached,
  /// The operation `op` holds what it needs (a ref it pushes, a robot, a field of a pull request): of this process, it
  /// would wait for it in the queue of the session; of another process, the registry refuses it as `busy`
  Busy {
    op: OperationId,
  },
  /// The last sync didn't build the branch: `problem` the reason of its stack problem, where the sync says it
  SyncFailed {
    problem: Option<String>,
  },
  NoCommits,
  /// The remote branch has every commit of the branch
  UpToDate,
  /// Its open pull request is a draft: nobody is asked to review it
  Draft {
    number: u32,
  },
  /// The robot starts from a comment on the pull request, which runs the default checks: no profile to choose
  CommentRoute {
    number: u32,
  },
  /// Every commit of the branch has an issue reference
  AllReferenced,
  /// A robot lands the branch (`reason`, its hold): it lands whatever a park says
  ParkHeld {
    reason: Reason,
  },
  /// Push with Children has nothing to push beyond the branch itself
  NothingBeyond {
    why: NothingBeyond,
  },
  /// All of it landed: nothing is left to review in a new pull request
  Landed {
    landed: Landed,
  },
  /// The pull request is the user's own: GitHub takes no review of its author
  OwnPullRequest,
  /// The user approved the commit the pull request shows
  Approved {
    commit: String,
  },
  /// The branch Patronus pushes into for the baseline is known after the first sync
  TargetUnread,
  /// The baseline is no branch of origin, which Patronus pushes into
  NoTarget,
  /// The pull request goes into another branch than the one Patronus pushes into: Patronus would merge it there
  OtherBase {
    number: u32,
    base: String,
    target: String,
  },
}

/// Why Push with Children of a branch pushes nothing beyond the branch itself
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum NothingBeyond {
  /// The branches stacked on it whose push drops remote commits: each asks first in its row
  AskFirst { names: Vec<String> },
  /// Only the branch has commits to push: its own push sends them
  OnlyBranch,
  /// Merged Safe Pushes landed the branch and every branch stacked on it whole: `by` each robot once, in the order they
  /// merged, `own` when the robot of the whole stack of the branch landed them all; `target` of the first
  Landed { by: Vec<RobotOf>, own: bool, target: String },
  /// The branches are on the remote; the stack branch of the whole stack of the root stays, and why
  StackBranch { along: StackBranchAlong },
  /// The branches are on the remote
  OnRemote,
}

impl Availability {
  /// Disabled with `reason`, enabled without one
  #[must_use]
  pub fn of(reason: Option<ActionBlocker>) -> Self {
    reason.map_or(Self::Enabled, |reason| Self::Disabled { reason })
  }
}
