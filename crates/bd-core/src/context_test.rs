use crate::context::{read_app_branch_prefix, read_app_patronus_new_api};

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

/// The app rewrites its settings file in place (the store plugin truncates, then writes) after every sync, while `bd`
/// reads the prefix beside it: every read gets the app's prefix, never silently the git one of an empty or cut file
#[test]
fn the_prefix_of_the_app_is_read_while_the_app_rewrites_its_settings_file() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("settings.json");
  // as large as the settings of an app that has seen a few repositories, so that a read lands within a write
  let recent: Vec<String> = (0..2000).map(|index| format!(r#"{{"path": "/Users/jane/projects/repository-{index}"}}"#)).collect();
  let settings = format!(r#"{{"appSettings": {{"globalUserBranchPrefix": "jane"}}, "recentProjects": [{}]}}"#, recent.join(", "));
  std::fs::write(&file, &settings).unwrap();
  let writing = std::sync::atomic::AtomicBool::new(true);

  std::thread::scope(|scope| {
    scope.spawn(|| {
      while writing.load(std::sync::atomic::Ordering::Relaxed) {
        std::fs::write(&file, &settings).unwrap();
      }
    });
    let misses = (0..300).filter(|_| read_app_branch_prefix(&file).as_deref() != Some("jane")).count();
    writing.store(false, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(misses, 0, "reads without the prefix of the app");
  });
}

/// A file that stays cut (the app stopped within its write) is ignored after the wait, until the app writes it again
#[test]
fn a_settings_file_that_stays_cut_gives_no_prefix() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("settings.json");
  std::fs::write(&file, r#"{"appSettings": {"globalUserBranchPrefix": "ja"#).unwrap();
  assert_eq!(read_app_branch_prefix(&file), None);
}
