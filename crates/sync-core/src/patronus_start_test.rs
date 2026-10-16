use crate::patronus_start::{
  LandingRobot, allows_push_mode, find_branch_robot, find_landing_ancestor, find_landing_descendant, is_push_into_robot_risky, is_start_blocked, start_refusal,
};
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use sync_types::patronus::{PatronusProfile, PatronusPushMode, PatronusRobot, PatronusRobotStatus};

fn robot(id: &str, branch_name: Option<&str>, status: PatronusRobotStatus, start_time: u32) -> PatronusRobot {
  PatronusRobot {
    id: id.to_string(),
    name: "IJPL-1 fix".to_string(),
    status,
    push_mode: PatronusPushMode::RebaseAutosquash,
    branch_name: branch_name.map(str::to_string),
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
  assert!(is_start_blocked(&running, Some("tip")));
  assert!(!is_start_blocked(&running, Some("fixed")));
  assert!(!is_start_blocked(&robot("r", Some("IJPL-1"), PatronusRobotStatus::Failure, 1), Some("tip")));
  assert_eq!(
    start_refusal(Some(&running), Some("tip"), None, None, "IJPL-1").as_deref(),
    Some("A Safe Push of the last commit is running")
  );
  assert_eq!(
    start_refusal(Some(&dry_run(running.clone())), Some("tip"), None, None, "IJPL-1").as_deref(),
    Some("A Dry Run of the last commit is running")
  );
  // a fix was committed since the robot started: a new one takes it
  assert_eq!(start_refusal(Some(&running), Some("fixed"), None, None, "IJPL-1"), None);

  assert!(is_push_into_robot_risky(&running, "IJPL-1"));
  assert!(!is_push_into_robot_risky(&dry_run(running.clone()), "IJPL-1"));
  assert!(!is_push_into_robot_risky(&robot("r", None, PatronusRobotStatus::Running, 1), "IJPL-1"));
}

#[test]
fn a_safe_push_waits_for_a_robot_that_lands_an_ancestor_or_the_branch_itself() {
  let robots: HashMap<&str, PatronusRobot> = HashMap::from([
    ("auth", dry_run(robot("auth", Some("auth"), PatronusRobotStatus::Running, 1))),
    ("login", robot("login", Some("login"), PatronusRobotStatus::Running, 2)),
  ]);
  let robot_of = |name: &str| robots.get(name);
  let ancestors = vec!["auth".to_string(), "login".to_string()];
  let landing = find_landing_ancestor(&ancestors, robot_of).unwrap();
  assert_eq!(
    landing,
    LandingRobot {
      branch: "login".to_string(),
      robot: &robots["login"]
    }
  );
  assert_eq!(find_landing_ancestor(&ancestors[..1], robot_of), None);
  assert_eq!(
    start_refusal(None, Some("tip"), Some(&landing), None, "oauth").as_deref(),
    Some("A Safe Push of login is running: wait until it merges")
  );

  let ancestors_of = |name: &str| match name {
    "login" => vec!["auth".to_string()],
    "oauth" => vec!["auth".to_string(), "login".to_string()],
    _ => Vec::new(),
  };
  let robots: HashMap<&str, PatronusRobot> = HashMap::from([("oauth", robot("oauth", Some("oauth"), PatronusRobotStatus::Running, 1))]);
  let robot_of = |name: &str| robots.get(name);
  let landing = find_landing_descendant("login", ["auth", "login", "oauth"], ancestors_of, robot_of).unwrap();
  assert_eq!(landing.branch, "oauth");
  assert_eq!(
    start_refusal(None, Some("tip"), None, Some(&landing), "login").as_deref(),
    Some("A Safe Push of oauth is running: it lands login with it")
  );
  assert_eq!(find_landing_descendant("oauth", ["auth", "login", "oauth"], ancestors_of, robot_of), None);
}

#[test]
fn a_profile_allows_the_modes_it_lists_and_any_when_it_lists_none() {
  let profile = |modes: Vec<PatronusPushMode>| PatronusProfile {
    id: "a".to_string(),
    name: "A".to_string(),
    description: None,
    allowed_push_modes: modes,
  };
  assert!(allows_push_mode(&profile(Vec::new()), PatronusPushMode::DryRun));
  assert!(!allows_push_mode(&profile(vec![PatronusPushMode::DryRun]), PatronusPushMode::RebaseAutosquash));
}
