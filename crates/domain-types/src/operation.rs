//! The operations of the remote (docs/design/rust-domain-authority.md, "Планирование и исполнение операций"): what
//! the user or an agent asks for (`Intent`), what it would do now (`Plan`, with the `Fingerprint` of the facts it was
//! prepared on), how it ended (`Outcome`, effect by effect), what a caller hears while it runs (`OperationEvent`), and
//! the operations of a session that run or wait (`RunningOperation`, the `operations` part of the view), each with the
//! resources it holds (`Resource`), which `domain_policy::admission` admits a new one by (`Admission`).

use crate::RemoteStatusUpdate;
use crate::error::{ApiError, HolderTool, LockHolderInfo, OperationKind};
use crate::landing::{ApprovalsSkip, Confirmation, FactWait, Land, LandProfiles, PlannedEffect, StartRoute, Subject};
use crate::patronus::{PatronusPushMode, PatronusRobot};
use crate::pull_request::{CreatedPullRequest, PullRequestReviewEvent, RequestedReviewers};
use crate::reason::Reason;
use crate::session::Version;
use crate::youtrack::YouTrackStateChange;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The id of an operation: the ULID text of `sync_utils::operation_id::OperationId`, the `operation_id` of its log
/// lines; a string in TypeScript
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(transparent)]
pub struct OperationId(pub String);

impl fmt::Display for OperationId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

/// What the user or an agent asks of the remote (the contract's `Intent`, the families of `s9-engine-remote`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Intent {
  /// A Safe Push or a Dry Run of a subject: retarget, cancel, push and the start, as its plan orders them
  Land(Land),
  /// A push of a subject and of what its scope takes along
  Push(Push),
  /// A restart of a robot of the user, with the push before it when the branch moved past its commit
  RestartRobot { robot_id: String },
  /// A cancel of a running robot of the user
  CancelRobot { robot_id: String },
  /// The open pull request `number` goes into the base its rule names (`domain_policy::pull_request::base_of`)
  Retarget { number: u32 },
  /// The pull request of a subject is opened, after the push it needs
  CreatePullRequest(CreatePullRequest),
  /// `reviewers` are asked to review the open pull request of a subject, none: the code owners Patronus suggests that
  /// Request Review… preselects (`domain_policy::review::candidates`); `push_first`: the work its branch has for the
  /// remote is pushed first, so the reviewers see it (D5), else such work refuses it (`Reason::NeedsPush`)
  RequestReview { subject: Subject, reviewers: Vec<String>, push_first: bool },
  /// The pull request `number` is closed with `comment`
  ClosePullRequest { number: u32, comment: String },
  /// The open pull requests of the branches a merged robot landed whole besides its own are closed
  /// (`domain_policy::landed::pull_requests_to_close`)
  CloseLandedPullRequests { robot_id: String },
  /// A review of the pull request `number` of someone else on the commit the user reviewed, and the Safe Push of it
  /// after an approval (`then_land`: a `/patronus merge` comment, only while GitHub still shows that commit); no
  /// verdict: the Safe Push alone, the user approved the commit before
  SubmitReview {
    number: u32,
    verdict: Option<PullRequestReviewEvent>,
    commit: String,
    body: Option<String>,
    then_land: bool,
  },
  /// The YouTrack issue `key` goes into `state` (Mark Fixed; its undo the same intent, back to the value it had, while it
  /// is in `expected`); without `expected`, only an open issue changes
  SetIssueState { key: String, state: String, expected: Option<String> },
}

impl Intent {
  /// What it does, by its family: a `Land` a Safe Push or a Dry Run by its mode
  #[must_use]
  pub const fn kind(&self) -> OperationKind {
    match self {
      Self::Land(land) if matches!(land.mode, PatronusPushMode::DryRun) => OperationKind::DryRun,
      Self::Land(_) => OperationKind::SafePush,
      Self::Push(_) => OperationKind::Push,
      Self::RestartRobot { .. } => OperationKind::RestartRobot,
      Self::CancelRobot { .. } => OperationKind::CancelRobot,
      Self::Retarget { .. } => OperationKind::Retarget,
      Self::CreatePullRequest(_) => OperationKind::CreatePullRequest,
      Self::RequestReview { .. } => OperationKind::RequestReview,
      Self::ClosePullRequest { .. } => OperationKind::ClosePullRequest,
      Self::CloseLandedPullRequests { .. } => OperationKind::CloseLandedPullRequests,
      Self::SubmitReview { .. } => OperationKind::SubmitReview,
      Self::SetIssueState { .. } => OperationKind::SetIssueState,
    }
  }

  /// The subject it names, as the view keys it (`Subject::key`: the branch, `stack:<root>` a whole stack); `None` for an
  /// intent of a robot, a pull request or an issue
  #[must_use]
  pub fn subject(&self) -> Option<String> {
    match self {
      Self::Land(Land { subject, .. }) | Self::Push(Push { subject, .. }) | Self::CreatePullRequest(CreatePullRequest { subject, .. }) | Self::RequestReview { subject, .. } => {
        Some(subject.key())
      }
      _ => None,
    }
  }
}

/// A push of a subject: of a branch, the stack branch of a whole stack, an unapplied branch (`PushScope::Unapplied`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Push {
  pub subject: Subject,
  pub scope: PushScope,
  /// The remote tip of the subject the user saw (`RemoteStatusUpdate::remote_tip`): the push replaces the remote branch
  /// only while it is there; `None`: the status showed no remote branch
  pub expect_remote: Option<String>,
  /// It replaces commits of the remote the branch lacks, someone's work (`bd push --force`, the Force Push the user
  /// chose): its plan asks nothing then; without it a push that drops them asks first (`Confirmation::ForcePushDrops`)
  pub force: bool,
  /// The Safe Push of the branch is restarted on the pushed commit, canceled first while it runs (`--restart-running`,
  /// the push of the toast of a remote that moved under a restart)
  pub restart_running: bool,
}

/// What a push takes along with its subject (`domain_policy::push::push_plan` orders it)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PushScope {
  /// The subject alone
  Alone,
  /// The branches it is built on that have work for the remote, root first (Push Stack, the button)
  WithAncestors,
  /// The branches built on it, in one atomic push, parents first (Push with Children)
  WithChildren,
  /// The branch up to `commit` (Push Up to Here)
  UpTo { commit: String },
  /// The unapplied branch of that name, under its own name, so the remote keeps a copy
  Unapplied,
}

/// A pull request to open for a subject: of its virtual branch into the target, of the stack branch of a whole stack
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CreatePullRequest {
  pub subject: Subject,
  pub title: String,
  pub body: String,
  /// The user of the token is its assignee ("Assign to me")
  pub assign_self: bool,
  /// Logins; the author's own is left out, GitHub refuses it
  pub reviewers: Vec<String>,
  /// "Continue on GitHub": the user opens it in GitHub's form, which shows nothing of a branch GitHub doesn't have yet;
  /// the plan pushes what it needs and waits until GitHub has the branch (`PlannedEffect::WaitForMirror`), and the page
  /// opens the form
  #[serde(default, rename = "onGitHub")]
  pub on_github: bool,
}

/// What an operation holds while it runs or waits, which a new one that needs it waits for
/// (`domain_policy::admission`): two resources are one when they are equal
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Resource {
  /// A branch it pushes or moves, by its full name (`{prefix}/virtual/auth`, `{prefix}/stack/air`)
  Ref { name: String },
  /// A branch a robot it starts lands (a `Land` reserves what its plan lands), by name
  Lands { branch: String },
  /// A robot it restarts or cancels
  Robot { id: String },
  /// A field of a pull request it writes: two writes of one pull request wait for each other only on the same field
  PullRequest { number: u32, field: PullRequestField },
  /// A YouTrack issue whose state it sets
  Issue { key: String },
}

/// A field of a pull request an operation writes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PullRequestField {
  Base,
  Reviewers,
  /// Open or closed
  State,
  /// A review of the user
  Review,
}

/// Where an operation runs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Origin {
  /// In the process of the session: a request of it joins or waits for the operation
  ThisProcess,
  /// In another process (its record in the operations registry of the repository, `operations::registry_file`): a
  /// request of it is busy, it can't wait for a process
  OtherProcess { tool: HolderTool, pid: u32 },
}

/// What an operation does now
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OperationStep {
  /// Waits for the operation `behind`, which holds a resource it needs, and is admitted again after it, in the order
  /// the waiting ones came
  Queued {
    behind: OperationId,
  },
  /// Waits for the sync that brings the tip of its subject into the table
  WaitingForSync,
  /// Waits for the repository lock another process holds, or an operation of this process before it in the line of its
  /// process; `holder` as the record beside the lock names it, told again as its time grows (about once a second), `None`
  /// while no record names a live holder
  WaitingForLock {
    holder: Option<LockHolderInfo>,
  },
  /// Reads the facts it needs and checks what it lands (the target, the landing gate)
  Checking,
  /// Moves the pull request of its subject into the base its rule names
  Retargeting,
  /// Cancels the robot its start replaces
  CancelingRobot,
  Pushing,
  /// Starts or restarts a robot
  Starting,
  /// Writes to GitHub or YouTrack: a pull request, its reviewers, a review, its close, the state of an issue
  Writing,
}

/// The move of a row that an operation makes while it runs (docs/design/landing.md): the node of the row leaves the
/// user's moves at once, not when GitHub or Patronus answer. The subject is a branch, or `stack:<root>` a whole stack
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RunningMove {
  /// A robot of `mode` starts: a Safe Push comes to land the row with what it lands beside, a Dry Run only checks it
  Starting { subject: String, mode: PatronusPushMode },
  /// The robot of the row restarts
  Restarting { subject: String, mode: PatronusPushMode },
  /// The pull request of the row is being opened: the reviewers or Patronus answer next
  CreatingPullRequest { subject: String },
  /// A review of the pull request `number` of the row is being requested: the reviewers answer next
  RequestingReview { subject: String, number: u32 },
}

impl RunningMove {
  /// The row whose move it is
  #[must_use]
  pub fn subject(&self) -> &str {
    match self {
      Self::Starting { subject, .. } | Self::Restarting { subject, .. } | Self::CreatingPullRequest { subject } | Self::RequestingReview { subject, .. } => subject,
    }
  }
}

/// An operation of a session that runs or waits (the `operations` part of the view), as `operations::registry` admitted
/// it
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RunningOperation {
  pub id: OperationId,
  pub intent: Intent,
  /// What it holds while it runs or waits
  pub resources: Vec<Resource>,
  pub step: OperationStep,
  pub origin: Origin,
  /// The moves of rows it makes while it runs
  pub moves: Vec<RunningMove>,
}

/// What `domain_policy::admission` answers a new operation of this process over those that run or wait
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
  /// Nothing it needs is held: it runs now
  Run,
  /// The same intent runs in this process: it gets the outcome of `op`, and runs nothing of its own
  Join { op: OperationId },
  /// `op`, the newest operation of this process that holds a resource it needs, runs or waits: it waits behind it
  Queue { op: OperationId },
  /// An operation of another process holds a resource it needs, or runs the same intent
  Busy { holder: OperationId },
}

/// What an intent does now, prepared over the facts of its session (the contract's `Plan`): it runs only while the facts
/// still have its `fingerprint`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Plan {
  pub intent: Intent,
  pub fingerprint: Fingerprint,
  /// What a robot it starts lands, root first; empty when it starts none
  pub lands: Vec<String>,
  /// What it does once nothing refuses it, in order
  pub effects: Vec<PlannedEffect>,
  /// What the user accepts before it runs
  pub confirmations: Vec<Confirmation>,
  /// What refuses it, in order; no confirmation lifts one
  pub blockers: Vec<Reason>,
  /// The facts it waits for, which nobody answered yet (or whose read failed)
  pub waits: Vec<FactWait>,
  /// The merge profiles a start offers besides the one it plans
  pub alternatives: Option<LandProfiles>,
  /// The open pull request a start comments on (`/patronus merge`), or the one `create_pull_request` takes
  pub pull_request: Option<u32>,
  /// Why the approvals don't hold a start back (`domain_policy::review::skips_approvals`): its profile skips the
  /// review, or the user said to start without them
  pub approvals_skipped_by: Option<ApprovalsSkip>,
}

impl Plan {
  /// How the robot of a start starts: through the API with the profile, or from a pull request; none when it starts
  /// no robot (a start that opens the pull request instead, another intent)
  #[must_use]
  pub fn start_route(&self) -> Option<&StartRoute> {
    PlannedEffect::start_route_in(&self.effects)
  }
}

/// What a window asks of an intent before it runs it (`prepare`): its plan in the session of the repository
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PrepareParams {
  pub repository_path: String,
  pub intent: Intent,
}

/// What a window asks to run (`execute`): the intent, the fingerprint of the plan it showed, and the confirmations the
/// user accepted of it; the plan is prepared again and runs only on that fingerprint with every confirmation of it
/// accepted
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ExecuteParams {
  pub repository_path: String,
  pub intent: Intent,
  pub fingerprint: Fingerprint,
  #[serde(default)]
  pub accepted: Vec<Confirmation>,
}

/// What a window asks to stop (`cancel_operation`): an operation of this process that waits for the repository lock, or
/// whose next effect has not started
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CancelOperationParams {
  pub op: OperationId,
}

/// The hash of a plan with what it was prepared on (`operations::witness`): the intent and what it decided, the tips of
/// the refs it moves and the remote branches it leases, the pull requests and robots it reads. Equal only within one
/// process
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(transparent)]
pub struct Fingerprint(pub String);

/// How an operation ended (the contract's `Outcome`): each effect of its plan with its status, in order
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
  pub op: OperationId,
  pub effects: Vec<EffectRecord>,
  /// The event of the object it changed, whose open toast its own replaces in place
  pub news_key: Option<NewsKey>,
  /// The version of the view after the patch of what it changed; `None` when no session of its repository is open
  pub version_after: Option<Version>,
  /// What its effects answered, by its family
  pub result: OperationResult,
  pub stats: OperationStats,
}

/// What an operation answered besides the status of each effect, by its family (the contract's `Outcome.result`)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum OperationResult {
  /// Nothing besides its effects
  #[default]
  None,
  /// A start of a Safe Push or a Dry Run
  Land(Box<LandResult>),
  /// A restart or a cancel of a robot
  Robot(Box<RobotResult>),
  /// The pull request a creation opened, or the open one GitHub has of its head (`created: false`)
  PullRequest(Box<CreatedPullRequest>),
  /// Whom a review request asked, and whom GitHub refused with its reason
  Reviewers(Box<RequestedReviewers>),
  /// The Safe Push of a reviewed pull request of someone else: its comment and the robot it started
  Review(Box<ReviewResult>),
  /// The state field of an issue and the value it had (`Intent::SetIssueState`): its undo sets that value again
  Issue(Box<YouTrackStateChange>),
  /// A push: the remote statuses it went over once more, the Safe Push it restarted on the pushed commit
  Push(Box<PushResult>),
}

/// What a push answered (`Intent::Push`): the toast of the push and its Activity entry name it
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PushResult {
  /// The remote status of each pushed branch, in the order of the push, that the push went over once more: the remote
  /// branch had moved since the status the user saw, without commits the branch lacks (D4); empty when the first push
  /// went
  pub pushed_over: Vec<RemoteStatusUpdate>,
  /// The Safe Push of the branch the push restarted on the pushed commit (`Push::restart_running`), canceled first
  /// while it ran
  pub restarted: Option<PatronusRobot>,
}

/// What the Safe Push of a reviewed pull request answered (`Intent::SubmitReview` with `then_land`)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewResult {
  /// The `/patronus merge` comment it posted
  pub comment: Option<StartComment>,
  /// The robot it started, once the user's robots listed it soon after the reply (Patronus may count it as the
  /// author's: then none, and the pull request tells how it goes)
  pub robot: Option<PatronusRobot>,
}

/// What a restart or a cancel of a robot answered: the toast of the robot and the Activity entry of the page name it
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotResult {
  /// The robot as the operation left it: the new one a restart started, the one a cancel canceled (canceling), or the
  /// one that had finished before the cancel; none when a restart did not get to the restart
  pub robot: Option<PatronusRobot>,
  /// The running robot a restart that pushes first canceled before its push
  pub canceled: Option<PatronusRobot>,
  /// The cancel found the robot finished: nothing was canceled, and `robot` is as it ended
  pub already_finished: bool,
}

/// What a start of a Safe Push or a Dry Run answered: the toast and the Activity entry of the page name it
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandResult {
  /// The robot it started through the API, or the one its comment started once Patronus listed it
  pub robot: Option<PatronusRobot>,
  /// The running Safe Push it canceled, as Patronus answered (or as listed when it had finished meanwhile)
  pub canceled: Option<PatronusRobot>,
  /// The `/patronus` comment it posted on the pull request of the subject
  pub comment: Option<StartComment>,
  /// What the checks before the effects could not do in full
  pub notes: Vec<StartNote>,
}

/// The `/patronus` comment of a start
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StartComment {
  /// The pull request it is on
  pub number: u32,
  pub url: String,
  /// The bot accepted it; false: it did not react within the wait, see the pull request
  pub accepted: bool,
}

/// What a check before the effects of a start could not do in full: the start went on
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StartNote {
  /// The fetch of `target` ("origin/master") failed: the check replayed what lands onto the target fetched last
  TargetFetchFailed { target: String, error: String },
  /// Whether Patronus can rebase what lands onto `target` was not checked
  TargetNotChecked { target: String, error: String },
  /// Whether the whole stack uses a name only a branch outside it adds was not checked
  LandingNotChecked { error: String },
}

/// An effect of a plan and how it went
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct EffectRecord {
  pub effect: PlannedEffect,
  pub status: EffectStatus,
}

/// How an effect went. The first one that fails stops the operation: the effects after it are skipped, and none of the
/// effects before it is undone
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EffectStatus {
  Done,
  Failed {
    error: ApiError,
  },
  /// The answer was lost after the request went out: whether it happened is not known
  Unknown,
  /// Not made: an effect before it failed (`reason` `None`), or the facts changed before it (`plan_changed`)
  Skipped {
    reason: Option<ApiError>,
  },
}

/// What an operation cost
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct OperationStats {
  /// The git processes it started, by subcommand
  pub git_processes: BTreeMap<String, u32>,
  pub http_requests: u32,
  pub millis: u32,
}

/// What the caller of an operation hears while it runs
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OperationEvent {
  /// It went on to `step`
  Step { op: OperationId, step: OperationStep },
  /// Its push sent `percent` of what it sends; `line` is what git said last
  Progress { op: OperationId, percent: u32, line: String },
  /// It ended, or the operation it joined did
  Done { outcome: Box<Outcome> },
}

/// The object and the event an outcome or a news tells, one toast each (`toastKeys` of `src/lib/event-toast.ts`, which
/// adds the repository)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum NewsKey {
  /// A robot: started, restarted, canceling, failed, merged
  Robot { robot_id: String },
  /// The robots of a subject in one push mode: a restart or a new start replaces the toast of the older one
  RobotSubject { subject: String, mode: PatronusPushMode },
  /// A push of a branch
  Push { branch: String },
  /// The approvals of a subject that waited for them
  Approval { subject: String },
  /// A pull request of someone else: the request of the user's review, the review, the Safe Push of it
  Review { number: u32 },
  /// A YouTrack issue
  Issue { key: String },
}

#[cfg(test)]
#[path = "operation_test.rs"]
mod operation_test;
