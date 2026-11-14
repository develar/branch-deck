//! The epoch of a worktree (docs/design/git-operations.md, "The epoch of a worktree"): a slot of a lease pool keeps its
//! path from one lease to the next, so what the app records of a worktree by its path (the time of its last sync and the
//! rest of `repositoryState`, the frame of its session, a session kept idle) may be of an earlier lease. The epoch names
//! the lease the slot holds now (its id), or its last return while it is free; a path outside a pool has none. A record
//! of a worktree that names another epoch is of another lease and counts as absent: the one rule every reader of the
//! state of a worktree by its path applies. Read without git: the `.git` file of the worktree, its admin directory and
//! the state of the pool.

use crate::lease::{Pool, canonical_path};
use std::path::Path;

/// The epoch of the worktree at `worktree`: the id of the lease of its slot, `returned <n>` of a free slot by the number
/// of its last return, `free` of a free slot never returned by this build; `None` for a worktree that is no slot of a
/// pool (the main checkout, a worktree of `git worktree add`) or whose pool cannot be read
#[must_use]
pub fn worktree_epoch(worktree: &Path) -> Option<String> {
  let linked = git_ops::lease_slot::linked_worktree(worktree)?;
  let store = Pool::new(&linked.git_common_dir).read().ok()?;
  let path = canonical_path(worktree);
  let slot = store.slots.iter().find(|slot| slot.path == path)?;
  Some(match (&slot.lease, slot.returned) {
    (Some(lease), _) => lease.id.clone(),
    (None, Some(returned)) => format!("returned {returned}"),
    (None, None) => "free".to_owned(),
  })
}
