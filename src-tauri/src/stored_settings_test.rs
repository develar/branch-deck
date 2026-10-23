use crate::stored_settings::stored_settings;

/// The init script of the main window always gets an object: the file as it is, else `{}` for a missing file and for
/// one the app cut within a write
#[test]
fn a_cut_settings_file_gives_the_page_an_empty_object() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("settings.json");
  assert_eq!(stored_settings(&file), "{}", "no file");

  let settings = r#"{"appSettings": {"globalUserBranchPrefix": "jane"}}"#;
  std::fs::write(&file, settings).unwrap();
  assert_eq!(stored_settings(&file), settings);

  for cut in ["", r#"{"appSettings": {"globalUserBra"#, "[1, 2]"] {
    std::fs::write(&file, cut).unwrap();
    assert_eq!(stored_settings(&file), "{}", "{cut:?}");
  }
}
