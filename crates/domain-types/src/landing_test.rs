use crate::landing::Subject;
use crate::patronus::PatronusRobot;
use crate::repository_view::RobotOf;
use serde_json::json;

fn robot(id: &str) -> PatronusRobot {
  serde_json::from_value(json!({
    "id": id,
    "name": "feat: login",
    "status": "running",
    "pushMode": "rebaseAutosquash",
    "branchName": null,
    "sourceRevision": null,
    "targetBranch": "master",
    "startTime": 1,
    "finishTime": null,
    "cancellationReason": null,
    "url": "https://patronus.test/robots/1",
  }))
  .expect("a robot")
}

#[test]
fn a_subject_serializes_as_its_kind_and_name() {
  let branch = Subject::Branch { name: "ui".to_string() };
  let stack = Subject::WholeStack { root: "auth".to_string() };
  assert_eq!(serde_json::to_value(&branch).unwrap(), json!({ "kind": "branch", "name": "ui" }));
  assert_eq!(serde_json::to_value(&stack).unwrap(), json!({ "kind": "wholeStack", "root": "auth" }));
  // and back: the page may send a subject it was given
  assert_eq!(serde_json::from_value::<Subject>(json!({ "kind": "branch", "name": "ui" })).unwrap(), branch);
  assert_eq!(serde_json::from_value::<Subject>(json!({ "kind": "wholeStack", "root": "auth" })).unwrap(), stack);
  assert_eq!(branch.name(), "ui");
  assert_eq!(stack.name(), "auth");
}

#[test]
fn the_robot_of_a_subject_names_its_branch_and_whether_it_is_the_whole_stack() {
  let robot = robot("r1");
  assert_eq!(
    Subject::Branch { name: "ui".to_string() }.robot_of(&robot),
    RobotOf {
      branch: "ui".to_string(),
      robot_id: "r1".to_string(),
      whole_stack: false,
    }
  );
  assert_eq!(
    Subject::WholeStack { root: "auth".to_string() }.robot_of(&robot),
    RobotOf {
      branch: "auth".to_string(),
      robot_id: "r1".to_string(),
      whole_stack: true,
    }
  );
}
