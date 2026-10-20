//! A lock file that serializes history rewrites between processes: the app and parallel `bd` calls. The in-process
//! mutexes (ref renames, notes) don't cover another process. Every rewrite of the checked out branch takes it in its
//! core, through the `RewriteOptions` it is called with (AGENTS.md, "Repository lock").

use crate::commit_ids::MIN_ID_LENGTH;
use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::checked_out_branch;
use git_ops::rewrite_guard::HeadMoved;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sync_utils::process_scan::{is_running, process_started};
use tracing::{debug, instrument, warn};

/// How long an operation waits for another one to finish
pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const LOCK_FILE_NAME: &str = "branch-deck.lock";

/// Another process (or another operation of this one) held the lock for the whole timeout
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
  "{} holds {} (waited {:.1} s); retry when it is done",
  .holder.as_ref().map_or_else(|| "Another Branch Deck operation".to_string(), ToString::to_string),
  .path.display(),
  .waited.as_secs_f64()
)]
pub struct RepositoryLocked {
  pub path: PathBuf,
  pub waited: Duration,
  /// Who holds it, as the holder wrote it next to the lock (`LockHolder`); `None` when unknown
  pub holder: Option<LockHolder>,
}

/// Who holds a lock: written next to the lock file (`<lock>.holder`) once the lock is taken, deleted before it is
/// released. The lock file itself can't carry it: on Windows another process can't read a locked file
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockHolder {
  /// The command line of the holder, its program by its file name: `bd check --fix`
  pub command: String,
  pub pid: u32,
  /// When the process started (`process_started`): a pid that the system gave to another process tells apart
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub process_started: Option<u64>,
  /// When the lock was taken, in seconds since the epoch
  pub since: u64,
}

impl fmt::Display for LockHolder {
  /// "`bd check --fix` (pid 4242, for 12 min 3 s)"
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let held = now_seconds().saturating_sub(self.since);
    let held = if held >= 60 {
      format!("{} min {} s", held / 60, held % 60)
    } else {
      format!("{held} s")
    };
    write!(f, "`{}` (pid {}, for {held})", self.command, self.pid)
  }
}

/// The longest command line a holder record keeps
const MAX_HOLDER_COMMAND: usize = 200;

fn now_seconds() -> u64 {
  SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

impl LockHolder {
  /// This process, taking a lock now
  fn of_this_process() -> Self {
    let mut args = std::env::args_os();
    let program = args.next().map_or_else(String::new, |program| {
      Path::new(&program)
        .file_name()
        .map_or_else(|| program.to_string_lossy().into_owned(), |name| name.to_string_lossy().into_owned())
    });
    let mut command = std::iter::once(program)
      .chain(args.map(|arg| arg.to_string_lossy().into_owned()))
      .collect::<Vec<_>>()
      .join(" ");
    if command.len() > MAX_HOLDER_COMMAND {
      let cut = (0..=MAX_HOLDER_COMMAND).rev().find(|index| command.is_char_boundary(*index)).unwrap_or(0);
      command.truncate(cut);
      command.push('…');
    }
    let pid = std::process::id();
    Self {
      command,
      pid,
      process_started: process_started(pid),
      since: now_seconds(),
    }
  }

  /// The holder record of the lock `lock_path`; `None` when there is none, it doesn't parse, or its process is gone
  /// (killed before it deleted the record)
  fn read(lock_path: &Path) -> Option<Self> {
    let text = std::fs::read_to_string(holder_path(lock_path)).ok()?;
    let holder: Self = serde_json::from_str(&text).ok()?;
    is_running(holder.pid, holder.process_started).then_some(holder)
  }

  /// Write the record of the lock `lock_path` through a temporary file and a rename, so a reader never sees half of it
  fn write(&self, lock_path: &Path) -> Result<PathBuf> {
    let path = holder_path(lock_path);
    let temporary = path.with_extension(format!("holder.{}.tmp", self.pid));
    std::fs::write(&temporary, serde_json::to_vec(self)?).with_context(|| format!("Cannot write {}", temporary.display()))?;
    std::fs::rename(&temporary, &path).with_context(|| format!("Cannot write {}", path.display()))?;
    Ok(path)
  }
}

/// `<lock>.holder`
fn holder_path(lock_path: &Path) -> PathBuf {
  let mut path = lock_path.as_os_str().to_owned();
  path.push(".holder");
  PathBuf::from(path)
}

/// An exclusive lock on `$GIT_COMMON_DIR/branch-deck.lock`, released when dropped
#[derive(Debug)]
pub struct RepositoryLock {
  _file: File,
  /// The holder record this lock wrote (`LockHolder`), deleted before the lock is released
  holder: Option<PathBuf>,
}

impl Drop for RepositoryLock {
  /// The record goes first: once the lock is free, the next holder writes its own, which a later delete would take
  fn drop(&mut self) {
    if let Some(holder) = &self.holder
      && let Err(e) = std::fs::remove_file(holder)
    {
      warn!(path = %holder.display(), error = %e, "cannot delete the holder record of the lock");
    }
  }
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
  lock_file(git_common_dir.join(LOCK_FILE_NAME), timeout, on_wait)
}

/// An exclusive lock on the file `path`, created when missing, as `lock_repository_in` takes it: the lease pool has a
/// lock of its own (`lease`), so a lease never waits for a long update
pub fn lock_file(path: PathBuf, timeout: Duration, on_wait: &dyn Fn()) -> Result<RepositoryLock> {
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
        debug!(path = %path.display(), "locked");
        // a lock without its record still locks: the record only names the holder to the ones that wait
        let holder = LockHolder::of_this_process()
          .write(&path)
          .inspect_err(|e| warn!(error = %format!("{e:#}"), "cannot write the holder record of the lock"))
          .ok();
        return Ok(RepositoryLock { _file: file, holder });
      }
      Err(TryLockError::WouldBlock) if started.elapsed() < timeout => {
        if !waiting {
          waiting = true;
          on_wait();
        }
        std::thread::sleep(POLL_INTERVAL.min(timeout));
      }
      Err(TryLockError::WouldBlock) => {
        let holder = LockHolder::read(&path);
        return Err(anyhow::Error::new(RepositoryLocked {
          path,
          waited: started.elapsed(),
          holder,
        }));
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
