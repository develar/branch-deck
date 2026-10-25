use crate::facts::{RemoteBranch, Robots};
use crate::land::{BranchStart, StackStart, Start, on_last_commit, restart_refusal, start_refusal, took_as_is};
use domain_types::fixtures::{robot, robot_of, stack_robot};
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldKind, HoldView, SpanView};
use domain_types::stack::{BlockReason, BranchStack, StackBlock, StackBranchState, StackBranchUpdate};
use pretty_assertions::assert_eq;

const SAFE_PUSH: PatronusPushMode = PatronusPushMode::RebaseAutosquash;
const DRY_RUN: PatronusPushMode = PatronusPushMode::DryRun;

fn running(id: &str, branch: &str) -> PatronusRobot {
  robot(id, Some(branch), PatronusRobotStatus::Running, 1)
}

/// login, with nothing in its way
fn start<'a>() -> BranchStart<'a> {
  BranchStart {
    name: "login",
    not_built: None,
    stack: None,
    last_commit: None,
    span: None,
    landing_ancestor: None,
    hold: None,
    pushes: true,
  }
}

/// The robot of the commit a branch is at, by its hashes or by its up-to-date remote branch: a running or canceling one
/// refuses a start of the same commits; a finished one, or one on a commit the branch moved past, does not
#[test]
fn a_robot_on_the_last_commit_is_one_that_runs_on_the_branch_as_it_is() {
  let remote = RemoteBranch {
    tip: Some("tip"),
    up_to_date: true,
    drops: 0,
  };
  let up_to_date = Some(remote);
  assert!(took_as_is("tip", Some("tip"), None));
  assert!(took_as_is("tip", Some("rebuilt"), up_to_date));
  assert!(!took_as_is("tip", None, up_to_date));
  assert!(!took_as_is("tip", Some("new"), Some(RemoteBranch { up_to_date: false, ..remote })));

  let at = |status: PatronusRobotStatus| on_last_commit(&robot("r", Some("login"), status, 1), Some("tip"), None);
  assert!(at(PatronusRobotStatus::Running));
  assert!(at(PatronusRobotStatus::Canceling));
  assert!(!at(PatronusRobotStatus::Failure));
  assert!(!at(PatronusRobotStatus::Canceled));
  assert!(!on_last_commit(&running("r", "login"), Some("new"), None));
}

/// A start of a branch, in the order of the refusals: blocked, a robot on the last commit, then for a Safe Push the
/// robot of a branch built on it that lands it, then one that lands an ancestor
#[test]
fn a_safe_push_waits_for_a_block_the_last_commit_a_robot_that_lands_it_then_one_that_lands_an_ancestor() {
  let last = running("last", "login");
  let lands_it = running("s", "session");
  let lands_ancestor = running("a", "auth");
  let robots = Robots::new([&last, &lands_it, &lands_ancestor]);
  let span = SpanView {
    by: robot_of("session", "s"),
    carried: None,
  };
  let ancestor = robot_of("auth", "a");
  let stack = BranchStack {
    blocked_by: Some(StackBlock {
      parent: "auth".to_string(),
      reason: BlockReason::Conflict,
    }),
    ..BranchStack::default()
  };
  let all = BranchStart {
    not_built: Some(BlockReason::Blocked),
    stack: Some(&stack),
    last_commit: Some(&last),
    span: Some(&span),
    landing_ancestor: Some(&ancestor),
    ..start()
  };
  let refusal = |start: BranchStart<'_>| start_refusal(&Start::Branch(start), SAFE_PUSH, &robots);
  assert_eq!(
    refusal(all),
    Some(Reason::Blocked {
      branch: "login".to_string(),
      by: Some("auth".to_string())
    })
  );
  let all = BranchStart { not_built: None, ..all };
  assert_eq!(
    refusal(all),
    Some(Reason::LastCommit {
      robot_id: "last".to_string(),
      mode: SAFE_PUSH
    })
  );
  let all = BranchStart { last_commit: None, ..all };
  assert_eq!(
    refusal(all),
    Some(Reason::Descendant {
      branch: "login".to_string(),
      by: robot_of("session", "s"),
      mode: SAFE_PUSH,
      carried: None
    })
  );
  let all = BranchStart { span: None, ..all };
  assert_eq!(
    refusal(all),
    Some(Reason::Ancestor {
      landing: ancestor.clone(),
      mode: SAFE_PUSH
    })
  );
  assert_eq!(refusal(start()), None);
  // an error of the branch is its own step, not a refusal of the start
  assert_eq!(
    refusal(BranchStart {
      not_built: Some(BlockReason::Error),
      ..start()
    }),
    None
  );
  // a fact whose robot the user's robots lack refuses nothing
  assert_eq!(
    start_refusal(&Start::Branch(BranchStart { span: Some(&span), ..start() }), SAFE_PUSH, &Robots::default()),
    None
  );
}

/// A Dry Run lands nothing: it waits only for a block and the robot on its last commit, and for a Safe Push of a branch
/// built on it while its own push would go into what that Safe Push lands (D15); its own Safe Push on an older commit
/// is the hold of another rule
#[test]
fn a_dry_run_waits_for_the_last_commit_and_for_the_span_hold_of_the_push_it_makes() {
  let lands_it = running("s", "session");
  let robots = Robots::new([&lands_it]);
  let span = SpanView {
    by: robot_of("session", "s"),
    carried: None,
  };
  let hold = HoldView {
    kind: HoldKind::Span,
    by: robot_of("session", "s"),
    carried: None,
  };
  let held = BranchStart {
    span: Some(&span),
    landing_ancestor: Some(&span.by),
    hold: Some(&hold),
    ..start()
  };
  assert_eq!(
    start_refusal(&Start::Branch(held), DRY_RUN, &robots),
    Some(Reason::Held {
      branch: "login".to_string(),
      hold: hold.clone(),
      mode: SAFE_PUSH,
      target: "master".to_string()
    })
  );
  // nothing to push: the dry run starts from the remote, which the Safe Push doesn't need
  assert_eq!(start_refusal(&Start::Branch(BranchStart { pushes: false, ..held }), DRY_RUN, &robots), None);
  let own = HoldView {
    kind: HoldKind::Own,
    ..hold.clone()
  };
  assert_eq!(start_refusal(&Start::Branch(BranchStart { hold: Some(&own), ..held }), DRY_RUN, &robots), None);
}

fn stack(state: StackBranchState) -> StackBranchUpdate {
  StackBranchUpdate {
    root: "auth".to_string(),
    branch_name: "user/stack/auth".to_string(),
    members: vec!["auth".to_string(), "login".to_string()],
    commit_count: 2,
    state,
    tip: Some("stack-tip".to_string()),
    blocked_by: None,
    conflict: None,
    error: None,
    remote: None,
  }
}

/// A start of the whole stack: a stack branch the sync did not build, the robot on its commit, then for a Safe Push the
/// robot of a member that lands a part of it; a Dry Run waits for no member
#[test]
fn a_whole_stack_waits_for_its_build_its_last_commit_then_a_member_that_lands_a_part() {
  let whole = stack_robot("w", "auth", PatronusRobotStatus::Running, "stack-tip");
  let member = running("l", "login");
  let robots = Robots::new([&whole, &member]);
  let built = stack(StackBranchState::Built);
  let landing = robot_of("login", "l");
  let start = StackStart {
    stack: &built,
    last_commit: Some(&whole),
    landing_member: Some(&landing),
  };
  let refusal = |start: StackStart<'_>, mode| start_refusal(&Start::WholeStack(start), mode, &robots);
  let conflict = stack(StackBranchState::Conflict);
  assert!(matches!(
    refusal(StackStart { stack: &conflict, ..start }, SAFE_PUSH),
    Some(Reason::StackNotBuilt {
      state: StackBranchState::Conflict,
      ..
    })
  ));
  assert!(matches!(refusal(start, DRY_RUN), Some(Reason::LastCommit { .. })));
  let start = StackStart { last_commit: None, ..start };
  assert_eq!(
    refusal(start, SAFE_PUSH),
    Some(Reason::Ancestor {
      landing: landing.clone(),
      mode: SAFE_PUSH
    })
  );
  assert_eq!(refusal(start, DRY_RUN), None);
}

/// A restart is the start of its robot's subject that the robot itself does not refuse and that pushes nothing; a
/// subject landed whole refuses it last
#[test]
fn a_restart_is_refused_as_a_start_without_its_own_robot_then_by_a_landing() {
  let own = running("own", "login");
  let lands_it = running("s", "session");
  let robots = Robots::new([&own, &lands_it]);
  let hold = HoldView {
    kind: HoldKind::Span,
    by: robot_of("session", "s"),
    carried: None,
  };
  let restart = BranchStart {
    last_commit: Some(&own),
    hold: Some(&hold),
    ..start()
  };
  assert_eq!(restart_refusal(&Start::Branch(restart), DRY_RUN, &robots, None), None);
  let landed = Reason::Landed {
    branch: "login".to_string(),
    by: robot_of("session", "m"),
    own: false,
    target: "master".to_string(),
  };
  assert_eq!(restart_refusal(&Start::Branch(restart), DRY_RUN, &robots, Some(landed.clone())), Some(landed));
  let span = SpanView {
    by: robot_of("session", "s"),
    carried: None,
  };
  assert!(matches!(
    restart_refusal(&Start::Branch(BranchStart { span: Some(&span), ..restart }), SAFE_PUSH, &robots, None),
    Some(Reason::Descendant { .. })
  ));
}
