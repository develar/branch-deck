//! The settings as the bridge knows them (openspec/specs/repository/ide-embedding/spec.md, "Repositories"): the
//! backend holds the one writer of the settings, so the bridge reads the few keys of its own rules (the recent projects
//! and the root picked last, the switch of the notifications) from the snapshot of the main window
//! (`x-branch-deck/bootstrap`) and from each `settings_changed` after it, each key at the newest version it heard; the
//! bridge writes through the backend (`set_setting`).

use serde_json::{Map, Value};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The values of the settings, each with the version it was heard at
#[derive(Default)]
pub struct SettingsMirror(Mutex<Map<String, Value>>);

/// A value and its version
fn entry(value: &Value, version: u64) -> Value {
  serde_json::json!({ "value": value, "version": version })
}

impl SettingsMirror {
  fn values(&self) -> MutexGuard<'_, Map<String, Value>> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// The value of `key`, null when the bridge heard none
  pub fn value(&self, key: &str) -> Value {
    self.values().get(key).map_or(Value::Null, |entry| entry["value"].clone())
  }

  /// A snapshot of the settings at `version`: each key whose value the bridge heard at a newer version keeps it
  pub fn snapshot(&self, values: &Value, version: u64) {
    let Some(values) = values.as_object() else {
      return;
    };
    let mut known = self.values();
    for (key, value) in values {
      if known.get(key).and_then(|entry| entry["version"].as_u64()).is_none_or(|heard| heard <= version) {
        known.insert(key.clone(), entry(value, version));
      }
    }
  }

  /// A change of the settings (`SettingsChange`, as its JSON): its key takes its value, unless it heard a newer one
  pub fn changed(&self, change: &Value) {
    let (Some(key), Some(version)) = (change["key"].as_str(), change["version"].as_u64()) else {
      return;
    };
    let mut known = self.values();
    if known.get(key).and_then(|entry| entry["version"].as_u64()).is_none_or(|heard| heard <= version) {
      known.insert(key.to_owned(), entry(&change["value"], version));
    }
  }
}

#[cfg(test)]
#[path = "settings_test.rs"]
mod settings_test;
