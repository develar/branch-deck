//! `SettingsService`: the one writer of a settings file. Every change, from any window or from Rust, is a
//! `SettingsPatch` that `set` applies to the copy in memory under one mutex, in the order it arrives, and answers at once
//! with the `SettingsChange` (the new version and the whole key) that the caller sends to every window. The disk lags:
//! one writer thread saves the stored form (`SettingsFile::save`: only the non-default fields, a temporary file renamed
//! over the file) at most `debounce` after the first unsaved change, so a burst of changes costs one write; `flush`
//! saves now and waits (the exit of the app). A failed save keeps the value in memory, is retried by the next change or
//! flush, and is reported to `on_save_error` once per streak of failures.
//!
//! The value of `appSettings` that the windows get (`snapshot`, `SettingsChange::value`) is its full form: every field
//! Rust knows with its default (`AppSettings`), so the page takes the defaults of these fields from Rust; the file keeps
//! only what differs. A patch of a field Rust knows is validated: a value of the wrong type or out of range is refused,
//! and nothing changes.

use crate::file::SettingsFile;
use domain_types::settings::{APP_SETTINGS_KEY, AppSettings, SettingsChange, SettingsPatch};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use sync_utils::span::in_current_span;
use tracing::warn;

/// How long the writer waits after the first unsaved change before it saves, so a burst of changes is one write
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(500);

/// A save of the settings file that failed
#[derive(Debug, Clone, thiserror::Error)]
#[error("cannot save the settings to {}: {error}", path.display())]
pub struct SaveError {
  pub path: PathBuf,
  #[source]
  pub error: Arc<std::io::Error>,
}

/// Why a patch or a flush did not succeed
#[derive(Debug, Clone, thiserror::Error)]
pub enum SettingsError {
  /// A field Rust knows got a value of the wrong type or out of range; nothing changed
  #[error("{key}.{field} cannot be {value}: {expected}")]
  Invalid {
    key: String,
    field: String,
    value: String,
    expected: &'static str,
  },
  /// A field patch of a key whose value is no object, or a value of `appSettings` that is no object
  #[error("{key} is no object")]
  NotAnObject { key: String },
  /// The last save failed; the value stays in memory
  #[error(transparent)]
  Save(#[from] SaveError),
}

type SaveErrorListener = Arc<dyn Fn(&SaveError) + Send + Sync>;

/// The version of the last change of a key and who asked for it
#[derive(Debug, Clone)]
struct KeyChange {
  version: u32,
  origin: String,
}

#[derive(Default)]
struct State {
  file: SettingsFile,
  /// The version of the last applied patch; 0 for the file as it was loaded
  version: u32,
  /// The last change of each key changed since the load, a removed key too
  changes: BTreeMap<String, KeyChange>,
  /// When the first change that no save has taken yet arrived
  dirty_since: Option<Instant>,
  /// A flush waits for a save of every change before it
  flush_requested: bool,
  /// The version of the last save the writer finished, and of the last one that succeeded
  attempted: u32,
  saved: u32,
  /// The error of the last save when it failed; a failure after a success starts a new streak
  last_error: Option<SaveError>,
  shutdown: bool,
}

struct Shared {
  path: PathBuf,
  debounce: Duration,
  state: Mutex<State>,
  /// Wakes the writer: a change, a flush, the shutdown
  wake: Condvar,
  /// Wakes a flush: a save finished
  saved: Condvar,
  on_save_error: Mutex<Option<SaveErrorListener>>,
}

impl Shared {
  fn lock(&self) -> MutexGuard<'_, State> {
    self.state.lock().unwrap_or_else(PoisonError::into_inner)
  }
}

/// The one writer of a settings file (see the module documentation)
pub struct SettingsService {
  shared: Arc<Shared>,
  writer: Option<JoinHandle<()>>,
}

impl SettingsService {
  /// The writer of the settings file `path`, loaded as `SettingsFile::load` reads it (a missing or broken file is the
  /// defaults), saving `DEFAULT_DEBOUNCE` after a change
  pub fn open(path: PathBuf) -> std::io::Result<Self> {
    Self::open_with_debounce(path, DEFAULT_DEBOUNCE)
  }

  /// `open` with another delay of the save
  pub fn open_with_debounce(path: PathBuf, debounce: Duration) -> std::io::Result<Self> {
    let file = SettingsFile::load(&path);
    let shared = Arc::new(Shared {
      path,
      debounce,
      state: Mutex::new(State { file, ..State::default() }),
      wake: Condvar::new(),
      saved: Condvar::new(),
      on_save_error: Mutex::new(None),
    });
    let writer_shared = Arc::clone(&shared);
    let writer = std::thread::Builder::new()
      .name("settings-writer".to_owned())
      .spawn(in_current_span(move || write_loop(&writer_shared)))?;
    Ok(Self { shared, writer: Some(writer) })
  }

  /// The settings file this service writes
  #[must_use]
  pub fn path(&self) -> &Path {
    &self.shared.path
  }

  /// Apply `patch` from `origin` to the settings in memory and answer the change for the windows; the save follows
  /// within the debounce. A value of a field Rust knows that is invalid is refused and changes nothing
  pub fn set(&self, patch: SettingsPatch, origin: &str) -> Result<SettingsChange, SettingsError> {
    let mut state = self.shared.lock();
    let key = patch.key().to_owned();
    let value = apply_patch(&mut state.file, patch)?;
    state.version += 1;
    let version = state.version;
    state.changes.insert(
      key.clone(),
      KeyChange {
        version,
        origin: origin.to_owned(),
      },
    );
    state.dirty_since.get_or_insert_with(Instant::now);
    drop(state);
    self.shared.wake.notify_one();
    Ok(SettingsChange {
      version,
      key,
      value,
      origin: origin.to_owned(),
    })
  }

  /// Save every change made before now and wait for it; the error of that save when it failed
  pub fn flush(&self) -> Result<(), SettingsError> {
    let mut state = self.shared.lock();
    let target = state.version;
    if state.saved < target {
      state.flush_requested = true;
      self.shared.wake.notify_one();
      while state.attempted < target || state.flush_requested {
        state = self.shared.saved.wait(state).unwrap_or_else(PoisonError::into_inner);
      }
    }
    if state.saved >= target {
      return Ok(());
    }
    Err(state.last_error.clone().map_or_else(
      || {
        SettingsError::Save(SaveError {
          path: self.shared.path.clone(),
          error: Arc::new(std::io::Error::other("the save did not run")),
        })
      },
      SettingsError::Save,
    ))
  }

  /// The version of the settings and the value of every key, as a window starts with them
  #[must_use]
  pub fn snapshot(&self) -> (u32, Value) {
    let state = self.shared.lock();
    let mut values = state.file.other.clone();
    values.insert(APP_SETTINGS_KEY.to_owned(), app_value(&state.file.app));
    (state.version, Value::Object(values))
  }

  /// The last change of each key changed after `version` (a snapshot's), in the order of their versions; a removed key
  /// comes with `null`
  #[must_use]
  pub fn changes_since(&self, version: u32) -> Vec<SettingsChange> {
    let state = self.shared.lock();
    let mut changes: Vec<SettingsChange> = state
      .changes
      .iter()
      .filter(|(_, change)| change.version > version)
      .map(|(key, change)| SettingsChange {
        version: change.version,
        key: key.clone(),
        value: key_value(&state.file, key),
        origin: change.origin.clone(),
      })
      .collect();
    changes.sort_by_key(|change| change.version);
    changes
  }

  /// The settings of the app as they are now
  #[must_use]
  pub fn app(&self) -> AppSettings {
    self.shared.lock().file.app.clone()
  }

  /// Call `listener` when a save fails after a save that succeeded (or the first save), not for each retry that fails
  /// again
  pub fn on_save_error(&self, listener: impl Fn(&SaveError) + Send + Sync + 'static) {
    *self.shared.on_save_error.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(listener));
  }
}

impl Drop for SettingsService {
  /// Save what is unsaved and stop the writer
  fn drop(&mut self) {
    self.shared.lock().shutdown = true;
    self.shared.wake.notify_one();
    if let Some(writer) = self.writer.take()
      && writer.join().is_err()
    {
      warn!("the settings writer panicked");
    }
  }
}

/// The writer thread: wait for a change that is `debounce` old, a flush or the shutdown; save a copy taken under the
/// lock, outside it; record the outcome and wake the flushes
fn write_loop(shared: &Shared) {
  let mut state = shared.lock();
  loop {
    let due = state.dirty_since.map(|since| since + shared.debounce);
    let write_now = state.flush_requested || (state.shutdown && due.is_some()) || due.is_some_and(|due| due <= Instant::now());
    if !write_now {
      if state.shutdown {
        return;
      }
      state = match due {
        Some(due) => {
          shared
            .wake
            .wait_timeout(state, due.saturating_duration_since(Instant::now()))
            .unwrap_or_else(PoisonError::into_inner)
            .0
        }
        None => shared.wake.wait(state).unwrap_or_else(PoisonError::into_inner),
      };
      continue;
    }

    let version = state.version;
    state.dirty_since = None;
    state.flush_requested = false;
    if state.saved >= version {
      // a flush of what is saved already
      state.attempted = version;
      shared.saved.notify_all();
      continue;
    }
    let file = state.file.clone();
    drop(state);

    let result = file.save(&shared.path);

    state = shared.lock();
    state.attempted = state.attempted.max(version);
    let report = match result {
      Ok(()) => {
        state.saved = state.saved.max(version);
        state.last_error = None;
        None
      }
      Err(error) => {
        let error = SaveError {
          path: shared.path.clone(),
          error: Arc::new(error),
        };
        warn!(%error, version, "the settings stay in memory until a save succeeds");
        let first_of_streak = state.last_error.replace(error.clone()).is_none();
        first_of_streak.then_some(error)
      }
    };
    shared.saved.notify_all();
    if let Some(error) = report {
      drop(state);
      let listener = shared.on_save_error.lock().unwrap_or_else(PoisonError::into_inner).clone();
      if let Some(listener) = listener {
        listener(&error);
      }
      state = shared.lock();
    }
  }
}

/// Apply `patch` to `file` and answer the whole value of its key after it; on an error `file` is unchanged
fn apply_patch(file: &mut SettingsFile, patch: SettingsPatch) -> Result<Value, SettingsError> {
  if patch.key() == APP_SETTINGS_KEY {
    let stored = match patch {
      SettingsPatch::Fields { fields, .. } => {
        let mut stored = file.app.to_full();
        merge_fields(&mut stored, fields);
        stored
      }
      SettingsPatch::Value { value: Value::Null, .. } => Map::new(),
      SettingsPatch::Value { value: Value::Object(stored), .. } => stored,
      SettingsPatch::Value { key, .. } => return Err(SettingsError::NotAnObject { key }),
    };
    file.app = validated_app_settings(stored)?;
    return Ok(app_value(&file.app));
  }

  let value = match patch {
    SettingsPatch::Fields { key, fields } => {
      let mut object = match file.other.get(&key) {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(object)) => object.clone(),
        Some(_) => return Err(SettingsError::NotAnObject { key }),
      };
      merge_fields(&mut object, fields);
      // every field back at its default: the key goes, as the page stores only what differs (decision 5)
      let value = if object.is_empty() { Value::Null } else { Value::Object(object) };
      (key, value)
    }
    SettingsPatch::Value { key, value } => (key, value),
  };
  let (key, value) = value;
  if value.is_null() {
    file.other.remove(&key);
  } else {
    file.other.insert(key, value.clone());
  }
  Ok(value)
}

/// Set each field of `fields` in `object`; a `null` field is removed
fn merge_fields(object: &mut Map<String, Value>, fields: Map<String, Value>) {
  for (field, value) in fields {
    if value.is_null() {
      object.remove(&field);
    } else {
      object.insert(field, value);
    }
  }
}

/// The settings of a stored `appSettings` object, each field Rust knows checked: a boolean is a boolean, the zoom a
/// finite positive number, the prefix a string, trimmed (none when nothing is left)
fn validated_app_settings(mut stored: Map<String, Value>) -> Result<AppSettings, SettingsError> {
  let invalid = |field: &str, value: &Value, expected: &'static str| SettingsError::Invalid {
    key: APP_SETTINGS_KEY.to_owned(),
    field: field.to_owned(),
    value: value.to_string(),
    expected,
  };
  for field in ["patronusNewApi", "autoSyncOnFocus", "systemTitleBar"] {
    if let Some(value) = stored.get(field)
      && !value.is_boolean()
    {
      return Err(invalid(field, value, "a boolean"));
    }
  }
  if let Some(value) = stored.get("zoom")
    && !value.as_f64().is_some_and(|zoom| zoom.is_finite() && zoom > 0.0)
  {
    return Err(invalid("zoom", value, "a finite positive number"));
  }
  match stored.remove("globalUserBranchPrefix") {
    None => {}
    Some(Value::String(prefix)) => {
      let prefix = prefix.trim();
      if !prefix.is_empty() {
        stored.insert("globalUserBranchPrefix".to_owned(), Value::String(prefix.to_owned()));
      }
    }
    Some(value) => return Err(invalid("globalUserBranchPrefix", &value, "a string")),
  }
  Ok(AppSettings::from_stored(stored))
}

/// The value of `appSettings` the windows get: every field Rust knows, with its default (`AppSettings::to_full`)
fn app_value(app: &AppSettings) -> Value {
  Value::Object(app.to_full())
}

/// The value of `key` as the windows get it; `null` when the file has no such key
fn key_value(file: &SettingsFile, key: &str) -> Value {
  if key == APP_SETTINGS_KEY {
    app_value(&file.app)
  } else {
    file.other.get(key).cloned().unwrap_or(Value::Null)
  }
}
