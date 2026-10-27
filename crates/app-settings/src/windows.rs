//! Which window gets which key of the settings: every window starts from a snapshot of the keys it gets and hears their
//! changes; the keys only the main window keeps (`MAIN_ONLY_KEYS`) reach no sub-window. One rule for the app
//! (`src-tauri/src/settings.rs`) and the E2E server

use crate::file::{LINKED_PULL_REQUESTS_KEY, REPOSITORY_STATE_KEY};
use crate::service::SettingsService;
use domain_types::settings::SettingsChange;
use serde_json::Value;

/// The label of the main window
pub const MAIN_WINDOW: &str = "main";

/// The keys only the main window reads and writes: the Activity log (`src/stores/activity.ts`), the history of the
/// robots (`src/stores/robot-history.ts`) and what the syncs record of each repository (`repositoryState`,
/// `src/stores/settings/repositoryState.ts`), which changes with every sync, and the links of Reviews, which only Rust
/// reads. The Activity window gets the log over its window link, not from the settings
pub const MAIN_ONLY_KEYS: &[&str] = &["activity", "robotHistory", REPOSITORY_STATE_KEY, LINKED_PULL_REQUESTS_KEY];

/// Whether only the main window gets the key `key`
#[must_use]
pub fn is_main_only(key: &str) -> bool {
  MAIN_ONLY_KEYS.contains(&key)
}

/// Whether the window `label` gets the key `key`
#[must_use]
pub fn window_gets(label: &str, key: &str) -> bool {
  label == MAIN_WINDOW || !is_main_only(key)
}

/// The snapshot the window `label` starts from: the version and the values of the keys it gets
#[must_use]
pub fn window_snapshot(service: &SettingsService, label: &str) -> (u32, Value) {
  let (version, mut values) = service.snapshot();
  if let Value::Object(values) = &mut values {
    values.retain(|key, _| window_gets(label, key));
  }
  (version, values)
}

/// The changes after `version` that the window `label` gets, for a page that starts to listen after its snapshot was
/// taken
#[must_use]
pub fn window_changes_since(service: &SettingsService, label: &str, version: u32) -> Vec<SettingsChange> {
  let mut changes = service.changes_since(version);
  changes.retain(|change| window_gets(label, &change.key));
  changes
}
