use app_settings::file::SettingsFile;

/// The check of Auto-sync on Focus in the native menu: the settings as the writer loads the file (`SettingsFile`), the
/// defaults for one that does not parse (`configure_app_menu` reads `auto_sync_on_focus`)
fn loaded_auto_sync_on_focus(text: &str) -> bool {
  SettingsFile::parse(text).map_or_default(|file| file.app).auto_sync_on_focus
}

#[test]
fn auto_sync_on_focus_is_checked_as_the_stored_setting() {
  assert!(!loaded_auto_sync_on_focus(r#"{"appSettings":{"autoSyncOnFocus":false}}"#));
  assert!(loaded_auto_sync_on_focus(r#"{"appSettings":{"autoSyncOnFocus":true}}"#));
}

#[test]
fn without_a_stored_setting_auto_sync_on_focus_is_checked() {
  for store in ["{}", "", "not json", r#"{"appSettings":{}}"#, r#"{"appSettings":{"autoSyncOnFocus":"no"}}"#] {
    assert!(loaded_auto_sync_on_focus(store), "{store}");
  }
}

/// The check of Unattended Mode in the native menu as the menu is built: the mode as the writer loads the file, off for
/// one that does not parse (`configure_app_menu` reads `unattended`)
fn loaded_unattended(text: &str) -> bool {
  SettingsFile::parse(text).map_or_default(|file| file.app).unattended
}

#[test]
fn unattended_mode_is_checked_as_the_stored_mode() {
  assert!(loaded_unattended(r#"{"appSettings":{"unattended":true}}"#));
  for store in ["{}", "", "not json", r#"{"appSettings":{"unattended":false}}"#, r#"{"appSettings":{"unattended":"yes"}}"#] {
    assert!(!loaded_unattended(store), "{store}");
  }
}

/// A click of Unattended Mode checks the item back as the mode is, so a switch on that fails leaves no check, and sends
/// `unattended_mode` to the main window, which switches the mode; the main window checks the item once the setting changes
#[test]
fn a_click_of_unattended_mode_shows_the_mode_again_and_sends_the_switch_to_the_main_window() {
  assert_eq!(crate::menu::unattended_click(true), (false, "unattended_mode"));
  assert_eq!(crate::menu::unattended_click(false), (true, "unattended_mode"));
}
