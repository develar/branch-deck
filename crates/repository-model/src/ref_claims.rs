//! The remote refs this process writes now, by `$GIT_COMMON_DIR` and full name: a push of the app or of `bd`
//! (`operations::push`) and the fetch of the remote branches whose pull requests moved (`moved_remote::fetch_asked`)
//! claim the refs they write, so the two never race for one remote-tracking ref. A push waits until the fetch of its
//! refs ended, as its outcome is newer than the fetch; a fetch takes only the refs no push writes and leaves the others
//! to the push. Two pushes of one ref never meet here: the registry of the operations admits them one after the other
//! (`Resource::Ref`), but this set also covers the sessions of two worktrees of one repository.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use tokio::sync::Notify;

/// The refs claimed in this process
static CLAIMED_REFS: RefClaims = RefClaims::new();

struct RefClaims {
  refs: Mutex<BTreeSet<(PathBuf, String)>>,
  released: Notify,
}

/// The refs of a claim, released when dropped
pub struct RefClaim {
  refs: Vec<(PathBuf, String)>,
}

impl RefClaims {
  const fn new() -> Self {
    Self {
      refs: Mutex::new(BTreeSet::new()),
      released: Notify::const_new(),
    }
  }

  /// The set changes in a few lines that don't panic, so a poisoned one is still in order
  fn lock(&self) -> MutexGuard<'_, BTreeSet<(PathBuf, String)>> {
    self.refs.lock().unwrap_or_else(PoisonError::into_inner)
  }
}

/// Claims all of `names` of `git_common_dir` at once, waiting without a bound while a claim of this process holds one of
/// them (a fetch of it runs; one fetch, so the wait is short); `on_wait` is called once when it waits
pub async fn claim_refs(git_common_dir: &Path, names: &[String], on_wait: impl FnOnce()) -> RefClaim {
  let refs: Vec<(PathBuf, String)> = names.iter().map(|name| (git_common_dir.to_path_buf(), name.clone())).collect();
  let mut on_wait = Some(on_wait);
  loop {
    let released = CLAIMED_REFS.released.notified();
    tokio::pin!(released);
    // registered before the check, so a release between the check and the wait wakes it
    released.as_mut().enable();
    {
      let mut claimed = CLAIMED_REFS.lock();
      if !refs.iter().any(|key| claimed.contains(key)) {
        claimed.extend(refs.iter().cloned());
        return RefClaim { refs };
      }
    }
    if let Some(on_wait) = on_wait.take() {
      on_wait();
    }
    released.await;
  }
}

/// Claims those of `names` of `git_common_dir` no claim of this process holds, at once, without waiting: answers the
/// claim and the names it claimed
#[must_use]
pub fn claim_free_refs(git_common_dir: &Path, names: &[String]) -> (RefClaim, BTreeSet<String>) {
  let mut claimed = CLAIMED_REFS.lock();
  let free: Vec<(PathBuf, String)> = names
    .iter()
    .map(|name| (git_common_dir.to_path_buf(), name.clone()))
    .filter(|key| !claimed.contains(key))
    .collect();
  claimed.extend(free.iter().cloned());
  drop(claimed);
  let names = free.iter().map(|(_, name)| name.clone()).collect();
  (RefClaim { refs: free }, names)
}

impl Drop for RefClaim {
  fn drop(&mut self) {
    if self.refs.is_empty() {
      return;
    }
    let mut claimed = CLAIMED_REFS.lock();
    for key in &self.refs {
      claimed.remove(key);
    }
    drop(claimed);
    CLAIMED_REFS.released.notify_waiters();
  }
}

#[cfg(test)]
#[path = "ref_claims_test.rs"]
mod ref_claims_test;
