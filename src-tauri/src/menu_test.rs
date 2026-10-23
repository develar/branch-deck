use app_settings::file::SettingsFile;

/// The check of Auto-sync on Focus in the native menu: the settings as the writer loads the file (`SettingsFile`), the
/// defaults for one that does not parse (`configure_app_menu` reads `auto_sync_on_focus`)
fn stored_auto_sync_on_focus(text: &str) -> bool {
  SettingsFile::parse(text).map_or_default(|file| file.app).auto_sync_on_focus
}

#[test]
fn auto_sync_on_focus_is_checked_as_the_stored_setting() {
  assert!(!stored_auto_sync_on_focus(r#"{"appSettings":{"autoSyncOnFocus":false}}"#));
  assert!(stored_auto_sync_on_focus(r#"{"appSettings":{"autoSyncOnFocus":true}}"#));
}

#[test]
fn without_a_stored_setting_auto_sync_on_focus_is_checked() {
  for store in ["{}", "", "not json", r#"{"appSettings":{}}"#, r#"{"appSettings":{"autoSyncOnFocus":"no"}}"#] {
    assert!(stored_auto_sync_on_focus(store), "{store}");
  }
}
