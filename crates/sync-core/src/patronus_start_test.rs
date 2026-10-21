use crate::patronus_start::{
  CarriedCommits, LandingRobot, RobotSource, allows_push_mode, carried_ids, carried_words, find_branch_robot, find_landing_ancestor, find_landing_descendant, find_landing_member,
  find_landing_stack, find_stack_robot, is_push_into_robot_risky, is_start_blocked, start_refusal,
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
  assert_eq!(
    start_refusal(Some(&running), Some("tip"), None, None, None, "IJPL-1").as_deref(),
    Some("A Safe Push of the last commit is running")
  );
  assert_eq!(
    start_refusal(Some(&dry_run(running.clone())), Some("tip"), None, None, None, "IJPL-1").as_deref(),
    Some("A Dry Run of the last commit is running")
  );
  assert_eq!(
    start_refusal(Some(&running), Some("rebuilt"), Some("tip"), None, None, "IJPL-1").as_deref(),
    Some("A Safe Push of the last commit is running")
  );
  // a fix was committed since the robot started: a new one takes it
  assert_eq!(start_refusal(Some(&running), Some("fixed"), None, None, None, "IJPL-1"), None);

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

/// Both ways: the Safe Push of the whole stack lands every branch of it, so a branch's start waits for it; a robot of a
/// branch lands a part of the stack, so the start of the whole stack waits for it. A dry run lands nothing
#[test]
fn the_whole_stack_and_its_branches_wait_for_each_other() {
  let stacks: HashMap<&str, PatronusRobot> = HashMap::from([("auth", stack_robot("stack", "auth", PatronusRobotStatus::Running, "stack-tip"))]);
  let landing = find_landing_stack(Some("auth"), |root| stacks.get(root)).unwrap();
  assert_eq!(
    landing,
    LandingRobot {
      branch: "auth".to_string(),
      robot: &stacks["auth"],
      whole_stack: true,
      carried: None,
    }
  );
  assert_eq!(
    start_refusal(None, Some("tip"), None, None, Some(&landing), "login").as_deref(),
    Some("A Safe Push of the whole auth stack is running: it lands login with it")
  );
  assert_eq!(find_landing_stack(None, |root| stacks.get(root)), None, "a branch of no stack that forks");
  let dry: HashMap<&str, PatronusRobot> = HashMap::from([("auth", dry_run(stacks["auth"].clone()))]);
  assert_eq!(find_landing_stack(Some("auth"), |root| dry.get(root)), None);

  let robots: HashMap<&str, PatronusRobot> = HashMap::from([
    ("auth", dry_run(robot("auth", Some("auth"), PatronusRobotStatus::Running, 1))),
    ("session", robot("session", Some("session"), PatronusRobotStatus::Running, 2)),
  ]);
  let landing = find_landing_member(["auth", "login", "session"], |name| robots.get(name)).unwrap();
  assert_eq!((landing.branch.as_str(), landing.whole_stack), ("session", false));
  assert_eq!(
    start_refusal(None, Some("stack-tip"), None, Some(&landing), None, &RobotSource::WholeStack("auth").subject()).as_deref(),
    Some("A Safe Push of session is running: wait until it merges")
  );
  assert_eq!(find_landing_member(["auth", "login"], |name| robots.get(name)), None);
  // the robot of the whole stack on its last commit
  assert_eq!(
    start_refusal(Some(&stacks["auth"]), Some("stack-tip"), None, None, None, "the whole auth stack").as_deref(),
    Some("A Safe Push of the last commit is running")
  );
}

#[test]
fn a_safe_push_waits_for_a_robot_that_lands_an_ancestor_or_the_branch_itself() {
  let robots: HashMap<&str, PatronusRobot> = HashMap::from([
    ("auth", dry_run(robot("auth", Some("auth"), PatronusRobotStatus::Running, 1))),
    ("login", robot("login", Some("login"), PatronusRobotStatus::Running, 2)),
  ]);
  let robot_of = |name: &str| robots.get(name);
  let ancestors = vec!["auth".to_string(), "login".to_string()];
  let landing = find_landing_ancestor(&ancestors, robot_of, |_| None).unwrap();
  assert_eq!(
    landing,
    LandingRobot {
      branch: "login".to_string(),
      robot: &robots["login"],
      whole_stack: false,
      carried: None,
    }
  );
  assert_eq!(find_landing_ancestor(&ancestors[..1], robot_of, |_| None), None);
  assert_eq!(
    start_refusal(None, Some("tip"), None, Some(&landing), None, "oauth").as_deref(),
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
    start_refusal(None, Some("tip"), None, None, Some(&landing), "login").as_deref(),
    Some("A Safe Push of oauth is running: it lands login with it")
  );
  assert_eq!(find_landing_descendant("oauth", ["auth", "login", "oauth"], ancestors_of, robot_of), None);
  // as its content told: the commits of login the robot's commit has
  let carried = LandingRobot {
    carried: Some(CarriedCommits {
      commits: vec!["1a2b3c4d5e6f".to_string()],
      amended: Vec::new(),
    }),
    ..landing.clone()
  };
  assert_eq!(
    start_refusal(None, Some("tip"), None, None, Some(&carried), "login").as_deref(),
    Some("A Safe Push of oauth is running: it lands 1 commit of login with it (1a2b3c4d)")
  );

  // a side branch built on login waits for the robot of oauth, which lands login: it would land login a second time
  let span_of = |ancestor: &str| find_landing_descendant(ancestor, ["auth", "login", "oauth"], ancestors_of, robot_of);
  let landing = find_landing_ancestor(&["auth".to_string(), "login".to_string()], robot_of, span_of).unwrap();
  assert_eq!(landing.branch, "oauth");
  assert_eq!(
    start_refusal(None, Some("tip"), None, Some(&landing), None, "side-fix").as_deref(),
    Some("A Safe Push of oauth is running: wait until it merges")
  );
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
  let carried = CarriedCommits {
    commits: ids.clone(),
    amended: vec![ids[1].clone()],
  };
  assert_eq!(carried_words("login", None), "login");
  assert_eq!(carried_ids(None), "");
  assert_eq!(carried_words("login", Some(&carried)), "4 commits of login");
  assert_eq!(carried_ids(Some(&carried)), " (aaaaaaaa, bbbbbbbb amended since, cccccccc, …)");
}
