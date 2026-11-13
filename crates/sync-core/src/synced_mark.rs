//! The mark of a worktree that a sync of Branch Deck built its branches for (`branch-deck/synced` in the git directory of
//! the worktree, beside the landed state): each sync that succeeds leaves it, of the app and of `brd` alike, so the
//! session of the app knows that the worktree was synced before, also by `brd sync` in a terminal, which writes no
//! state of the app (`SessionHost::synced_before`; openspec/specs/repository/onboarding/spec.md, "A focus syncs nothing
//! before the first sync"). It is read without a git process.

use git_ops::lease_slot::linked_worktree;
use std::path::{Path, PathBuf};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::timestamp::unix_now;
use tracing::warn;

/// The mark in the git directory of a worktree, as `rev-parse --git-path` names it: a linked worktree has its own
pub const SYNCED_FILE: &str = "branch-deck/synced";

/// Leaves the mark at `path` (`RepoFacts::synced_mark`) once: a sync that finds it writes nothing. It says when the
/// first sync ended; a failure is only logged, as the sync is done
pub fn mark_synced(path: &Path) {
  if path.exists() {
    return;
  }
  let written = path
    .parent()
    .map_or(Ok(()), std::fs::create_dir_all)
    .and_then(|()| write_atomically(path, format!("{}\n", unix_now()).as_bytes(), AtomicWrite::default()));
  if let Err(error) = written {
    warn!("cannot mark {} synced: {error}", path.display());
  }
}

/// Whether a sync of Branch Deck succeeded in the worktree whose top level is `worktree` (its mark is there), without a
/// git process: the git directory is `.git` of the main worktree, or the admin directory a linked worktree names
pub fn synced_before(worktree: &str) -> bool {
  git_dir_of(Path::new(worktree)).is_some_and(|git_dir| git_dir.join(SYNCED_FILE).is_file())
}

fn git_dir_of(worktree: &Path) -> Option<PathBuf> {
  let dot_git = worktree.join(".git");
  if dot_git.is_dir() {
    return Some(dot_git);
  }
  linked_worktree(worktree).map(|linked| linked.git_dir)
}
