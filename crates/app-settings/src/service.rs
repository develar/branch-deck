//! `SettingsService`: the one writer of the settings files of the app data directory. Every change, from any window or
//! from Rust, is a `SettingsPatch` that `set` applies to the copy in memory under one mutex, in the order it arrives,
//! and answers at once with the `SettingsChange` (the new version and the whole key) that the caller sends to every
//! window. A key lives in one file (`StoreFile::of`): `repositoryState`, which every sync, fetch and update changes, in
//! `repository-state.json`, every other key in `settings.json`, which so changes only when the user changes a setting or
//! the list of projects; the versions count the changes of both. The disk lags: one writer thread saves the stored form of each changed file
//! (`SettingsFile::save`: only the non-default fields, a temporary file renamed over the file) at most `debounce` after
//! the first unsaved change, so a burst of changes costs one write, and a file nothing changed is not written; `flush`
//! saves now and waits (the exit of the app). A failed save keeps the value in memory and is retried by itself, first
//! `debounce` after it, each next time after twice the delay before, at most `MAX_RETRY_DELAY` (or `debounce`, if
//! longer) apart, and by the next change or flush; it is reported to `on_save_error` once per streak of failures.
//!
//! The value of `appSettings` that the windows get (`snapshot`, `SettingsChange::value`) is its full form: every field
//! Rust knows with its default (`AppSettings`), so the page takes the defaults of these fields from Rust; the file keeps
//! only what differs. A patch of a field Rust knows is validated: a value of the wrong type or out of range is refused,
//! and nothing changes.
//!
//! A `settings.json` of before keeps the fields of `repositoryState` in its recent projects: the load moves them
//! (`SettingsFile::take_repository_state`) into `repository-state.json`, which is saved within the debounce, and
//! `settings.json` loses them with its next save, not before.

use crate::file::{REPOSITORY_STATE_FILE_NAME, REPOSITORY_STATE_KEY, SETTINGS_FILE_NAME, SettingsFile};
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

/// The longest delay between two retries of a failed save, unless the debounce is longer
pub const MAX_RETRY_DELAY: Duration = Duration::from_mins(1);

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

/// A file of the app data directory the service writes; each key lives in one
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoreFile {
  Settings,
  RepositoryState,
}

impl StoreFile {
  /// The order of a save: the state first, so values a load moved out of `settings.json` are on disk before a save of
  /// `settings.json` drops them there
  const SAVE_ORDER: [Self; 2] = [Self::RepositoryState, Self::Settings];

  /// The file of `key`: `repositoryState` in `repository-state.json`, every other key in `settings.json`
  fn of(key: &str) -> Self {
    if key == REPOSITORY_STATE_KEY { Self::RepositoryState } else { Self::Settings }
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Settings => SETTINGS_FILE_NAME,
      Self::RepositoryState => REPOSITORY_STATE_FILE_NAME,
    }
  }
}

/// A file in memory, with the versions of its last change and of its last save
#[derive(Default)]
struct StoredFile {
  content: SettingsFile,
  /// The version of the last change of the file: none while nothing changed it since the load, `Some(0)` for values
  /// the load moved into it
  changed: Option<u32>,
  /// The version of the last change the last successful save had; none before the first
  saved: Option<u32>,
}

impl StoredFile {
  fn is_unsaved(&self) -> bool {
    self.changed > self.saved
  }
}

#[derive(Default)]
struct State {
  settings_file: StoredFile,
  repository_state_file: StoredFile,
  /// The version of the last applied patch; 0 for the files as they were loaded
  version: u32,
  /// The last change of each key changed since the load, a removed key too
  changes: BTreeMap<String, KeyChange>,
  /// When the first change that no save has taken yet arrived
  dirty_since: Option<Instant>,
  /// When the writer tries a failed save again by itself; none while the last pass saved everything
  retry_at: Option<Instant>,
  /// The delay before `retry_at`, which doubles with each failure of a streak
  retry_delay: Duration,
  /// A flush waits for a save of every change before it
  flush_requested: bool,
  /// The passes of the writer that started and that finished: a flush waits for a pass that started after it asked
  passes_started: u64,
  passes_finished: u64,
  /// The error of the last failed save; a pass whose saves all succeed ends the streak
  last_error: Option<SaveError>,
  shutdown: bool,
}

impl State {
  const fn file(&self, file: StoreFile) -> &StoredFile {
    match file {
      StoreFile::Settings => &self.settings_file,
      StoreFile::RepositoryState => &self.repository_state_file,
    }
  }

  const fn file_mut(&mut self, file: StoreFile) -> &mut StoredFile {
    match file {
      StoreFile::Settings => &mut self.settings_file,
      StoreFile::RepositoryState => &mut self.repository_state_file,
    }
  }

  /// The files as `SettingsFile::load` reads them from `directory`; the fields of `repositoryState` that a
  /// `settings.json` of before keeps in its recent projects move into `repository-state.json`, where a value of the
  /// same repository wins, being newer, and that file is due for a save
  fn load(directory: &Path) -> Self {
    let mut settings = SettingsFile::load(&directory.join(SETTINGS_FILE_NAME));
    settings.other.remove(REPOSITORY_STATE_KEY);
    let mut repository_state = SettingsFile::load(&directory.join(REPOSITORY_STATE_FILE_NAME));
    repository_state.app = AppSettings::default();
    repository_state.other.retain(|key, _| key == REPOSITORY_STATE_KEY);

    let moved = settings.take_repository_state();
    let mut state = Self::default();
    if !moved.is_empty() {
      let entries = repository_state.other.entry(REPOSITORY_STATE_KEY).or_insert_with(|| Value::Object(Map::new()));
      if !entries.is_object() {
        *entries = Value::Object(Map::new());
      }
      if let Value::Object(entries) = entries {
        for (path, fields) in moved {
          entries.entry(path).or_insert(fields);
        }
      }
      state.repository_state_file.changed = Some(0);
      state.dirty_since = Some(Instant::now());
    }
    state.settings_file.content = settings;
    state.repository_state_file.content = repository_state;
    state
  }
}

struct Shared {
  /// The app data directory with the files
  directory: PathBuf,
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

  fn path_of(&self, file: StoreFile) -> PathBuf {
    self.directory.join(file.name())
  }
}

/// The one writer of the settings files (see the module documentation)
pub struct SettingsService {
  shared: Arc<Shared>,
  writer: Option<JoinHandle<()>>,
}

impl SettingsService {
  /// The writer of the settings files in the app data directory `directory` (`settings.json` and
  /// `repository-state.json`), loaded as `SettingsFile::load` reads them (a missing or broken file is the defaults),
  /// saving `DEFAULT_DEBOUNCE` after a change
  pub fn open(directory: PathBuf) -> std::io::Result<Self> {
    Self::open_with_debounce(directory, DEFAULT_DEBOUNCE)
  }

  /// `open` with another delay of the save
  pub fn open_with_debounce(directory: PathBuf, debounce: Duration) -> std::io::Result<Self> {
    let state = State::load(&directory);
    let shared = Arc::new(Shared {
      directory,
      debounce,
      state: Mutex::new(state),
      wake: Condvar::new(),
      saved: Condvar::new(),
      on_save_error: Mutex::new(None),
    });
    let writer_shared = Arc::clone(&shared);
    let writer = std::thread::Builder::new()
      .name("settings-writer".to_owned())
      .spawn(in_current_span(move || write_loop(&writer_shared)))?;
    // values the load moved wait for the debounce, as a change does
    shared.wake.notify_one();
    Ok(Self { shared, writer: Some(writer) })
  }

  /// Apply `patch` from `origin` to the settings in memory and answer the change for the windows; the save of the
  /// file of its key follows within the debounce. A value of a field Rust knows that is invalid is refused and changes
  /// nothing
  pub fn set(&self, patch: SettingsPatch, origin: &str) -> Result<SettingsChange, SettingsError> {
    let mut state = self.shared.lock();
    let key = patch.key().to_owned();
    let file = StoreFile::of(&key);
    let value = apply_patch(&mut state.file_mut(file).content, patch)?;
    state.version += 1;
    let version = state.version;
    state.file_mut(file).changed = Some(version);
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
    let needed = StoreFile::SAVE_ORDER.map(|file| (file, state.file(file).changed));
    if needed.iter().all(|&(file, _)| !state.file(file).is_unsaved()) {
      return Ok(());
    }
    state.flush_requested = true;
    let started_before = state.passes_started;
    self.shared.wake.notify_one();
    // the passes run one after another: once the one after those started before the request finished, it has saved
    // every change of now
    while state.passes_finished <= started_before {
      state = self.shared.saved.wait(state).unwrap_or_else(PoisonError::into_inner);
    }
    let failed = needed.iter().find(|&&(file, changed)| state.file(file).saved < changed);
    let Some(&(file, _)) = failed else {
      return Ok(());
    };
    Err(state.last_error.clone().map_or_else(
      || {
        SettingsError::Save(SaveError {
          path: self.shared.path_of(file),
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
    let mut values = state.settings_file.content.other.clone();
    values.insert(APP_SETTINGS_KEY.to_owned(), app_value(&state.settings_file.content.app));
    values.extend(state.repository_state_file.content.other.clone());
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
        value: key_value(&state, key),
        origin: change.origin.clone(),
      })
      .collect();
    changes.sort_by_key(|change| change.version);
    changes
  }

  /// The settings of the app as they are now
  #[must_use]
  pub fn app(&self) -> AppSettings {
    self.shared.lock().settings_file.content.app.clone()
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

/// The writer thread: wait for a change that is `debounce` old, a flush or the shutdown; save a copy of each changed
/// file taken under the lock, outside it; record the outcome and wake the flushes
fn write_loop(shared: &Shared) {
  let mut state = shared.lock();
  loop {
    let due = match (state.dirty_since.map(|since| since + shared.debounce), state.retry_at) {
      (Some(changed), Some(retry)) => Some(changed.min(retry)),
      (changed, retry) => changed.or(retry),
    };
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

    state.passes_started += 1;
    state.dirty_since = None;
    state.retry_at = None;
    state.flush_requested = false;
    #[expect(clippy::needless_collect, reason = "the copies are taken under the lock, the saves run outside it")]
    let unsaved: Vec<(StoreFile, Option<u32>, SettingsFile)> = StoreFile::SAVE_ORDER
      .into_iter()
      .filter(|&file| state.file(file).is_unsaved())
      .map(|file| (file, state.file(file).changed, state.file(file).content.clone()))
      .collect();
    drop(state);

    let results: Vec<(StoreFile, Option<u32>, std::io::Result<()>)> = unsaved
      .into_iter()
      .map(|(file, changed, content)| (file, changed, content.save(&shared.path_of(file))))
      .collect();

    state = shared.lock();
    let mut failure = None;
    for (file, changed, result) in results {
      match result {
        Ok(()) => {
          let stored = state.file_mut(file);
          stored.saved = stored.saved.max(changed);
        }
        Err(error) => {
          let error = SaveError {
            path: shared.path_of(file),
            error: Arc::new(error),
          };
          warn!(%error, version = changed, "the settings stay in memory until a save succeeds");
          failure.get_or_insert(error);
        }
      }
    }
    let report = match failure {
      None => {
        state.last_error = None;
        None
      }
      Some(error) => {
        let first_of_streak = state.last_error.replace(error.clone()).is_none();
        // a transient failure (a handle of an indexer on Windows) must not wait for a change that may never come; the
        // last pass of a shutdown gives up
        if !state.shutdown {
          state.retry_delay = if first_of_streak {
            shared.debounce
          } else {
            (state.retry_delay * 2).min(MAX_RETRY_DELAY.max(shared.debounce))
          };
          state.retry_at = Some(Instant::now() + state.retry_delay);
        }
        first_of_streak.then_some(error)
      }
    };
    state.passes_finished += 1;
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

/// The value of `key` as the windows get it; `null` when its file has no such key
fn key_value(state: &State, key: &str) -> Value {
  if key == APP_SETTINGS_KEY {
    return app_value(&state.settings_file.content.app);
  }
  state.file(StoreFile::of(key)).content.other.get(key).cloned().unwrap_or(Value::Null)
}
