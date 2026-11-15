//! What a worktree keeps of its own history in its git directory, which the git directory of a slot of a lease pool
//! outlives (openspec/specs/cli/lease/spec.md, "A return takes out what the lease left"): the mark of a sync
//! (`synced_mark`) and the landed state (`landed`, where each branch was at the last sync and a report of what landed
//! not dismissed yet). Each is of one lease of the slot, so all of them go as the lease ends, by the one list here
//! (`LEASE_FILES`): the return of the lease, a takeover and a reclaim, and the get that reuses a slot an older build left
//! with them. The next lease inherits none: a window that opens it shows the Ready to Sync card, and nothing landed is
//! reported of a branch it never had. Their locks stay (`landed.lock`), as no clean rule deletes a lock of git or of
//! Branch Deck.
//!
//! This module is their one writer (`lease_state_scan_test.rs`): `synced_mark` and `landed` decide what the files hold,
//! and write and remove them only here, each write under the guard of its caller (`LeaseGuard`). A sync of the app's
//! session writes only while the slot holds the lease the run began under, so a run that a return and a new lease
//! overtook writes nothing into the files of the new lease; its writes before the return go with the forget of the
//! return, and one between that forget and the end of the return with the forget of the get that reuses the slot
//! (`lease_pool::lease::move_reused_slot`), which comes after its reservation changed the epoch.

use crate::landed::LANDED_FILE;
use crate::synced_mark::{SYNCED_DIR, SYNCED_FILE};
use git_ops::lease_slot::git_dir_of;
use history_rewrite::repository_lock::{RepositoryLock, lock_file};
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::fs_retry;
use tracing::{debug, warn};

/// The files and the directories of the git directory of a worktree that are of its lease, as `rev-parse --git-path`
/// names them
pub const LEASE_FILES: [&str; 3] = [SYNCED_DIR, SYNCED_FILE, LANDED_FILE];

/// Whether the worktree still holds the lease that the work that writes the files of its lease began under, asked right
/// before each write (`write_lease_file`, `remove_lease_file`). The app's session asks the epoch of the slot
/// (`GitSessionHost::run`); `brd` in a terminal, an intent of the user and a test write at any time (`any`)
#[derive(Clone, Default)]
pub struct LeaseGuard(Option<Arc<dyn Fn() -> bool + Send + Sync>>);

impl LeaseGuard {
  /// Writes at any time
  #[must_use]
  pub fn any() -> Self {
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

/// Writes `bytes` into the file of the lease at `path` through a temporary file and a rename, with its directory, while
/// `guard` holds; answers whether it wrote
///
/// # Errors
/// The creation of the directory or the write failed
pub(crate) fn write_lease_file(guard: &LeaseGuard, path: &Path, bytes: &[u8]) -> std::io::Result<bool> {
  if !guard.holds(path) {
    return Ok(false);
  }
  if let Some(dir) = path.parent() {
    std::fs::create_dir_all(dir)?;
  }
  write_atomically(path, bytes, AtomicWrite::default())?;
  Ok(true)
}

/// Removes the file of the lease at `path` while `guard` holds, one gone already being no failure; a failure is only
/// logged, as `what`
pub(crate) fn remove_lease_file(guard: &LeaseGuard, path: &Path, what: &str) {
  if !guard.holds(path) {
    return;
  }
  match fs_retry::remove_file(path) {
    Err(error) if error.kind() != std::io::ErrorKind::NotFound => warn!("cannot remove {what} {}: {error}", path.display()),
    _ => {}
  }
}

/// The lock `lock` of a read-modify-write of the landed state at `path` (`landed`), beside it, within `timeout`; the
/// lock is no file of the lease
///
/// # Errors
/// The directory of the lock could not be made, or the lock was not taken in time
pub(crate) fn lock_landed(path: &Path, lock: &str, timeout: Duration) -> anyhow::Result<RepositoryLock> {
  let dir = path.parent().ok_or_else(|| anyhow::anyhow!("landed state path without a directory"))?;
  std::fs::create_dir_all(dir)?;
  lock_file(dir.join(lock), timeout, &|| debug!("waiting for the lock of the landed state"))
}

/// Takes every file of the lease (`LEASE_FILES`) out of the git directory of the worktree whose top level is `worktree`;
/// one that is gone already is no failure
///
/// # Errors
/// The first removal that failed; the others are tried all the same
pub fn forget_lease_state(worktree: &Path) -> std::io::Result<()> {
  let Some(git_dir) = git_dir_of(worktree) else {
    return Ok(());
  };
  let mut failed = None;
  for file in LEASE_FILES {
    let path = git_dir.join(file);
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
