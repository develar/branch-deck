//! Why an action of the domain is refused, or what the user accepts before it (a gate of a route into master,
//! `domain_policy::landing_route`), as a code with the facts its words need, never the words
//! (docs/design/rust-domain-authority.md, s8-policy): `domain_policy` decides it (`holds`, `land`: a start's blockers too,
//! `land::decide`), the page words it
//! (`src/presenters/reasons.ts`), `bd` and the engine in their own words (`repository_model::refusal_words`,
//! `repository_model::push_guard`). `crates/domain-policy/reasons.json` lists every code with an example, which
//! the formatters of both sides are tested on.

use crate::landing::LandingRefusal;
use crate::patronus::PatronusPushMode;
use crate::repository_view::{Carried, HoldView, RobotOf};
use crate::stack::{BlockReason, StackBranchConflict, StackBranchState};
use serde::{Deserialize, Serialize};

/// Why a start, a push, a restart or a review request is refused. A robot code names the push mode and the target of its robot, so a
/// formatter needs nothing else; the words of a context (a hold says "nothing of it is left to push") belong to the
/// field the reason is read from
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Reason {
  /// The sync left the branch blocked: its build lacks the state of `by`, the branch it is built on (none known)
  Blocked { branch: String, by: Option<String> },
  /// A robot runs on the commit the subject is at, or on its remote branch that has the same commits
  LastCommit { robot_id: String, mode: PatronusPushMode },
  /// A running robot of a branch built on `branch`, or of its whole stack, lands it with more: `carried` the commits of
  /// it the robot's commit has, as its content told
  Descendant {
    branch: String,
    by: RobotOf,
    mode: PatronusPushMode,
    carried: Option<Carried>,
  },
  /// A running robot lands a part of what the start would land: an ancestor, a side branch built on one; for the whole
  /// stack a branch of it
  Ancestor { landing: RobotOf, mode: PatronusPushMode },
  /// A merged Safe Push landed all of the branch: `own` its own robot
  Landed { branch: String, by: RobotOf, own: bool, target: String },
  /// The fetched baseline (`upstream`) has every commit of the subject, until the update archives it
  LandedIn { upstream: String },
  /// GitHub merged the pull request `number` of the subject with the commits it has (D13)
  PullRequestMerged { number: u32 },
  /// A running Safe Push holds the push of the branch (`hold`), which the start would make
  Held {
    branch: String,
    hold: HoldView,
    mode: PatronusPushMode,
    target: String,
  },
  /// The stack of `root` doesn't fork: it has no stack branch
  NoStackBranch { root: String },
  /// The stack branch of `root` is no build of its commits: `state` as the sync left it (a `built` one without a tip is
  /// not built yet), the member that blocked it, the commit that conflicts, the error of a stale one
  StackNotBuilt {
    root: String,
    state: StackBranchState,
    blocked_by: Option<String>,
    conflict: Option<StackBranchConflict>,
    error: Option<String>,
  },
  /// The stack branch of `root` holds the commits of `member`, which is parked: it stays local
  StackParked { root: String, member: String },
  /// Merged Safe Pushes landed every branch of the stack of `root` whole: `by` each robot once, in the order they merged,
  /// `own` when the robot of the whole stack landed them all; `target` of the first
  StackLanded { root: String, by: Vec<RobotOf>, own: bool, target: String },
  /// The robot of the whole stack of `root` runs on its stack branch: a push of it fails the robot at the end
  StackHeld { root: String, robot_id: String, mode: PatronusPushMode },
  /// The last sync did not build `branch` (`block` says why): its virtual branch is an older build
  CantPush { branch: String, block: BlockReason },
  /// The push of `branch` drops `behind` commits of its remote branch it lacks, someone's work: its row asks first, and a
  /// start pushes them away only when the user says so (`LandingAllowances::allow_force`, D12)
  DropsRemote { branch: String, behind: u32 },
  /// The remote has an older version of `branch` (it was rebased, amended or reworded since its push): a push of a part
  /// of it would rewind its pull request, so it pushes whole (Push Up to Here)
  RewrittenSincePush { branch: String },
  /// `branch` stays local: its own line parks it (`with` none), or it is built on the parked `with` and carries its
  /// commits; `note` the note of that line, empty without one
  Parked { branch: String, with: Option<String>, note: String },
  /// `branch` (a branch, or the root of a whole stack) has work its remote branch lacks: a review request pushes it
  /// first only when asked to (`bd request-review --push`), so the reviewers see the commits (D5)
  NeedsPush { branch: String },
  /// Patronus names no code owner with a GitHub login to ask for the missing approvals of `groups`: the reviewers are
  /// named instead (`bd request-review --reviewer`)
  NoReviewer { groups: Vec<String> },
  /// Patronus doesn't protect `target`: no robot pushes into it
  Unprotected { target: String },
  /// The pull request `number` a robot starts from is a draft: Patronus merges it once it is ready for review
  Draft { number: u32 },
  /// The open pull request `number` of the subject goes into `base`, and Patronus merges a pull request into its base: its
  /// approvals count for no Safe Push into `target` (D3b); a retarget, the user's, fixes it
  OtherBase { number: u32, base: String, target: String },
  /// The code owners haven't approved: a Safe Push fails without the approvals of `groups`
  ApprovalsMissing { groups: Vec<String> },
  /// The push of new commits dismisses the approvals of pull request `number` that Patronus counts (the rules of
  /// `target` dismiss stale reviews on push): the robot would start without them and fail
  DismissesApprovals { number: u32, target: String },
  /// What lands would not build, as the landing gate proves it: a dangling reference, a missing declaration (boxed: the
  /// largest of the codes by far)
  LandingRefused { refusal: Box<LandingRefusal> },
  /// Patronus rebases what lands onto the newest target first, and `commit` (a copy of the virtual branch) conflicts with
  /// it in `files`: the robot would fail at once
  TargetConflict { commit: String, files: Vec<String> },
  /// The repository lands by Patronus (`branchdeck.landing`), and `origin` is not on Space: no robot lands it
  NotOnSpace,
  /// The token may not push into `repository` (`viewerPermission` below write): neither a push into `target` nor a merge
  /// of a pull request lands from it
  NoWriteAccess { repository: String, target: String },
  /// The rules of `target` take a change only through a pull request: a push into it is refused, unless the user may
  /// bypass the rule (a maintainer or an admin, whom a ruleset may let)
  BaselineRequiresPullRequest { target: String },
  /// The rules of `target` take a change only through the merge queue
  BaselineRequiresMergeQueue { target: String },
  /// The rules of `target` let only the users a ruleset lets bypass it update it
  BaselineRestrictsUpdates { target: String },
  /// The rules of `target` take only signed commits, and the copies Branch Deck writes are unsigned
  BaselineRequiresSignatures { target: String },
  /// The rules of `target` require status checks to pass, which a push of the copy lands without
  BaselineRequiresStatusChecks { target: String },
  /// The baseline `baseline` is a local branch: no remote branch takes a push of what lands
  LocalBaseline { baseline: String },
  /// The pull requests are in `repository`, which `origin` doesn't push to (a mirror): a merge there lands nothing
  /// into the target of `origin`
  GitHubMirror { repository: String },
  /// The pull requests of the branches are not on GitHub: none is merged there
  NotOnGitHub,
  /// The repository lands by a push into its target: a Dry Run is a robot of Patronus, which no push runs
  DryRunNeedsPatronus,
  /// The server refused a push into `target` before in this session (a hook, a protected branch), with `message`, what
  /// it said: a push of the copy may be refused again, so the gate of the route asks first
  BaselineRefusedPush { target: String, message: String },
  /// The fetched baseline `upstream` (`origin/master`) has commits the fork point of the checked out branch lacks, which
  /// the copies are built on: a push of one is no fast-forward, so the update comes first and builds it anew on them
  BaselineAhead { upstream: String },
}

impl Reason {
  /// The robot the reason waits for, by id: the one on the last commit, the one that lands the subject or a part of
  /// it, the one that holds its push
  #[must_use]
  pub fn robot_id(&self) -> Option<&str> {
    match self {
      Self::LastCommit { robot_id, .. } | Self::StackHeld { robot_id, .. } => Some(robot_id),
      Self::Descendant { by, .. } | Self::Landed { by, .. } => Some(&by.robot_id),
      Self::Ancestor { landing, .. } => Some(&landing.robot_id),
      Self::Held { hold, .. } => Some(&hold.by.robot_id),
      Self::Blocked { .. }
      | Self::NoStackBranch { .. }
      | Self::StackNotBuilt { .. }
      | Self::StackParked { .. }
      | Self::StackLanded { .. }
      | Self::LandedIn { .. }
      | Self::PullRequestMerged { .. }
      | Self::CantPush { .. }
      | Self::DropsRemote { .. }
      | Self::RewrittenSincePush { .. }
      | Self::Parked { .. }
      | Self::NeedsPush { .. }
      | Self::NoReviewer { .. }
      | Self::Unprotected { .. }
      | Self::Draft { .. }
      | Self::OtherBase { .. }
      | Self::ApprovalsMissing { .. }
      | Self::DismissesApprovals { .. }
      | Self::LandingRefused { .. }
      | Self::TargetConflict { .. }
      | Self::NotOnSpace
      | Self::NoWriteAccess { .. }
      | Self::BaselineRequiresPullRequest { .. }
      | Self::BaselineRequiresMergeQueue { .. }
      | Self::BaselineRestrictsUpdates { .. }
      | Self::BaselineRequiresSignatures { .. }
      | Self::BaselineRequiresStatusChecks { .. }
      | Self::LocalBaseline { .. }
      | Self::GitHubMirror { .. }
      | Self::NotOnGitHub
      | Self::DryRunNeedsPatronus
      | Self::BaselineRefusedPush { .. }
      | Self::BaselineAhead { .. } => None,
    }
  }
}
