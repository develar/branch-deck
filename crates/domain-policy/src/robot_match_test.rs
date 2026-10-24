use crate::robot_match::{RobotIndex, RobotMatches, Tips, is_started_on, match_table};
use domain_types::fixtures::{robot, stack_robot};
use domain_types::landing::Subject;
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn branch_robot<'a>(robots: &'a [PatronusRobot], name: &str, local_tip: Option<&str>, remote_tip: Option<&str>) -> Option<&'a str> {
  RobotIndex::new(robots).branch_robot(name, local_tip, remote_tip).map(|robot| robot.id.as_str())
}

fn whole_stack_robot<'a>(robots: &'a [PatronusRobot], root: &str, local_tip: Option<&str>, remote_tip: Option<&str>) -> Option<&'a str> {
  RobotIndex::new(robots).stack_robot(root, local_tip, remote_tip).map(|robot| robot.id.as_str())
}

#[test]
fn the_robot_of_a_branch_is_the_running_one_then_the_newest_and_a_temporary_ref_by_its_revision() {
  let robots = vec![
    robot("old", Some("IJPL-1"), PatronusRobotStatus::Failure, 1),
    robot("new", Some("IJPL-1"), PatronusRobotStatus::Canceled, 3),
    robot("running", Some("IJPL-1"), PatronusRobotStatus::Running, 2),
    robot("other", Some("IJPL-2"), PatronusRobotStatus::Running, 4),
  ];
  assert_eq!(branch_robot(&robots, "IJPL-1", Some("tip"), None), Some("running"));
  assert_eq!(branch_robot(&robots[..2], "IJPL-1", Some("tip"), None), Some("new"));

  // a Safe Push from the IDE or the CLI: a temporary ref, matched by the local or the pushed tip
  let temporary = vec![robot("cli", None, PatronusRobotStatus::Running, 5)];
  assert_eq!(branch_robot(&temporary, "IJPL-1", Some("tip"), None), Some("cli"));
  assert_eq!(branch_robot(&temporary, "IJPL-1", Some("fixed"), Some("tip")), Some("cli"));
  assert_eq!(branch_robot(&temporary, "IJPL-1", Some("fixed"), None), None);

  // a robot of a temporary ref on the tip wins over the branch's own by the same preference: running over finished
  let both = vec![
    robot("own", Some("IJPL-1"), PatronusRobotStatus::Failure, 9),
    robot("cli", None, PatronusRobotStatus::Running, 5),
  ];
  assert_eq!(branch_robot(&both, "IJPL-1", Some("tip"), None), Some("cli"));
  assert_eq!(branch_robot(&both, "IJPL-1", Some("fixed"), None), Some("own"));
}

/// The robot of the whole stack starts from the stack branch: it is the stack's, never a branch's, even at a tip a
/// branch has; a temporary ref at the tip of the stack branch is the stack's
#[test]
fn the_robot_of_the_whole_stack_belongs_to_its_root_and_to_no_branch() {
  let robots = vec![
    robot("auth", Some("auth"), PatronusRobotStatus::Failure, 1),
    stack_robot("stack", "auth", PatronusRobotStatus::Running, "tip"),
    stack_robot("other", "docs", PatronusRobotStatus::Running, "tip"),
  ];
  assert_eq!(branch_robot(&robots, "auth", Some("tip"), None), Some("auth"));
  assert_eq!(branch_robot(&robots[1..], "login", Some("tip"), Some("tip")), None);
  assert_eq!(whole_stack_robot(&robots, "auth", Some("stack-tip"), None), Some("stack"));
  assert_eq!(whole_stack_robot(&robots[..1], "auth", Some("tip"), None), None, "a branch's robot is not the stack's");
  let temporary = vec![robot("cli", None, PatronusRobotStatus::Running, 5)];
  assert_eq!(whole_stack_robot(&temporary, "auth", Some("fixed"), Some("tip")), Some("cli"));

  assert!(is_started_on(&robots[1], &Subject::WholeStack { root: "auth".to_string() }));
  assert!(!is_started_on(&robots[1], &Subject::Branch { name: "auth".to_string() }));
  assert!(
    !is_started_on(&temporary[0], &Subject::Branch { name: "auth".to_string() }),
    "a temporary ref is no branch's start"
  );
}

/// One index answers every branch and whole stack of a table: the ids of those that have a robot, by name and by root
#[test]
fn the_table_gets_the_robot_of_each_branch_and_whole_stack_that_has_one() {
  let robots = vec![
    robot("auth", Some("auth"), PatronusRobotStatus::Running, 3),
    robot("cli", None, PatronusRobotStatus::Success, 2),
    stack_robot("stack", "auth", PatronusRobotStatus::Running, "stack-tip"),
  ];
  let index = RobotIndex::new(&robots);
  let branches = [
    Tips {
      of: "auth",
      local: Some("a1"),
      remote: None,
    },
    Tips {
      of: "login",
      local: Some("l1"),
      remote: Some("tip"),
    },
    Tips {
      of: "docs",
      local: Some("d1"),
      remote: None,
    },
  ];
  let stacks = [
    Tips {
      of: "auth",
      local: Some("stack-tip"),
      remote: None,
    },
    Tips {
      of: "docs",
      local: None,
      remote: None,
    },
  ];
  let owned = |pairs: &[(&str, &str)]| pairs.iter().map(|(key, id)| ((*key).to_string(), (*id).to_string())).collect::<BTreeMap<_, _>>();
  assert_eq!(
    match_table(&index, branches, stacks),
    RobotMatches {
      branches: owned(&[("auth", "auth"), ("login", "cli")]),
      stacks: owned(&[("auth", "stack")]),
    }
  );
  assert_eq!(match_table(&RobotIndex::new(&[]), branches, stacks), RobotMatches::default());
}

/// One case of `src/test-support/robot-match-cases.json`, which the page's `findBranchRobot` passes too
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchCase {
  name: String,
  robots: Vec<PatronusRobot>,
  branch: Option<String>,
  stack: Option<String>,
  local_tip: Option<String>,
  remote_tip: Option<String>,
  expected: Option<String>,
}

/// The robots match their branches and whole stacks as the page matches them: the cases both run
#[test]
fn the_robots_match_their_branches_as_the_page_matches_them() {
  let cases: Vec<MatchCase> = serde_json::from_str(include_str!("../../../src/test-support/robot-match-cases.json")).unwrap();
  assert!(cases.len() >= 10);
  for case in cases {
    let (local, remote) = (case.local_tip.as_deref(), case.remote_tip.as_deref());
    let found = match (&case.branch, &case.stack) {
      (Some(branch), _) => branch_robot(&case.robots, branch, local, remote),
      (None, Some(root)) => whole_stack_robot(&case.robots, root, local, remote),
      (None, None) => panic!("{}: no branch and no stack", case.name),
    };
    assert_eq!(found, case.expected.as_deref(), "{}", case.name);
  }
}
