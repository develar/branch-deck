use crate::robot_history::{ROBOT_HISTORY_KEY, forget_robot_history, legacy_robot_history};
use crate::service::SettingsService;
use domain_types::patronus::{FinishedRobot, PatronusPushMode, PatronusRobotStatus, SampleProfile};
use domain_types::settings::SettingsPatch;
use pretty_assertions::assert_eq;
use serde_json::json;

fn robot(id: &str, profile: SampleProfile) -> FinishedRobot {
  FinishedRobot {
    id: id.to_owned(),
    status: PatronusRobotStatus::Success,
    push_mode: PatronusPushMode::RebaseAutosquash,
    target_branch: "master".to_owned(),
    start_time: 100,
    finish_time: 2500,
    profile,
  }
}

/// The entries the main window wrote come back by repository, with a profile known, known as none and not known; an
/// entry of another form is left out; once forgotten the key is gone and a second forget changes nothing
#[test]
fn the_history_of_the_main_window_is_read_by_repository_and_then_forgotten() {
  let dir = tempfile::tempdir().unwrap();
  let service = SettingsService::open_with_debounce(dir.path().to_path_buf(), std::time::Duration::from_hours(1)).unwrap();
  assert_eq!(legacy_robot_history(&service), None);
  let entry = |id: &str, repository: &str| json!({ "id": id, "repositoryPath": repository, "pushMode": "rebaseAutosquash", "targetBranch": "master", "startTime": 100, "finishTime": 2500, "status": "success" });
  let mut with_profile = entry("lsc", "/a");
  with_profile["profileId"] = json!("lsc");
  let mut without = entry("none", "/b");
  without["profileId"] = json!(null);
  let history = json!([entry("unknown", "/a"), with_profile, without, { "id": "broken", "repositoryPath": "/a" }]);
  let patch = SettingsPatch::Value {
    key: ROBOT_HISTORY_KEY.to_owned(),
    value: history,
  };
  service.set(patch, "main").unwrap();

  let read = legacy_robot_history(&service).unwrap();
  assert_eq!(
    read.get("/a"),
    Some(&vec![robot("unknown", SampleProfile::Unknown), robot("lsc", SampleProfile::Id("lsc".to_owned()))])
  );
  assert_eq!(read.get("/b"), Some(&vec![robot("none", SampleProfile::None)]));

  let change = forget_robot_history(&service).unwrap().expect("a change for the windows");
  assert_eq!(change.key, ROBOT_HISTORY_KEY);
  assert!(service.value(ROBOT_HISTORY_KEY).is_null());
  assert_eq!(legacy_robot_history(&service), None);
  assert_eq!(forget_robot_history(&service).unwrap(), None);
}
