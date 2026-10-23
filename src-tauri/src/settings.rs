//! The settings of the app in the windows: one `SettingsService` (`app_settings::service`) writes `settings.json` of the
//! app data directory, and every window starts from its copy and hears its changes
//! ([Distributed Settings](../../docs/design/distributed-settings.md)).
//!
//! - a window starts from the snapshot its init script sets (`init_script`): `window.__TAURI_STORE__`, the values by key,
//!   and `window.__TAURI_STORE_VERSION__`, the version they were taken at;
//! - a change of any window is a `set_setting` (`commands::settings`), and every window gets it as `settings_changed`
//!   (`emit_change`); a window applies a change newer than the version it applied;
//! - the keys only the main window keeps (`app_settings::windows::MAIN_ONLY_KEYS`) go only to the main window, and no
//!   sub-window starts with them;
//! - a save that fails reaches the main window as `settings_save_failed`, which says so.

use app_settings::file::SETTINGS_FILE_NAME;
use app_settings::service::SettingsService;
use app_settings::windows::{MAIN_WINDOW, is_main_only, window_snapshot};
use domain_types::settings::SettingsChange;
use std::path::Path;
use tauri::{AppHandle, Emitter, Runtime};
use tracing::warn;

/// The event of every applied change (`SettingsChange`), to every window
pub const SETTINGS_CHANGED_EVENT: &str = "settings_changed";

/// The event of a save of the settings that failed, to the main window: the message of the error
pub const SETTINGS_SAVE_FAILED_EVENT: &str = "settings_save_failed";

/// The writer of `settings.json` in `app_data_dir`, loaded now (a missing or broken file is the defaults)
pub fn open_settings(app_data_dir: &Path) -> std::io::Result<SettingsService> {
  SettingsService::open(app_data_dir.join(SETTINGS_FILE_NAME))
}

/// Tell the main window of a failed save, once per streak of failures (`SettingsService::on_save_error`)
pub fn report_save_errors<R: Runtime>(service: &SettingsService, app: AppHandle<R>) {
  service.on_save_error(move |error| {
    if let Err(e) = app.emit_to(MAIN_WINDOW, SETTINGS_SAVE_FAILED_EVENT, error.to_string()) {
      warn!(error = %e, "cannot tell the main window that the settings were not saved");
    }
  });
}

/// The part of the init script of the window `label` that hands it the settings, before the page's own scripts. The
/// values are serialized JSON, so the script always parses, a settings file cut within a write too (its snapshot is
/// the defaults)
pub fn init_script(service: &SettingsService, label: &str) -> String {
  let (version, values) = window_snapshot(service, label);
  format!("window.__TAURI_STORE__ = {values}; window.__TAURI_STORE_VERSION__ = {version};")
}

/// Send `change` to every window that gets its key
pub fn emit_change<R: Runtime>(app: &AppHandle<R>, change: &SettingsChange) {
  let sent = if is_main_only(&change.key) {
    app.emit_to(MAIN_WINDOW, SETTINGS_CHANGED_EVENT, change)
  } else {
    app.emit(SETTINGS_CHANGED_EVENT, change)
  };
  if let Err(e) = sent {
    warn!(error = %e, key = change.key, "cannot send the change of the settings to the windows");
  }
}
