//! The merge of a settings file that two processes write (the app and `brd mcp` of an IDE, each with its own
//! `SettingsService`): a writer remembers what it changed and has not saved yet, by key, and by field for a field patch
//! (`Unsaved`); its save puts only that on top of the file as the other process left it (`overlay`), so a change of the
//! other process to another key or another field of the same key stays; what the file has that differs from the copy
//! in memory, beside its own unsaved changes, is the other process's change (`changed_keys`).

use crate::file::SettingsFile;
use domain_types::settings::{APP_SETTINGS_KEY, AppSettings};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// What this process changed of one key that no save has taken yet, with the version of each change
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsavedKey {
  /// The whole value: a `SettingsPatch::Value`, or a field patch after one
  Whole(u32),
  /// These fields of the object: a `SettingsPatch::Fields`
  Fields(BTreeMap<String, u32>),
}

/// The unsaved changes of one file of this process, by key
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Unsaved(BTreeMap<String, UnsavedKey>);

impl Unsaved {
  /// Remember the change `version` of `key`: of the fields `fields` (a field patch), or of the whole value (`None`)
  pub fn record(&mut self, key: &str, fields: Option<Vec<String>>, version: u32) {
    let entry = self.0.entry(key.to_owned()).or_insert_with(|| UnsavedKey::Fields(BTreeMap::new()));
    match (entry, fields) {
      (UnsavedKey::Fields(known), Some(fields)) => known.extend(fields.into_iter().map(|field| (field, version))),
      (entry, _) => *entry = UnsavedKey::Whole(version),
    }
  }

  /// Forget the changes up to `version`, which a save has on disk now; a later change of the same key or field stays
  pub fn forget_saved(&mut self, version: u32) {
    self.0.retain(|_, unsaved| match unsaved {
      UnsavedKey::Whole(changed) => *changed > version,
      UnsavedKey::Fields(fields) => {
        fields.retain(|_, changed| *changed > version);
        !fields.is_empty()
      }
    });
  }

  /// `base`, a file as another process may have left it, with the unsaved changes of this process on top: each key or
  /// field this process changed takes its value in `mine`, the copy in memory; every other key and field stays as
  /// `base` has it
  #[must_use]
  pub fn overlay(&self, mut base: SettingsFile, mine: &SettingsFile) -> SettingsFile {
    for (key, unsaved) in &self.0 {
      if key == APP_SETTINGS_KEY {
        // the fields Rust knows in their full form on both sides, so a field back at its default is a field too
        let theirs = Value::Object(base.app.to_full());
        let ours = Value::Object(mine.app.to_full());
        base.app = match overlaid(Some(theirs), Some(&ours), unsaved) {
          Some(Value::Object(stored)) => AppSettings::from_stored(stored),
          _ => AppSettings::default(),
        };
        continue;
      }
      match overlaid(base.other.get(key).cloned(), mine.other.get(key), unsaved) {
        Some(value) => base.other.insert(key.clone(), value),
        None => base.other.remove(key),
      };
    }
    base
  }
}

/// The value of a key with the change of this process on top: its own value for a whole change; for a field change the
/// other's object with its own value of each field it changed (a field it removed removed), none when no field is left
fn overlaid(theirs: Option<Value>, ours: Option<&Value>, unsaved: &UnsavedKey) -> Option<Value> {
  let fields = match unsaved {
    UnsavedKey::Whole(_) => return ours.cloned(),
    UnsavedKey::Fields(fields) => fields,
  };
  let mut object = match theirs {
    Some(Value::Object(object)) => object,
    _ => Map::new(),
  };
  let ours = ours.and_then(Value::as_object);
  for field in fields.keys() {
    match ours.and_then(|ours| ours.get(field)) {
      Some(value) => {
        object.insert(field.clone(), value.clone());
      }
      None => {
        object.remove(field);
      }
    }
  }
  (!object.is_empty()).then_some(Value::Object(object))
}

/// The keys whose value differs between `before` and `after`, `appSettings` first when its value differs, then the
/// others by name; a key one of them lacks differs unless the other lacks it too
#[must_use]
pub fn changed_keys(before: &SettingsFile, after: &SettingsFile) -> Vec<String> {
  let mut keys = Vec::new();
  if before.app != after.app {
    keys.push(APP_SETTINGS_KEY.to_owned());
  }
  let mut others: Vec<&String> = before.other.keys().chain(after.other.keys()).collect();
  others.sort_unstable();
  others.dedup();
  keys.extend(others.into_iter().filter(|key| before.other.get(*key) != after.other.get(*key)).cloned());
  keys
}
