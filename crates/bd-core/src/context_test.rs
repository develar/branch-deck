use crate::context::app_settings_of;

/// The setting of the app is read as `bd` starts: on only when the file says so
#[test]
fn the_new_patronus_api_setting_of_the_app_is_read_from_its_settings_file() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("settings.json");
  assert!(!app_settings_of(None).patronus_new_api, "no app data directory");
  assert!(!app_settings_of(Some(&file)).patronus_new_api, "no file");
  for (text, expected) in [
    (r#"{"appSettings": {"patronusNewApi": true}}"#, true),
    (r#"{"appSettings": {"patronusNewApi": false}}"#, false),
    (r#"{"appSettings": {"globalUserBranchPrefix": "me"}}"#, false),
    (r#"{"appSettings": {"patronusNewApi": "yes"}}"#, false),
    ("not json", false),
  ] {
    std::fs::write(&file, text).unwrap();
    assert_eq!(app_settings_of(Some(&file)).patronus_new_api, expected, "{text}");
  }
}

/// The prefix of the app is the field of its settings file, with fields of the wrong type and the keys only the page
/// reads beside it
#[test]
fn the_prefix_of_the_app_is_read_from_its_settings_file() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("settings.json");
  std::fs::write(
    &file,
    r#"{"appSettings": {"globalUserBranchPrefix": "jane", "zoom": "large", "reviewSidebar": true}, "recentProjects": []}"#,
  )
  .unwrap();
  let settings = app_settings_of(Some(&file));
  assert_eq!(settings.global_user_branch_prefix.as_deref(), Some("jane"));
  assert!((settings.zoom - 1.0).abs() < f64::EPSILON, "a zoom of the wrong type is its default");
}

/// A cut file (only a writer that does not rename leaves one) gives the defaults: no prefix of the app, so the prefix
/// comes from git and the output names that source
#[test]
fn a_settings_file_that_is_cut_gives_the_defaults() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("settings.json");
  std::fs::write(&file, r#"{"appSettings": {"globalUserBranchPrefix": "ja"#).unwrap();
  let settings = app_settings_of(Some(&file));
  assert_eq!(settings.global_user_branch_prefix, None);
  assert!(!settings.patronus_new_api);
}
