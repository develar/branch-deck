use crate::patronus_start::{RobotSource, allows_push_mode, carried_ids, carried_words, find_branch_robot, find_stack_robot, is_push_into_robot_risky, is_start_blocked};
use pretty_assertions::assert_eq;
use sync_types::patronus::{PatronusProfile, PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use sync_types::repository_view::Carried;

fn robot(id: &str, branch_name: Option<&str>, status: PatronusRobotStatus, start_time: u32) -> PatronusRobot {
  PatronusRobot {
    id: id.to_string(),
    name: "IJPL-1 fix".to_string(),
    status,
    push_mode: PatronusPushMode::RebaseAutosquash,
    branch_name: branch_name.map(str::to_string),
    stack_root: None,
    source_revision: Some("tip".to_string()),
    target_branch: "master".to_string(),
    start_time,
    finish_time: None,
    cancellation_reason: None,
    url: format!("https://patronus/robot/{id}"),
    pull_request: None,
    profile: None,
  }
}

/// A robot of the whole stack of `root`, started on its stack branch at `revision`
fn stack_robot(id: &str, root: &str, status: PatronusRobotStatus, revision: &str) -> PatronusRobot {
  PatronusRobot {
    stack_root: Some(root.to_string()),
    source_revision: Some(revision.to_string()),
    ..robot(id, None, status, 1)
  }
}

fn dry_run(robot: PatronusRobot) -> PatronusRobot {
  PatronusRobot {
    push_mode: PatronusPushMode::DryRun,
    ..robot
  }
}

#[test]
fn the_robot_of_a_branch_is_the_running_one_then_the_newest_and_a_temporary_ref_by_its_revision() {
  let robots = vec![
    robot("old", Some("IJPL-1"), PatronusRobotStatus::Failure, 1),
    robot("new", Some("IJPL-1"), PatronusRobotStatus::Canceled, 3),
    robot("running", Some("IJPL-1"), PatronusRobotStatus::Running, 2),
    robot("other", Some("IJPL-2"), PatronusRobotStatus::Running, 4),
  ];
  assert_eq!(find_branch_robot(&robots, "IJPL-1", Some("tip"), None).map(|robot| robot.id.as_str()), Some("running"));
  assert_eq!(find_branch_robot(&robots[..2], "IJPL-1", Some("tip"), None).map(|robot| robot.id.as_str()), Some("new"));

  // a Safe Push from the IDE or the CLI: a temporary ref, matched by the local or the pushed tip
  let temporary = vec![robot("cli", None, PatronusRobotStatus::Running, 5)];
  assert_eq!(find_branch_robot(&temporary, "IJPL-1", Some("tip"), None).map(|robot| robot.id.as_str()), Some("cli"));
  assert_eq!(
    find_branch_robot(&temporary, "IJPL-1", Some("fixed"), Some("tip")).map(|robot| robot.id.as_str()),
    Some("cli")
  );
  assert_eq!(find_branch_robot(&temporary, "IJPL-1", Some("fixed"), None), None);
}

#[test]
fn a_start_waits_for_a_robot_on_the_last_commit_and_a_push_guards_a_running_safe_push() {
  let running = robot("r", Some("IJPL-1"), PatronusRobotStatus::Running, 1);
  assert!(is_start_blocked(&running, Some("tip"), None));
  assert!(!is_start_blocked(&running, Some("fixed"), None));
  assert!(!is_start_blocked(&robot("r", Some("IJPL-1"), PatronusRobotStatus::Failure, 1), Some("tip"), None));
  // the robot took the remote branch, which has the same commits, rebuilt since on a newer baseline
  assert!(is_start_blocked(&running, Some("rebuilt"), Some("tip")));

  assert!(is_push_into_robot_risky(&running, RobotSource::Branch("IJPL-1")));
  assert!(!is_push_into_robot_risky(&dry_run(running.clone()), RobotSource::Branch("IJPL-1")));
  assert!(!is_push_into_robot_risky(&robot("r", None, PatronusRobotStatus::Running, 1), RobotSource::Branch("IJPL-1")));
  // a push of the stack branch fails the robot of the whole stack, and only it
  let whole = stack_robot("s", "IJPL-1", PatronusRobotStatus::Running, "stack-tip");
  assert!(is_push_into_robot_risky(&whole, RobotSource::WholeStack("IJPL-1")));
  assert!(!is_push_into_robot_risky(&whole, RobotSource::Branch("IJPL-1")));
  assert!(!is_push_into_robot_risky(&running, RobotSource::WholeStack("IJPL-1")));
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
  assert_eq!(find_branch_robot(&robots, "auth", Some("tip"), None).map(|robot| robot.id.as_str()), Some("auth"));
  assert_eq!(find_branch_robot(&robots[1..], "login", Some("tip"), Some("tip")), None);
  assert_eq!(find_stack_robot(&robots, "auth", Some("stack-tip"), None).map(|robot| robot.id.as_str()), Some("stack"));
  assert_eq!(find_stack_robot(&robots[..1], "auth", Some("tip"), None), None, "a branch's robot is not the stack's");
  let temporary = vec![robot("cli", None, PatronusRobotStatus::Running, 5)];
  assert_eq!(find_stack_robot(&temporary, "auth", Some("fixed"), Some("tip")).map(|robot| robot.id.as_str()), Some("cli"));
}

#[test]
fn a_profile_allows_the_modes_it_lists_and_any_when_it_lists_none() {
  let profile = |modes: Vec<PatronusPushMode>| PatronusProfile {
    id: "a".to_string(),
    name: "A".to_string(),
    description: None,
    allowed_push_modes: modes,
    skips_review: None,
  };
  assert!(allows_push_mode(&profile(Vec::new()), PatronusPushMode::DryRun));
  assert!(!allows_push_mode(&profile(vec![PatronusPushMode::DryRun]), PatronusPushMode::RebaseAutosquash));
}

/// A refusal names the commits a running robot lands as its content told: their number, and for `bd` the first three
/// ids, an amended one marked
#[test]
fn the_commits_a_robot_lands_are_named_by_their_number_and_ids() {
  let ids: Vec<String> = ["aaaaaaaa1", "bbbbbbbb2", "cccccccc3", "dddddddd4"].iter().map(|id| (*id).to_string()).collect();
  let carried = Carried {
    commits: ids.clone(),
    amended: vec![ids[1].clone()],
  };
  assert_eq!(carried_words("login", None), "login");
  assert_eq!(carried_ids(None), "");
  assert_eq!(carried_words("login", Some(&carried)), "4 commits of login");
  assert_eq!(carried_ids(Some(&carried)), " (aaaaaaaa, bbbbbbbb amended since, cccccccc, …)");
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
      (Some(branch), _) => find_branch_robot(&case.robots, branch, local, remote),
      (None, Some(root)) => find_stack_robot(&case.robots, root, local, remote),
      (None, None) => panic!("{}: no branch and no stack", case.name),
    };
    assert_eq!(found.map(|robot| robot.id.clone()), case.expected, "{}", case.name);
  }
}
