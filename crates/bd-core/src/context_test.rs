use crate::context::read_app_patronus_new_api;

/// The setting of the app is read as `bd` starts: on only when the file says so
#[test]
fn the_new_patronus_api_setting_of_the_app_is_read_from_its_settings_file() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("settings.json");
  assert!(!read_app_patronus_new_api(&file), "no file");
  for (text, expected) in [
    (r#"{"appSettings": {"patronusNewApi": true}}"#, true),
    (r#"{"appSettings": {"patronusNewApi": false}}"#, false),
    (r#"{"appSettings": {"globalUserBranchPrefix": "me"}}"#, false),
    (r#"{"appSettings": {"patronusNewApi": "yes"}}"#, false),
    ("not json", false),
  ] {
    std::fs::write(&file, text).unwrap();
    assert_eq!(read_app_patronus_new_api(&file), expected, "{text}");
  }
}
