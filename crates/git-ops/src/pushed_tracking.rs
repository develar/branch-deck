//! The remote-tracking refs a push left behind (docs/design/git-operations.md, "Push and land"). git's push writes the
//! remote-tracking ref of each ref it pushed as it ends, but leaves it as it was, with a warning, when another git holds
//! that ref's lock at that moment (the fetch of the remote branches whose pull requests moved,
//! `sync_core::push_lease::fetch_remote_branches`, moves it in its own transaction). The push knows the head it sent and
//! the one it replaced (its lease), so it decides from the values of the refs, never from git's words: a ref still at
//! the head it replaced moves to the pushed head, with that head as its old value; a ref a fetch moved meanwhile stays.

use crate::ref_transaction::RefTransaction;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::BTreeMap;
use tracing::{debug, info};

/// The operation of the reflog entry of a ref it moves (after `ref_transaction::REFLOG_PREFIX`), which the push time of
/// a remote status counts as a push of this clone, as git's own `update by push`
pub const PUSH_TRACKING_OPERATION: &str = "push tracking ref";

/// A ref a push sent: its remote-tracking ref, the remote head the push replaced (`None`: the branch was new) and the
/// head it sent
#[derive(Debug, Clone, Copy)]
pub struct PushedTracking<'a> {
  pub tracking_ref: &'a str,
  pub replaced: Option<&'a str>,
  pub pushed: &'a str,
}

/// Move each of `pushed` whose remote-tracking ref is still at the head the push replaced (`now`, its tips read after
/// the push; `None` for a ref that does not exist) to the pushed head, in one `RefTransaction` with the replaced head as
/// the old value of each: no process when none lags. A refused transaction (a fetch moved one meanwhile, and its value
/// stands) moves none and is only logged; answers the refs it moved
pub fn catch_up_pushed_tracking(git_executor: &GitCommandExecutor, repo_path: &str, pushed: &[PushedTracking<'_>], now: &BTreeMap<String, Option<String>>) -> Vec<String> {
  let lagging: Vec<&PushedTracking<'_>> = pushed
    .iter()
    .filter(|pushed| now.get(pushed.tracking_ref).map(Option::as_deref) == Some(pushed.replaced) && pushed.replaced != Some(pushed.pushed))
    .collect();
  if lagging.is_empty() {
    return Vec::new();
  }
  let mut transaction = RefTransaction::new();
  for pushed in &lagging {
    match pushed.replaced {
      Some(replaced) => transaction.update(pushed.tracking_ref, pushed.pushed, Some(replaced)),
      None => transaction.create(pushed.tracking_ref, pushed.pushed),
    };
  }
  let moved: Vec<String> = lagging.iter().map(|pushed| pushed.tracking_ref.to_string()).collect();
  match transaction.commit(git_executor, repo_path, PUSH_TRACKING_OPERATION) {
    Ok(()) => {
      info!(?moved, "git's push left remote-tracking refs behind: moved to the pushed heads");
      moved
    }
    Err(e) => {
      debug!(?moved, error = %format!("{e:#}"), "a remote-tracking ref the push left behind moved meanwhile: its value stays");
      Vec::new()
    }
  }
}
