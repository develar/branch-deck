use crate::issue::issue_plan;
use domain_types::landing::PlannedEffect;
use domain_types::operation::Intent;
use pretty_assertions::assert_eq;

/// Mark Fixed and its undo are one intent: the change of the state field of the issue, the plan the intent's (the same
/// intent, the same fingerprint; another value, another plan)
#[test]
fn a_change_of_the_state_of_an_issue_is_its_one_effect_and_its_undo_the_same_intent() {
  let fixed = Intent::SetIssueState {
    key: "ABC-1".to_owned(),
    state: "Fixed".to_owned(),
    expected: None,
  };
  let plan = issue_plan(&fixed).unwrap();
  assert_eq!(
    plan.effects,
    [PlannedEffect::SetIssueState {
      key: "ABC-1".to_owned(),
      state: "Fixed".to_owned(),
    }]
  );
  assert_eq!(issue_plan(&fixed).unwrap().fingerprint, plan.fingerprint);
  let undo = Intent::SetIssueState {
    key: "ABC-1".to_owned(),
    state: "Open".to_owned(),
    expected: Some("Fixed".to_owned()),
  };
  assert_ne!(issue_plan(&undo).unwrap().fingerprint, plan.fingerprint);
  issue_plan(&Intent::CancelRobot { robot_id: "r1".to_owned() }).unwrap_err();
}
