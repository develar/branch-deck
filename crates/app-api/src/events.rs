//! The events of the app to its windows, each named once here, and to whom each goes: a host sends them as its windows
//! hear them (`AppEvents` of `transport.rs`: the app by Tauri's `Emitter`, the e2e server on the event stream of each
//! page), and a page listens by the same name (`listen` of `@tauri-apps/api/event`). The menu of the app sends its own
//! commands to the main window by their ids (`src-tauri/src/menu.rs`).

use crate::transport::{AppEvents, WindowId};
use app_settings::windows::is_main_only;
use domain_types::settings::SettingsChange;
use serde::Serialize;
use sync_core::repository_config::RepositoryConfigKey;
use tracing::warn;

/// A change the writer of the settings applied (`SettingsChange`), to every window that gets its key
/// ([Distributed Settings](../../../docs/design/distributed-settings.md))
pub const SETTINGS_CHANGED_EVENT: &str = "settings_changed";

/// A save of the settings that failed, to the main window: the message of the error
pub const SETTINGS_SAVE_FAILED_EVENT: &str = "settings_save_failed";

/// A journal Activity reads changed, to every window: a write of the page, an operation that ended (the watch of the
/// operations registry), a read that answered late (spec/repository/activity.spec.md)
pub const ACTIVITY_CHANGED_EVENT: &str = "activity-changed";

/// A sign-in or a sign-out changed an account, to every window: a window without a session reads the status again
/// (`get_auth_status`); the sessions read their fact `auth` themselves (spec/repository/github-sign-in.spec.md)
pub const AUTH_CHANGED_EVENT: &str = "auth-changed";

/// A write of Settings → Repository, to the main window: it syncs, or reloads the branch prefix
/// (spec/repository/settings.spec.md)
pub const REPOSITORY_CONFIG_CHANGED_EVENT: &str = "repository_config_changed";

/// The payload of `repository_config_changed`
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryConfigChanged {
  pub repository_path: String,
  pub key: RepositoryConfigKey,
}

/// Sends `change` to every window that gets its key: a key only the main window keeps to the main window alone
/// (`app_settings::windows::MAIN_ONLY_KEYS`)
pub fn settings_changed(events: &dyn AppEvents, change: &SettingsChange) {
  let Some(payload) = payload_of(SETTINGS_CHANGED_EVENT, change) else {
    return;
  };
  if is_main_only(&change.key) {
    events.emit_to(&WindowId::main(), SETTINGS_CHANGED_EVENT, payload);
  } else {
    events.emit(SETTINGS_CHANGED_EVENT, payload);
  }
}

/// Tells the main window that a save of the settings failed, with `message`
pub fn settings_save_failed(events: &dyn AppEvents, message: String) {
  events.emit_to(&WindowId::main(), SETTINGS_SAVE_FAILED_EVENT, serde_json::Value::String(message));
}

/// Tells every window that a journal Activity reads changed
pub fn activity_changed(events: &dyn AppEvents) {
  events.emit(ACTIVITY_CHANGED_EVENT, serde_json::Value::Null);
}

/// Tells every window that an account changed
pub fn auth_changed(events: &dyn AppEvents) {
  events.emit(AUTH_CHANGED_EVENT, serde_json::Value::Null);
}

/// Tells the main window that `key` of the config of `repository_path` was written
pub fn repository_config_changed(events: &dyn AppEvents, repository_path: String, key: RepositoryConfigKey) {
  if let Some(payload) = payload_of(REPOSITORY_CONFIG_CHANGED_EVENT, &RepositoryConfigChanged { repository_path, key }) {
    events.emit_to(&WindowId::main(), REPOSITORY_CONFIG_CHANGED_EVENT, payload);
  }
}

/// `value` as the payload of `event`; none, logged, when it has no JSON
fn payload_of(event: &str, value: &impl Serialize) -> Option<serde_json::Value> {
  serde_json::to_value(value)
    .inspect_err(|error| warn!(%error, event, "cannot send the event: its payload has no JSON"))
    .ok()
}
