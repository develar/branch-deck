//! A lock file that serializes history rewrites between processes: the app and parallel `bd` calls. The in-process
//! mutexes (ref renames, notes) don't cover another process. Every rewrite of the checked out branch takes it in its
//! core, through the `RewriteOptions` it is called with (AGENTS.md, "Repository lock").

use crate::commit_ids::MIN_ID_LENGTH;
use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::checked_out_branch;
use git_ops::rewrite_guard::HeadMoved;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
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
  lock_repository_in(Path::new(common_dir.trim()), timeout, on_wait)
}

/// `lock_repository_with` for a caller that knows `$GIT_COMMON_DIR` (absolute, `bd`'s context): no git process
#[instrument(skip(on_wait))]
pub fn lock_repository_in(git_common_dir: &Path, timeout: Duration, on_wait: &dyn Fn()) -> Result<RepositoryLock> {
  let path = git_common_dir.join(LOCK_FILE_NAME);
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

/// What every rewrite of the checked out branch takes besides its request (`*_core(…, options)`): the HEAD the request
/// was made for and the repository lock. The core takes the lock before it reads anything, so two rewrites never
/// interleave, also when one runs in the app and the other in `bd`
#[derive(Debug, Clone)]
pub struct RewriteOptions<'a> {
  /// The HEAD the request was made for; abbreviated ids are fine. Another HEAD is `HeadMoved`
  pub expect_head: Option<String>,
  /// How long the rewrite waits for another holder of the lock before it fails with `RepositoryLocked`
  pub lock_timeout: Duration,
  /// The repository lock the caller holds, so that it can write more under it; `None` takes the lock for the rewrite
  pub held_lock: Option<&'a RepositoryLock>,
}

impl Default for RewriteOptions<'_> {
  fn default() -> Self {
    Self {
      expect_head: None,
      lock_timeout: DEFAULT_LOCK_TIMEOUT,
      held_lock: None,
    }
  }
}

impl RewriteOptions<'_> {
  /// The lock of the rewrite, held until the guard is dropped: none when the caller holds it, else `lock_repository`
  /// with `lock_timeout`
  pub fn lock(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<RepositoryLock>> {
    match self.held_lock {
      Some(_) => Ok(None),
      None => lock_repository(git_executor, repository_path, self.lock_timeout).map(Some),
    }
  }

  /// `ensure_expected_head` of `expect_head`, the branch read only for the error
  pub fn ensure_head(&self, git_executor: &GitCommandExecutor, repository_path: &str, head: &str) -> Result<(), HeadMoved> {
    ensure_expected_head(self.expect_head.as_deref(), head, || {
      checked_out_branch(git_executor, repository_path).unwrap_or_else(|_| "HEAD".to_string())
    })
  }

  /// HEAD as a full id when `expect_head` is set and HEAD is still that commit, for a git-ops rewrite that compares the
  /// full id; `None`, without a process, when no HEAD is expected
  pub fn expected_full_head(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<String>> {
    if self.expect_head.is_none() {
      return Ok(None);
    }
    let head = git_executor.execute_command(&["rev-parse", "--verify", "HEAD^{commit}"], repository_path)?;
    self.ensure_head(git_executor, repository_path, &head)?;
    Ok(Some(head))
  }
}

/// Refuse with `HeadMoved` unless `head` (a full id) is `expected`, or starts with it when `expected` is an abbreviated
/// id of at least `MIN_ID_LENGTH` characters; `branch` names the checked out branch for the error. No `expected` is no
/// check
pub fn ensure_expected_head(expected: Option<&str>, head: &str, branch: impl FnOnce() -> String) -> Result<(), HeadMoved> {
  match expected {
    Some(expected) if !(expected.len() >= MIN_ID_LENGTH && head.starts_with(expected)) => Err(HeadMoved {
      branch: branch(),
      expected: expected.to_string(),
      actual: head.to_string(),
    }),
    _ => Ok(()),
  }
}

#[cfg(test)]
#[path = "repository_lock_test.rs"]
mod repository_lock_test;
