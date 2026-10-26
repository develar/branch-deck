use crate::facts::RemoteBranch;
use crate::holds::{PushStep, StackHoldFacts, branch_hold, push_blocker, restart_action, stack_branch_hold};
use domain_types::fixtures::{robot, robot_of, stack_robot, whole_stack_robot_of};
use domain_types::patronus::{PatronusPullRequestLink, PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::reason::Reason;
use domain_types::repository_view::{Carried, HoldKind, LandedByView, RestartAction, RobotOf, SpanView};
use domain_types::session::{Parked, ParkedMember};
use domain_types::stack::{BlockReason, StackBranchState, StackBranchUpdate};
use pretty_assertions::assert_eq;

fn landed(by: RobotOf, after: &[&str]) -> LandedByView {
  LandedByView {
    by,
    own: false,
    landed: vec!["a1".to_string()],
    after: after.iter().map(|id| (*id).to_string()).collect(),
  }
}

fn span(by: RobotOf) -> SpanView {
  SpanView {
    by,
    carried: Some(Carried {
      commits: vec!["a1".to_string()],
      amended: Vec::new(),
    }),
  }
}

fn with_status(id: &str, branch: &str, status: PatronusRobotStatus) -> PatronusRobot {
  robot(id, Some(branch), status, 1)
}

/// The hold of a branch, by its kind and robot: a landing that left nothing, else its own running Safe Push, else the
/// first running one that lands it; a canceling robot or a dry run holds nothing
#[test]
fn a_branch_is_held_by_what_landed_it_whole_then_its_own_safe_push_then_one_that_lands_it() {
  let own = with_status("own", "auth", PatronusRobotStatus::Running);
  let by_span = span(robot_of("login", "l"));
  let hold = |landed_by: Option<&LandedByView>, robot: Option<&PatronusRobot>, running: Option<&SpanView>| {
    branch_hold("auth", landed_by, robot, running).map(|hold| (hold.kind, hold.by.robot_id))
  };
  let whole_landing = landed(robot_of("login", "m"), &[]);
  assert_eq!(hold(Some(&whole_landing), Some(&own), Some(&by_span)), Some((HoldKind::Landed, "m".to_string())));
  let work_left = landed(robot_of("login", "m"), &["a2"]);
  assert_eq!(hold(Some(&work_left), Some(&own), Some(&by_span)), Some((HoldKind::Own, "own".to_string())));
  assert_eq!(hold(None, None, Some(&by_span)), Some((HoldKind::Span, "l".to_string())));

  let canceling = with_status("own", "auth", PatronusRobotStatus::Canceling);
  assert_eq!(hold(None, Some(&canceling), None), None);
  let dry_run = PatronusRobot {
    push_mode: PatronusPushMode::DryRun,
    ..own
  };
  assert_eq!(hold(None, Some(&dry_run), None), None);
  // a robot of a temporary ref matched to the row runs on no branch: a push moves nothing it took
  let temporary = robot("tmp", None, PatronusRobotStatus::Running, 1);
  assert_eq!(hold(None, Some(&temporary), None), None);
  // the span keeps what the content told
  assert_eq!(branch_hold("auth", None, None, Some(&by_span)).and_then(|hold| hold.carried), by_span.carried);
}

fn stack(state: StackBranchState, tip: Option<&str>) -> StackBranchUpdate {
  StackBranchUpdate {
    root: "auth".to_string(),
    branch_name: "user/stack/auth".to_string(),
    members: vec!["auth".to_string(), "login".to_string()],
    commit_count: 2,
    state,
    tip: tip.map(str::to_string),
    blocked_by: None,
    conflict: None,
    error: None,
    remote: None,
  }
}

/// The stack branch is held by a parked member first, then by merged Safe Pushes that landed every member, then by its
/// own running robot, then by a build the sync did not make
#[test]
fn a_stack_branch_is_held_by_a_parked_member_then_landed_members_then_its_robot_then_no_build() {
  let built = stack(StackBranchState::Built, Some("stack-tip"));
  let parked = ParkedMember {
    member: "login".to_string(),
    parked: Parked {
      note: String::new(),
      with: Some("auth".to_string()),
    },
  };
  let merged = PatronusRobot {
    finish_time: Some(5),
    ..with_status("m", "login", PatronusRobotStatus::Success)
  };
  let login = robot_of("login", "m");
  let running = stack_robot("w", "auth", PatronusRobotStatus::Running, "stack-tip");
  let facts = StackHoldFacts {
    stack: &built,
    parked: Some(&parked),
    landed: Some(vec![(&login, &merged), (&login, &merged)]),
    robot: Some(&running),
  };
  assert_eq!(
    stack_branch_hold(&facts),
    Some(Reason::StackParked {
      root: "auth".to_string(),
      member: "auth".to_string()
    })
  );
  let facts = StackHoldFacts { parked: None, ..facts };
  assert_eq!(
    stack_branch_hold(&facts),
    Some(Reason::StackLanded {
      root: "auth".to_string(),
      by: vec![robot_of("login", "m")],
      own: false,
      target: "master".to_string()
    })
  );
  let facts = StackHoldFacts { landed: None, ..facts };
  assert_eq!(
    stack_branch_hold(&facts),
    Some(Reason::StackHeld {
      root: "auth".to_string(),
      robot_id: "w".to_string(),
      mode: PatronusPushMode::RebaseAutosquash
    })
  );
  let canceling = PatronusRobot {
    status: PatronusRobotStatus::Canceling,
    ..running.clone()
  };
  assert_eq!(
    stack_branch_hold(&StackHoldFacts {
      robot: Some(&canceling),
      ..facts.clone()
    }),
    None
  );
  let blocked = stack(StackBranchState::Blocked, Some("old"));
  assert!(matches!(
    stack_branch_hold(&StackHoldFacts {
      stack: &blocked,
      robot: None,
      ..facts.clone()
    }),
    Some(Reason::StackNotBuilt {
      state: StackBranchState::Blocked,
      ..
    })
  ));
  let no_tip = stack(StackBranchState::Built, None);
  assert!(matches!(
    stack_branch_hold(&StackHoldFacts {
      stack: &no_tip,
      robot: None,
      ..facts
    }),
    Some(Reason::StackNotBuilt { .. })
  ));
}

/// The robot of the whole stack landed every member: its own landing
#[test]
fn every_member_landed_by_the_robot_of_the_whole_stack_is_its_own_landing() {
  let built = stack(StackBranchState::Built, Some("stack-tip"));
  let merged = PatronusRobot {
    finish_time: Some(5),
    ..stack_robot("w", "auth", PatronusRobotStatus::Success, "stack-tip")
  };
  let by = whole_stack_robot_of("auth", "w");
  let facts = StackHoldFacts {
    stack: &built,
    parked: None,
    landed: Some(vec![(&by, &merged), (&by, &merged)]),
    robot: None,
  };
  assert!(matches!(stack_branch_hold(&facts), Some(Reason::StackLanded { own: true, .. })));
}

fn step(name: &str, not_built: Option<BlockReason>, drops: u32, held: bool) -> PushStep<'_> {
  PushStep {
    name,
    not_built,
    remote: Some(RemoteBranch {
      tip: Some("remote"),
      up_to_date: false,
      drops,
      others: 0,
    }),
    held,
  }
}

/// The push of an action is refused by a branch of its chain the sync did not build or whose push drops remote
/// commits, up to the first held ancestor, whose robot lands it and the rest below
#[test]
fn the_push_of_an_action_stops_at_a_held_ancestor_and_refuses_a_branch_not_built_or_dropping_commits() {
  assert_eq!(push_blocker([step("login", None, 0, false), step("auth", None, 0, false)]), None);
  assert_eq!(
    push_blocker([step("login", None, 0, false), step("auth", Some(BlockReason::Conflict), 0, false)]),
    Some(Reason::CantPush {
      branch: "auth".to_string(),
      block: BlockReason::Conflict
    })
  );
  assert_eq!(
    push_blocker([step("login", None, 2, false), step("auth", Some(BlockReason::Conflict), 0, false)]),
    Some(Reason::DropsRemote {
      branch: "login".to_string(),
      behind: 2
    })
  );
  // the held ancestor lands with its robot: nothing below it is pushed
  assert_eq!(push_blocker([step("login", None, 0, false), step("auth", Some(BlockReason::Conflict), 0, true)]), None);
  // the branch itself held: its own push says so, the chain goes on
  assert!(push_blocker([step("login", None, 0, true), step("auth", Some(BlockReason::Error), 0, false)]).is_some());
  // a remote not compared yet drops nothing known
  let unknown = PushStep {
    remote: None,
    ..step("login", None, 0, false)
  };
  assert_eq!(push_blocker([unknown]), None);
}

fn remote(tip: &str, up_to_date: bool) -> RemoteBranch<'_> {
  RemoteBranch {
    tip: Some(tip),
    up_to_date,
    drops: 0,
    others: 0,
  }
}

/// A restart of a failed robot: Patronus restarts it on the remote while the remote has the branch, else the branch is
/// pushed first; a running robot of a branch that moved on is canceled and pushed over; a dry run passed restarts only
/// on a newer remote commit; one of a temporary ref, one that merged and one a Space merge request refused for its
/// approvals offer none
#[test]
fn a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has() {
  let failed = with_status("f", "auth", PatronusRobotStatus::Failure);
  assert_eq!(restart_action(&failed, Some("tip"), Some(remote("tip", true)), false), Some(RestartAction::Restart));
  assert_eq!(restart_action(&failed, Some("new"), Some(remote("tip", false)), false), Some(RestartAction::PushAndRestart));
  // rebuilt on a newer baseline, the remote has the same commits: a plain restart
  assert_eq!(restart_action(&failed, Some("rebuilt"), Some(remote("tip", true)), false), Some(RestartAction::Restart));
  // the remote status is not known yet: the robot took the branch as Patronus has it
  assert_eq!(restart_action(&failed, Some("new"), None, false), Some(RestartAction::Restart));
  // no row: the restart takes the remote branch as Patronus has it
  assert_eq!(restart_action(&failed, None, None, false), Some(RestartAction::Restart));

  let running = with_status("r", "auth", PatronusRobotStatus::Running);
  assert_eq!(
    restart_action(&running, Some("new"), Some(remote("tip", false)), false),
    Some(RestartAction::PushAndRestart)
  );
  assert_eq!(restart_action(&running, Some("tip"), Some(remote("tip", true)), false), None);
  assert_eq!(restart_action(&running, Some("new"), Some(remote("tip", false)), true), None);
  let of_pull_request = PatronusRobot {
    pull_request: Some(PatronusPullRequestLink {
      number: 7,
      url: "https://github.com/acme/air/pull/7".to_string(),
    }),
    ..running
  };
  assert_eq!(
    restart_action(&of_pull_request, Some("new"), Some(remote("tip", false)), true),
    Some(RestartAction::PushAndRestart)
  );

  let passed = PatronusRobot {
    push_mode: PatronusPushMode::DryRun,
    ..with_status("d", "auth", PatronusRobotStatus::Success)
  };
  assert_eq!(restart_action(&passed, Some("tip"), Some(remote("newer", true)), false), Some(RestartAction::Restart));
  assert_eq!(restart_action(&passed, Some("tip"), Some(remote("tip", true)), false), None);
  let merged = with_status("m", "auth", PatronusRobotStatus::Success);
  assert_eq!(restart_action(&merged, Some("tip"), Some(remote("newer", true)), false), None);
  let temporary = robot("tmp", None, PatronusRobotStatus::Failure, 1);
  assert_eq!(restart_action(&temporary, Some("tip"), Some(remote("tip", true)), false), None);

  let whole_stack = stack_robot("w", "auth", PatronusRobotStatus::Failure, "stack-tip");
  assert_eq!(
    restart_action(&whole_stack, Some("new-stack-tip"), Some(remote("stack-tip", false)), false),
    Some(RestartAction::PushAndRestart)
  );
  assert_eq!(
    restart_action(&whole_stack, Some("stack-tip"), Some(remote("stack-tip", true)), false),
    Some(RestartAction::Restart)
  );
}
