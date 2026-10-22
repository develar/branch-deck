use crate::landing::Subject;
use crate::repository_view::RobotOf;
use serde_json::json;

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
  assert!(!branch.is_whole_stack());
  assert!(stack.is_whole_stack());
}

#[test]
fn a_subject_names_itself_in_words() {
  assert_eq!(Subject::Branch { name: "auth".to_string() }.words(), "auth");
  assert_eq!(Subject::WholeStack { root: "auth".to_string() }.words(), "the whole auth stack");
}

#[test]
fn the_robot_of_a_subject_names_its_branch_and_whether_it_is_the_whole_stack() {
  let of_branch = RobotOf {
    branch: "ui".to_string(),
    robot_id: "r1".to_string(),
    whole_stack: false,
  };
  let of_stack = RobotOf {
    branch: "auth".to_string(),
    robot_id: "r1".to_string(),
    whole_stack: true,
  };
  assert_eq!(Subject::Branch { name: "ui".to_string() }.robot_of("r1"), of_branch);
  assert_eq!(Subject::WholeStack { root: "auth".to_string() }.robot_of("r1"), of_stack);
  // and back from the wire form, the flag read as the enum
  assert_eq!(of_branch.subject(), Subject::Branch { name: "ui".to_string() });
  assert_eq!(of_stack.subject(), Subject::WholeStack { root: "auth".to_string() });
}
