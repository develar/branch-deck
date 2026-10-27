//! The lock of the fetches of the baseline (docs/design/git-operations.md, "Update project", Fetch lock): a fetch of
//! the baseline (`baseline::fetch_baseline`: the app's Update Project and Fetch, `bd update`) holds
//! `$GIT_COMMON_DIR/branch-deck/fetch.lock` for its whole fetch, its holder record naming the tracking ref it writes
//! (`LockHolder::subject`). A second fetch of any process waits for it while the holder lives (the line of its own
//! process first, then the file lock), its progress naming the holder, and takes the result of a holder of the same
//! tracking ref instead of a transfer of its own: the ref moved while it waited. A ref that did not move (the remote had
//! nothing new, the holder failed or was killed, which frees the lock with its process) and a holder of another
//! tracking ref leave the fetch to it, under the lock. Without the lock, two fetches of one ref (the app beside
//! `bd update` of a lease, one common dir) ran two transfers, and the second one to write the ref failed on git's own
//! ref lock (`cannot lock ref`).

use crate::baseline::BaselineUpstream;
use crate::transfer_progress::TransferProgress;
use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::{git_common_dir, is_ancestor, resolve_commit};
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, LockHolder, RepositoryLock, lock_file_while_held_for};
use std::path::Path;
use tracing::{debug, warn};

/// The lock file in `$GIT_COMMON_DIR/branch-deck/`; its holder record is `fetch.lock.holder` beside it
const FETCH_LOCK: &str = "fetch.lock";

/// What a fetch of the baseline does once its turn came
#[derive(Debug)]
pub(crate) enum FetchTurn {
  /// A fetch of the same tracking ref ended while this one waited, and the ref moved: its result is this one's. The tip
  /// before it when the move was a forced update (not a fast-forward), as the fetch itself returns it
  Joined { previous: Option<String> },
  /// The fetch is this one's, under the lock, held until dropped
  Own(RepositoryLock),
}

/// The turn of a fetch of `upstream`: the lock, after the fetches before it (as long as their holders live; only a lock
/// that no record names for `DEFAULT_LOCK_TIMEOUT` refuses, `RepositoryLocked`), or the result of one of them.
/// `on_progress` hears whom the fetch waits for ("Fetching origin/master with `bd update` (pid 4242, for 3 s)…"), when
/// the holder changes and about once a second. Without a wait no process runs but the `rev-parse` of the common dir
/// when the executor does not know it; a wait for a holder of the same ref reads the ref when it starts and once the
/// lock is free, and the ancestry of a moved ref
pub(crate) fn take_fetch_turn(git_executor: &GitCommandExecutor, repo_path: &str, upstream: &BaselineUpstream, on_progress: &mut dyn FnMut(TransferProgress)) -> Result<FetchTurn> {
  let dir = Path::new(&git_common_dir(git_executor, repo_path)?).join("branch-deck");
  std::fs::create_dir_all(&dir).with_context(|| format!("Cannot create {}", dir.display()))?;
  let tracking_ref = upstream.tracking_ref.as_str();
  // the tip of the tracking ref when a holder that fetches it was first seen; none while no such holder was
  let mut before: Option<Option<String>> = None;
  let mut on_holder = |holder: &LockHolder| {
    if before.is_none() && holder.subject.as_deref() == Some(tracking_ref) {
      match resolve_commit(git_executor, repo_path, tracking_ref) {
        Ok(tip) => before = Some(tip),
        // no join then: the fetch is this one's once the lock is free
        Err(e) => warn!(error = %format!("{e:#}"), "cannot read the tracking ref before the wait for another fetch"),
      }
    }
    on_progress(waiting_progress(&upstream.name, holder));
  };
  let lock = lock_file_while_held_for(dir.join(FETCH_LOCK), tracking_ref, DEFAULT_LOCK_TIMEOUT, &mut on_holder, &|| false)?;
  let Some(before) = before else {
    return Ok(FetchTurn::Own(lock));
  };
  match resolve_commit(git_executor, repo_path, tracking_ref)? {
    Some(after) if before.as_deref() != Some(after.as_str()) => {
      let previous = match before {
        Some(before) if !is_ancestor(git_executor, repo_path, &before, &after)? => Some(before),
        _ => None,
      };
      debug!(tracking_ref, ?previous, "joined the fetch of another holder");
      Ok(FetchTurn::Joined { previous })
    }
    _ => {
      debug!(tracking_ref, "the fetch waited for did not move the tracking ref, fetching");
      Ok(FetchTurn::Own(lock))
    }
  }
}

/// The progress of a fetch that waits for `holder`: the line names it, the percent is 0 (the run of the page goes on by
/// the usual time of a fetch, `realFraction` of `update-progress.ts`)
fn waiting_progress(upstream: &str, holder: &LockHolder) -> TransferProgress {
  TransferProgress {
    percent: 0,
    transferred: None,
    speed: None,
    line: format!("Fetching {upstream} with {holder}…"),
  }
}

#[cfg(test)]
#[path = "fetch_lock_test.rs"]
mod fetch_lock_test;
