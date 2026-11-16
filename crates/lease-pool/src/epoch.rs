//! The epoch of a worktree (docs/design/git-operations.md, "The epoch of a worktree"): a slot of a lease pool keeps its
//! path from one lease to the next, so what the app records of a worktree by its path (the time of its last sync and the
//! rest of `repositoryState`, the frame of its session, a session kept idle) may be of an earlier lease. The epoch names
//! the lease the slot holds now (its id), or its last return while it is free; a path outside a pool has none. A record
//! of a worktree that names another known epoch is of another lease and counts as absent: the one rule every reader of
//! the state of a worktree by its path applies. An epoch the files cannot tell is `Unknown`, which decides nothing.
//!
//! Read without git and without the lock of the pool: the `.git` file of the worktree, its admin directory and the state
//! of the pool, which every writer replaces by a rename (`state_file`), so a read sees one whole state. A `.git` file
//! that names a git directory without a `commondir` (a submodule, a checkout of `git init --separate-git-dir`) is no
//! linked worktree, so no slot, which a read that fails never says. Only what the epoch needs is parsed (`PoolSlots`): a
//! state a later build wrote, with a state of a lease or a field this build does not know, still tells it. The parse of
//! a state is kept by its bytes, and what a `.git` file names by its bytes, so a read of an unchanged slot of an
//! unchanged pool reads two small files and parses nothing.

use crate::lease::POOL_FILE;
use domain_types::worktree_epoch::WorktreeEpoch;
use git_ops::lease_slot::STATE_DIR;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use sync_core::lease_state::LeaseGuard;
use sync_utils::paths::display_path;

/// The epoch of the worktree at `worktree`: `NotASlot` for a path that is no slot of a pool (the main checkout, a worktree
/// of `git worktree add`, a submodule, a checkout of a separate git directory, a path that is no git worktree, a
/// repository without a pool), `Lease` with the id of the lease of its slot or `returned <n>` of a free slot
/// (`returned <n> reclaimed <lease id>` while `Slot::reclaimed` names one), `Unknown` when a file that tells could not
/// be read or parsed
#[must_use]
pub fn worktree_epoch(worktree: &Path) -> WorktreeEpoch {
  let dot_git = worktree.join(".git");
  match std::fs::symlink_metadata(&dot_git) {
    Ok(metadata) if metadata.is_dir() => return WorktreeEpoch::NotASlot,
    Ok(_) => {}
    Err(error) if error.kind() == ErrorKind::NotFound => return WorktreeEpoch::NotASlot,
    Err(_) => return WorktreeEpoch::Unknown,
  }
  let linked = match linked_of(worktree, &dot_git) {
    Ok(Some(linked)) => linked,
    Ok(None) => return WorktreeEpoch::NotASlot,
    Err(Unreadable) => return WorktreeEpoch::Unknown,
  };
  match pool_slots(&linked.pool_file) {
    Ok(slots) => slots.get(&linked.path).cloned().map_or(WorktreeEpoch::NotASlot, WorktreeEpoch::Lease),
    Err(Unreadable) => WorktreeEpoch::Unknown,
  }
}

/// The guard of the writes of a work that begins now in the worktree at `worktree` into the files of its lease
/// (`sync_core::lease_state::LeaseGuard`): `brd -C <slot>` and an intent of the app write them only while the slot holds
/// the lease it holds now, so a work that a return and a new lease of the slot overtook writes nothing into the files of
/// the new one, from a directory outside the slot too, which the process check of a return does not see. A worktree that
/// is no slot writes at any time, as no next lease inherits its files
#[must_use]
pub fn lease_guard(worktree: &Path) -> LeaseGuard {
  lease_guard_under(worktree, worktree_epoch(worktree))
}

/// `lease_guard` of a work that began under `began`, the epoch of the worktree at `worktree` when it began (the lease of
/// the view of the session an intent was asked on): it writes while the worktree is of that epoch, at any time when it
/// is no slot. An epoch the files could not tell writes while they still cannot tell, as the guard of the session does
#[must_use]
pub fn lease_guard_under(worktree: &Path, began: WorktreeEpoch) -> LeaseGuard {
  if began == WorktreeEpoch::NotASlot {
    return LeaseGuard::any();
  }
  let worktree = worktree.to_path_buf();
  LeaseGuard::while_held(move || worktree_epoch(&worktree) == began)
}

/// A file that tells the epoch could not be read or parsed
struct Unreadable;

impl From<std::io::Error> for Unreadable {
  fn from(_error: std::io::Error) -> Self {
    Self
  }
}

/// What the epoch reads of a linked worktree: the state file of the pool of its common directory and its path as the
/// state names a slot
#[derive(Clone)]
struct Linked {
  pool_file: PathBuf,
  path: String,
}

/// What a `.git` file names by the top level of its worktree, with the bytes it had then: none for one that names no
/// linked worktree
type KeptWorktrees = HashMap<PathBuf, (Vec<u8>, Option<Linked>)>;

/// What each `.git` file read so far names
static WORKTREES: Mutex<Option<KeptWorktrees>> = Mutex::new(None);

/// The linked worktree at `worktree`, whose `.git` file is `dot_git`, `None` for a `.git` file of another kind: one
/// that names no git directory, or a git directory without a `commondir` (a submodule, a separate git directory) or not
/// under the `worktrees` of its common directory. A file that cannot be read is no answer (`Unreadable`)
fn linked_of(worktree: &Path, dot_git: &Path) -> Result<Option<Linked>, Unreadable> {
  let bytes = std::fs::read(dot_git)?;
  if let Some((kept, linked)) = WORKTREES.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(HashMap::new).get(worktree)
    && *kept == bytes
  {
    return Ok(linked.clone());
  }
  let linked = read_linked(worktree, &bytes)?;
  WORKTREES
    .lock()
    .unwrap_or_else(PoisonError::into_inner)
    .get_or_insert_with(HashMap::new)
    .insert(worktree.to_path_buf(), (bytes, linked.clone()));
  Ok(linked)
}

fn read_linked(worktree: &Path, dot_git: &[u8]) -> Result<Option<Linked>, Unreadable> {
  let text = String::from_utf8_lossy(dot_git);
  let Some(admin) = text.lines().next().and_then(|line| line.strip_prefix("gitdir:")).map(str::trim) else {
    return Ok(None);
  };
  let admin = std::fs::canonicalize(worktree.join(admin))?;
  let common = match std::fs::read_to_string(admin.join("commondir")) {
    Ok(common) => common,
    Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
    Err(error) => return Err(error.into()),
  };
  let common = std::fs::canonicalize(admin.join(common.trim()))?;
  if admin.parent() != Some(common.join("worktrees").as_path()) {
    return Ok(None);
  }
  Ok(Some(Linked {
    pool_file: common.join(STATE_DIR).join(POOL_FILE),
    path: display_path(&std::fs::canonicalize(worktree)?),
  }))
}

/// What the epoch reads of the state of a pool: the path of each slot and its lease or last return, with the lease that a
/// reclaim or a takeover took from it. Every other field, and the state of a lease, is not read
#[derive(Deserialize)]
struct PoolSlots {
  #[serde(default)]
  slots: Vec<SlotEpoch>,
}

#[derive(Deserialize)]
struct SlotEpoch {
  path: String,
  #[serde(default)]
  lease: Option<LeaseId>,
  #[serde(default)]
  returned: Option<u64>,
  #[serde(default)]
  reclaimed: Option<ReclaimedId>,
}

#[derive(Deserialize)]
struct LeaseId {
  id: String,
}

#[derive(Deserialize)]
struct ReclaimedId {
  lease_id: String,
}

/// The epoch of each slot of the state `bytes` of a pool by its path, `None` for a state that cannot be parsed
pub(crate) fn slot_epochs(bytes: &[u8]) -> Option<HashMap<String, String>> {
  let parsed: PoolSlots = serde_json::from_slice(bytes).ok()?;
  let epochs = parsed.slots.into_iter().map(|slot| {
    let epoch = match (slot.lease, slot.returned) {
      (Some(lease), _) => lease.id,
      // a reclaim can give a number of an earlier free period of the slot again (`lease::stamp_reclaimed`)
      (None, Some(returned)) => match slot.reclaimed {
        Some(reclaimed) => format!("returned {returned} reclaimed {}", reclaimed.lease_id),
        None => format!("returned {returned}"),
      },
      (None, None) => "free".to_owned(),
    };
    (slot.path, epoch)
  });
  Some(epochs.collect())
}

/// The epoch of each slot of a pool by its path, of the state file as it was when read
type SlotEpochs = Arc<HashMap<String, String>>;

/// The slots of a state file by its path, with the bytes it had when parsed
type KeptPools = HashMap<PathBuf, (Vec<u8>, SlotEpochs)>;

/// The slots of the state files parsed so far
static POOLS: Mutex<Option<KeptPools>> = Mutex::new(None);

/// The epoch of each slot of the pool whose state is `file`: empty for a pool that has no state yet. Parsed only when its
/// bytes changed since the last parse: a size, a time and an inode tell a change apart on APFS only (Linux keeps times by
/// the millisecond and gives an inode again, Windows has none and keeps times by the tick)
fn pool_slots(file: &Path) -> Result<SlotEpochs, Unreadable> {
  let bytes = match std::fs::read(file) {
    Ok(bytes) => bytes,
    Err(error) if error.kind() == ErrorKind::NotFound => return Ok(SlotEpochs::default()),
    Err(error) => return Err(error.into()),
  };
  if let Some((kept, slots)) = POOLS.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(HashMap::new).get(file)
    && *kept == bytes
  {
    return Ok(Arc::clone(slots));
  }
  let slots: SlotEpochs = Arc::new(slot_epochs(&bytes).ok_or(Unreadable)?);
  POOLS
    .lock()
    .unwrap_or_else(PoisonError::into_inner)
    .get_or_insert_with(HashMap::new)
    .insert(file.to_path_buf(), (bytes, Arc::clone(&slots)));
  Ok(slots)
}
