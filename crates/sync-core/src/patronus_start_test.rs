use crate::patronus_start::{allows_push_mode, carried_ids, carried_words, is_push_into_robot_risky};
use domain_types::fixtures::{robot, stack_robot};
use domain_types::landing::Subject;
use domain_types::patronus::{PatronusProfile, PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::repository_view::Carried;
use pretty_assertions::assert_eq;

fn branch(name: &str) -> Subject {
  Subject::Branch { name: name.to_string() }
}

fn whole_stack(root: &str) -> Subject {
  Subject::WholeStack { root: root.to_string() }
}

fn dry_run(robot: PatronusRobot) -> PatronusRobot {
  PatronusRobot {
    push_mode: PatronusPushMode::DryRun,
    ..robot
  }
}

#[test]
fn a_push_into_the_branch_of_a_running_safe_push_is_risky_a_dry_run_or_another_branch_is_not() {
  let running = robot("r", Some("IJPL-1"), PatronusRobotStatus::Running, 1);
  assert!(is_push_into_robot_risky(&running, &branch("IJPL-1")));
  assert!(!is_push_into_robot_risky(&dry_run(running.clone()), &branch("IJPL-1")));
  assert!(!is_push_into_robot_risky(&robot("r", None, PatronusRobotStatus::Running, 1), &branch("IJPL-1")));
  // a push of the stack branch fails the robot of the whole stack, and only it
  let whole = stack_robot("s", "IJPL-1", PatronusRobotStatus::Running, "stack-tip");
  assert!(is_push_into_robot_risky(&whole, &whole_stack("IJPL-1")));
  assert!(!is_push_into_robot_risky(&whole, &branch("IJPL-1")));
  assert!(!is_push_into_robot_risky(&running, &whole_stack("IJPL-1")));
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
