//! A JSON state file in `$GIT_COMMON_DIR/branch-deck/` with a lock of its own: the lease pool (`leases.json`,
//! `leases.lock`) and the lend table of the host slots (`host-slots.json`, `host-slots.lock`). A read takes no lock (the
//! file is replaced by a rename, so a read sees one whole state); a change runs under the lock and writes only a state
//! it changed

use anyhow::{Context, Result, anyhow};
use history_rewrite::repository_lock::{RepositoryLock, lock_file};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use tracing::debug;

/// The directory of the state files in `$GIT_COMMON_DIR`
pub(crate) const STATE_DIR: &str = "branch-deck";

pub(crate) struct StateFile {
  dir: PathBuf,
  file: &'static str,
  lock: &'static str,
  /// What a read or a write failed on, "the lease state"
  what: &'static str,
}

impl StateFile {
  /// The file `file` with the lock `lock` in `branch-deck/` of `git_common_dir`
  pub(crate) fn new(git_common_dir: &Path, file: &'static str, lock: &'static str, what: &'static str) -> Self {
    Self {
      dir: git_common_dir.join(STATE_DIR),
      file,
      lock,
      what,
    }
  }

  /// `branch-deck/` of the common dir
  pub(crate) fn dir(&self) -> &Path {
    &self.dir
  }

  /// The state, without the lock; a missing file is the default state
  pub(crate) fn read<T: DeserializeOwned + Default>(&self) -> Result<T> {
    let path = self.dir.join(self.file);
    match std::fs::read(&path) {
      Ok(bytes) => serde_json::from_slice(&bytes).with_context(|| format!("Cannot read {} {}", self.what, path.display())),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
      Err(error) => Err(anyhow!(error).context(format!("Cannot read {} {}", self.what, path.display()))),
    }
  }

  /// Run `change` on the state under the lock and write the state it leaves; the lock is released at once
  pub(crate) fn update<T, R>(&self, timeout: Duration, change: impl FnOnce(&mut T) -> Result<R>) -> Result<R>
  where
    T: DeserializeOwned + Serialize + Default + Clone + PartialEq,
  {
    let _lock = self.lock_at(self.lock, timeout, "waiting for the lock of the state")?;
    let mut state: T = self.read()?;
    let before = state.clone();
    let result = change(&mut state)?;
    if state != before {
      let path = self.dir.join(self.file);
      write_atomically(&path, &serde_json::to_vec_pretty(&state)?, AtomicWrite::default()).with_context(|| format!("Cannot write {}", path.display()))?;
    }
    Ok(result)
  }

  /// The lock file `name` beside the state
  pub(crate) fn lock_at(&self, name: &str, timeout: Duration, waiting: &str) -> Result<RepositoryLock> {
    std::fs::create_dir_all(&self.dir).with_context(|| format!("Cannot create {}", self.dir.display()))?;
    lock_file(self.dir.join(name), timeout, &|| debug!("{waiting} ({name})"))
  }
}
