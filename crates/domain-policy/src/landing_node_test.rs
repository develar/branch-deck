use crate::landing_node::{NodeFacts, is_users_move, landing_node};
use crate::next_step_test::{Fixture, failed, failure, fresh, group, missing, open, readiness, reviewer, robot_on_tip, signed_out};
use domain_types::fixtures::{robot_of, whole_stack_robot_of};
use domain_types::landing::{Landed, LandedBy};
use domain_types::next_step::{LandingNode, LandingNodeKind, NodeLabel, StackPullRequest, StepFact, StepFailure, StepKind};
use domain_types::patronus::{MissingApprovalsReason, PatronusAvailability, PatronusPushMode, PatronusRobotStatus, ReviewReadiness, UnknownReadinessReason};
use domain_types::pull_request::{PullRequestAvailability, PullRequestBase, ReviewerState};
use domain_types::reason::Reason;
use domain_types::repository_view::RobotOf;
use domain_types::session::{BranchStatus, Fact, FactState, Parked, RobotsHead};
use pretty_assertions::assert_eq;

/// The node of `fixture` with the row's `status` and the robot that lands it with more
fn node_of(fixture: &Fixture, status: BranchStatus, lands_with: Option<&RobotOf>) -> LandingNode {
  let standing = fixture.standing();
  landing_node(&NodeFacts {
    step: fixture.facts(&standing),
    status,
    blocked_by: (status == BranchStatus::Blocked).then_some("core"),
    lands_with,
    push_pending: false,
  })
}

fn node(fixture: &Fixture) -> LandingNode {
  node_of(fixture, BranchStatus::Unchanged, None)
}

fn kind_and_label(node: &LandingNode) -> (LandingNodeKind, NodeLabel) {
  (node.kind, node.label.clone())
}

/// What the subject is itself (parked, a failure, landed, blocked, its own robot running) stays through a running move
/// of the row; a parked branch a Safe Push lands already is landing; a sync of the row passes
#[test]
fn the_node_of_the_subject_itself_parked_failing_landed_blocked_or_its_robot_running_stays_through_a_move() {
  let parked = Parked {
    note: "after the release".to_string(),
    with: None,
  };
  let local = Fixture::branch().with(|it| it.parked = Some(parked.clone()));
  let kept = node(&local);
  assert_eq!((kept.kind, kept.move_shows), (LandingNodeKind::Parked, true), "a move of a parked branch is made");
  let conflict = node_of(&local, BranchStatus::MergeConflict, None);
  assert_eq!((conflict.label, conflict.move_shows), (NodeLabel::Parked { parked, conflict: true }, false));
  let failed_sync = node_of(&Fixture::branch(), BranchStatus::Error, None);
  assert_eq!(
    kind_and_label(&failed_sync),
    (
      LandingNodeKind::NeedsFix,
      NodeLabel::SyncFailed {
        status: BranchStatus::Error,
        problem: None
      }
    )
  );
  let landed = Landed {
    by: LandedBy::Baseline {
      upstream: "origin/master".to_string(),
    },
    after: Vec::new(),
  };
  let in_master = node(&Fixture::branch().with(|it| it.landed = Some(landed.clone())));
  assert_eq!(
    (in_master.kind, in_master.label, in_master.move_shows),
    (LandingNodeKind::InMaster, NodeLabel::Landed { landed }, false)
  );
  let blocked = node_of(&Fixture::branch().with(|it| it.blocked = true), BranchStatus::Blocked, None);
  assert_eq!((blocked.kind, blocked.shows.as_deref()), (LandingNodeKind::Waiting, Some("core")));
  let syncing = node(&Fixture::branch().with(|it| it.syncing = true));
  assert_eq!(syncing.kind, LandingNodeKind::Ready, "a sync passes: the node keeps the state it started from");
  let running = node(&Fixture::branch().with(|it| it.robot = Some((robot_on_tip(PatronusRobotStatus::Running), None, None, false))));
  assert_eq!((running.kind, running.robot, running.move_shows), (LandingNodeKind::Landing, true, false));
  let dry = Fixture::branch().with(|it| {
    let mut robot = robot_on_tip(PatronusRobotStatus::Running);
    robot.push_mode = PatronusPushMode::DryRun;
    it.robot = Some((robot, None, None, false));
  });
  assert_eq!(node(&dry).kind, LandingNodeKind::Waiting, "a dry run lands nothing");
  let oauth = robot_of("oauth", "r9");
  let with = node_of(&local, BranchStatus::Unchanged, Some(&oauth));
  assert_eq!(kind_and_label(&with), (LandingNodeKind::Landing, NodeLabel::LandsWith { by: oauth }));
}

/// Ready once Patronus judged the approvals; otherwise the start is the user's step, and Patronus checks them then. A
/// robot that lands an ancestor holds it: it waits, offering to show that branch (not the whole stack's)
#[test]
fn is_ready_to_land_once_patronus_judged_the_approvals_else_the_start_is_the_users_step() {
  assert_eq!(kind_and_label(&node(&Fixture::branch())), (LandingNodeKind::Ready, NodeLabel::Ready { lands: 2 }));
  let unknown = Fixture::branch().with(readiness(ReviewReadiness::Unknown {
    reason: UnknownReadinessReason::Skipped,
  }));
  assert_eq!(
    kind_and_label(&node(&unknown)),
    (LandingNodeKind::YourStep, NodeLabel::AtStart { lands: 2, unlisted: None })
  );
  let signed_out = Fixture::branch().with(|it| it.listing = signed_out());
  let unlisted = NodeLabel::AtStart {
    lands: 2,
    unlisted: Some(PullRequestAvailability::SignedOut),
  };
  assert_eq!(kind_and_label(&node(&signed_out)), (LandingNodeKind::YourStep, unlisted));
  let ancestor = |landing: RobotOf| {
    Fixture::branch().with(|it| {
      it.start = Some(Reason::Ancestor {
        landing,
        mode: PatronusPushMode::RebaseAutosquash,
      });
    })
  };
  let below = node(&ancestor(robot_of("core", "r5")));
  assert_eq!((below.kind, below.shows.as_deref()), (LandingNodeKind::Waiting, Some("core")));
  assert_eq!(node(&ancestor(whole_stack_robot_of("air", "r6"))).shows, None);
}

/// The other steps: the user's (a retarget, a draft, a pull request, a review), a fix (changes requested, a failed
/// robot), a wait (the reviewers, a base that may be meant, the pull request of the whole stack)
#[test]
fn waits_for_others_the_reviewers_a_base_that_may_be_meant_and_is_the_users_step_otherwise() {
  let kind = |fixture: Fixture| node(&fixture).kind;
  let virtual_base = PullRequestBase::Virtual {
    from: "core".to_string(),
    retarget: "master".to_string(),
  };
  assert_eq!(kind(Fixture::branch().with(open(virtual_base, |_| {}))), LandingNodeKind::YourStep);
  assert_eq!(
    kind(Fixture::branch().with(open(PullRequestBase::Other { base: "release".to_string() }, |_| {}))),
    LandingNodeKind::Waiting
  );
  assert_eq!(
    kind(Fixture::branch().with(open(PullRequestBase::Target, |it| it.is_draft = true))),
    LandingNodeKind::YourStep
  );
  let groups = |requested: &[&str], changes: &[&str]| missing(vec![group("IJPL Code Platform", requested, &[], changes)], MissingApprovalsReason::ApprovalsMissing);
  assert_eq!(kind(Fixture::branch().with(readiness(groups(&[], &[])))), LandingNodeKind::YourStep);
  let in_stack = Fixture::branch().with(readiness(groups(&[], &[]))).with(|it| {
    it.stack_pull_request = Some(StackPullRequest {
      number: 12,
      root: "air".to_string(),
    });
  });
  assert_eq!(kind(in_stack), LandingNodeKind::Waiting);
  let with_open = |answer| Fixture::branch().with(open(PullRequestBase::Target, |_| {})).with(readiness(answer));
  assert_eq!(kind(with_open(groups(&["ikorennoy"], &["ikorennoy"]))), LandingNodeKind::NeedsFix);
  assert_eq!(kind(with_open(groups(&[], &[]))), LandingNodeKind::YourStep);
  let awaiting = node(&with_open(groups(&["ikorennoy"], &[])));
  assert!(matches!(&awaiting.label, NodeLabel::Step { step } if matches!(step.kind, StepKind::Awaiting { .. })));
  assert_eq!(awaiting.kind, LandingNodeKind::Waiting);
  let failed_robot = node(&Fixture::branch().with(failed(Some(false), true)));
  assert_eq!((failed_robot.kind, failed_robot.robot), (LandingNodeKind::NeedsFix, true));
}

/// Before the first answers the node waits as the step does, and the header counts no move of it; a failed read of
/// Patronus says it, a login being the user's step; why a robot failed decides whether a restart helps
#[test]
fn waits_for_the_first_answers_as_the_step_does_and_says_a_read_that_failed() {
  let pending = node(&Fixture::branch().with(|it| it.robots_head = Fact::default()));
  assert_eq!(
    (pending.kind, pending.pending, pending.label),
    (LandingNodeKind::Waiting, true, NodeLabel::Waits { fact: StepFact::Patronus })
  );
  let not_logged_in = Fixture::branch().with(|it| {
    it.robots_head = Fact {
      state: FactState::Unavailable { reason: failure("not logged in") },
      ..fresh(RobotsHead {
        availability: PatronusAvailability::NotLoggedIn,
        github_repository: None,
      })
    }
  });
  let login = node(&not_logged_in);
  assert_eq!(
    kind_and_label(&login),
    (
      LandingNodeKind::YourStep,
      NodeLabel::Unavailable {
        failure: StepFailure::NotLoggedIn
      }
    )
  );
  let problems = node(&Fixture::branch().with(failed(None, true)));
  assert_eq!((problems.pending, problems.label), (true, NodeLabel::Waits { fact: StepFact::RobotProblems }));
  let not_given = Fixture::branch().with(|it| it.robot = Some((robot_on_tip(PatronusRobotStatus::Failure), None, Some("timeout".to_string()), true)));
  let given = node(&not_given);
  assert_eq!(
    (given.kind, given.label),
    (
      LandingNodeKind::NeedsFix,
      NodeLabel::RobotFailed {
        problems: Some("timeout".to_string())
      }
    )
  );
  let readiness = node(&Fixture::branch().with(|it| it.readiness = None));
  assert!(readiness.pending && matches!(&readiness.label, NodeLabel::Step { step } if step.kind == StepKind::Pending { fact: StepFact::Readiness }));
}

/// Off Space a branch lands through its pull request: the node says what it waits for, or the push it needs first
#[test]
fn off_space_a_branch_lands_through_its_pull_request() {
  let off_space = |change: fn(&mut Fixture)| {
    Fixture::branch().with(|it| {
      it.robots_head = fresh(RobotsHead {
        availability: PatronusAvailability::NotPatronus,
        github_repository: None,
      });
      change(it);
    })
  };
  let label = |fixture: Fixture| node(&fixture).label;
  assert_eq!(label(off_space(|it| it.listing = Fact::default())), NodeLabel::Waits { fact: StepFact::PullRequests });
  assert_eq!(label(off_space(|it| it.listing = fresh(PullRequestAvailability::NotGitHub))), NodeLabel::Pushed);
  assert_eq!(label(off_space(|_| {})), NodeLabel::NoPullRequest);
  assert_eq!(
    label(off_space(|it| open(PullRequestBase::Target, |pr| pr.is_draft = true)(it))),
    NodeLabel::Draft { number: 7 }
  );
  let changes = off_space(|it| {
    open(PullRequestBase::Target, |pr| {
      pr.reviewers = vec![reviewer("ikorennoy", ReviewerState::ChangesRequested, false)];
    })(it);
  });
  let by = vec!["ikorennoy".to_string()];
  assert_eq!(label(changes), NodeLabel::ChangesRequestedBy { number: 7, by: by.clone() });
  let waits = off_space(|it| open(PullRequestBase::Target, |pr| pr.reviewers = vec![reviewer("ikorennoy", ReviewerState::Requested, true)])(it));
  assert_eq!(label(waits), NodeLabel::WaitsFor { number: 7, reviewers: by });
  assert_eq!(label(off_space(|it| open(PullRequestBase::Target, |_| {})(it))), NodeLabel::LandsThrough { number: 7 });
}

#[test]
fn a_filled_node_is_the_users_move() {
  let moves: Vec<LandingNodeKind> = [
    LandingNodeKind::Ready,
    LandingNodeKind::YourStep,
    LandingNodeKind::NeedsFix,
    LandingNodeKind::Landing,
    LandingNodeKind::Waiting,
    LandingNodeKind::Parked,
    LandingNodeKind::InMaster,
  ]
  .into_iter()
  .filter(|kind| is_users_move(*kind))
  .collect();
  assert_eq!(moves, vec![LandingNodeKind::Ready, LandingNodeKind::YourStep, LandingNodeKind::NeedsFix]);
}
