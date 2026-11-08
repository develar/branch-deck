use crate::landing_route::{AGENT_PULL_REQUEST_OFF_UNDER_PUSH, LandingSetting};
use crate::pull_request::ViewerPermission;
use serde_json::json;

#[test]
fn the_setting_is_read_as_git_keeps_it_and_written_back_the_same() {
  for setting in [LandingSetting::Patronus, LandingSetting::Push, LandingSetting::PullRequest] {
    assert_eq!(LandingSetting::parse(setting.config_value()), Some(setting));
  }
  assert_eq!(LandingSetting::parse(" Pull-Request "), Some(LandingSetting::PullRequest));
  assert_eq!(LandingSetting::parse("merge"), None);
  assert_eq!(LandingSetting::parse(""), None);
  // the page names it as every other value of the bindings
  assert_eq!(serde_json::to_value(LandingSetting::PullRequest).unwrap(), json!("pullRequest"));
}

/// Under the Push route the work lands without a pull request: the agent rule of the pull request is off, with the reason
#[test]
fn only_the_push_route_turns_the_agent_pull_request_off() {
  let off: Vec<Option<&str>> = [LandingSetting::Patronus, LandingSetting::Push, LandingSetting::PullRequest]
    .into_iter()
    .map(LandingSetting::agent_pull_request_off)
    .collect();
  assert_eq!(off, vec![None, Some(AGENT_PULL_REQUEST_OFF_UNDER_PUSH), None]);
}

#[test]
fn a_permission_of_github_writes_from_write_and_bypasses_from_maintain() {
  let parsed: Vec<Option<ViewerPermission>> = ["READ", "TRIAGE", "WRITE", "MAINTAIN", "ADMIN", "NONE"].into_iter().map(ViewerPermission::parse).collect();
  assert_eq!(
    parsed,
    vec![
      Some(ViewerPermission::Read),
      Some(ViewerPermission::Triage),
      Some(ViewerPermission::Write),
      Some(ViewerPermission::Maintain),
      Some(ViewerPermission::Admin),
      None
    ]
  );
  let writes: Vec<bool> = parsed.iter().flatten().map(|permission| permission.can_write()).collect();
  assert_eq!(writes, vec![false, false, true, true, true]);
  let bypasses: Vec<bool> = parsed.iter().flatten().map(|permission| permission.may_bypass()).collect();
  assert_eq!(bypasses, vec![false, false, false, true, true]);
}
