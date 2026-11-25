use crate::push_guard::{own_hold_words, span_hold_words, stack_hold_words};
use domain_types::fixtures::robot;
use domain_types::landing::Subject;
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use pretty_assertions::assert_eq;

/// The words of a hold name who started a robot of someone else (a reviewer's Safe Merge from the merge request), after
/// what it is of; of the user's own robot nobody
#[test]
fn the_hold_words_name_who_started_a_robot_of_someone_else() {
  let mine = PatronusRobot {
    url: "https://patronus/robot/r1".to_string(),
    ..robot("r1", Some("auth"), PatronusRobotStatus::Running, 1)
  };
  let theirs = PatronusRobot {
    started_by: Some("Jane Doe".to_string()),
    ..mine.clone()
  };
  assert_eq!(
    own_hold_words("auth", &mine, None, "restart it on the current branch"),
    "A Safe Push of auth is running: a push moves the branch it took its commit from, so it won't land and fails the Safe Push at the end. Wait until it merges, or restart it on the current branch (https://patronus/robot/r1)"
  );
  assert_eq!(
    own_hold_words("auth", &theirs, Some("test/virtual/auth"), "restart it on the current branch"),
    "A Safe Push of auth that Jane Doe started is running on test/virtual/auth: a push moves the branch it took its commit from, so it won't land and fails the Safe Push at the end. Wait until it merges, or restart it on the current branch (https://patronus/robot/r1)"
  );
  assert!(
    span_hold_words("auth", &Subject::branch("login"), &theirs, None).starts_with("A Safe Push of login that Jane Doe started is running: it lands auth with it"),
    "{}",
    span_hold_words("auth", &Subject::branch("login"), &theirs, None)
  );
  assert!(stack_hold_words("auth", &theirs).contains("that Jane Doe started is running on it"));
}
