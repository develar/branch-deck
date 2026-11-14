//! The mark of a worktree that a sync of Branch Deck built its branches for (`branch-deck/synced.d/` in the git
//! directory of the worktree, beside the landed state): each sync that succeeds names its branch prefix there, of the
//! app and of `brd` alike, so the session of the app of that prefix knows that the worktree was synced before, also by
//! `brd sync` in a terminal, which writes no state of the app (`SessionHost::synced_before`; openspec/specs/repository/
//! onboarding/spec.md, "A focus syncs nothing before the first sync"). A sync under another prefix (`brd sync --prefix
//! x`) marks that prefix only. Each prefix is a file of its own, its name the hex of the prefix, written by a rename: two
//! syncs under two prefixes at once both land, as no write reads another. It is read without a git process.
//!
//! A build before kept every prefix as a line of one file, `branch-deck/synced` (and one before it a time there): the
//! read takes its lines too, a time never naming a prefix, and the next mark moves its prefixes into files of their own
//! and removes it.

use git_ops::lease_slot::linked_worktree;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::fs_retry;
use tracing::warn;

/// The mark of a build before in the git directory of a worktree, as `rev-parse --git-path` names it: a linked worktree
/// has its own. One branch prefix per line (or the time of a sync, of the build before it)
pub const SYNCED_FILE: &str = "branch-deck/synced";

/// The directory of the mark beside `SYNCED_FILE`: a file for each prefix a sync succeeded under
pub const SYNCED_DIR: &str = "branch-deck/synced.d";

/// Names `prefix` in the mark of the git directory whose `SYNCED_FILE` is `mark` (`RepoFacts::synced_mark`), and moves
/// the prefixes of a mark of a build before into files of their own. A failure is only logged, as the sync is done
pub fn mark_synced(mark: &Path, prefix: &str) {
  let dir = dir_of(mark);
  let legacy = std::fs::read_to_string(mark).ok();
  let mut marked: Vec<&str> = legacy.as_deref().map(prefixes_of).into_iter().flatten().collect();
  marked.push(prefix);
  for prefix in marked {
    let file = dir.join(file_name_of(prefix));
    if file.exists() {
      continue;
    }
    let written = std::fs::create_dir_all(&dir).and_then(|()| write_atomically(&file, prefix.as_bytes(), AtomicWrite::default()));
    if let Err(error) = written {
      warn!("cannot mark {} synced under {prefix}: {error}", dir.display());
      return;
    }
  }
  if legacy.is_some() {
    remove(mark, "the mark of a build before");
  }
}

/// Whether a sync of Branch Deck under `prefix` succeeded in the worktree whose top level is `worktree` (its mark names
/// it), without a git process: the git directory is `.git` of the main worktree, or the admin directory a linked worktree
/// names
pub fn synced_before(worktree: &str, prefix: &str) -> bool {
  let Some(git_dir) = git_dir_of(Path::new(worktree)) else {
    return false;
  };
  git_dir.join(SYNCED_DIR).join(file_name_of(prefix)).exists()
    || std::fs::read_to_string(git_dir.join(SYNCED_FILE)).is_ok_and(|legacy| prefixes_of(&legacy).any(|line| line == prefix))
}

/// The prefixes a mark of a build before names: its lines but the time of a sync, which the build before it wrote
fn prefixes_of(legacy: &str) -> impl Iterator<Item = &str> {
  legacy.lines().filter(|line| !line.is_empty() && !line.bytes().all(|byte| byte.is_ascii_digit()))
}

/// The name of the file of `prefix` in the mark: its bytes in hex, so a prefix with `/` is one file
fn file_name_of(prefix: &str) -> String {
  prefix.bytes().fold(String::with_capacity(prefix.len() * 2), |mut name, byte| {
    let _ = write!(name, "{byte:02x}");
    name
  })
}

/// The directory of the mark beside `SYNCED_FILE` at `mark`
fn dir_of(mark: &Path) -> PathBuf {
  mark.with_file_name(Path::new(SYNCED_DIR).file_name().unwrap_or_default())
}

/// Takes the mark out of the worktree whose top level is `worktree`, of this build and of a build before: a lease slot
/// that goes back to its pool (`brd lease return`), whose git directory the next lease of the slot gets, which no sync
/// built anything for yet
///
/// # Errors
/// The removal of the mark failed
pub fn forget_synced(worktree: &Path) -> std::io::Result<()> {
  let Some(git_dir) = git_dir_of(worktree) else {
    return Ok(());
  };
  match fs_retry::remove_dir_all(git_dir.join(SYNCED_DIR)) {
    Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
    _ => {}
  }
  match fs_retry::remove_file(git_dir.join(SYNCED_FILE)) {
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
    removed => removed,
  }
}

/// Removes `path`, a file gone already being no failure; a failure is only logged
fn remove(path: &Path, what: &str) {
  match fs_retry::remove_file(path) {
    Err(error) if error.kind() != std::io::ErrorKind::NotFound => warn!("cannot remove {what} {}: {error}", path.display()),
    _ => {}
  }
}

fn git_dir_of(worktree: &Path) -> Option<PathBuf> {
  let dot_git = worktree.join(".git");
  if dot_git.is_dir() {
    return Some(dot_git);
  }
  linked_worktree(worktree).map(|linked| linked.git_dir)
}
