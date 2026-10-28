//! The settings of the app in the windows: one `SettingsService` (`app_settings::service`) writes `settings.json` and
//! `repository-state.json` of the app data directory, and every window starts from its copy and hears its changes
//! ([Distributed Settings](../../docs/design/distributed-settings.md)).
//!
//! - a window starts from the snapshot its init script sets (`init_script`): `window.__TAURI_STORE__`, the values by key,
//!   and `window.__TAURI_STORE_VERSION__`, the version they were taken at; a sub-window's page takes the changes made
//!   after it when it loaded (`sub_window_loaded`, by the version its build remembered), the main window's page once
//!   it listens (`get_settings_changes`);
//! - a change of any window is a `set_setting` (`app_api::app::settings`), and every window gets it as
//!   `settings_changed` (`app_api::events::settings_changed`); a window applies a change newer than the version it
//!   applied;
//! - the keys only the main window keeps (`app_settings::windows::MAIN_ONLY_KEYS`) go only to the main window, and no
//!   sub-window starts with them;
//! - a save that fails reaches the main window as `settings_save_failed`, which says so.

use crate::api::TauriEvents;
use app_api::events::settings_save_failed;
use app_settings::service::SettingsService;
use app_settings::windows::window_snapshot;
use std::path::Path;
use tauri::AppHandle;

/// The writer of the settings files in `app_data_dir`, loaded now (a missing or broken file is the defaults)
pub fn open_settings(app_data_dir: &Path) -> std::io::Result<SettingsService> {
  SettingsService::open(app_data_dir.to_path_buf())
}

/// Tell the main window of a failed save, once per streak of failures (`SettingsService::on_save_error`)
pub fn report_save_errors(service: &SettingsService, app: AppHandle) {
  let events = TauriEvents(app);
  service.on_save_error(move |error| settings_save_failed(&events, error.to_string()));
}

/// The part of the init script of the window `label` that hands it the settings, before the page's own scripts, and the
/// version of that snapshot, after which a sub-window's page takes the changes when it loads (`sub_window_loaded`). The
/// values are serialized JSON, so the script always parses, a settings file cut within a write too (its snapshot is
/// the defaults)
pub fn init_script(service: &SettingsService, label: &str) -> (String, u32) {
  let (version, values) = window_snapshot(service, label);
  (format!("window.__TAURI_STORE__ = {values}; window.__TAURI_STORE_VERSION__ = {version};"), version)
}
