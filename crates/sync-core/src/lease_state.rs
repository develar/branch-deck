//! What a worktree keeps of its own history in its git directory, which the git directory of a slot of a lease pool
//! outlives (openspec/specs/cli/lease/spec.md, "A return takes out what the lease left"): the mark of a sync
//! (`synced_mark`) and the landed state (`landed`, where each branch was at the last sync and a report of what landed
//! not dismissed yet). Each is of one lease of the slot, so all of them go as the lease ends, by the one list here
//! (`LEASE_FILES`): the return of the lease, a takeover and a reclaim, and the get that reuses a slot an older build left
//! with them. The next lease inherits none: a window that opens it shows the Ready to Sync card, and nothing landed is
//! reported of a branch it never had. Their locks stay (`landed.lock`), as no clean rule deletes a lock of git or of
//! Branch Deck.

use crate::landed::STATE_FILE;
use crate::synced_mark::{SYNCED_DIR, SYNCED_FILE};
use git_ops::lease_slot::git_dir_of;
use std::path::Path;
use sync_utils::fs_retry;

/// The files and the directories of the git directory of a worktree that are of its lease, as `rev-parse --git-path`
/// names them
pub const LEASE_FILES: [&str; 3] = [SYNCED_DIR, SYNCED_FILE, STATE_FILE];

/// Takes every file of the lease (`LEASE_FILES`) out of the git directory of the worktree whose top level is `worktree`;
/// one that is gone already is no failure
///
/// # Errors
/// The first removal that failed; the others are tried all the same
pub fn forget_lease_state(worktree: &Path) -> std::io::Result<()> {
  let Some(git_dir) = git_dir_of(worktree) else {
    return Ok(());
  };
  let mut failed = None;
  for file in LEASE_FILES {
    let path = git_dir.join(file);
    let removed = if path.is_dir() { fs_retry::remove_dir_all(&path) } else { fs_retry::remove_file(&path) };
    match removed {
      Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
        failed.get_or_insert(error);
      }
      _ => {}
    }
  }
  failed.map_or(Ok(()), Err)
}
