use crate::landing::{Land, ProfileChoice, Subject};
use crate::operation::{Intent, OperationId, OperationStep, Push, PushScope, Resource, RunningMove};
use crate::patronus::{LandingAllowances, PatronusPushMode};
use serde_json::json;

#[test]
fn an_intent_serializes_as_its_kind_with_the_fields_of_its_family() {
  let land = Intent::Land(Land {
    subject: Subject::Branch { name: "auth".to_string() },
    mode: PatronusPushMode::RebaseAutosquash,
    profile: ProfileChoice::Default,
    without_approvals: false,
    create_pull_request: false,
    allow: LandingAllowances::default(),
  });
  let value = serde_json::to_value(&land).unwrap();
  assert_eq!(value["kind"], json!("land"));
  assert_eq!(value["subject"], json!({ "kind": "branch", "name": "auth" }));
  assert_eq!(serde_json::from_value::<Intent>(value).unwrap(), land);

  let push = Intent::Push(Push {
    subject: Subject::WholeStack { root: "air".to_string() },
    scope: PushScope::UpTo { commit: "c1".to_string() },
    expect_remote: None,
    force: false,
    restart_running: true,
  });
  assert_eq!(
    serde_json::to_value(&push).unwrap(),
    json!({
      "kind": "push",
      "subject": { "kind": "wholeStack", "root": "air" },
      "scope": { "kind": "upTo", "commit": "c1" },
      "expectRemote": null,
      "force": false,
      "restartRunning": true,
    })
  );
  assert_eq!(
    serde_json::to_value(Intent::RestartRobot { robot_id: "r1".to_string() }).unwrap(),
    json!({ "kind": "restartRobot", "robotId": "r1" })
  );
}

#[test]
fn an_operation_id_is_its_text_and_a_resource_its_kind_and_key() {
  let id = OperationId("01JAX".to_string());
  assert_eq!(serde_json::to_value(&id).unwrap(), json!("01JAX"));
  assert_eq!(id.to_string(), "01JAX");
  assert_eq!(
    serde_json::to_value(OperationStep::Queued { behind: id }).unwrap(),
    json!({ "kind": "queued", "behind": "01JAX" })
  );
  assert_eq!(
    serde_json::to_value(Resource::Ref {
      name: "user/virtual/auth".to_string()
    })
    .unwrap(),
    json!({ "kind": "ref", "name": "user/virtual/auth" })
  );
}

#[test]
fn a_running_move_names_its_row() {
  let review = RunningMove::RequestingReview {
    subject: "stack:air".to_string(),
    number: 12,
  };
  assert_eq!(review.subject(), "stack:air");
  assert_eq!(
    serde_json::to_value(&review).unwrap(),
    json!({ "kind": "requestingReview", "subject": "stack:air", "number": 12 })
  );
}
