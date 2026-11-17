//! What a worktree keeps of its own history in its git directory, which the git directory of a slot of a lease pool
//! outlives (openspec/specs/cli/lease/spec.md, "A return takes out what the lease left"): the mark of a sync
//! (`synced_mark`) and the landed state (`landed`, where each branch was at the last sync and a report of what landed
//! not dismissed yet). Each is of one lease of the slot, so all of them go as the lease ends, by the one list here
//! (`LeasePaths::all`): the return of the lease, a takeover and a reclaim, and the get that reuses a slot an older build
//! left with them. The next lease inherits none: a window that opens it shows the Ready to Sync card, and nothing landed
//! is reported of a branch it never had. Their locks stay (`lease.lock`, `landed.lock`), as no clean rule deletes a lock
//! of git or of Branch Deck.
//!
//! This module names them and is their one writer (`operations/src/lease_files_scan_test.rs`): `LeasePaths` builds the
//! directory of the lease in a git directory and every path in it; `synced_mark` and `landed` decide what the files
//! hold, and write and remove them only here, each write under the guard of its caller (`LeaseGuard`): a work of a slot
//! writes only while the slot holds the lease the work began under (the run of the app's session, `brd -C <slot>`, an
//! intent of a window), so a work that a return and a new lease overtook writes nothing into the files of the new lease.
//! The check of the guard and the write hold the lock of the lease (`lease.lock`), which the forget of a return and of
//! a get that reuses a slot takes too: a write that passed the check lands before the forget, which takes it out, and
//! one after the forget sees the epoch of the new lease, as the forget of the get comes after its reservation changed it.

use git_ops::lease_slot::git_dir_of;
use history_rewrite::repository_lock::{RepositoryLock, lock_file};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::fs_retry;
use tracing::{debug, warn};

/// The directory of the files of the lease in the git directory of a worktree, as `rev-parse --git-path` names it: a
/// linked worktree has its own
pub const LEASE_DIR: &str = git_ops::lease_slot::STATE_DIR;

/// How long a write or a forget waits for another process that holds the lock of the lease: the other one writes a
/// small file or removes three
const LEASE_LOCK_TIMEOUT: Duration = Duration::from_secs(10);

/// The paths of the files of the lease of a worktree, in the directory of its git directory that keeps them
/// (`LEASE_DIR`): the one place that builds them
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeasePaths {
  dir: PathBuf,
}

impl LeasePaths {
  /// The files of the lease in `dir`, the `LEASE_DIR` of a git directory as `rev-parse --git-path` answers it
  #[must_use]
  pub const fn in_dir(dir: PathBuf) -> Self {
    Self { dir }
  }

  /// The files of the lease in the git directory `git_dir`
  #[must_use]
  pub fn of_git_dir(git_dir: &Path) -> Self {
    Self::in_dir(git_dir.join(LEASE_DIR))
  }

  /// The files of the lease of the worktree whose top level is `worktree`, in the git directory its `.git` names
  /// (`git_dir_of`), without a git process; `None` for a path that is no worktree
  #[must_use]
  pub fn of_worktree(worktree: &Path) -> Option<Self> {
    git_dir_of(worktree).map(|git_dir| Self::of_git_dir(&git_dir))
  }

  /// The landed state (`landed`)
  #[must_use]
  pub fn landed(&self) -> PathBuf {
    self.dir.join("landed.json")
  }

  /// The mark of a sync: a file for each prefix a sync succeeded under (`synced_mark`)
  #[must_use]
  pub fn marks(&self) -> PathBuf {
    self.dir.join("synced.d")
  }

  /// The mark of a build before: one prefix per line, or the time of a sync of the build before it
  #[must_use]
  pub fn legacy_mark(&self) -> PathBuf {
    self.dir.join("synced")
  }

  /// Every file and directory of the lease, the ones a return takes out
  #[must_use]
  pub fn all(&self) -> [PathBuf; 3] {
    [self.marks(), self.legacy_mark(), self.landed()]
  }

  /// The lock of a read-modify-write of the landed state (`landed`)
  fn landed_lock(&self) -> PathBuf {
    self.dir.join("landed.lock")
  }

  /// The lock of the writes of the files of the lease and of their forget
  pub(crate) fn lease_lock(&self) -> PathBuf {
    self.dir.join("lease.lock")
  }

  /// Whether `path` is in the directory of the lease
  fn holds(&self, path: &Path) -> bool {
    path.starts_with(&self.dir)
  }
}

/// Whether the worktree still holds the lease that the work that writes the files of its lease began under, asked under
/// the lock of the lease right before each write (`write_lease_file`, `remove_lease_file`). The app's session asks the
/// epoch of the slot (`GitSessionHost::run`), and so do `brd` in a slot and an intent of a window
/// (`lease_pool::epoch::lease_guard`); only a worktree that is no slot, and a test, write at any time (`any`)
#[derive(Clone, Default)]
pub struct LeaseGuard(Option<Arc<dyn Fn() -> bool + Send + Sync>>);

impl LeaseGuard {
  /// Writes at any time: of a worktree that is no slot of a lease pool, whose files no next lease inherits
  #[must_use]
  pub const fn any() -> Self {
    Self(None)
  }

  /// Writes only while `holds` answers that the worktree holds the lease the work began under
  #[must_use]
  pub fn while_held(holds: impl Fn() -> bool + Send + Sync + 'static) -> Self {
    Self(Some(Arc::new(holds)))
  }

  fn holds(&self, path: &Path) -> bool {
    let holds = self.0.as_ref().is_none_or(|holds| holds());
    if !holds {
      warn!(path = %path.display(), "the worktree holds another lease since the work began: its file of the lease stays as it is");
    }
    holds
  }
}

impl fmt::Debug for LeaseGuard {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(if self.0.is_some() { "LeaseGuard(while held)" } else { "LeaseGuard(any)" })
  }
}

/// The lock of the lease of `paths`, its directory made first; `on_wait` once when another holds it
fn lock_lease(paths: &LeasePaths, on_wait: &dyn Fn()) -> std::io::Result<RepositoryLock> {
  std::fs::create_dir_all(&paths.dir)?;
  lock_file(paths.lease_lock(), LEASE_LOCK_TIMEOUT, on_wait).map_err(std::io::Error::other)
}

/// Writes `bytes` into the file of the lease `paths` at `path` through a temporary file and a rename, with its
/// directory, while `guard` holds; the check and the write under the lock of the lease. Answers whether it wrote
///
/// # Errors
/// The lock was not taken, the creation of the directory or the write failed
pub(crate) fn write_lease_file(guard: &LeaseGuard, paths: &LeasePaths, path: &Path, bytes: &[u8]) -> std::io::Result<bool> {
  debug_assert!(paths.holds(path), "{} is no file of the lease in {}", path.display(), paths.dir.display());
  let _lock = lock_lease(paths, &|| debug!("waiting for the lock of the lease"))?;
  if !guard.holds(path) {
    return Ok(false);
  }
  if let Some(dir) = path.parent() {
    std::fs::create_dir_all(dir)?;
  }
  write_atomically(path, bytes, AtomicWrite::default())?;
  Ok(true)
}

/// Removes the file of the lease `paths` at `path` while `guard` holds, under the lock of the lease, one gone already
/// being no failure; a failure is only logged, as `what`. A file that is not there takes no lock and asks no guard: a
/// removal that finds it gone before the lock comes before a write that makes it again, as one that took the lock
/// first would, and one that waited for the lock would find it gone the same. Every sync removes the mark of a build
/// before (`mark_synced`), which is almost never there, and took the lock for it: a holder file written and removed
/// beside the operations registry at each sync (in a full E2E run the events of that holder file fell from 28 466 to
/// 18 061 without it), and a wait of up to `LEASE_LOCK_TIMEOUT` behind another process at the end of the sync
pub(crate) fn remove_lease_file(guard: &LeaseGuard, paths: &LeasePaths, path: &Path, what: &str) {
  debug_assert!(paths.holds(path), "{} is no file of the lease in {}", path.display(), paths.dir.display());
  // not followed: a link left there is a file of the lease to remove, whatever it names
  if std::fs::symlink_metadata(path).is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound) {
    return;
  }
  let _lock = match lock_lease(paths, &|| debug!("waiting for the lock of the lease")) {
    Ok(lock) => lock,
    Err(error) => {
      warn!("cannot remove {what} {}: {error}", path.display());
      return;
    }
  };
  if !guard.holds(path) {
    return;
  }
  match fs_retry::remove_file(path) {
    Err(error) if error.kind() != std::io::ErrorKind::NotFound => warn!("cannot remove {what} {}: {error}", path.display()),
    _ => {}
  }
}

/// The lock of a read-modify-write of the landed state of `paths` (`landed`), beside it, within `timeout`; the lock is
/// no file of the lease
///
/// # Errors
/// The directory of the lock could not be made, or the lock was not taken in time
pub(crate) fn lock_landed(paths: &LeasePaths, timeout: Duration) -> anyhow::Result<RepositoryLock> {
  std::fs::create_dir_all(&paths.dir)?;
  lock_file(paths.landed_lock(), timeout, &|| debug!("waiting for the lock of the landed state"))
}

/// Takes every file of the lease (`LeasePaths::all`) out of the git directory of the worktree whose top level is
/// `worktree`, under the lock of the lease; one that is gone already is no failure
///
/// # Errors
/// The lock was not taken, or the first removal that failed; the others are tried all the same
pub fn forget_lease_state(worktree: &Path) -> std::io::Result<()> {
  forget_lease_state_with(worktree, &|| debug!("waiting for the lock of the lease to forget it"))
}

/// `forget_lease_state`, `on_wait` called once when another holds the lock of the lease
pub(crate) fn forget_lease_state_with(worktree: &Path, on_wait: &dyn Fn()) -> std::io::Result<()> {
  let Some(paths) = LeasePaths::of_worktree(worktree) else {
    return Ok(());
  };
  let _lock = lock_lease(&paths, on_wait)?;
  let mut failed = None;
  for path in paths.all() {
    let removed = if path.is_dir() { fs_retry::remove_dir_all(&path) } else { fs_retry::remove_file(&path) };
    match removed {
      Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
        failed.get_or_insert(error);
      }
      _ => {}
    }
  }
  failed.map_or(Ok(()), Err)
}
