//! The next step of a subject and the node of its row, as Rust decides them for the app and `bd`
//! (spec/branch/next-step.spec.md, "The table"; spec/branch/branch-row.spec.md, "Tree"): `domain_policy::next_step`
//! answers `NextStep`, the first row of the table that matches, `domain_policy::landing_node` the `LandingNode`. Both
//! carry the facts their words need, never the words: the page words them (`src/presenters/`), `bd` in its own.

use crate::error::ApiError;
use crate::landing::{Landed, LandingRefusal};
use crate::patronus::MissingReviewGroup;
use crate::pull_request::PullRequestAvailability;
use crate::reason::Reason;
use crate::repository_view::RobotOf;
use crate::session::{BranchStatus, Parked};
use serde::Serialize;

/// A fact of the remote the next step decides on, in the order the step waits for them
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum StepFact {
  /// Whether Patronus lands the repository, its robots and the view of what they hold and land
  Patronus,
  /// The pull requests GitHub lists
  PullRequests,
  /// What the Safe Push of the branch needs of a branch outside what it lands (the landing needs of the view)
  LandingNeeds,
  /// Why the robot of the subject failed: for the approvals or not
  RobotProblems,
  /// The review readiness of the subject as it is
  Readiness,
}

/// Why a read the next step decides on failed: no start is decided on it
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StepFailure {
  /// The Safe Push CLI is not logged in to Patronus
  NotLoggedIn,
  /// Patronus fails, and it never listed the robots of the repository
  PatronusError { message: String },
  /// The session couldn't derive what the robots hold and land, and no earlier view stands
  ViewFailed { reason: ApiError },
  /// Patronus didn't give why the robot of the subject failed (`message`)
  ProblemsFailed { message: String },
}

impl StepFailure {
  /// The fact whose read failed
  #[must_use]
  pub const fn fact(&self) -> StepFact {
    match self {
      Self::NotLoggedIn | Self::PatronusError { .. } | Self::ViewFailed { .. } => StepFact::Patronus,
      Self::ProblemsFailed { .. } => StepFact::RobotProblems,
    }
  }
}

/// What helps after a read failed: the step of row 0 instead of a start
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum StepHelp {
  /// The login of the Safe Push CLI, for a terminal
  CopyLoginCommand,
  /// Ask Patronus again
  RefreshPatronus,
}

/// The row of the table that decided the step (spec/branch/next-step.spec.md, "The table"); `NoSlot` a repository that
/// is not on Space, which has no slot
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum StepRow {
  #[serde(rename = "noSlot")]
  NoSlot,
  #[serde(rename = "0")]
  Zero,
  #[serde(rename = "1")]
  One,
  #[serde(rename = "1′")]
  OnePrime,
  #[serde(rename = "2")]
  Two,
  #[serde(rename = "3")]
  Three,
  #[serde(rename = "5")]
  Five,
  #[serde(rename = "5′")]
  FivePrime,
  #[serde(rename = "6")]
  Six,
  #[serde(rename = "7")]
  Seven,
  #[serde(rename = "8")]
  Eight,
  #[serde(rename = "9")]
  Nine,
  #[serde(rename = "10")]
  Ten,
  #[serde(rename = "8′")]
  EightPrime,
  #[serde(rename = "10′")]
  TenPrime,
  #[serde(rename = "11")]
  Eleven,
}

impl StepRow {
  /// The name of the row in the table: "0", "1′", "11"; "no slot" for `NoSlot`
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::NoSlot => "no slot",
      Self::Zero => "0",
      Self::One => "1",
      Self::OnePrime => "1′",
      Self::Two => "2",
      Self::Three => "3",
      Self::Five => "5",
      Self::FivePrime => "5′",
      Self::Six => "6",
      Self::Seven => "7",
      Self::Eight => "8",
      Self::Nine => "9",
      Self::Ten => "10",
      Self::EightPrime => "8′",
      Self::TenPrime => "10′",
      Self::Eleven => "11",
    }
  }
}

/// The open pull request of the whole stack a branch is in, whose approvals count for the Safe Push of the whole stack
/// only (row 7, "In #12")
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackPullRequest {
  pub number: u32,
  pub root: String,
}

/// What the step is, with the facts its words need (the table's rows)
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StepKind {
  /// The repository is not on Space: no slot, the reasons of the subject stay
  Hidden,
  /// Row 1: nothing to start now, the Status cell says why (a sync, a failure, a block, a hold, parked, landed, a
  /// robot that runs or merged)
  Settled,
  /// Row 0: `fact` has no answer yet: "Checking…", never a step a later answer turns into another
  Pending { fact: StepFact },
  /// Row 0: the read of a fact failed: what helps (`help`), else the start (the restart of the robot `robot_id`)
  /// disabled with the failure
  Unavailable {
    failure: StepFailure,
    help: Option<StepHelp>,
    robot_id: Option<String>,
  },
  /// Row 1′: the build of what the Safe Push lands needs a commit of a branch outside it, as the landing gate refuses
  /// the start
  NeedsBranch { refusals: Vec<LandingRefusal> },
  /// Rows 2 and 3: restart the robot `robot_id` (`RepositoryView.patronus.view.restarts` says how)
  Restart { robot_id: String },
  /// Row 3 while nobody can tell whether the approvals count: the pull request collects them, a restart may fail
  /// again; `number` the open one, `None` a pull request to create
  RobotPullRequest { number: Option<u32> },
  /// Row 5: the open pull request goes into the virtual branch `base`: retarget it into the target
  Retarget { number: u32, base: String },
  /// Row 5′: the open pull request goes into another base, which may be meant: it opens
  OtherBase { number: u32, base: String },
  /// Row 6: an open draft, which Patronus merges once it is ready for review
  Draft { number: u32 },
  /// Row 7: the approvals of `groups` are missing and no pull request is open; `stack` the open pull request of the
  /// whole stack the branch is in ("In #12"); `repository` whether the GitHub repository of the branches is known (a
  /// pull request can be created)
  CreatePullRequest {
    groups: Vec<MissingReviewGroup>,
    stack: Option<StackPullRequest>,
    repository: bool,
  },
  /// Row 8: a member of a missing group requested changes on `number`
  ChangesRequested { number: u32, groups: Vec<MissingReviewGroup> },
  /// Row 9: missing groups ask fewer people than the approvals they need (`groups`, the ones short of requests, or all
  /// of them); `no_eligible`: nobody of a group can approve, no request helps
  RequestReview { number: u32, groups: Vec<MissingReviewGroup>, no_eligible: bool },
  /// Row 10: someone of every missing group is asked: the wait (`groups` with whom the pull request asks)
  Awaiting { number: u32, groups: Vec<MissingReviewGroup> },
  /// Row 8′: Patronus can't judge the approvals, and `by` requested changes on `number` (bots left out)
  ChangesRequestedUnjudged { number: u32, by: Vec<String> },
  /// Row 10′: Patronus can't judge the approvals, and `number` waits for `reviewers` (bots left out)
  AwaitingUnjudged { number: u32, reviewers: Vec<String> },
  /// Row 11: "Safe Push" (also row 2 where Patronus can't restart, row 3 approved outside the robot's pull request)
  SafePush,
}

/// Why "Safe Push" (or a Dry Run) can't open now, the same on every surface
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StepBlocker {
  /// A reason of the rules: the robots (`landing.start` of the view), what landed it, its build blocked, parked
  Rule {
    reason: Reason,
  },
  /// The sync runs
  Syncing,
  /// The branch failed to sync: `problem` the reason of its stack problem, where the sync says it
  SyncFailed {
    problem: Option<String>,
  },
  NoCommits,
  /// The step's own reason (row 0's fact, 5′'s base, 6's draft, 7 to 10's approvals): its words are the step's
  Step,
  /// The step stands on a stale answer of the readiness: its action waits for the new one
  StaleReadiness,
}

/// The retarget "⋯" offers of an open pull request into another base than the target
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StepRetarget {
  pub number: u32,
  pub base: String,
  pub target: String,
}

/// The next step of a subject (spec/branch/next-step.spec.md, "The table"), the first row that matches
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NextStep {
  pub row: StepRow,
  pub kind: StepKind,
  /// Why "Safe Push" can't open now; `None` when it can
  pub safe_push: Option<StepBlocker>,
  /// Why a Dry Run can't start: never the approvals, a draft, another base or a robot that lands an ancestor
  pub dry_run: Option<StepBlocker>,
  /// The step depends on the readiness of the subject: the session asks Patronus for it
  pub needs_readiness: bool,
  /// The step stands on a stale answer of the readiness: it stays while the new one loads, its action waits for it
  pub stale_readiness: bool,
  /// What landed all of the subject (row 1): what disables its starts and its Create Pull Request…; `None` while work
  /// is left
  pub landed: Option<Landed>,
  /// The retarget of an open pull request into another base, for "⋯"
  pub retarget: Option<StepRetarget>,
  /// Rows 7 and 11 while GitHub doesn't list the pull requests: a Safe Push without review is offered beside, with why
  /// it can't start now (the reasons of the subject, never the approvals)
  pub without_review: Option<WithoutReview>,
}

/// The Safe Push without review a step offers beside it
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WithoutReview {
  pub blocker: Option<StepBlocker>,
}

/// Where a subject is on its way into master, the node of its row (docs/design/landing.md): filled while the next step
/// is the user's and on this row, hollow while it waits for others, is done, or stays local
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum LandingNodeKind {
  /// Patronus counts the approvals, a Safe Push lands it
  Ready,
  /// The user's step first: a pull request, a review, a retarget, a branch its build needs, a Safe Push whose
  /// approvals Patronus checks at the start
  YourStep,
  /// A conflict, a failed robot, changes requested
  NeedsFix,
  Landing,
  /// Waits for others: reviewers, its parent, a robot, an answer
  Waiting,
  /// Kept local, never pushed
  Parked,
  InMaster,
}

/// Why the node is what it is, with the facts its words need
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum NodeLabel {
  /// It stays local (`parked`), `conflict`: it doesn't build either
  Parked {
    parked: Parked,
    conflict: bool,
  },
  /// The sync failed (`status`), `problem` the reason of its stack problem
  SyncFailed {
    status: BranchStatus,
    problem: Option<String>,
  },
  /// Nothing of it is left to land
  Landed {
    landed: Landed,
  },
  NoCommits,
  /// Its parent `by` doesn't build
  Blocked {
    by: Option<String>,
  },
  /// Its own robot runs: a Dry Run checks it, a Safe Push lands it (`failing`: a required check failed)
  RobotRunning {
    dry_run: bool,
    failing: bool,
  },
  /// A running Safe Push of a branch built on it, or of the whole stack, lands it
  LandsWith {
    by: RobotOf,
  },
  /// A fact has no answer yet
  Waits {
    fact: StepFact,
  },
  /// A read of Patronus failed
  Unavailable {
    failure: StepFailure,
  },
  /// A running Safe Push holds it
  Held,
  /// Its robot failed on it as it is; `problems` why Patronus didn't tell why
  RobotFailed {
    problems: Option<String>,
  },
  /// A start can't now (`reason`): it lands with a robot that lands a part of it, later
  StartWaits {
    reason: Reason,
  },
  /// Patronus counts the approvals: a Safe Push of `lands` branches lands it
  Ready {
    lands: u32,
  },
  /// A Safe Push of `lands` branches, whose approvals Patronus checks when the robot starts; `unlisted` why GitHub
  /// lists no pull request
  AtStart {
    lands: u32,
    unlisted: Option<PullRequestAvailability>,
  },
  /// The reason of the step of the node (the step with no start refused by the robots): a pull request, a review, a
  /// retarget, a draft, a wait
  Step {
    step: Box<NextStep>,
  },
  /// Off Space: not on the remote yet, push it
  PushIt,
  /// Off Space: pushed, it lands through its pull request (GitHub doesn't list them)
  Pushed,
  /// Off Space: no open pull request
  NoPullRequest,
  /// Off Space: the pull request `number` is a draft
  Draft {
    number: u32,
  },
  /// Off Space: `by` requested changes on `number`
  ChangesRequestedBy {
    number: u32,
    by: Vec<String>,
  },
  /// Off Space: GitHub's decision on `number` requests changes
  ChangesRequested {
    number: u32,
  },
  /// Off Space: the checks of `number` fail
  ChecksFail {
    number: u32,
  },
  /// Off Space: `number` waits for `reviewers`
  WaitsFor {
    number: u32,
    reviewers: Vec<String>,
  },
  /// Off Space: it lands through `number`
  LandsThrough {
    number: u32,
  },
}

/// The node of a subject
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandingNode {
  pub kind: LandingNodeKind,
  pub label: NodeLabel,
  /// It waits for the first answer of a fact: the header counts no move of it yet
  pub pending: bool,
  /// The label tells the state of the robot of the subject: its hover shows the robot with its actions
  pub robot: bool,
  /// The branch the label names, which the hover offers to show (the parent a blocked branch waits for, the branch whose
  /// robot lands a part of it)
  pub shows: Option<String>,
  /// A running move of the row (a start, a review request) shows in place of this node, the state after the move; a
  /// node of the subject itself (parked, a failure, landed, blocked, its robot running) stays
  pub move_shows: bool,
}

/// The next step and the node of a subject, the part of the view of a session (`RepositoryView.next_steps`)
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NextStepView {
  pub step: NextStep,
  pub node: LandingNode,
}
