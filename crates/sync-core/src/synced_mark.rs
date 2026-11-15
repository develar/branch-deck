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
//!
//! The mark is a file of the lease of a slot of a lease pool: it is written and removed only through `lease_state`,
//! under the guard of the sync (`LeaseGuard`), so a run of the app's session marks no slot leased again meanwhile.

use crate::lease_state::{LeaseGuard, LeasePaths, remove_lease_file, write_lease_file};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::Path;
use tracing::warn;

/// Names `prefix` in the mark of the files of the lease `paths` (`LeasePaths::marks`, `RepoFacts::lease`), and moves the
/// prefixes of a mark of a build before (`LeasePaths::legacy_mark`) into files of their own, each write while `guard`
/// holds. A failure is only logged, as the sync is done
pub fn mark_synced(guard: &LeaseGuard, paths: &LeasePaths, prefix: &str) {
  let (dir, mark) = (paths.marks(), paths.legacy_mark());
  let legacy = std::fs::read_to_string(&mark).ok();
  let mut marked: Vec<&str> = legacy.as_deref().map(prefixes_of).into_iter().flatten().collect();
  marked.push(prefix);
  for prefix in marked {
    let file = dir.join(file_name_of(prefix));
    if !names(&file, prefix) {
      match write_lease_file(guard, paths, &file, prefix.as_bytes()) {
        Ok(true) => {}
        Ok(false) => return,
        Err(error) => {
          warn!("cannot mark {} synced under {prefix}: {error}", dir.display());
          return;
        }
      }
    }
    if let Some(hex) = hex_name_of(prefix) {
      remove_lease_file(guard, paths, &dir.join(hex), "the mark of a build before");
    }
  }
  if legacy.is_some() {
    remove_lease_file(guard, paths, &mark, "the mark of a build before");
  }
}

/// Whether a sync of Branch Deck under `prefix` succeeded in the worktree whose top level is `worktree` (its mark names
/// it), without a git process, in the git directory its `.git` names (`git_dir_of`)
pub fn synced_before(worktree: &str, prefix: &str) -> bool {
  let Some(paths) = LeasePaths::of_worktree(Path::new(worktree)) else {
    return false;
  };
  let dir = paths.marks();
  names(&dir.join(file_name_of(prefix)), prefix)
    || hex_name_of(prefix).is_some_and(|hex| dir.join(hex).exists())
    || std::fs::read_to_string(paths.legacy_mark()).is_ok_and(|legacy| prefixes_of(&legacy).any(|line| line == prefix))
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
