use crate::projects::{selected_project, synced_before};
use crate::service::SettingsService;
use domain_types::settings::SettingsPatch;
use serde_json::{Value, json};
use std::time::Duration;

fn service_with(values: &[(&str, Value)]) -> (tempfile::TempDir, SettingsService) {
  let directory = tempfile::tempdir().unwrap();
  // no save within the test: the values are the writer's
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), Duration::from_hours(1)).unwrap();
  for (key, value) in values {
    service
      .set(
        SettingsPatch::Value {
          key: (*key).to_owned(),
          value: value.clone(),
        },
        "test",
      )
      .unwrap();
  }
  (directory, service)
}

#[test]
fn the_selected_project_is_the_first_recent_one() {
  let (_directory, service) = service_with(&[("recentProjects", json!([{ "path": "/work/a" }, { "path": "/work/b" }]))]);
  assert_eq!(selected_project(&service), Some("/work/a".to_owned()));

  let (_directory, none) = service_with(&[("recentProjects", json!([]))]);
  assert_eq!(selected_project(&none), None);
  let (_directory, unset) = service_with(&[]);
  assert_eq!(selected_project(&unset), None);
}

#[test]
fn a_project_was_synced_before_when_its_state_has_the_time_of_a_sync() {
  let state = json!({ "/work/a": { "lastSyncTime": 1_700_000_000_000_u64, "lastBranchCount": 3 }, "/work/b": { "lastBranchCount": 3 } });
  let (_directory, service) = service_with(&[("repositoryState", state)]);
  assert!(synced_before(&service, "/work/a"));
  assert!(!synced_before(&service, "/work/b"));
  assert!(!synced_before(&service, "/work/c"));
}
