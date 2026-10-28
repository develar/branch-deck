use crate::attention::{BranchCarriers, RobotOnSubject, StackCarriers, branch_attention, failed_as_is, stack_attention};
use crate::facts::RemoteBranch;
use domain_types::fixtures::{robot, stack_robot};
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};

/// A robot of auth in `status` that took the commit `source`
fn on_commit(source: Option<&str>, status: PatronusRobotStatus) -> PatronusRobot {
  PatronusRobot {
    source_revision: source.map(str::to_string),
    ..robot("r", Some("auth"), status, 1)
  }
}

/// The remote branch at `tip`; `up_to_date`: it has what the subject has by content
const fn remote(tip: &str, up_to_date: bool) -> RemoteBranch<'_> {
  RemoteBranch {
    tip: Some(tip),
    up_to_date,
    drops: 0,
    others: 0,
  }
}

/// A robot failed on the subject as it is: on its tip, a Dry Run too, or on the same commits before a sync rebuilt them
/// on a newer baseline (the remote at the robot's commit has them by content); a subject fixed since, one the sync
/// builds, a remote that is an older piece, a robot still running and one that took no commit ask nothing
#[test]
fn a_robot_failed_on_the_subject_as_it_is_only_on_its_tip_or_on_the_same_commits_rebuilt() {
  let failed = on_commit(Some("tip"), PatronusRobotStatus::Failure);
  let dry_run = PatronusRobot {
    push_mode: PatronusPushMode::DryRun,
    ..failed.clone()
  };
  let running = on_commit(Some("tip"), PatronusRobotStatus::Running);
  let no_commit = on_commit(None, PatronusRobotStatus::Failure);
  let on = |robot, tip, remote| RobotOnSubject {
    robot,
    tip,
    remote,
    cached: false,
  };
  let cases = [
    ("on its tip", on(&failed, Some("tip"), None), true),
    ("a dry run on its tip", on(&dry_run, Some("tip"), None), true),
    (
      "rebuilt on a newer baseline, the remote at its commit",
      on(&failed, Some("rebuilt"), Some(remote("tip", true))),
      true,
    ),
    ("fixed: the remote lacks the fix", on(&failed, Some("fixed"), Some(remote("tip", false))), false),
    ("fixed and pushed", on(&failed, Some("fixed"), Some(remote("fixed", true))), false),
    ("fixed, the remote status not known yet", on(&failed, Some("fixed"), None), false),
    ("the sync builds it", on(&failed, None, Some(remote("tip", true))), false),
    (
      "the remote at its commit is an older piece",
      on(&failed, Some("rebuilt"), Some(remote("tip", false))),
      false,
    ),
    ("still running", on(&running, Some("tip"), None), false),
    ("no commit of its own", on(&no_commit, Some("tip"), None), false),
  ];
  for (case, subject, expected) in cases {
    assert_eq!(failed_as_is(&subject), expected, "{case}");
  }
}

/// The failed robot of a branch as it is needs the user while nothing else holds the branch: parked, held by a running
/// Safe Push of a branch built on it or landed whole by a merged one, all of it in the fetched baseline, or a part of it
/// landed by a running robot of a branch it is built on, its row waits and its old failure asks for nothing; a robot of
/// the cached frame of an earlier run needs nothing either
#[test]
fn the_failed_robot_of_a_branch_needs_the_user_unless_something_else_holds_the_branch() {
  let failed = on_commit(Some("tip"), PatronusRobotStatus::Failure);
  let on = RobotOnSubject {
    robot: &failed,
    tip: Some("tip"),
    remote: None,
    cached: false,
  };
  let free = BranchCarriers::default();
  let cases = [
    ("nothing else holds it", free, true),
    ("parked", BranchCarriers { parked: true, ..free }, false),
    ("held or landed whole by another robot", BranchCarriers { held: true, ..free }, false),
    ("the fetched baseline has all of it", BranchCarriers { in_baseline: true, ..free }, false),
    ("a running robot of an ancestor lands a part of it", BranchCarriers { landing_part: true, ..free }, false),
  ];
  for (case, carriers, expected) in cases {
    assert_eq!(branch_attention(&on, carriers), expected, "{case}");
  }
  let fixed = RobotOnSubject { tip: Some("fixed"), ..on };
  assert!(!branch_attention(&fixed, free), "a branch fixed since needs nothing");
  let cached = RobotOnSubject { cached: true, ..on };
  assert!(!branch_attention(&cached, free), "a robot of the cached frame: no count a fresh answer takes back");
}

/// The failed robot of a whole stack needs the user only on a stack branch the sync built, and not while a running robot
/// of a branch of it lands a part of it, nor as the cached frame of an earlier run has it
#[test]
fn the_failed_robot_of_a_whole_stack_needs_the_user_only_on_a_built_stack_branch_no_robot_of_a_branch_lands() {
  let failed = stack_robot("whole", "air", PatronusRobotStatus::Failure, "stack-tip");
  let on = RobotOnSubject {
    robot: &failed,
    tip: Some("stack-tip"),
    remote: None,
    cached: false,
  };
  let built = StackCarriers { built: true, landing_part: false };
  assert!(stack_attention(&on, built));
  assert!(
    !stack_attention(&on, StackCarriers { built: false, ..built }),
    "a stack branch not rebuilt: the stack lands in pieces"
  );
  assert!(
    !stack_attention(&on, StackCarriers { landing_part: true, ..built }),
    "a robot of a branch of it lands a part of it"
  );
  assert!(
    !stack_attention(&RobotOnSubject { tip: Some("rebuilt"), ..on }, built),
    "a stack branch changed since needs nothing"
  );
  assert!(!stack_attention(&RobotOnSubject { cached: true, ..on }, built), "a robot of the cached frame");
}
