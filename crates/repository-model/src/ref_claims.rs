//! The remote refs this process writes now, by `$GIT_COMMON_DIR` and full name: a push of the app or of `bd`
//! (`operations::land_execute`) and the fetch of the remote branches whose pull requests moved
//! (`moved_remote::fetch_asked`) claim the free ones of the refs they write, without a wait. A fetch leaves the refs a
//! push writes to the push, whose outcome is newer, so it fetches nothing for them. A push waits for no fetch of its
//! refs: the fetch moves each remote-tracking ref from the tip it read before (`sync_core::push_lease`,
//! `fetch_remote_branches`), so it never writes over the head the push set. Two pushes of one ref never meet here: the
//! registry of the operations admits them one after the other (`Resource::Ref`), but this set also covers the sessions
//! of two worktrees of one repository.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The refs claimed in this process
static CLAIMED_REFS: RefClaims = RefClaims::new();

struct RefClaims {
  refs: Mutex<BTreeSet<(PathBuf, String)>>,
}

/// The refs of a claim, released when dropped
pub struct RefClaim {
  refs: Vec<(PathBuf, String)>,
}

impl RefClaims {
  const fn new() -> Self {
    Self {
      refs: Mutex::new(BTreeSet::new()),
    }
  }

  /// The set changes in a few lines that don't panic, so a poisoned one is still in order
  fn lock(&self) -> MutexGuard<'_, BTreeSet<(PathBuf, String)>> {
    self.refs.lock().unwrap_or_else(PoisonError::into_inner)
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
  }
}

#[cfg(test)]
#[path = "ref_claims_test.rs"]
mod ref_claims_test;
