//! `SettingsService`: the one writer of the settings files of the app data directory. Every change, from any window or
//! from Rust, is a `SettingsPatch` that `set` applies to the copy in memory under one mutex, in the order it arrives,
//! and answers at once with the `SettingsChange` (the new version and the whole key) that the caller sends to every
//! window. A key lives in one file (`StoreFile::of`): `repositoryState`, which every sync, fetch and update changes, and
//! `linkedPullRequests` in `repository-state.json`, every other key in `settings.json`, which so changes only when the user changes a setting or
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
//!
//! Two processes may share the directory, each with its one writer (the app, and `brd mcp` of an IDE beside it), so a
//! save never writes the copy in memory over the file: under the lock of the directory (`file::lock_directory`) it reads
//! the file as it is, puts on top only what this process changed and has not saved yet, by key and by field
//! (`merge::Unsaved`), and renames that over the file; a change of the other process to another key or field stays.
//! What the file has beside the unsaved changes of this process is the other's change: the save takes it into memory,
//! and so does a read of the file that its watch asks for (`watch`) when the other process renamed a file over it.
//! Each such key is a change of its own (the next version, origin `OTHER_PROCESS_ORIGIN`), which `changes_since`
//! answers and `on_external_change` tells, as `set` answers a change of a window, so the pages take it the same way.
//!
//! An entry of `repositoryState` is of the epoch of its worktree (`lease_pool::epoch::worktree_epoch`, which the host
//! gives, `set_worktree_epochs`): a slot of a lease pool keeps its path from one lease to the next. A write of an entry
//! names the epoch it was written under (`EPOCH_FIELD`, which no window writes), and an entry of another known epoch is
//! of an earlier lease: the writer forgets it before the key leaves it (`snapshot`, `changes_since`, `value`, the change
//! of a `set`), so neither Rust nor a window reads it. An epoch the files could not tell (`WorktreeEpoch::Unknown`)
//! forgets nothing and stamps nothing anew: the entry keeps the epoch it names. The epochs are read before the mutex of
//! the writer is taken, never under it.

use crate::file::{REPOSITORY_STATE_FILE_NAME, REPOSITORY_STATE_KEY, REPOSITORY_STATE_KEYS, SETTINGS_FILE_NAME, SettingsFile, lock_directory};
use crate::merge::{Unsaved, changed_keys};
use crate::watch::{Changed, watch};
use domain_types::settings::{APP_SETTINGS_KEY, AppSettings, SettingsChange, SettingsPatch};
use domain_types::worktree_epoch::WorktreeEpoch;
use notify::RecommendedWatcher;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use sync_utils::span::in_current_span;
use tracing::{debug, warn};

/// How long the writer waits after the first unsaved change before it saves, so a burst of changes is one write
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(500);

/// The longest delay between two retries of a failed save, unless the debounce is longer
pub const MAX_RETRY_DELAY: Duration = Duration::from_mins(1);

/// The origin of a change the writer of another process made, which this one read from the file
pub const OTHER_PROCESS_ORIGIN: &str = "other-process";

/// The origin of the change that forgot the entries of `repositoryState` of another epoch of their worktree
pub const EPOCH_ORIGIN: &str = "epoch";

/// The field of an entry of `repositoryState` that names the epoch of its worktree it was written under; none for a path
/// outside a lease pool
pub const EPOCH_FIELD: &str = "epoch";

/// The epoch of the worktree at a path (`lease_pool::epoch::worktree_epoch`), which the host gives the writer
pub type WorktreeEpochs = Arc<dyn Fn(&str) -> WorktreeEpoch + Send + Sync>;

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

type ChangeListener = Arc<dyn Fn(&SettingsChange) + Send + Sync>;

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

  /// The file of `key`: `repositoryState`, `linkedPullRequests` and `ideRootPicks` in `repository-state.json`, every
  /// other key in `settings.json`
  fn of(key: &str) -> Self {
    if REPOSITORY_STATE_KEYS.contains(&key) { Self::RepositoryState } else { Self::Settings }
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Settings => SETTINGS_FILE_NAME,
      Self::RepositoryState => REPOSITORY_STATE_FILE_NAME,
    }
  }

  /// Whether the watch asked to read this file again
  const fn is_named_by(self, changed: Changed) -> bool {
    match self {
      Self::Settings => changed.settings,
      Self::RepositoryState => changed.repository_state,
    }
  }

  /// `content` with only the keys that live in this file
  fn confined(self, mut content: SettingsFile) -> SettingsFile {
    match self {
      Self::Settings => {
        for key in REPOSITORY_STATE_KEYS {
          content.other.remove(key);
        }
      }
      Self::RepositoryState => {
        content.app = AppSettings::default();
        content.other.retain(|key, _| REPOSITORY_STATE_KEYS.contains(&key.as_str()));
      }
    }
    content
  }

  /// This file in `directory` as it is now, with only its keys and, for `settings.json`, without the fields of a sync
  /// that a file of before kept in its recent projects (the load moved them); none when it is missing, an error when it
  /// can't be read or does not parse
  fn read(self, directory: &Path) -> std::io::Result<Option<SettingsFile>> {
    let content = SettingsFile::read(&directory.join(self.name()))?;
    Ok(content.map(|content| {
      let mut content = self.confined(content);
      if self == Self::Settings {
        content.take_repository_state();
      }
      content
    }))
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
  /// What this process changed and no save has taken yet, which a save puts on top of the file as it is
  unsaved: Unsaved,
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
  /// The files the watch asked to read again
  reread: Changed,
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
    let mut settings = StoreFile::Settings.confined(SettingsFile::load(&directory.join(SETTINGS_FILE_NAME)));
    let mut repository_state = StoreFile::RepositoryState.confined(SettingsFile::load(&directory.join(REPOSITORY_STATE_FILE_NAME)));

    let moved = settings.take_repository_state();
    let mut state = Self::default();
    if !moved.is_empty() {
      // a field of each repository, so the save keeps the repositories another process recorded meanwhile
      state.repository_state_file.unsaved.record(REPOSITORY_STATE_KEY, Some(moved.keys().cloned().collect()), 0);
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

  /// Take `disk`, the file `file` as the other process left it, with the unsaved changes of this process on top, into
  /// memory: each key that differs is a change of the other process, with the next version. Answers those changes
  fn take_disk(&mut self, file: StoreFile, disk: SettingsFile) -> Vec<SettingsChange> {
    let stored = self.file_mut(file);
    let merged = stored.unsaved.overlay(disk, &stored.content);
    let keys = changed_keys(&stored.content, &merged);
    if keys.is_empty() {
      return Vec::new();
    }
    stored.content = merged;
    keys
      .into_iter()
      .map(|key| {
        self.version += 1;
        let version = self.version;
        self.changes.insert(
          key.clone(),
          KeyChange {
            version,
            origin: OTHER_PROCESS_ORIGIN.to_owned(),
          },
        );
        SettingsChange {
          version,
          value: key_value(self, &key),
          key,
          origin: OTHER_PROCESS_ORIGIN.to_owned(),
        }
      })
      .collect()
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
  on_external_change: Mutex<Option<ChangeListener>>,
  /// The epochs of the worktrees (`set_worktree_epochs`); none: every path is outside a pool
  epochs: Mutex<Option<WorktreeEpochs>>,
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
  /// The watch of the directory for the writes of another process; none when the system refused it
  watcher: Option<RecommendedWatcher>,
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
      on_external_change: Mutex::new(None),
      epochs: Mutex::new(None),
    });
    let writer_shared = Arc::clone(&shared);
    let writer = std::thread::Builder::new()
      .name("settings-writer".to_owned())
      .spawn(in_current_span(move || write_loop(&writer_shared)))?;
    // values the load moved wait for the debounce, as a change does
    shared.wake.notify_one();
    let woken = Arc::downgrade(&shared);
    let watcher = watch(&shared.directory, move |changed| {
      let Some(shared) = woken.upgrade() else {
        return;
      };
      let mut state = shared.lock();
      state.reread.settings |= changed.settings;
      state.reread.repository_state |= changed.repository_state;
      drop(state);
      shared.wake.notify_one();
    });
    if watcher.is_some() {
      // what another process saved between the load and the start of the watch
      shared.lock().reread = Changed::BOTH;
      shared.wake.notify_one();
    }
    Ok(Self {
      shared,
      writer: Some(writer),
      watcher,
    })
  }

  /// Apply `patch` from `origin` to the settings in memory and answer the change for the windows; the save of the
  /// file of its key follows within the debounce. A value of a field Rust knows that is invalid is refused and changes
  /// nothing
  pub fn set(&self, mut patch: SettingsPatch, origin: &str) -> Result<SettingsChange, SettingsError> {
    let now = (patch.key() == REPOSITORY_STATE_KEY).then(|| self.epochs_now(written_paths(&patch))).flatten();
    let mut state = self.shared.lock();
    let key = patch.key().to_owned();
    // the change carries the whole key: without the entries of an earlier lease, forgotten first
    if let Some(now) = &now {
      forget_other_epochs(&mut state, now);
      stamp_epochs(&mut patch, now, &state);
    }
    let file = StoreFile::of(&key);
    let fields = match &patch {
      SettingsPatch::Fields { fields, .. } => Some(fields.keys().cloned().collect()),
      SettingsPatch::Value { .. } => None,
    };
    let value = apply_patch(&mut state.file_mut(file).content, patch)?;
    state.version += 1;
    let version = state.version;
    let stored = state.file_mut(file);
    stored.changed = Some(version);
    stored.unsaved.record(&key, fields, version);
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

  /// The epochs of the worktrees the entries of `repositoryState` are of (`lease_pool::epoch::worktree_epoch`): from now
  /// a write of an entry names its epoch, and an entry of another epoch is forgotten before the key leaves the writer
  pub fn set_worktree_epochs(&self, epochs: impl Fn(&str) -> WorktreeEpoch + Send + Sync + 'static) {
    *self.shared.epochs.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(epochs));
  }

  fn epochs(&self) -> Option<WorktreeEpochs> {
    self.shared.epochs.lock().unwrap_or_else(PoisonError::into_inner).clone()
  }

  /// The epoch now of each path an entry of `repositoryState` names an epoch for, and of `written`, the paths a patch
  /// writes: read without the mutex, which the writer holds again only to apply them; none without the epochs
  fn epochs_now(&self, written: Vec<String>) -> Option<HashMap<String, WorktreeEpoch>> {
    let epochs = self.epochs()?;
    let mut paths: BTreeSet<String> = stamped_paths(&self.shared.lock());
    paths.extend(written);
    Some(
      paths
        .into_iter()
        .map(|path| {
          let epoch = epochs(&path);
          (path, epoch)
        })
        .collect(),
    )
  }

  /// The state, the entries of `repositoryState` of another known epoch of their worktree forgotten
  fn current(&self) -> MutexGuard<'_, State> {
    let now = self.epochs_now(Vec::new());
    let mut state = self.shared.lock();
    if let Some(now) = &now
      && forget_other_epochs(&mut state, now)
    {
      self.shared.wake.notify_one();
    }
    state
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
    let state = self.current();
    let mut values = state.settings_file.content.other.clone();
    values.insert(APP_SETTINGS_KEY.to_owned(), app_value(&state.settings_file.content.app));
    values.extend(state.repository_state_file.content.other.clone());
    (state.version, Value::Object(values))
  }

  /// The last change of each key changed after `version` (a snapshot's), in the order of their versions; a removed key
  /// comes with `null`
  #[must_use]
  pub fn changes_since(&self, version: u32) -> Vec<SettingsChange> {
    let state = self.current();
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

  /// The value of `key` as it is now; `null` for a key that is not set
  #[must_use]
  pub fn value(&self, key: &str) -> Value {
    if key == REPOSITORY_STATE_KEY {
      return key_value(&self.current(), key);
    }
    key_value(&self.shared.lock(), key)
  }

  /// The app data directory of the files: the journal of Activity of the page lies beside them
  /// (`operations::journal::record_page`)
  #[must_use]
  pub fn directory(&self) -> &Path {
    &self.shared.directory
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

  /// Call `listener` with each change of another process this writer takes from the file (origin
  /// `OTHER_PROCESS_ORIGIN`), on the writer thread, in the order of their versions, before the flush that took it returns;
  /// the caller sends it to the windows as it sends the answer of `set`
  pub fn on_external_change(&self, listener: impl Fn(&SettingsChange) + Send + Sync + 'static) {
    *self.shared.on_external_change.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(listener));
  }
}

impl Drop for SettingsService {
  /// Stop the watch, save what is unsaved and stop the writer
  fn drop(&mut self) {
    drop(self.watcher.take());
    self.shared.lock().shutdown = true;
    self.shared.wake.notify_one();
    if let Some(writer) = self.writer.take()
      && writer.join().is_err()
    {
      warn!("the settings writer panicked");
    }
  }
}

/// A save the writer makes of a file: the version of its last change, its copy and its unsaved changes as they were
/// when the pass took them
struct PendingSave {
  file: StoreFile,
  changed: Option<u32>,
  content: SettingsFile,
  unsaved: Unsaved,
}

/// What a save of a file did: its outcome, and the file as it is on disk after it, when the save knows
struct SaveOutcome {
  file: StoreFile,
  changed: Option<u32>,
  result: std::io::Result<()>,
  disk: Option<SettingsFile>,
}

/// The writer thread: wait for a change that is `debounce` old, a flush, the shutdown or a read the watch asked for;
/// save each changed file (its copy taken under the lock, merged into the file under the lock of the directory, outside
/// the lock of the memory) and read each file the watch named; take what another process changed into memory, record
/// the outcome, tell the changes of the other process and wake the flushes
fn write_loop(shared: &Shared) {
  let mut state = shared.lock();
  loop {
    let due = match (state.dirty_since.map(|since| since + shared.debounce), state.retry_at) {
      (Some(changed), Some(retry)) => Some(changed.min(retry)),
      (changed, retry) => changed.or(retry),
    };
    let write_now = state.flush_requested || (state.shutdown && due.is_some()) || due.is_some_and(|due| due <= Instant::now());
    let reread = std::mem::take(&mut state.reread);
    if !write_now && reread == Changed::default() {
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

    // a pass that only reads is no pass a flush waits for, and leaves the debounce of the changes as it is
    let mut saves = Vec::new();
    if write_now {
      state.passes_started += 1;
      state.dirty_since = None;
      state.retry_at = None;
      state.flush_requested = false;
      saves.extend(StoreFile::SAVE_ORDER.into_iter().filter(|&file| state.file(file).is_unsaved()).map(|file| {
        let stored = state.file(file);
        PendingSave {
          file,
          changed: stored.changed,
          content: stored.content.clone(),
          unsaved: stored.unsaved.clone(),
        }
      }));
    }
    drop(state);

    let outcomes = save_files(shared, saves);
    // a file the pass saved is known as it is on disk now
    let read: Vec<(StoreFile, Option<SettingsFile>)> = StoreFile::SAVE_ORDER
      .into_iter()
      .filter(|&file| file.is_named_by(reread) && !outcomes.iter().any(|outcome| outcome.file == file))
      .map(|file| (file, read_file(shared, file)))
      .collect();

    state = shared.lock();
    let mut failure = None;
    let mut taken = Vec::new();
    for outcome in outcomes {
      match outcome.result {
        Ok(()) => {
          let stored = state.file_mut(outcome.file);
          stored.saved = stored.saved.max(outcome.changed);
          if let Some(changed) = outcome.changed {
            stored.unsaved.forget_saved(changed);
          }
        }
        Err(error) => {
          let error = SaveError {
            path: shared.path_of(outcome.file),
            error: Arc::new(error),
          };
          warn!(%error, version = outcome.changed, "the settings stay in memory until a save succeeds");
          failure.get_or_insert(error);
        }
      }
      taken.extend(outcome.disk.map(|disk| (outcome.file, disk)));
    }
    taken.extend(read.into_iter().filter_map(|(file, disk)| disk.map(|disk| (file, disk))));
    let external: Vec<SettingsChange> = taken.into_iter().flat_map(|(file, disk)| state.take_disk(file, disk)).collect();

    let report = if write_now { record_failure(shared, &mut state, failure) } else { None };
    // the reports go out before the pass ends, so a `flush` that waits for this pass returns after them
    if report.is_some() || !external.is_empty() {
      drop(state);
      if let Some(error) = report {
        let listener = shared.on_save_error.lock().unwrap_or_else(PoisonError::into_inner).clone();
        if let Some(listener) = listener {
          listener(&error);
        }
      }
      let listener = shared.on_external_change.lock().unwrap_or_else(PoisonError::into_inner).clone();
      if let Some(listener) = listener {
        for change in &external {
          listener(change);
        }
      }
      state = shared.lock();
    }
    if write_now {
      state.passes_finished += 1;
      shared.saved.notify_all();
    }
  }
}

/// Record the failure of a pass that saved, or the end of a streak of failures, and schedule the retry; answers the
/// error to report, the first of a streak
fn record_failure(shared: &Shared, state: &mut State, failure: Option<SaveError>) -> Option<SaveError> {
  let Some(error) = failure else {
    state.last_error = None;
    return None;
  };
  let first_of_streak = state.last_error.replace(error.clone()).is_none();
  // a transient failure (a handle of an indexer on Windows) must not wait for a change that may never come; the last
  // pass of a shutdown gives up
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

/// Save each of `saves` under the lock of the directory, which the writer of another process takes too: read the file
/// as it is (a missing or broken one is the copy in memory), put the unsaved changes of this process on top and rename
/// that over the file. A failure of the lock fails every save
fn save_files(shared: &Shared, saves: Vec<PendingSave>) -> Vec<SaveOutcome> {
  if saves.is_empty() {
    return Vec::new();
  }
  let lock = lock_directory(&shared.directory);
  saves
    .into_iter()
    .map(|save| {
      let PendingSave { file, changed, content, unsaved } = save;
      if let Err(error) = &lock {
        let lock_path = shared.directory.join(crate::file::LOCK_FILE_NAME);
        let error = std::io::Error::new(error.kind(), format!("cannot lock {}: {error}", lock_path.display()));
        return SaveOutcome {
          file,
          changed,
          result: Err(error),
          disk: None,
        };
      }
      let base = file.read(&shared.directory).unwrap_or_else(|error| {
        warn!(file = %shared.path_of(file).display(), %error, "the file is written again from memory");
        None
      });
      let merged = match base {
        Some(base) => unsaved.overlay(base, &content),
        None => content,
      };
      let (result, disk) = match merged.save(&shared.path_of(file)) {
        Ok(()) => (Ok(()), Some(merged)),
        Err(error) => (Err(error), None),
      };
      SaveOutcome { file, changed, result, disk }
    })
    .collect()
}

/// The file `file` as the watch found it changed; none when it is missing or can't be read, which takes nothing into
/// memory
fn read_file(shared: &Shared, file: StoreFile) -> Option<SettingsFile> {
  file
    .read(&shared.directory)
    .inspect_err(|error| debug!(file = %shared.path_of(file).display(), %error, "the changed settings file is not read"))
    .ok()
    .flatten()
}

/// The entries of `repositoryState` that `patch` writes (the fields of a window, the whole value of a fixture of the E2E
/// server), mutable
fn written_entries(patch: &mut SettingsPatch) -> Option<&mut Map<String, Value>> {
  match patch {
    SettingsPatch::Fields { key, fields } if key == REPOSITORY_STATE_KEY => Some(fields),
    SettingsPatch::Value {
      key,
      value: Value::Object(entries),
    } if key == REPOSITORY_STATE_KEY => Some(entries),
    _ => None,
  }
}

/// The paths whose entries of `repositoryState` `patch` writes
fn written_paths(patch: &SettingsPatch) -> Vec<String> {
  match patch {
    SettingsPatch::Fields { key, fields } if key == REPOSITORY_STATE_KEY => fields.keys().cloned().collect(),
    SettingsPatch::Value {
      key,
      value: Value::Object(entries),
    } if key == REPOSITORY_STATE_KEY => entries.keys().cloned().collect(),
    _ => Vec::new(),
  }
}

/// The epoch an entry of `repositoryState` names
fn epoch_of(entry: &Value) -> Option<&str> {
  entry.get(EPOCH_FIELD).and_then(Value::as_str)
}

/// The paths whose entries of `repositoryState` name an epoch
fn stamped_paths(state: &State) -> BTreeSet<String> {
  match state.repository_state_file.content.other.get(REPOSITORY_STATE_KEY) {
    Some(Value::Object(entries)) => entries.iter().filter(|(_, entry)| epoch_of(entry).is_some()).map(|(path, _)| path.clone()).collect(),
    _ => BTreeSet::new(),
  }
}

/// Names in each entry of `repositoryState` that `patch` writes the epoch of its worktree now, from `now`
/// (`EPOCH_FIELD`, removed for a path outside a pool): the page never writes it, so its entries are always of the epoch
/// they were written under. An entry whose epoch is unknown keeps the one the writer has of it, so a failed read stamps
/// nothing anew
fn stamp_epochs(patch: &mut SettingsPatch, now: &HashMap<String, WorktreeEpoch>, state: &State) {
  let stored = state.repository_state_file.content.other.get(REPOSITORY_STATE_KEY);
  let Some(entries) = written_entries(patch) else {
    return;
  };
  for (path, entry) in entries.iter_mut() {
    let Value::Object(entry) = entry else {
      continue;
    };
    let epoch = match now.get(path).and_then(WorktreeEpoch::recorded) {
      Some(recorded) => recorded.map(str::to_owned),
      None => stored.and_then(|stored| stored.get(path)).and_then(epoch_of).map(str::to_owned),
    };
    match epoch {
      Some(epoch) => entry.insert(EPOCH_FIELD.to_owned(), Value::String(epoch)),
      None => entry.remove(EPOCH_FIELD),
    };
  }
}

/// Removes from `repositoryState` each entry written under an epoch of its worktree that `now` knows to be another
/// (`WorktreeEpoch::outdates`), as a change of the writer (`EPOCH_ORIGIN`) that the next save writes; an entry that names
/// no epoch, whose epoch is unknown, or which `now` has not read (written since), stays. Whether it removed one
fn forget_other_epochs(state: &mut State, now: &HashMap<String, WorktreeEpoch>) -> bool {
  let Some(Value::Object(entries)) = state.repository_state_file.content.other.get_mut(REPOSITORY_STATE_KEY) else {
    return false;
  };
  let stale: Vec<String> = entries
    .iter()
    .filter(|(path, entry)| now.get(*path).is_some_and(|epoch| epoch.outdates(epoch_of(entry))))
    .map(|(path, _)| path.clone())
    .collect();
  if stale.is_empty() {
    return false;
  }
  for path in &stale {
    entries.remove(path);
  }
  if entries.is_empty() {
    state.repository_state_file.content.other.remove(REPOSITORY_STATE_KEY);
  }
  state.version += 1;
  let version = state.version;
  let stored = state.file_mut(StoreFile::RepositoryState);
  stored.changed = Some(version);
  stored.unsaved.record(REPOSITORY_STATE_KEY, Some(stale), version);
  state.changes.insert(
    REPOSITORY_STATE_KEY.to_owned(),
    KeyChange {
      version,
      origin: EPOCH_ORIGIN.to_owned(),
    },
  );
  state.dirty_since.get_or_insert_with(Instant::now);
  true
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
