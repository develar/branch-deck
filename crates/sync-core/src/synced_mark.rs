//! The mark of a worktree that a sync of Branch Deck built its branches for (`branch-deck/synced.d/` in the git
//! directory of the worktree, beside the landed state): each sync that succeeds names its branch prefix there, of the
//! app and of `brd` alike, so the session of the app of that prefix knows that the worktree was synced before, also by
//! `brd sync` in a terminal, which writes no state of the app (`SessionHost::synced_before`; openspec/specs/repository/
//! onboarding/spec.md, "A focus syncs nothing before the first sync"). A sync under another prefix (`brd sync --prefix
//! x`) marks that prefix only. Each prefix is a file of its own, written by a rename: two syncs under two prefixes at once
//! both land, as no write reads another. Its name is the SHA-256 of the prefix in hex, as long for every prefix (a name
//! of the prefix's own bytes was too long for a prefix of more than about 120 bytes), and its content is the prefix,
//! which the read checks. It is read without a git process, in the git directory the `.git` of the worktree names: the
//! admin directory of a linked worktree, `.git/modules/<name>` of a submodule, the directory of a separate git directory.
//!
//! A build before kept every prefix as a line of one file, `branch-deck/synced` (and one before it a time there), and
//! one after it a file per prefix named by the hex of its bytes: the read takes both, a time never naming a prefix, and
//! the next mark moves their prefixes into files of this build and removes them.

use git_ops::lease_slot::git_dir_of;
use sha2::{Digest, Sha256};
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
    if !names(&file, prefix) {
      let written = std::fs::create_dir_all(&dir).and_then(|()| write_atomically(&file, prefix.as_bytes(), AtomicWrite::default()));
      if let Err(error) = written {
        warn!("cannot mark {} synced under {prefix}: {error}", dir.display());
        return;
      }
    }
    if let Some(hex) = hex_name_of(prefix) {
      remove(&dir.join(hex), "the mark of a build before");
    }
  }
  if legacy.is_some() {
    remove(mark, "the mark of a build before");
  }
}

/// Whether a sync of Branch Deck under `prefix` succeeded in the worktree whose top level is `worktree` (its mark names
/// it), without a git process, in the git directory its `.git` names (`git_dir_of`)
pub fn synced_before(worktree: &str, prefix: &str) -> bool {
  let Some(git_dir) = git_dir_of(Path::new(worktree)) else {
    return false;
  };
  let dir = git_dir.join(SYNCED_DIR);
  names(&dir.join(file_name_of(prefix)), prefix)
    || hex_name_of(prefix).is_some_and(|hex| dir.join(hex).exists())
    || std::fs::read_to_string(git_dir.join(SYNCED_FILE)).is_ok_and(|legacy| prefixes_of(&legacy).any(|line| line == prefix))
}

/// Whether the file of a mark at `file` names `prefix`: its content is the prefix, and nothing else of the same hash
fn names(file: &Path, prefix: &str) -> bool {
  std::fs::read(file).is_ok_and(|content| content == prefix.as_bytes())
}

/// The prefixes a mark of a build before names: its lines but the time of a sync, which the build before it wrote
fn prefixes_of(legacy: &str) -> impl Iterator<Item = &str> {
  legacy.lines().filter(|line| !line.is_empty() && !line.bytes().all(|byte| byte.is_ascii_digit()))
}

/// The name of the file of `prefix` in the mark: the SHA-256 of its bytes in hex, one name of 64 characters for every
/// prefix, a prefix with `/` and a long one too
fn file_name_of(prefix: &str) -> String {
  hex(&Sha256::digest(prefix.as_bytes()))
}

/// The name of the file of `prefix` in the mark of the build before: its bytes in hex, none for a prefix whose name no
/// file system takes (255 bytes)
fn hex_name_of(prefix: &str) -> Option<String> {
  (prefix.len() <= 127).then(|| hex(prefix.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
  bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut name, byte| {
    let _ = write!(name, "{byte:02x}");
    name
  })
}

/// The directory of the mark beside `SYNCED_FILE` at `mark`
fn dir_of(mark: &Path) -> PathBuf {
  mark.with_file_name(Path::new(SYNCED_DIR).file_name().unwrap_or_default())
}

/// Removes `path`, a file gone already being no failure; a failure is only logged
fn remove(path: &Path, what: &str) {
  match fs_retry::remove_file(path) {
    Err(error) if error.kind() != std::io::ErrorKind::NotFound => warn!("cannot remove {what} {}: {error}", path.display()),
    _ => {}
  }
}
