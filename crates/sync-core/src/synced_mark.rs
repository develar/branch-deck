//! The mark of a worktree that a sync of Branch Deck built its branches for (`branch-deck/synced` in the git directory of
//! the worktree, beside the landed state): each sync that succeeds names its branch prefix there, of the app and of
//! `brd` alike, so the session of the app of that prefix knows that the worktree was synced before, also by `brd sync`
//! in a terminal, which writes no state of the app (`SessionHost::synced_before`; openspec/specs/repository/onboarding/
//! spec.md, "A focus syncs nothing before the first sync"). A sync under another prefix (`brd sync --prefix x`) marks
//! that prefix only. It is read without a git process.

use git_ops::lease_slot::linked_worktree;
use std::path::{Path, PathBuf};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::fs_retry;
use tracing::warn;

/// The mark in the git directory of a worktree, as `rev-parse --git-path` names it: a linked worktree has its own. One
/// branch prefix per line, each one a sync succeeded under
pub const SYNCED_FILE: &str = "branch-deck/synced";

/// Names `prefix` in the mark at `path` (`RepoFacts::synced_mark`) once: a sync that finds it there writes nothing. A
/// failure is only logged, as the sync is done
pub fn mark_synced(path: &Path, prefix: &str) {
  let marked = std::fs::read_to_string(path).unwrap_or_default();
  if names(&marked, prefix) {
    return;
  }
  let content = format!("{marked}{prefix}\n");
  let written = path
    .parent()
    .map_or(Ok(()), std::fs::create_dir_all)
    .and_then(|()| write_atomically(path, content.as_bytes(), AtomicWrite::default()));
  if let Err(error) = written {
    warn!("cannot mark {} synced: {error}", path.display());
  }
}

/// Whether a sync of Branch Deck under `prefix` succeeded in the worktree whose top level is `worktree` (its mark names
/// it), without a git process: the git directory is `.git` of the main worktree, or the admin directory a linked worktree
/// names
pub fn synced_before(worktree: &str, prefix: &str) -> bool {
  git_dir_of(Path::new(worktree))
    .and_then(|git_dir| std::fs::read_to_string(git_dir.join(SYNCED_FILE)).ok())
    .is_some_and(|marked| names(&marked, prefix))
}

fn names(marked: &str, prefix: &str) -> bool {
  marked.lines().any(|line| line == prefix)
}

/// Takes the mark out of the worktree whose top level is `worktree`: a lease slot that goes back to its pool
/// (`brd lease return`), whose git directory the next lease of the slot gets, which no sync built anything for yet
///
/// # Errors
/// The removal of the mark failed
pub fn forget_synced(worktree: &Path) -> std::io::Result<()> {
  let Some(mark) = git_dir_of(worktree).map(|git_dir| git_dir.join(SYNCED_FILE)) else {
    return Ok(());
  };
  match fs_retry::remove_file(&mark) {
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
    removed => removed,
  }
}

fn git_dir_of(worktree: &Path) -> Option<PathBuf> {
  let dot_git = worktree.join(".git");
  if dot_git.is_dir() {
    return Some(dot_git);
  }
  linked_worktree(worktree).map(|linked| linked.git_dir)
}
