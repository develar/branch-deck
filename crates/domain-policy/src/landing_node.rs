//! Where a branch or a whole stack is on its way into master, the node of its row (spec/branch/branch-row.spec.md,
//! "Tree"; spec/branch/next-step.spec.md, "The surfaces"; docs/design/landing.md), for the app and `bd` alike: the first
//! state that matches over the facts of its next step (`next_step::StepFacts`) with what the step doesn't tell apart.
//! A sync of the row passes, so the node keeps the state it started from; a parked branch, a failure, a landing, a
//! block and its own robot running are the subject's own; the rest the next step decides, as the slot shows it, with no
//! start refused by the robots (a robot that lands an ancestor is said apart). A running move of the row (a start, a
//! review request) is the page's until the operations move into Rust (s10): it shows in place of a node that says so
//! (`move_shows`).

use crate::next_step::{Chip, Listing, Needs, PatronusAnswer, Slot, StepFacts, changes_requested_by, landed_whole, next_step, problems_failed, problems_pending, slot};
use crate::readiness::waiting_reviewers;
use domain_types::next_step::{LandingNode, LandingNodeKind, NodeLabel, StepFact, StepFailure, StepKind};
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
  /// A running Safe Push (not a Dry Run) of a branch built on it, or of the whole stack, lands it
  pub lands_with: Option<&'a RobotOf>,
  /// The remote lacks work of it: the step a repository without Patronus and GitHub shows
  pub push_pending: bool,
  /// The branches a Safe Push of it lands now (1 for a lone one)
  pub lands: u32,
}

const fn node(kind: LandingNodeKind, label: NodeLabel) -> LandingNode {
  LandingNode {
    kind,
    label,
    pending: false,
    robot: false,
    shows: None,
    move_shows: true,
  }
}

/// A node of the subject itself, which a running move of the row doesn't replace
fn own(kind: LandingNodeKind, label: NodeLabel) -> LandingNode {
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

/// The sync failed on it, or left it conflicting
const fn conflicts(facts: &NodeFacts<'_>) -> bool {
  facts.step.sync_failed || matches!(facts.status, BranchStatus::Error | BranchStatus::MergeConflict | BranchStatus::AnalyzingConflict)
}

/// The node of the subject of `facts`: the first state that matches (module docs)
#[must_use]
pub fn landing_node(facts: &NodeFacts<'_>) -> LandingNode {
  // a sync of the row passes: the node keeps the state it started from
  let step = StepFacts { syncing: false, ..facts.step };
  let slot = slot(&step);
  let landed = landed_whole(&step, slot);
  let running = matches!(slot, Slot::Running { .. });
  // a parked branch is no move, a conflict of it neither, unless something lands it already
  if let Some(parked) = step.parked
    && landed.is_none()
    && !running
    && facts.lands_with.is_none()
  {
    let conflict = conflicts(facts);
    let label = NodeLabel::Parked { parked: parked.clone(), conflict };
    return LandingNode {
      move_shows: !conflict,
      ..own(LandingNodeKind::Parked, label)
    };
  }
  if conflicts(facts) {
    let problem = step.problem.map(str::to_owned);
    return own(LandingNodeKind::NeedsFix, NodeLabel::SyncFailed { status: facts.status, problem });
  }
  if let Some(landed) = landed {
    return own(LandingNodeKind::InMaster, NodeLabel::Landed { landed });
  }
  if step.commit_count == 0 {
    return own(LandingNodeKind::Waiting, NodeLabel::NoCommits);
  }
  if facts.status == BranchStatus::Blocked {
    let by = facts.blocked_by.map(str::to_owned);
    return LandingNode {
      shows: by.clone(),
      ..own(LandingNodeKind::Waiting, NodeLabel::Blocked { by })
    };
  }
  if let Slot::Running { dry_run, failing } = slot {
    let kind = if dry_run { LandingNodeKind::Waiting } else { LandingNodeKind::Landing };
    return robot(kind, NodeLabel::RobotRunning { dry_run, failing }, false);
  }
  if let Some(by) = facts.lands_with {
    return node(LandingNodeKind::Landing, NodeLabel::LandsWith { by: by.clone() });
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
    StepKind::Retarget { .. } | StepKind::Draft { .. } | StepKind::NeedsBranch { .. } | StepKind::RobotPullRequest { .. } | StepKind::RequestReview { .. } => {
      LandingNodeKind::YourStep
    }
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
  let lands = facts.lands;
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
