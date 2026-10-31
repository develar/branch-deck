//! Where a branch or a whole stack is on its way into master, the node of its row (spec/branch/branch-row.spec.md,
//! "Tree"; spec/branch/next-step.spec.md, "The surfaces"; docs/design/landing.md), for the app and `bd` alike: the first
//! state that matches over the facts of its next step (`next_step::StepFacts`) with what the step doesn't tell apart.
//! A sync of the row passes, so the node keeps the state it started from; a parked branch, all of it in master, a
//! failure, a block and its own robot running are the subject's own (`own_facts`, which the Status cell picks from too,
//! `row_status`), in that order: a failure stays the user's fix while a robot lands it (the conflict came after the
//! push, and the robot lands the remote copy), the ring of the node says the robot runs; a block yields to a Safe Push
//! that will land it (its own or one that lands it with more, neither failing nor canceling), whose move it is. The rest
//! the next step decides, as the slot shows it, with no
//! start refused by the robots (a robot that lands an ancestor is said apart). The page lays a running move of the row
//! (a start, a review request: the `moves` of an operation of the view) over the node: it shows in place of a node that
//! says so (`move_shows`). A repository that lands by a push into the target is ready once its push can land it
//! (`push_ready`), the refusal of the gate of the route a wait; one that lands by a merge of the pull request on GitHub
//! once GitHub can merge it (`merge_ready`), landing while its auto-merge is enabled, and what holds the merge back
//! (`merge_held`) a fix, the user's step or a wait.

use crate::land::merges_on_github;
use crate::next_step::{Chip, Listing, Needs, PatronusAnswer, Slot, StepFacts, changes_requested_by, next_step, problems_failed, problems_pending};
use crate::own_facts::{OwnFacts, own_facts};
use crate::readiness::waiting_reviewers;
use domain_types::landing_route::LandingRoute;
use domain_types::next_step::{LandingNode, LandingNodeKind, NextStep, NodeLabel, StepBlocker, StepFact, StepFailure, StepKind};
use domain_types::patronus::ReviewReadiness;
use domain_types::pull_request::{ChecksState, PullRequestState, ReviewDecision};
use domain_types::reason::Reason;
use domain_types::repository_view::RobotOf;
use domain_types::session::{BranchStatus, FactState};

/// What decides the node beside the facts of the next step, which don't tell these apart
#[derive(Clone, Copy, Debug)]
pub struct NodeFacts<'a> {
  pub step: StepFacts<'a>,
  /// The status of its row; `Unchanged` for a whole stack
  pub status: BranchStatus,
  /// The parent of a blocked branch
  pub blocked_by: Option<&'a str>,
  /// A running Safe Push (not a Dry Run, not canceling) of a branch built on it, or of the whole stack, lands it
  pub lands_with: Option<&'a RobotOf>,
  /// That Safe Push will fail: a required check failed
  pub lands_failing: bool,
  /// The remote lacks work of it: the step a repository without Patronus and GitHub shows. The page lays a push of it
  /// that runs over the node: it shows the pushed branch from the click on (`presentLandingNode`)
  pub push_pending: bool,
}

/// The node is filled: the next step is the user's, a fix first (`NeedsFix`, `YourStep`, `Ready`); the others wait for
/// others, land, are parked or in master
#[must_use]
pub const fn is_users_move(kind: LandingNodeKind) -> bool {
  matches!(kind, LandingNodeKind::NeedsFix | LandingNodeKind::YourStep | LandingNodeKind::Ready)
}

const fn node(kind: LandingNodeKind, label: NodeLabel) -> LandingNode {
  LandingNode {
    kind,
    label,
    pending: false,
    robot: false,
    shows: None,
    move_shows: true,
    ring: false,
  }
}

/// A node of the subject itself, which a running move of the row doesn't replace
fn own_node(kind: LandingNodeKind, label: NodeLabel) -> LandingNode {
  LandingNode {
    move_shows: false,
    ..node(kind, label)
  }
}

/// A state of the robot of the subject itself, which the hover shows with its actions
fn robot(kind: LandingNodeKind, label: NodeLabel, move_shows: bool) -> LandingNode {
  LandingNode {
    robot: true,
    move_shows,
    ..node(kind, label)
  }
}

/// Nobody can tell yet where it is: a fact has no answer
fn waits(label: NodeLabel) -> LandingNode {
  LandingNode {
    pending: true,
    ..node(LandingNodeKind::Waiting, label)
  }
}

/// The node of the subject of `facts`: the first state that matches (module docs), with the ring of its own robot running
#[must_use]
pub fn landing_node(facts: &NodeFacts<'_>) -> LandingNode {
  let own = own_facts(facts);
  LandingNode {
    ring: matches!(own.slot, Slot::Running { .. }),
    ..first_state(facts, &own)
  }
}

fn first_state(facts: &NodeFacts<'_>, own: &OwnFacts) -> LandingNode {
  // a sync of the row passes: the node keeps the state it started from
  let step = StepFacts { syncing: false, ..facts.step };
  let slot = own.slot;
  let running = matches!(slot, Slot::Running { .. });
  // a parked branch is no move, a conflict of it neither, unless something lands it already
  if own.parked
    && let Some(parked) = step.parked
    && own.landed.is_none()
    && !running
    && facts.lands_with.is_none()
  {
    let label = NodeLabel::Parked {
      parked: parked.clone(),
      conflict: own.conflict,
    };
    return LandingNode {
      move_shows: !own.conflict,
      ..own_node(LandingNodeKind::Parked, label)
    };
  }
  // all of it is in master: a conflict of what is left to build is moot, the update archives it
  if let Some(landed) = &own.landed {
    return own_node(LandingNodeKind::InMaster, NodeLabel::Landed { landed: landed.clone() });
  }
  // the fix is the user's move even while a robot lands it: the conflict came after the push, of new work or of a
  // baseline the robot meets too, and the robot lands the remote copy, whose push it holds
  if own.conflict {
    let problem = step.problem.map(str::to_owned);
    return own_node(LandingNodeKind::NeedsFix, NodeLabel::SyncFailed { status: facts.status, problem });
  }
  if step.commit_count == 0 {
    return own_node(LandingNodeKind::Waiting, NodeLabel::NoCommits);
  }
  // a Safe Push that will land it lands it whatever its parent builds to locally: the move is the robot's
  if own.blocked && !own.landing {
    let by = facts.blocked_by.map(str::to_owned);
    return LandingNode {
      shows: by.clone(),
      ..own_node(LandingNodeKind::Waiting, NodeLabel::Blocked { by })
    };
  }
  if let Slot::Running { dry_run, failing, .. } = slot {
    let kind = if dry_run { LandingNodeKind::Waiting } else { LandingNodeKind::Landing };
    return robot(kind, NodeLabel::RobotRunning { dry_run, failing }, false);
  }
  if let Some(by) = facts.lands_with {
    return node(LandingNodeKind::Landing, NodeLabel::LandsWith { by: by.clone() });
  }
  // a push into the target or a merge on GitHub lands it: its own table decides
  let merges = merges_on_github(step.route, !matches!(step.patronus, PatronusAnswer::NotOnSpace));
  if matches!(step.route, LandingRoute::Push { .. }) || merges {
    if step.held {
      return node(LandingNodeKind::Waiting, NodeLabel::Held);
    }
    return of_step(facts);
  }
  let failure = match step.patronus {
    // what the robots hold and land waits for the first answers of Patronus, as the step does
    PatronusAnswer::Pending => return waits(NodeLabel::Waits { fact: StepFact::Patronus }),
    PatronusAnswer::NotOnSpace => None,
    PatronusAnswer::Answers { failure } => failure,
  };
  // Patronus answers with a failure: a login is the user's step, an error a wait
  if let Some(failure) = failure {
    let kind = if *failure == StepFailure::NotLoggedIn {
      LandingNodeKind::YourStep
    } else {
      LandingNodeKind::Waiting
    };
    return node(kind, NodeLabel::Unavailable { failure: failure.clone() });
  }
  if step.held {
    return node(LandingNodeKind::Waiting, NodeLabel::Held);
  }
  if matches!(step.patronus, PatronusAnswer::NotOnSpace) {
    return without_patronus(facts);
  }
  let finished = matches!(slot, Slot::Finished(_)) && !step.landing_part;
  // why the robot failed decides whether a restart helps: the node waits for it, after the facts the step waits for
  // first (the pull requests, the landing needs)
  if problems_pending(step.robot) && !matches!(step.listing, Listing::Loading) && !matches!(step.needs, Needs::Computing) {
    return waits(NodeLabel::Waits { fact: StepFact::RobotProblems });
  }
  // Patronus didn't give why: a failure to fix, no restart offered as if it hadn't failed for the approvals
  if let Some(StepFailure::ProblemsFailed { message }) = problems_failed(step.robot).filter(|_| finished) {
    return robot(LandingNodeKind::NeedsFix, NodeLabel::RobotFailed { problems: Some(message) }, true);
  }
  // a robot of a branch below that lands a part of it runs: the failure of the earlier piece waits for it
  if finished && slot == Slot::Finished(Chip::Failed) {
    return robot(LandingNodeKind::NeedsFix, NodeLabel::RobotFailed { problems: None }, true);
  }
  of_step(facts)
}

/// The node the next step decides, with no start refused by the robots: a robot that lands an ancestor is said apart
fn of_step(facts: &NodeFacts<'_>) -> LandingNode {
  let step = next_step(&StepFacts {
    syncing: false,
    start: None,
    ..facts.step
  });
  let kind = match &step.kind {
    StepKind::SafePush | StepKind::Restart { .. } => return ready(facts),
    StepKind::PushToBaseline { .. } | StepKind::MergePullRequest { .. } => return push_ready(facts, step),
    StepKind::AutoMerging { .. } => LandingNodeKind::Landing,
    StepKind::MergeHeld { .. } => merge_held(&step),
    StepKind::OpenPullRequest
    | StepKind::Retarget { .. }
    | StepKind::LinkStack { .. }
    | StepKind::Draft { .. }
    | StepKind::NeedsBranch { .. }
    | StepKind::PickRemoteCommits { .. }
    | StepKind::UpdateProject { .. }
    | StepKind::RobotPullRequest { .. }
    | StepKind::RequestReview { .. } => LandingNodeKind::YourStep,
    // "In #12": its approvals count in the pull request of the whole stack, whose step is in the island header
    StepKind::CreatePullRequest { stack, .. } => {
      if stack.is_some() {
        LandingNodeKind::Waiting
      } else {
        LandingNodeKind::YourStep
      }
    }
    StepKind::ChangesRequested { .. } | StepKind::ChangesRequestedUnjudged { .. } => LandingNodeKind::NeedsFix,
    StepKind::OtherBase { .. } | StepKind::Awaiting { .. } | StepKind::AwaitingUnjudged { .. } | StepKind::Settled | StepKind::Hidden => LandingNodeKind::Waiting,
    // nobody knows yet whether it can land: never a node that a later answer turns into another
    StepKind::Pending { .. } | StepKind::Unavailable { .. } => return waits(NodeLabel::Step { step: Box::new(step) }),
  };
  node(kind, NodeLabel::Step { step: Box::new(step) })
}

/// A start can land it now, unless a robot that lands an ancestor runs (it lands with it later). Ready only when
/// Patronus judged the approvals; otherwise the start is still the user's step, and Patronus checks them then
fn ready(facts: &NodeFacts<'_>) -> LandingNode {
  let lands = facts.step.lands;
  if let Some(reason) = facts.step.start {
    // the hover offers to show the branch whose robot lands a part of it
    let shows = match reason {
      Reason::Ancestor { landing, .. } if !landing.whole_stack => Some(landing.branch.clone()),
      _ => None,
    };
    return LandingNode {
      shows,
      ..node(LandingNodeKind::Waiting, NodeLabel::StartWaits { reason: reason.clone() })
    };
  }
  let listed = matches!(facts.step.listing, Listing::Listed);
  let approved = listed
    && facts.step.readiness.is_some_and(|fact| {
      !matches!(
        fact.state,
        FactState::Loading | FactState::Unknown | FactState::Unavailable { .. } | FactState::Stale { reason: Some(_) }
      ) && fact
        .value
        .as_ref()
        .is_some_and(|value| matches!(value.readiness, ReviewReadiness::Approved { .. } | ReviewReadiness::NotRequired))
    });
  if approved {
    return node(LandingNodeKind::Ready, NodeLabel::Ready { lands });
  }
  let unlisted = match facts.step.listing {
    Listing::Unlisted { why } => why.cloned(),
    Listing::Loading | Listing::Listed => None,
  };
  node(LandingNodeKind::YourStep, NodeLabel::AtStart { lands, unlisted })
}

/// What holds the merge of the pull request back (`StepKind::MergeHeld`, its reason the blocker of the step): a
/// conflict, changes requested and failing required checks are fixes; a head behind its base, a branch to push and a
/// draft the user's step; approvals, running checks and what GitHub doesn't name a wait
const fn merge_held(step: &NextStep) -> LandingNodeKind {
  match &step.safe_push {
    Some(StepBlocker::Rule {
      reason: Reason::MergeConflict { .. } | Reason::MergeChangesRequested { .. } | Reason::MergeChecksFail { .. },
    }) => LandingNodeKind::NeedsFix,
    Some(StepBlocker::Rule {
      reason: Reason::MergeBehind { .. } | Reason::MergeHeadBehind { .. } | Reason::MergeDraft { .. },
    }) => LandingNodeKind::YourStep,
    _ => LandingNodeKind::Waiting,
  }
}

/// A push into the target, or a merge of the pull request, lands it now: the user's move, unless a robot that lands a
/// part of it runs or the gate of the route refuses it, which the node says as a wait
fn push_ready(facts: &NodeFacts<'_>, step: NextStep) -> LandingNode {
  let refused = match &step.safe_push {
    Some(StepBlocker::Rule { reason }) => Some(reason),
    _ => None,
  };
  if let Some(reason) = facts.step.start.or(refused) {
    return node(LandingNodeKind::Waiting, NodeLabel::StartWaits { reason: reason.clone() });
  }
  node(LandingNodeKind::Ready, NodeLabel::Step { step: Box::new(step) })
}

/// A repository not on Space lands a branch by its pull request; without one GitHub lists, the push is the step it
/// shows
fn without_patronus(facts: &NodeFacts<'_>) -> LandingNode {
  let step = &facts.step;
  // an unknown pull request is no "not in a pull request"
  match step.listing {
    Listing::Loading => return waits(NodeLabel::Waits { fact: StepFact::PullRequests }),
    Listing::Unlisted { .. } => {
      return if facts.push_pending {
        node(LandingNodeKind::YourStep, NodeLabel::PushIt)
      } else {
        node(LandingNodeKind::Waiting, NodeLabel::Pushed)
      };
    }
    Listing::Listed => {}
  }
  let Some(pull_request) = step.pull_request.map(|it| it.pull_request).filter(|it| it.state == PullRequestState::Open) else {
    return node(LandingNodeKind::YourStep, NodeLabel::NoPullRequest);
  };
  let number = pull_request.number;
  if pull_request.is_draft {
    return node(LandingNodeKind::YourStep, NodeLabel::Draft { number });
  }
  // it lands through the pull request: what it waits for is the author's to fix
  let by = changes_requested_by(pull_request);
  if !by.is_empty() {
    return node(LandingNodeKind::NeedsFix, NodeLabel::ChangesRequestedBy { number, by });
  }
  if pull_request.review_decision == Some(ReviewDecision::ChangesRequested) {
    return node(LandingNodeKind::NeedsFix, NodeLabel::ChangesRequested { number });
  }
  if pull_request.checks.as_ref().is_some_and(|checks| checks.state == ChecksState::Failing) {
    return node(LandingNodeKind::NeedsFix, NodeLabel::ChecksFail { number });
  }
  let reviewers: Vec<String> = waiting_reviewers(&pull_request.reviewers).into_iter().cloned().collect();
  if reviewers.is_empty() {
    node(LandingNodeKind::Waiting, NodeLabel::LandsThrough { number })
  } else {
    node(LandingNodeKind::Waiting, NodeLabel::WaitsFor { number, reviewers })
  }
}
