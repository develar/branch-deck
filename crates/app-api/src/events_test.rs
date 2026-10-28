//! To whom each event of the app goes (`events.rs`), over a host that records what it was told

use crate::events::{ACTIVITY_CHANGED_EVENT, REPOSITORY_CONFIG_CHANGED_EVENT, SETTINGS_CHANGED_EVENT, activity_changed, repository_config_changed, settings_changed};
use crate::transport::{AppEvents, WindowId};
use domain_types::settings::SettingsChange;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::Mutex;
use sync_core::repository_config::RepositoryConfigKey;

/// The events a host was told: the window of each (none: every window), its name and its payload
#[derive(Default)]
struct Told(Mutex<Vec<(Option<String>, String, Value)>>);

impl AppEvents for Told {
  fn emit(&self, event: &str, payload: Value) {
    self.0.lock().unwrap().push((None, event.to_owned(), payload));
  }

  fn emit_to(&self, window: &WindowId, event: &str, payload: Value) {
    self.0.lock().unwrap().push((Some(window.to_string()), event.to_owned(), payload));
  }
}

impl Told {
  fn taken(&self) -> Vec<(Option<String>, String, Value)> {
    std::mem::take(&mut self.0.lock().unwrap())
  }
}

fn change(key: &str) -> SettingsChange {
  SettingsChange {
    version: 7,
    key: key.to_owned(),
    value: json!({ "zoom": 1.1 }),
    origin: "settings".to_owned(),
  }
}

/// A change of a key every window keeps goes to every window, one the main window alone keeps to the main window alone
#[test]
fn a_change_of_the_settings_goes_to_the_windows_that_get_its_key() {
  let told = Told::default();
  settings_changed(&told, &change("appSettings"));
  settings_changed(&told, &change("repositoryState"));
  let payload = |key: &str| json!({ "version": 7, "key": key, "value": { "zoom": 1.1 }, "origin": "settings" });
  assert_eq!(
    told.taken(),
    [
      (None, SETTINGS_CHANGED_EVENT.to_owned(), payload("appSettings")),
      (Some("main".to_owned()), SETTINGS_CHANGED_EVENT.to_owned(), payload("repositoryState")),
    ]
  );
}

/// A write of the repository config goes to the main window with its repository and its key, a change of a journal to
/// every window
#[test]
fn a_config_write_goes_to_the_main_window_and_a_journal_change_to_every_window() {
  let told = Told::default();
  repository_config_changed(&told, "/repo".to_owned(), RepositoryConfigKey::BranchPrefix);
  activity_changed(&told);
  assert_eq!(
    told.taken(),
    [
      (
        Some("main".to_owned()),
        REPOSITORY_CONFIG_CHANGED_EVENT.to_owned(),
        json!({ "repositoryPath": "/repo", "key": "branchPrefix" })
      ),
      (None, ACTIVITY_CHANGED_EVENT.to_owned(), Value::Null),
    ]
  );
}
