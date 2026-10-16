//! A lock file that serializes history rewrites between processes: the app and parallel `bd` calls. The in-process
//! mutexes (ref renames, notes) don't cover another process.

use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{debug, instrument};

/// How long an operation waits for another one to finish
pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const LOCK_FILE_NAME: &str = "branch-deck.lock";

/// Another process (or another operation of this one) held the lock for the whole timeout
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Another Branch Deck operation holds {} (waited {:.1} s); retry when it is done", .path.display(), .waited.as_secs_f64())]
pub struct RepositoryLocked {
  pub path: PathBuf,
  pub waited: Duration,
}

/// An exclusive lock on `$GIT_COMMON_DIR/branch-deck.lock`, released when dropped
#[derive(Debug)]
pub struct RepositoryLock {
  _file: File,
}

/// Lock the repository, waiting up to `timeout` for the holder. All worktrees of a repository share the lock, as they
/// share their branches. A timeout is a `RepositoryLocked` error (`downcast_ref` it to tell it from others)
pub fn lock_repository(git_executor: &GitCommandExecutor, repository_path: &str, timeout: Duration) -> Result<RepositoryLock> {
  lock_repository_with(git_executor, repository_path, timeout, &|| {})
}

/// `lock_repository`; `on_wait` is called once, when another holds the lock and the wait starts (the update says so)
#[instrument(skip(git_executor, on_wait))]
pub fn lock_repository_with(git_executor: &GitCommandExecutor, repository_path: &str, timeout: Duration, on_wait: &dyn Fn()) -> Result<RepositoryLock> {
  let common_dir = git_executor.execute_command(&["rev-parse", "--path-format=absolute", "--git-common-dir"], repository_path)?;
  let path = PathBuf::from(common_dir.trim()).join(LOCK_FILE_NAME);
  let file = OpenOptions::new()
    .create(true)
    .truncate(false)
    .write(true)
    .open(&path)
    .with_context(|| format!("Cannot open {}", path.display()))?;

  let started = Instant::now();
  let mut waiting = false;
  loop {
    match file.try_lock() {
      Ok(()) => {
        debug!(path = %path.display(), "locked the repository");
        return Ok(RepositoryLock { _file: file });
      }
      Err(TryLockError::WouldBlock) if started.elapsed() < timeout => {
        if !waiting {
          waiting = true;
          on_wait();
        }
        std::thread::sleep(POLL_INTERVAL.min(timeout));
      }
      Err(TryLockError::WouldBlock) => {
        return Err(anyhow::Error::new(RepositoryLocked { path, waited: started.elapsed() }));
      }
      Err(TryLockError::Error(e)) => return Err(anyhow::Error::new(e).context(format!("Cannot lock {}", path.display()))),
    }
  }
}
