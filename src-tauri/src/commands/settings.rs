//! The commands of the settings: a window changes them (`set_setting`), and the main window catches up after it starts
//! to listen (`get_settings_changes`; a sub-window gets its catch-up from `sub_window_loaded`); the writer and the
//! events are `crate::settings`

use crate::operation::operation_now;
use crate::settings::emit_change;
use app_api::api_error::api_error;
use app_settings::service::SettingsService;
use app_settings::windows::window_changes_since;
use domain_types::error::ApiError;
use domain_types::settings::{SettingsChange, SettingsPatch};
use serde::Deserialize;
use specta::Type;
use tauri::{AppHandle, State, Webview};
use tracing::instrument;

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetSettingParams {
  pub patch: SettingsPatch,
}

/// Applies the change of the calling window to the settings and sends it to every window (`settings_changed`, with the
/// window's label as its origin); answers the change, whose version the window has applied then. The file follows
/// within the debounce of the writer. A value of a field Rust reads that is invalid is refused and changes nothing
#[tauri::command]
#[specta::specta]
#[instrument(skip(app, webview, service, params), fields(window = webview.label(), key = params.patch.key()))]
#[expect(clippy::needless_pass_by_value, reason = "tauri passes the app handle, the webview and the state of a command by value")]
pub fn set_setting(app: AppHandle, webview: Webview, service: State<'_, SettingsService>, params: SetSettingParams) -> Result<SettingsChange, ApiError> {
  operation_now(|| {
    let change = service
      .set(params.patch, webview.label())
      .map_err(|e| api_error(anyhow::Error::new(e).context("Cannot change the settings")))?;
    emit_change(&app, &change);
    Ok(change)
  })
}

/// The changes the calling window gets that came after `since`, the version of the snapshot its page started from: a
/// change made while the page loaded reached no listener. The main window's page asks once it listens (its init script
/// has the snapshot of the app's start, a reload too); a sub-window's gets them with its load (`sub_window_loaded`)
#[tauri::command]
#[specta::specta]
#[instrument(skip(webview, service), fields(window = webview.label()))]
#[expect(clippy::needless_pass_by_value, reason = "tauri passes the webview and the state of a command by value")]
pub fn get_settings_changes(webview: Webview, service: State<'_, SettingsService>, since: u32) -> Result<Vec<SettingsChange>, ApiError> {
  operation_now(|| Ok(window_changes_since(&service, webview.label(), since)))
}
