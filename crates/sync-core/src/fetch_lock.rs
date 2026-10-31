//! The lock of the fetches of the baseline (docs/design/git-operations.md, "Update project", Fetch lock): a fetch of
//! the baseline (`baseline::fetch_baseline`: the app's Update Project and Fetch, `bd update`) holds
//! `$GIT_COMMON_DIR/branch-deck/fetch.lock` for its whole fetch, its holder record naming the tracking ref it writes
//! (`LockHolder::subject`). A second fetch of any process waits for it while the holder lives (the line of its own
//! process first, then the file lock), its progress naming the holder, and takes the result of a holder of the same
//! tracking ref instead of a transfer of its own: that holder fetched it after the wait began (its outcome,
//! `fetch-outcomes.json`, which a holder writes before it releases the lock), also when the remote had nothing new, or
//! the ref moved while it waited. A holder that failed or was killed (which frees the lock with its process) and a
//! holder of another tracking ref leave the fetch to it, under the lock. The app stops a wait with its operation
//! (`canceled`, `LockWaitCanceled`). Without the lock, two fetches of one ref (the app beside `bd update` of a lease,
//! one common dir) ran two transfers, and the second one to write the ref failed on git's own ref lock (`cannot lock
//! ref`).

use crate::baseline::BaselineUpstream;
use crate::transfer_progress::TransferProgress;
use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::{git_common_dir, is_ancestor, resolve_commit};
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, LockHolder, RepositoryLock, lock_as_another_process_for, lock_file_while_held_for};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use tracing::{debug, warn};

/// The lock file in `$GIT_COMMON_DIR/branch-deck/`; its holder record is `fetch.lock.holder` beside it
const FETCH_LOCK: &str = "fetch.lock";

/// The outcomes beside the lock file: per tracking ref, when its last fetch that ended well ended (`Outcome`)
const FETCH_OUTCOMES: &str = "fetch-outcomes.json";

/// A fetch of a tracking ref that ended well, as its holder wrote it under the lock
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Outcome {
  /// When it ended, Unix milliseconds
  fetched_at: u64,
}

/// What a fetch of the baseline does once its turn came
#[derive(Debug)]
pub(crate) enum FetchTurn {
  /// A fetch of the same tracking ref ended well while this one waited (its outcome), or the ref moved meanwhile: its
  /// result is this one's. The tip before it when the move was a forced update (not a fast-forward), as the fetch
  /// itself returns it
  Joined { previous: Option<String> },
  /// The fetch is this one's, under the lock
  Own(OwnFetch),
}

/// A fetch under the fetch lock, held until dropped or `fetched`
#[derive(Debug)]
pub(crate) struct OwnFetch {
  lock: RepositoryLock,
  outcomes: PathBuf,
  tracking_ref: String,
}

impl OwnFetch {
  /// The fetch ended well: its outcome is written for the fetches that wait for the lock (best effort: without it they
  /// join only a ref that moved), then the lock goes
  pub(crate) fn fetched(self) {
    let mut outcomes = read_outcomes(&self.outcomes);
    outcomes.insert(
      self.tracking_ref.clone(),
      Outcome {
        fetched_at: unix_millis(SystemTime::now()),
      },
    );
    let written = serde_json::to_vec(&outcomes)
      .map_err(std::io::Error::other)
      .and_then(|content| write_atomically(&self.outcomes, &content, AtomicWrite::default()));
    if let Err(e) = written {
      warn!(error = %e, path = %self.outcomes.display(), "cannot write the outcome of the fetch");
    }
    drop(self.lock);
  }
}

/// The outcomes as they are on disk; none for a missing or unreadable file
fn read_outcomes(path: &Path) -> BTreeMap<String, Outcome> {
  std::fs::read(path).ok().and_then(|content| serde_json::from_slice(&content).ok()).unwrap_or_default()
}

fn unix_millis(time: SystemTime) -> u64 {
  time.duration_since(UNIX_EPOCH).map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
}

/// The turn of a fetch of `upstream`: the lock, after the fetches before it (as long as their holders live; only a lock
/// that no record names for `DEFAULT_LOCK_TIMEOUT` refuses, `RepositoryLocked`), or the result of one of them.
/// `on_progress` hears whom the fetch waits for ("Fetching origin/master with `bd update` (pid 4242, for 3 s)…"), when
/// the holder changes and about once a second; `canceled` (polled as often) ends the wait with `LockWaitCanceled`.
/// Without a wait no process runs but the `rev-parse` of the common dir when the executor does not know it; a wait for a
/// holder of the same ref reads the ref when it starts and once the lock is free, the ancestry of a moved ref, and the
/// outcomes of the fetches (a file)
pub(crate) fn take_fetch_turn(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  upstream: &BaselineUpstream,
  on_progress: &mut dyn FnMut(TransferProgress),
  canceled: &dyn Fn() -> bool,
) -> Result<FetchTurn> {
  let dir = Path::new(&git_common_dir(git_executor, repo_path)?).join("branch-deck");
  std::fs::create_dir_all(&dir).with_context(|| format!("Cannot create {}", dir.display()))?;
  let tracking_ref = upstream.tracking_ref.as_str();
  // the tip of the tracking ref and the time when a holder that fetches it was first seen; none while no such holder was
  let mut before: Option<(Option<String>, SystemTime)> = None;
  let mut on_holder = |holder: &LockHolder| {
    if before.is_none() && holder.subject.as_deref() == Some(tracking_ref) {
      let seen = SystemTime::now();
      match resolve_commit(git_executor, repo_path, tracking_ref) {
        Ok(tip) => before = Some((tip, seen)),
        // no join then: the fetch is this one's once the lock is free
        Err(e) => warn!(error = %format!("{e:#}"), "cannot read the tracking ref before the wait for another fetch"),
      }
    }
    on_progress(waiting_progress(&upstream.name, holder));
  };
  let lock = lock_file_while_held_for(dir.join(FETCH_LOCK), tracking_ref, DEFAULT_LOCK_TIMEOUT, &mut on_holder, canceled)?;
  let outcomes = dir.join(FETCH_OUTCOMES);
  let Some((before, seen)) = before else {
    return Ok(FetchTurn::Own(OwnFetch {
      lock,
      outcomes,
      tracking_ref: tracking_ref.to_string(),
    }));
  };
  // the holder waited for fetched the ref after the wait began, though the remote may have had nothing new
  let fetched_meanwhile = read_outcomes(&outcomes).get(tracking_ref).is_some_and(|outcome| outcome.fetched_at >= unix_millis(seen));
  match resolve_commit(git_executor, repo_path, tracking_ref)? {
    Some(after) if before.as_deref() != Some(after.as_str()) => {
      let previous = match before {
        Some(before) if !is_ancestor(git_executor, repo_path, &before, &after)? => Some(before),
        _ => None,
      };
      debug!(tracking_ref, ?previous, "joined the fetch of another holder");
      Ok(FetchTurn::Joined { previous })
    }
    _ if fetched_meanwhile => {
      debug!(tracking_ref, "joined the fetch of another holder, which found nothing new");
      Ok(FetchTurn::Joined { previous: None })
    }
    _ => {
      debug!(tracking_ref, "the fetch waited for did not move the tracking ref and did not end well, fetching");
      Ok(FetchTurn::Own(OwnFetch {
        lock,
        outcomes,
        tracking_ref: tracking_ref.to_string(),
      }))
    }
  }
}

/// The fetch lock of the repository at `git_common_dir` as another process holds it while it fetches `tracking_ref` (an
/// agent's `bd update`, `lock_as_another_process_for`), its record naming `command`: a fetch of this process waits for
/// it, then takes its result when the ref moved, else fetches itself. For the tests and the E2E server, which hold a fetch
/// of the app this way
///
/// # Errors
/// A lock held already, a directory that can't be made
pub fn hold_fetch_as_another_process(git_common_dir: &Path, tracking_ref: &str, command: &str) -> Result<RepositoryLock> {
  let dir = git_common_dir.join("branch-deck");
  std::fs::create_dir_all(&dir).with_context(|| format!("Cannot create {}", dir.display()))?;
  lock_as_another_process_for(dir.join(FETCH_LOCK), command, Some(tracking_ref))
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
