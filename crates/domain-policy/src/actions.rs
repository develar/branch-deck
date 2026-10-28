//! The actions a subject offers and whether each can run now, for every surface of the app (spec/branch/branch-row.spec.md,
//! "Branch menu"; spec/branch/whole-stack.spec.md; spec/branch/patronus.spec.md, "Restart and cancel";
//! docs/design/rust-domain-authority.md, s8-policy): `branch_actions`, `stack_actions` and `robot_actions` answer an
//! `ActionState` by stable id for each action the facts offer, disabled with the reason, never hidden for it;
//! `start_actions` those of a start whose answer was lost (spec/branch/patronus.spec.md, "A start whose answer was lost").
//!
//! - The starts take the reasons of the next step of the subject (`NextStep::safe_push`, `dry_run`, `without_review`):
//!   one rule for the slot, the menu, S and the popover. A repository that lands by a push into the target offers its
//!   push instead (`push-to-baseline`, of a branch and of a whole stack), none of the robot's starts; one that lands by a
//!   merge of the pull request on GitHub its merge (`merge-pull-request`) and, while its auto-merge is enabled, Cancel
//!   Auto-Merge (`cancel-auto-merge`).
//! - The push waits for the facts its button reads: parked, blocked, failed, held, a sync that rebuilds the branch,
//!   nothing to push. Push with Children says what it lacks beyond the branch (`NothingBeyond`).
//! - A rewrite needs commits and a build; the fixes of a stack problem run while the problem blocks the rest. A sync
//!   holds no rewrite back: the session admits it at once, named on the table its window showed (`TableWitness`), and
//!   decides it on HEAD as it is then (C14).
//! - A robot's restart waits for why it failed and for the approvals its step misses; a fact without an answer is never
//!   "no": `Pending`.
//!
//! What runs lies over them (`with_operations`, docs/design/operations.md, "Items"), from the operations of the session:
//! the item of an intent that runs or waits in this process is `Running` or `Queued` (a start joins it); a rewrite that
//! can run waits behind the newest operation of this process that holds HEAD (`Waits`: its start queues, never refused);
//! a remote action whose resource an operation holds is `Busy`, behind it in this process, refused by the registry of
//! the repository for another process. A menu item names no commits, no files and no message: of a move, an amend and
//! an added issue reference it only waits, their starts with what they name being operations of their own.
//!
//! Offered by the facts only (a lost start by its record); what the page alone knows decides the presence of a few
//! items (the fixups of the autosquash plan, the label of Ungroup, an orphaned pull request of a whole stack).

use crate::next_step::{Listing, PatronusAnswer};
use domain_types::actions::{ActionBlocker, ActionId, ActionState, Availability, NothingBeyond};
use domain_types::landing::Landed;
use domain_types::landing::Subject;
use domain_types::next_step::{NextStep, StepBlocker, StepFact, StepKind};
use domain_types::operation::{Intent, OperationId, OperationStep, Origin, PullRequestField, PushScope, Resource, RunningOperation};
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::pull_request::{PullRequest, PullRequestState, PullRequestView};
use domain_types::reason::Reason;
use domain_types::repository_view::{RestartAction, RobotRestart};
use domain_types::rewrite::{AutosquashScope, RewriteIntent};
use domain_types::session::{BranchStatus, ChildrenPush, Parked, StackBranchAlong};
use domain_types::stack::{StackFix, StackProblemView};
use domain_types::{PushAction, RemoteStatusUpdate};

/// What the row of a branch says of it, as its actions read it
#[derive(Clone, Copy, Debug)]
pub struct Row<'a> {
  pub name: &'a str,
  /// The sync rebuilds it: its push waits for the build
  pub syncing: bool,
  pub status: BranchStatus,
  /// The parent a blocked branch waits for
  pub blocked_by: Option<&'a str>,
  pub sync_failed: bool,
  /// Its first stack problem, with the fixes that run while it blocks the rest
  pub problem: Option<&'a StackProblemView>,
  pub commit_count: usize,
  pub parked: Option<&'a Parked>,
  /// The parent it declares (D8)
  pub declared_parent: Option<&'a str>,
  pub all_referenced: bool,
}

/// What Push with Children of a branch reads beside its order (`ChildrenPush`)
#[derive(Clone, Debug)]
pub struct Children<'a> {
  pub push: &'a ChildrenPush,
  /// A sync runs and a branch it pushes has no remote status yet: what is left to push is to come
  pub statuses_pending: bool,
  /// Merged Safe Pushes landed the branch and every branch stacked on it whole (`NothingBeyond::Landed`)
  pub landed: Option<NothingBeyond>,
}

/// The facts the actions of a branch are decided on
#[derive(Clone, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "independent facts of the branch, each read by its own actions")]
pub struct BranchFacts<'a> {
  pub row: Row<'a>,
  pub patronus: PatronusAnswer<'a>,
  /// Its next step (`domain_policy::next_step`)
  pub step: &'a NextStep,
  /// What holds its push (`Reason::Held` of a hold whose robot holds, `holds::branch_hold`) and whether a merged robot
  /// landed it (a park and Push with Children wait only for one that runs)
  pub hold: Option<&'a Reason>,
  pub hold_landed: bool,
  pub remote: Option<&'a RemoteStatusUpdate>,
  /// Push with Children; `None` for a branch nothing but parked branches is stacked on
  pub children: Option<Children<'a>>,
  /// Its pull request, any state, and the open one as the rules read it
  pub pull_request: Option<&'a PullRequest>,
  pub open_pull_request: Option<&'a PullRequestView>,
  pub listing: Listing<'a>,
  /// The GitHub repository of the branches is known
  pub repository: bool,
  /// The pull request whose comment starts its Safe Push (`StartRoute::Comment`)
  pub comment_route: Option<u32>,
  pub stacks_enabled: bool,
  /// The repository lands by a push of the copy into the target (`LandingRoute::Push`): its push is the start
  pub push_route: bool,
  /// The repository lands by a merge of the pull request on GitHub (`land::merges_on_github`): its merge is the start
  pub merge_route: bool,
}

/// The actions of a branch, in the order of the enum
#[must_use]
pub fn branch_actions(facts: &BranchFacts<'_>) -> Vec<ActionState> {
  let row = &facts.row;
  let step = facts.step;
  let mut actions = Vec::new();
  let mut offer = |id: ActionId, reason: Option<ActionBlocker>| actions.push(state(id, reason));
  // until Patronus answers its starts are offered, disabled with what they wait for: hidden only off Space, and where
  // the push into the target lands it
  if facts.push_route {
    offer(ActionId::PushToBaseline, step.safe_push.as_ref().map(of_step));
  } else if facts.merge_route {
    offer_merge(&mut offer, step);
  } else if !matches!(facts.patronus, PatronusAnswer::NotOnSpace) {
    offer(ActionId::SafePush, step.safe_push.as_ref().map(of_step));
    let profile = step
      .safe_push
      .as_ref()
      .map(of_step)
      .or_else(|| facts.comment_route.map(|number| ActionBlocker::CommentRoute { number }));
    offer(ActionId::SafePushWithProfile, profile);
    offer(ActionId::DryRun, step.dry_run.as_ref().map(of_step));
    offer(ActionId::CheckApprovals, None);
  }
  if let Some(without_review) = step.without_review.as_ref().filter(|_| !facts.push_route) {
    offer(ActionId::SafePushWithoutReview, without_review.blocker.as_ref().map(of_step));
  }
  offer(ActionId::Push, push_blocker(facts));
  if let Some(children) = &facts.children {
    offer(ActionId::PushWithChildren, children_blocker(facts, children));
  }
  // the remote has commits the branch lacks, a push would drop them: they can come in instead
  if facts.remote.is_some_and(|remote| remote.commits_behind > 0) {
    offer(ActionId::PickRemoteCommits, pick_remote_blocker(facts));
  }
  offer(ActionId::ReviewChanges, review_blocker(row));
  if facts.pull_request.is_some() {
    offer(ActionId::OpenPullRequest, None);
  }
  if let Some(open) = facts.open_pull_request {
    offer(ActionId::RequestReview, open.draft.then_some(ActionBlocker::Draft { number: open.number }));
  }
  if step.retarget.is_some() {
    offer(ActionId::RetargetPullRequest, None);
  }
  let live = facts.pull_request.is_some_and(|pull_request| pull_request.state != PullRequestState::Closed);
  if facts.repository && !live {
    // the pull request of the branch is not known yet: never a second one of a branch that has one
    let reason = if matches!(facts.listing, Listing::Loading) {
      Some(ActionBlocker::Pending { fact: StepFact::PullRequests })
    } else {
      landed_whole(step.landed.as_ref())
    };
    offer(ActionId::CreatePullRequest, reason);
  }
  let rewrite = rewrite_blocker(row);
  // a fix of the stack problem runs while the problem blocks the rest: a regroup keeps a problem it finds
  let fix = |fix: &dyn Fn(&StackFix) -> bool| {
    if row.problem.is_some_and(|problem| problem.fixes.iter().any(fix)) {
      None
    } else {
      rewrite.clone()
    }
  };
  offer(ActionId::Rename, fix(&|fix| *fix == StackFix::Rename));
  offer(ActionId::MoveTo, rewrite.clone());
  if facts.stacks_enabled {
    offer(ActionId::StackOn, fix(&|fix| matches!(fix, StackFix::StackOn(_))));
    if row.declared_parent.is_some() || row.problem.is_some_and(|problem| problem.fixes.contains(&StackFix::Unstack)) {
      offer(ActionId::Unstack, fix(&|fix| *fix == StackFix::Unstack));
    }
  }
  offer(ActionId::Ungroup, rewrite.clone());
  offer(ActionId::AmendChanges, rewrite.clone());
  offer(
    ActionId::AddIssueReference,
    rewrite.clone().or_else(|| row.all_referenced.then_some(ActionBlocker::AllReferenced)),
  );
  offer(ActionId::SquashFixups, rewrite.clone());
  offer(ActionId::Unapply, rewrite);
  match row.parked {
    // parked with its parent: only the parent's unpark
    Some(parked) if parked.with.is_some() => offer(ActionId::Unpark, None),
    Some(_) => {
      offer(ActionId::Park, None);
      offer(ActionId::Unpark, None);
    }
    None => {
      // a robot that lands it lands whatever a park says
      let held = facts.hold.filter(|_| !facts.hold_landed).map(|reason| ActionBlocker::ParkHeld { reason: reason.clone() });
      let no_commits = (row.commit_count == 0).then_some(ActionBlocker::NoCommits);
      offer(ActionId::Park, held.or(no_commits));
    }
  }
  actions
}

fn state(id: ActionId, reason: Option<ActionBlocker>) -> ActionState {
  ActionState {
    id,
    availability: Availability::of(reason),
  }
}

/// A reason of the next step of the subject, which the step words
fn of_step(blocker: &StepBlocker) -> ActionBlocker {
  ActionBlocker::Step { blocker: blocker.clone() }
}

/// What landed all of the subject; `None` while work is left after it
fn landed_whole(landed: Option<&Landed>) -> Option<ActionBlocker> {
  landed.filter(|landed| landed.is_whole()).map(|landed| ActionBlocker::Landed { landed: landed.clone() })
}

/// Why the push of the branch can't run: parked, blocked, failed to sync, no commits, held, rebuilt by a sync, up to
/// date. A sync that only runs is no reason: the push waits for it
fn push_blocker(facts: &BranchFacts<'_>) -> Option<ActionBlocker> {
  let row = &facts.row;
  if let Some(aside) = set_aside(row) {
    return Some(aside);
  }
  // the last sync did not build it: its virtual branch is an older build, which no push takes
  if row.sync_failed {
    return Some(ActionBlocker::SyncFailed {
      problem: row.problem.map(|problem| problem.reason.clone()),
    });
  }
  if row.commit_count == 0 {
    return Some(ActionBlocker::NoCommits);
  }
  if let Some(hold) = facts.hold {
    return Some(ActionBlocker::Rule { reason: hold.clone() });
  }
  if row.syncing {
    return Some(ActionBlocker::Syncing);
  }
  facts.remote.is_some_and(|remote| remote.action == PushAction::UpToDate).then_some(ActionBlocker::UpToDate)
}

/// Why the commits of the remote branch can't come into the branch: parked, blocked, held by a running Safe Push; a sync
/// holds it back no more than another rewrite (the pick names the remote tip the row showed)
fn pick_remote_blocker(facts: &BranchFacts<'_>) -> Option<ActionBlocker> {
  set_aside(&facts.row).or_else(|| facts.hold.map(|hold| ActionBlocker::Rule { reason: hold.clone() }))
}

/// The branch is set aside: parked (or built on a parked branch), or blocked by the branch it is built on
fn set_aside(row: &Row<'_>) -> Option<ActionBlocker> {
  row.parked.map(|parked| parked_reason(row.name, parked)).or_else(|| blocked(row))
}

fn parked_reason(name: &str, parked: &Parked) -> ActionBlocker {
  ActionBlocker::Rule {
    reason: Reason::Parked {
      branch: name.to_string(),
      with: parked.with.clone(),
      note: parked.note.clone(),
    },
  }
}

fn blocked(row: &Row<'_>) -> Option<ActionBlocker> {
  (row.status == BranchStatus::Blocked).then(|| ActionBlocker::Rule {
    reason: Reason::Blocked {
      branch: row.name.to_string(),
      by: row.blocked_by.map(str::to_string),
    },
  })
}

/// Why Push with Children can't run: parked, a running Safe Push holds the branch (a merged one leaves its children to
/// push), the statuses a running sync computes are to come, or nothing beyond the branch is left to push
fn children_blocker(facts: &BranchFacts<'_>, children: &Children<'_>) -> Option<ActionBlocker> {
  let row = &facts.row;
  if let Some(parked) = row.parked {
    return Some(parked_reason(row.name, parked));
  }
  if let Some(hold) = facts.hold.filter(|_| !facts.hold_landed) {
    return Some(ActionBlocker::Rule { reason: hold.clone() });
  }
  let push = children.push;
  if push.offers {
    return None;
  }
  if children.statuses_pending {
    return Some(ActionBlocker::Syncing);
  }
  let why = if !push.ask_first.is_empty() {
    NothingBeyond::AskFirst { names: push.ask_first.clone() }
  } else if push.order.as_ref().is_some_and(|order| order.iter().any(|name| name == row.name)) {
    NothingBeyond::OnlyBranch
  } else if let Some(landed) = &children.landed {
    landed.clone()
  } else {
    match &push.stack_branch {
      Some(along @ (StackBranchAlong::Unpublished | StackBranchAlong::NotBuilt { .. } | StackBranchAlong::Held { .. } | StackBranchAlong::DropsRemote)) => {
        NothingBeyond::StackBranch { along: along.clone() }
      }
      Some(StackBranchAlong::Along | StackBranchAlong::UpToDate) | None => NothingBeyond::OnRemote,
    }
  };
  Some(ActionBlocker::NothingBeyond { why })
}

/// Why the review of the branch can't open: no commits, its build blocked or failed (its virtual branch must be built
/// on its parents)
#[must_use]
pub fn review_blocker(row: &Row<'_>) -> Option<ActionBlocker> {
  if row.commit_count == 0 {
    return Some(ActionBlocker::NoCommits);
  }
  blocked(row).or_else(|| {
    row.sync_failed.then(|| ActionBlocker::SyncFailed {
      problem: row.problem.map(|problem| problem.reason.clone()),
    })
  })
}

/// Why a rewrite of the branch can't run: no commits, it failed to sync (only the fixes of its stack problem run); a sync
/// that rebuilds it is no reason (C14)
fn rewrite_blocker(row: &Row<'_>) -> Option<ActionBlocker> {
  if row.commit_count == 0 {
    return Some(ActionBlocker::NoCommits);
  }
  row.sync_failed.then(|| ActionBlocker::SyncFailed {
    problem: row.problem.map(|problem| problem.reason.clone()),
  })
}

/// What the whole stack of a root is, as its actions read it
#[derive(Clone, Copy, Debug)]
pub enum StackShape<'a> {
  /// The stack forks: its stack branch; `not_built` why the sync did not build it, `tip` whether it has one; `main_line`
  /// the last branch of the main line with its next step, which lands the stack in pieces while the stack branch is not
  /// built
  Forking {
    not_built: Option<&'a Reason>,
    tip: bool,
    main_line: Option<&'a NextStep>,
  },
  /// A straight chain: its last branch from the root, with how many branches it has and why its review can't open
  Chain { length: usize, review: Option<&'a ActionBlocker> },
}

/// The facts the actions of the whole stack of a root are decided on
#[derive(Clone, Debug)]
pub struct StackFacts<'a> {
  pub root: &'a str,
  pub patronus: PatronusAnswer<'a>,
  pub shape: StackShape<'a>,
  /// The next step of its start: of its stack branch (a stack that forks), of its last branch (a chain)
  pub step: Option<&'a NextStep>,
  /// The pull request of its stack branch, any state, and the open one as the rules read it
  pub pull_request: Option<&'a PullRequest>,
  pub open_pull_request: Option<&'a PullRequestView>,
  pub repository: bool,
  /// What landed all of it
  pub landed: Option<&'a Landed>,
  /// The repository lands by a push of the copy into the target (`LandingRoute::Push`): the push of its stack branch is
  /// its start
  pub push_route: bool,
  /// The repository lands by a merge of the pull request on GitHub: the merge of the pull request of its stack branch is
  /// its start
  pub merge_route: bool,
}

/// The merge of the pull request of a subject on the merge route, and Cancel Auto-Merge while its auto-merge is enabled
fn offer_merge(offer: &mut impl FnMut(ActionId, Option<ActionBlocker>), step: &NextStep) {
  if matches!(step.kind, StepKind::AutoMerging { .. }) {
    offer(ActionId::CancelAutoMerge, None);
  } else {
    offer(ActionId::MergePullRequest, step.safe_push.as_ref().map(of_step));
  }
}

/// The actions of the whole stack of a root, in the order of the enum; the robot of the whole stack has its own
/// (`robot_actions`), Push with Children is the root's
#[must_use]
pub fn stack_actions(facts: &StackFacts<'_>) -> Vec<ActionState> {
  let mut actions = Vec::new();
  let mut offer = |id: ActionId, reason: Option<ActionBlocker>| actions.push(state(id, reason));
  let offered = !matches!(facts.patronus, PatronusAnswer::NotOnSpace) && !facts.push_route && !facts.merge_route;
  // the push of a whole stack is of its stack branch: a stack that forks
  if let Some(step) = facts.step.filter(|_| facts.push_route && matches!(facts.shape, StackShape::Forking { .. })) {
    offer(ActionId::PushToBaseline, step.safe_push.as_ref().map(of_step));
  }
  // the whole stack's pull request is the one of its stack branch: a stack that forks
  if let Some(step) = facts.step.filter(|_| facts.merge_route && matches!(facts.shape, StackShape::Forking { .. })) {
    offer_merge(&mut offer, step);
  }
  if let Some(step) = facts.step.filter(|step| offered && step.kind != StepKind::Hidden) {
    offer(ActionId::SafePushWholeStack, step.safe_push.as_ref().map(of_step));
    offer(ActionId::DryRunWholeStack, step.dry_run.as_ref().map(of_step));
  }
  // while the sync didn't build its stack branch, the stack lands in pieces, its main line first
  if let StackShape::Forking {
    not_built: Some(_),
    main_line: Some(leaf),
    ..
  } = facts.shape
    && offered
  {
    offer(ActionId::SafePushMainLine, leaf.safe_push.as_ref().map(of_step));
  }
  let pull_request = facts.pull_request.filter(|pull_request| pull_request.state != PullRequestState::Closed);
  if pull_request.is_some() {
    offer(ActionId::OpenStackPullRequest, None);
  }
  if let Some(open) = facts.open_pull_request {
    offer(ActionId::RequestReview, open.draft.then_some(ActionBlocker::Draft { number: open.number }));
  }
  if let StackShape::Forking { not_built, .. } = facts.shape
    && facts.repository
    && pull_request.is_none()
  {
    let reason = not_built.map(|reason| ActionBlocker::Rule { reason: reason.clone() });
    offer(ActionId::CreateStackPullRequest, reason.or_else(|| landed_whole(facts.landed)));
  }
  match facts.shape {
    StackShape::Forking { not_built, tip, .. } => {
      let reason = (!tip).then(|| ActionBlocker::Rule {
        reason: not_built.cloned().unwrap_or_else(|| not_built_yet(facts.root)),
      });
      offer(ActionId::ReviewWholeStack, reason);
    }
    StackShape::Chain { length, review } if length >= 2 => offer(ActionId::ReviewWholeStack, review.cloned()),
    StackShape::Chain { .. } => {}
  }
  actions
}

/// A stack branch the sync built without a tip yet: "sync first"
fn not_built_yet(root: &str) -> Reason {
  Reason::StackNotBuilt {
    root: root.to_string(),
    state: domain_types::stack::StackBranchState::Built,
    blocked_by: None,
    conflict: None,
    error: None,
  }
}

/// Why Patronus failed the robot, as the session knows it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problems<'a> {
  /// It didn't fail, the problems came, or it is no robot of the table (nobody asks)
  Known,
  /// Still to come
  Pending,
  /// Patronus didn't give them
  Failed(&'a str),
}

/// The facts the actions of a robot are decided on
#[derive(Clone, Debug)]
pub struct RobotFacts<'a> {
  pub robot: &'a PatronusRobot,
  /// The restart it offers (`holds::restart_action`, `land::restart_refusal`)
  pub restart: Option<&'a RobotRestart>,
  /// Its push has a subject to push: the row of its branch, or the stack branch of its whole stack
  pub pushes: bool,
  /// It failed for missing approvals; `None` while Patronus didn't tell
  pub needs_review: Option<bool>,
  /// A required check of the running robot failed
  pub failing: bool,
  pub problems: Problems<'a>,
  /// The next step of the row of its branch: a restart of a robot that failed for the approvals its step still misses
  /// fails again
  pub step: Option<&'a NextStep>,
}

/// The actions of a robot, one list for every surface that shows it, in the order of the enum
#[must_use]
pub fn robot_actions(facts: &RobotFacts<'_>) -> Vec<ActionState> {
  let robot = facts.robot;
  let mut actions = Vec::new();
  let problems = match facts.problems {
    Problems::Known => None,
    Problems::Pending => Some(ActionBlocker::Pending { fact: StepFact::RobotProblems }),
    Problems::Failed(message) => Some(ActionBlocker::ProblemsFailed { message: message.to_string() }),
  };
  let restart_id = match facts.restart.map(|restart| restart.action) {
    Some(RestartAction::Restart) => Some(ActionId::RestartRobot),
    Some(RestartAction::PushAndRestart) if facts.pushes => Some(ActionId::PushAndRestartRobot),
    Some(RestartAction::PushAndRestart) | None => None,
  };
  if let (Some(id), Some(restart)) = (restart_id, facts.restart) {
    let approvals = facts
      .step
      .filter(|step| waits_for_approvals(&step.kind))
      .and_then(|step| step.safe_push.as_ref().map(of_step));
    let refusal = restart.refusal.clone().map(|reason| ActionBlocker::Rule { reason });
    actions.push(state(id, problems.clone().or(approvals).or(refusal)));
  }
  if robot.status == PatronusRobotStatus::Running {
    actions.push(state(ActionId::CancelRobot, None));
  }
  let prompt = if robot.status == PatronusRobotStatus::Failure {
    facts.needs_review != Some(true)
  } else {
    facts.failing && robot.status == PatronusRobotStatus::Running
  };
  if prompt {
    actions.push(state(ActionId::CopyRobotPrompt, problems.clone()));
    actions.push(state(ActionId::CopyRobotFullPrompt, problems));
  }
  actions.push(state(ActionId::OpenRobot, None));
  if robot.pull_request.is_some() {
    actions.push(state(ActionId::OpenRobotPullRequest, None));
  }
  actions
}

/// The actions of a start whose answer was lost (`OperationStep::StartUnknown`), of any process: Open in Patronus where
/// its robot would be (the robots of Patronus, the pull request of its comment), and Forget This Start, which frees what
/// it holds (Patronus may still run it); none for an operation that runs
#[must_use]
pub fn start_actions(operation: &RunningOperation) -> Vec<ActionState> {
  let OperationStep::StartUnknown { url, .. } = &operation.step else {
    return Vec::new();
  };
  let open = url.as_ref().map(|_| state(ActionId::OpenUnknownStart, None));
  open.into_iter().chain([state(ActionId::ForgetStart, None)]).collect()
}

/// The next steps that wait for approvals: a restart of a robot that failed for them would fail again
const fn waits_for_approvals(kind: &StepKind) -> bool {
  matches!(
    kind,
    StepKind::CreatePullRequest { .. }
      | StepKind::ChangesRequested { .. }
      | StepKind::RequestReview { .. }
      | StepKind::Awaiting { .. }
      | StepKind::ChangesRequestedUnjudged { .. }
      | StepKind::AwaitingUnjudged { .. }
  )
}

/// What is known of the branch Patronus pushes into for the baseline, which merges a reviewed pull request
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target<'a> {
  /// Patronus doesn't merge in the repository (or hasn't answered): no Safe Push
  NoPatronus,
  /// Before the first sync
  Unread,
  /// The baseline is no branch of origin
  None,
  Branch(&'a str),
}

/// The facts the actions of a pull request to review are decided on (Your Turn, Reviews, the review window)
#[derive(Clone, Copy, Debug)]
pub struct PullRequestFacts<'a> {
  pub number: u32,
  /// The commit it shows
  pub head: &'a str,
  /// The branch it goes into
  pub base: &'a str,
  /// The user's own
  pub own: bool,
  /// The user added it by its link
  pub linked: bool,
  /// The branch of the table an own pull request is of
  pub branch: Option<&'a str>,
  /// The commit the user approved last: the session's review, else GitHub's latest review of the user
  pub approved: Option<&'a str>,
  pub target: Target<'a>,
}

/// The actions of a pull request to review, one list for its row, its menu and the header of its review window:
/// Review, Approve (done once the user approved its head), the Safe Push Patronus makes of it into the target, Request
/// Changes, Open Pull Request, then Show Branch of an own one of the table and Remove from Reviews of a linked one. GitHub
/// takes no review of the author. The window's own state (the commit it shows, a write that runs) lies over them in the
/// page
#[must_use]
pub fn review_actions(facts: &PullRequestFacts<'_>) -> Vec<ActionState> {
  let own = facts.own.then_some(ActionBlocker::OwnPullRequest);
  let approved = facts
    .approved
    .filter(|commit| *commit == facts.head)
    .map(|commit| ActionBlocker::Approved { commit: commit.to_owned() });
  let mut actions = vec![state(ActionId::Review, None), state(ActionId::Approve, own.clone().or(approved))];
  let safe_push = match facts.target {
    Target::NoPatronus => None,
    Target::Unread => Some(Some(ActionBlocker::TargetUnread)),
    Target::None => Some(Some(ActionBlocker::NoTarget)),
    Target::Branch(target) if target != facts.base => Some(Some(ActionBlocker::OtherBase {
      number: facts.number,
      base: facts.base.to_owned(),
      target: target.to_owned(),
    })),
    Target::Branch(_) => Some(None),
  };
  if let Some(blocker) = safe_push {
    actions.push(state(ActionId::SafePushPullRequest, own.clone().or(blocker)));
  }
  actions.push(state(ActionId::RequestChanges, own));
  actions.push(state(ActionId::OpenPullRequest, None));
  if facts.branch.is_some() {
    actions.push(state(ActionId::ShowBranch, None));
  }
  if facts.linked {
    actions.push(state(ActionId::RemoveFromReviews, None));
  }
  actions
}

/// The subject whose actions the operations of the session lie over (`with_operations`), as the operations name it
#[derive(Clone, Copy, Debug)]
pub enum Of<'a> {
  /// A branch: `pushes` the full name of its virtual branch, which its push writes; `unparks` the branch its Unpark
  /// unparks (the parent of a branch parked with it, else itself); `pull_request` the number of its open pull request
  Branch {
    name: &'a str,
    pushes: Option<&'a str>,
    unparks: &'a str,
    pull_request: Option<u32>,
  },
  /// The whole stack of `root`: `main_line` the last branch of its main line, whose Safe Push lands the stack in pieces;
  /// `pull_request` the number of the open pull request of its stack branch
  WholeStack {
    root: &'a str,
    main_line: Option<&'a str>,
    pull_request: Option<u32>,
  },
  Robot {
    id: &'a str,
  },
  /// A start whose answer was lost, the operation `op`
  Start {
    op: &'a OperationId,
  },
}

/// The actions of the subject `of` with the operations of its session laid over them, `operations` in the order they
/// were admitted (the module's header): its own intent of this process runs (`Running`) or waits in the queue (`Queued`);
/// an action that can run otherwise keeps its availability but a rewrite, which waits behind the newest operation of
/// this process that holds HEAD (`Waits`), and a remote action whose resource an operation holds, or whose intent another
/// process runs (`Busy`); an action disabled for its own reason keeps it
#[must_use]
pub fn with_operations(actions: Vec<ActionState>, of: Of<'_>, operations: &[RunningOperation]) -> Vec<ActionState> {
  if operations.is_empty() {
    return actions;
  }
  actions.into_iter().map(|action| overlay(action, of, operations)).collect()
}

fn overlay(action: ActionState, of: Of<'_>, operations: &[RunningOperation]) -> ActionState {
  let id = action.id;
  let this_process = || operations.iter().filter(|operation| operation.origin == Origin::ThisProcess);
  if let Some(own) = this_process().find(|operation| is_own(id, of, &operation.intent)) {
    let op = own.id.clone();
    let availability = match &own.step {
      OperationStep::Queued { behind } => Availability::Queued { op, behind: behind.clone() },
      _ => Availability::Running { op },
    };
    return ActionState { id, availability };
  }
  if action.availability != Availability::Enabled {
    return action;
  }
  let availability = if is_rewrite(id) {
    this_process()
      .rfind(|operation| operation.resources.contains(&Resource::Head))
      .map(|holder| Availability::Waits { op: holder.id.clone() })
  } else {
    let needs = needs(id, of);
    operations
      .iter()
      .rfind(|operation| (operation.origin != Origin::ThisProcess && is_own(id, of, &operation.intent)) || operation.resources.iter().any(|held| needs.contains(held)))
      .map(|holder| Availability::Disabled {
        reason: ActionBlocker::Busy { op: holder.id.clone() },
      })
  };
  availability.map_or(action, |availability| ActionState { id, availability })
}

/// The actions that rewrite HEAD or the refs of Branch Deck: their starts queue behind each other in the session
const REWRITES: [ActionId; 12] = [
  ActionId::Rename,
  ActionId::MoveTo,
  ActionId::StackOn,
  ActionId::Unstack,
  ActionId::Ungroup,
  ActionId::AmendChanges,
  ActionId::AddIssueReference,
  ActionId::SquashFixups,
  ActionId::Unapply,
  ActionId::Park,
  ActionId::Unpark,
  ActionId::PickRemoteCommits,
];

fn is_rewrite(id: ActionId) -> bool {
  REWRITES.contains(&id)
}

/// What a remote action of `of` holds while it runs, which an operation that holds it first keeps it waiting for: the
/// ref a push writes, the field of the pull request it writes, the robot it restarts or cancels
fn needs(id: ActionId, of: Of<'_>) -> Vec<Resource> {
  let field = |number: Option<u32>, field: PullRequestField| number.map(|number| Resource::PullRequest { number, field }).into_iter().collect();
  match (of, id) {
    (Of::Branch { pushes, .. }, ActionId::Push | ActionId::PushWithChildren) => pushes.map(|name| Resource::Ref { name: name.to_owned() }).into_iter().collect(),
    (Of::Branch { pull_request, .. }, ActionId::RetargetPullRequest) => field(pull_request, PullRequestField::Base),
    (Of::Branch { pull_request, .. } | Of::WholeStack { pull_request, .. }, ActionId::CancelAutoMerge) => field(pull_request, PullRequestField::AutoMerge),
    (Of::Branch { pull_request, .. } | Of::WholeStack { pull_request, .. }, ActionId::RequestReview) => field(pull_request, PullRequestField::Reviewers),
    (Of::Robot { id: robot }, ActionId::RestartRobot | ActionId::PushAndRestartRobot | ActionId::CancelRobot) => vec![Resource::Robot { id: robot.to_owned() }],
    _ => Vec::new(),
  }
}

/// Whether `intent` is what the action `id` of `of` runs: a start of the action joins it
fn is_own(id: ActionId, of: Of<'_>, intent: &Intent) -> bool {
  match of {
    Of::Branch { name, unparks, pull_request, .. } => match (id, intent) {
      // the push into the target is the start of its route: the same intent
      (ActionId::SafePush | ActionId::SafePushWithProfile | ActionId::SafePushWithoutReview | ActionId::PushToBaseline | ActionId::MergePullRequest, Intent::Land(land)) => {
        land.mode != PatronusPushMode::DryRun && is_branch(&land.subject, name)
      }
      (ActionId::DryRun, Intent::Land(land)) => land.mode == PatronusPushMode::DryRun && is_branch(&land.subject, name),
      (ActionId::Push, Intent::Push(push)) => is_branch(&push.subject, name) && matches!(push.scope, PushScope::Alone | PushScope::WithAncestors | PushScope::UpTo { .. }),
      (ActionId::PushWithChildren, Intent::Push(push)) => is_branch(&push.subject, name) && push.scope == PushScope::WithChildren,
      (ActionId::RetargetPullRequest, Intent::Retarget { number }) | (ActionId::CancelAutoMerge, Intent::DisableAutoMerge { number }) => pull_request == Some(*number),
      (ActionId::RequestReview, Intent::RequestReview { subject, .. }) => is_branch(subject, name),
      (ActionId::CreatePullRequest, Intent::CreatePullRequest(create)) => is_branch(&create.subject, name),
      (_, Intent::Rewrite(rewrite)) => is_own_rewrite(id, name, unparks, rewrite),
      _ => false,
    },
    Of::WholeStack { root, main_line, pull_request } => match (id, intent) {
      (ActionId::SafePushWholeStack | ActionId::PushToBaseline | ActionId::MergePullRequest, Intent::Land(land)) => {
        land.mode != PatronusPushMode::DryRun && is_stack(&land.subject, root)
      }
      (ActionId::CancelAutoMerge, Intent::DisableAutoMerge { number }) => pull_request == Some(*number),
      (ActionId::DryRunWholeStack, Intent::Land(land)) => land.mode == PatronusPushMode::DryRun && is_stack(&land.subject, root),
      (ActionId::SafePushMainLine, Intent::Land(land)) => land.mode != PatronusPushMode::DryRun && main_line.is_some_and(|leaf| is_branch(&land.subject, leaf)),
      (ActionId::RequestReview, Intent::RequestReview { subject, .. }) => is_stack(subject, root),
      (ActionId::CreateStackPullRequest, Intent::CreatePullRequest(create)) => is_stack(&create.subject, root),
      _ => false,
    },
    Of::Robot { id: robot } => match (id, intent) {
      (ActionId::RestartRobot | ActionId::PushAndRestartRobot, Intent::RestartRobot { robot_id }) | (ActionId::CancelRobot, Intent::CancelRobot { robot_id }) => robot_id == robot,
      _ => false,
    },
    Of::Start { op } => matches!(intent, Intent::ForgetStart { op: forgotten } if id == ActionId::ForgetStart && forgotten == op),
  }
}

/// Whether `rewrite` is what the rewrite `id` of the branch `name` runs: the item names the branch and nothing else (a
/// move, an amend, an added issue reference name what the item doesn't: their starts are operations of their own)
fn is_own_rewrite(id: ActionId, name: &str, unparks: &str, rewrite: &RewriteIntent) -> bool {
  match (id, rewrite) {
    (ActionId::Rename, RewriteIntent::Rename { branch_name, .. })
    | (ActionId::StackOn, RewriteIntent::Restack { branch_name, .. })
    | (ActionId::Unstack, RewriteIntent::Unstack { branch_name, .. })
    | (ActionId::Ungroup, RewriteIntent::Ungroup { branch_name, .. })
    | (ActionId::Unapply, RewriteIntent::Unapply { branch_name, .. })
    | (ActionId::PickRemoteCommits, RewriteIntent::PickRemote { branch_name, .. })
    | (
      ActionId::SquashFixups,
      RewriteIntent::SquashFixups {
        scope: AutosquashScope::Branch(branch_name),
        ..
      },
    ) => branch_name == name,
    (ActionId::Park, RewriteIntent::Park { branches, .. }) => matches!(branches.as_slice(), [line] if line.branch_name == name),
    (ActionId::Unpark, RewriteIntent::Unpark { branch_names, .. }) => matches!(branch_names.as_slice(), [only] if only == unparks),
    _ => false,
  }
}

fn is_branch(subject: &Subject, name: &str) -> bool {
  matches!(subject, Subject::Branch { name: it } if it == name)
}

fn is_stack(subject: &Subject, root: &str) -> bool {
  matches!(subject, Subject::WholeStack { root: it } if it == root)
}
