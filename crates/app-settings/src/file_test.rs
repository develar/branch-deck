use crate::file::{ParseError, SettingsFile};
use domain_types::settings::AppSettings;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;

fn write(path: &Path, value: &Value) {
  std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn read(path: &Path) -> Value {
  serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// `path` loaded and saved again
fn round_trip(path: &Path) -> Value {
  SettingsFile::load(path).save(path).unwrap();
  read(path)
}

#[test]
fn a_missing_file_is_the_defaults_and_saves_as_an_empty_object() {
  let directory = tempfile::tempdir().unwrap();
  let path = directory.path().join("missing/settings.json");
  let settings = SettingsFile::load(&path);
  assert_eq!(settings, SettingsFile::default());
  assert_eq!(settings.app.zoom.to_bits(), 1.0_f64.to_bits());
  assert!(settings.app.auto_sync_on_focus);
  assert!(!settings.app.patronus_new_api);
  assert!(!settings.app.system_title_bar);
  assert_eq!(settings.app.global_user_branch_prefix, None);
  // the directory is created
  settings.save(&path).unwrap();
  assert_eq!(read(&path), json!({}));
}

#[test]
fn an_old_file_round_trips_as_the_page_wrote_it() {
  let directory = tempfile::tempdir().unwrap();
  let path = directory.path().join("settings.json");
  // as the store plugin of the page wrote it: the non-default fields only, a cached prefix still a string (before its
  // scope was kept), the stores only the page reads
  let old = json!({
    "appSettings": { "globalUserBranchPrefix": "jane", "zoom": 1.25, "notifyApprovals": false, "settingsPage": "patronus" },
    "recentProjects": { "items": [{ "path": "/work/repo", "cachedBranchPrefix": "jane" }] },
    "repositorySettings": { "/work/repo": { "lastSyncTime": 1_700_000_000, "lastBranchCount": 3 } },
    "diffViewSettings": { "mode": "unified" },
  });
  write(&path, &old);
  let settings = SettingsFile::load(&path);
  assert_eq!(settings.app.global_user_branch_prefix.as_deref(), Some("jane"));
  assert_eq!(settings.app.zoom.to_bits(), 1.25_f64.to_bits());
  assert_eq!(round_trip(&path), old);
}

#[test]
fn a_partially_invalid_file_keeps_the_valid_fields() {
  let directory = tempfile::tempdir().unwrap();
  let path = directory.path().join("settings.json");
  write(
    &path,
    &json!({
      "appSettings": {
        "zoom": "big",
        "patronusNewApi": "yes",
        "autoSyncOnFocus": false,
        "globalUserBranchPrefix": 5,
        "systemTitleBar": null,
        "reviewSidebar": false,
      },
    }),
  );
  let settings = SettingsFile::load(&path);
  assert_eq!(
    settings.app,
    AppSettings {
      auto_sync_on_focus: false,
      other: serde_json::from_value(json!({ "reviewSidebar": false })).unwrap(),
      ..AppSettings::default()
    }
  );
  // an invalid field is its default, which the stored form leaves out
  assert_eq!(round_trip(&path), json!({ "appSettings": { "autoSyncOnFocus": false, "reviewSidebar": false } }));
}

#[test]
fn a_zoom_that_is_not_positive_is_the_default() {
  for zoom in [json!(0), json!(-1.5), json!(null)] {
    let settings = SettingsFile::parse(&json!({ "appSettings": { "zoom": zoom } }).to_string()).unwrap();
    assert_eq!(settings.app.zoom.to_bits(), 1.0_f64.to_bits(), "{zoom}");
  }
}

#[test]
fn unknown_keys_round_trip() {
  let directory = tempfile::tempdir().unwrap();
  let path = directory.path().join("settings.json");
  let file = json!({
    "appSettings": { "systemTitleBar": true, "aSwitchOfANewerApp": { "nested": [1, 2] } },
    "aStoreOfANewerApp": [true, "x"],
  });
  write(&path, &file);
  let settings = SettingsFile::load(&path);
  assert!(settings.app.system_title_bar);
  assert_eq!(round_trip(&path), file);
}

#[test]
fn app_settings_that_are_all_defaults_delete_the_key() {
  let directory = tempfile::tempdir().unwrap();
  let path = directory.path().join("settings.json");
  write(
    &path,
    &json!({
      "appSettings": { "zoom": 1, "autoSyncOnFocus": true, "patronusNewApi": false, "systemTitleBar": false },
      "recentProjects": { "items": [] },
    }),
  );
  assert_eq!(round_trip(&path), json!({ "recentProjects": { "items": [] } }));
}

#[test]
fn a_file_that_does_not_parse_is_the_defaults() {
  let directory = tempfile::tempdir().unwrap();
  let path = directory.path().join("settings.json");
  // a write cut in the middle, as the store plugin left it when the app stopped
  std::fs::write(&path, r#"{"appSettings": {"globalUserBranchPrefix": "ja"#).unwrap();
  assert_eq!(SettingsFile::load(&path), SettingsFile::default());
  assert!(matches!(SettingsFile::parse(""), Err(ParseError::Json(_))));
  assert!(matches!(SettingsFile::parse("[]"), Err(ParseError::NotAnObject("an array"))));
}

#[test]
fn app_settings_that_are_no_object_are_the_defaults_and_the_save_drops_them() {
  let settings = SettingsFile::parse(r#"{"appSettings": 5, "recentProjects": {"items": []}}"#).unwrap();
  assert_eq!(settings.app, AppSettings::default());
  assert_eq!(Value::Object(settings.to_stored()), json!({ "recentProjects": { "items": [] } }));
}

#[test]
fn a_save_replaces_the_file_and_leaves_no_temporary_file() {
  let directory = tempfile::tempdir().unwrap();
  let path = directory.path().join("settings.json");
  write(&path, &json!({ "appSettings": { "zoom": 2 } }));
  // a whole zoom is written as the page writes it
  assert_eq!(round_trip(&path), json!({ "appSettings": { "zoom": 2 } }));
  let mut settings = SettingsFile::load(&path);
  settings.app.zoom = 1.5;
  settings.app.global_user_branch_prefix = Some("jane".to_owned());
  settings.save(&path).unwrap();
  assert_eq!(read(&path), json!({ "appSettings": { "zoom": 1.5, "globalUserBranchPrefix": "jane" } }));
  let names: Vec<_> = std::fs::read_dir(directory.path()).unwrap().map(|entry| entry.unwrap().file_name()).collect();
  assert_eq!(names, ["settings.json"]);
}

#[test]
fn the_fields_of_the_repository_state_leave_the_recent_projects_by_path_and_nothing_else_changes() {
  let mut settings = SettingsFile::parse(
    r#"{"recentProjects":[{"path":"/a","lastSyncTime":1,"lastBranchCount":2,"updateTimings":{"fetch":[3]},"parkedCollapsed":true},{"lastSyncTime":4},"text",{"path":"/b"}],"other":{"lastSyncTime":5}}"#,
  )
  .unwrap();
  let taken = settings.take_repository_state();
  assert_eq!(
    Value::Object(taken),
    json!({ "/a": { "lastSyncTime": 1, "lastBranchCount": 2, "updateTimings": { "fetch": [3] } } })
  );
  assert_eq!(
    Value::Object(settings.to_stored()),
    json!({
      "recentProjects": [{ "path": "/a", "parkedCollapsed": true }, { "lastSyncTime": 4 }, "text", { "path": "/b" }],
      "other": { "lastSyncTime": 5 },
    })
  );
  assert!(settings.take_repository_state().is_empty(), "taken once");

  let mut not_a_list = SettingsFile::parse(r#"{"recentProjects":{"path":"/a","lastSyncTime":1}}"#).unwrap();
  assert!(not_a_list.take_repository_state().is_empty());
}
