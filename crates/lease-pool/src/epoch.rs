//! The epoch of a worktree (docs/design/git-operations.md, "The epoch of a worktree"): a slot of a lease pool keeps its
//! path from one lease to the next, so what the app records of a worktree by its path (the time of its last sync and the
//! rest of `repositoryState`, the frame of its session, a session kept idle) may be of an earlier lease. The epoch names
//! the lease the slot holds now (its id), or its last return while it is free; a path outside a pool has none. A record
//! of a worktree that names another known epoch is of another lease and counts as absent: the one rule every reader of
//! the state of a worktree by its path applies. An epoch the files cannot tell is `Unknown`, which decides nothing.
//!
//! Read without git and without the lock of the pool: the `.git` file of the worktree, its admin directory and the state
//! of the pool, which every writer replaces by a rename (`state_file`), so a read sees one whole state. Only what the
//! epoch needs is parsed (`PoolSlots`): a state a later build wrote, with a state of a lease or a field this build does
//! not know, still tells it. The parsed slots are kept by the identity of the file (its size, its time and, on Unix, its
//! inode, which each rename changes), and what a linked worktree names by the identity of its `.git` file, so a read of
//! an unchanged slot of an unchanged pool is two `stat` calls.

use crate::lease::POOL_FILE;
use crate::state_file::STATE_DIR;
use domain_types::worktree_epoch::WorktreeEpoch;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;
use sync_utils::paths::display_path;

/// The epoch of the worktree at `worktree`: `NotASlot` for a path that is no slot of a pool (the main checkout, a worktree
/// of `git worktree add`, a path that is no git worktree, a repository without a pool), `Lease` with the id of the lease
/// of its slot or `returned <n>` of a free slot, `Unknown` when a file that tells could not be read or parsed
#[must_use]
pub fn worktree_epoch(worktree: &Path) -> WorktreeEpoch {
  let dot_git = worktree.join(".git");
  let identity = match std::fs::symlink_metadata(&dot_git) {
    Ok(metadata) if metadata.is_dir() => return WorktreeEpoch::NotASlot,
    Ok(metadata) => FileIdentity::of(&metadata),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return WorktreeEpoch::NotASlot,
    Err(_) => return WorktreeEpoch::Unknown,
  };
  let Some(linked) = linked_of(worktree, identity) else {
    return WorktreeEpoch::Unknown;
  };
  let Some(slots) = pool_slots(&linked.pool_file) else {
    return WorktreeEpoch::Unknown;
  };
  slots.get(&linked.path).cloned().map_or(WorktreeEpoch::NotASlot, WorktreeEpoch::Lease)
}

/// What the epoch reads of a linked worktree: the state file of the pool of its common directory and its path as the
/// state names a slot
#[derive(Clone)]
struct Linked {
  pool_file: PathBuf,
  path: String,
}

/// The linked worktrees read so far, by their top level, with the identity of their `.git` file then: a worktree
/// added anew at the path writes another one
static WORKTREES: Mutex<Option<HashMap<PathBuf, (FileIdentity, Linked)>>> = Mutex::new(None);

/// The linked worktree at `worktree`, whose `.git` file has `identity`: its admin directory names the common one; a
/// `.git` file of another kind (a submodule) is told apart by that directory only, so whatever stops the read is no
/// answer (`None`)
fn linked_of(worktree: &Path, identity: FileIdentity) -> Option<Linked> {
  if let Some((kept, linked)) = WORKTREES.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(HashMap::new).get(worktree)
    && *kept == identity
  {
    return Some(linked.clone());
  }
  let worktree_dirs = git_ops::lease_slot::linked_worktree(worktree)?;
  let linked = Linked {
    pool_file: worktree_dirs.git_common_dir.join(STATE_DIR).join(POOL_FILE),
    path: display_path(&std::fs::canonicalize(worktree).ok()?),
  };
  WORKTREES
    .lock()
    .unwrap_or_else(PoisonError::into_inner)
    .get_or_insert_with(HashMap::new)
    .insert(worktree.to_path_buf(), (identity, linked.clone()));
  Some(linked)
}

/// What the epoch reads of the state of a pool: the path of each slot and its lease or last return. Every other field,
/// and the state of a lease, is not read
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
}

#[derive(Deserialize)]
struct LeaseId {
  id: String,
}

/// The identity of a state file: a rename over it changes it
#[derive(Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
  len: u64,
  modified: Option<SystemTime>,
  inode: u64,
}

impl FileIdentity {
  fn of(metadata: &std::fs::Metadata) -> Self {
    Self {
      len: metadata.len(),
      modified: metadata.modified().ok(),
      inode: inode_of(metadata),
    }
  }
}

#[cfg(unix)]
fn inode_of(metadata: &std::fs::Metadata) -> u64 {
  std::os::unix::fs::MetadataExt::ino(metadata)
}

#[cfg(not(unix))]
const fn inode_of(_metadata: &std::fs::Metadata) -> u64 {
  0
}

/// The epoch of each slot of a pool by its path, of the state file as it was when read
type SlotEpochs = Arc<HashMap<String, String>>;

/// The slots of the state files read so far, by the path of each file, with the identity it had then
static POOLS: Mutex<Option<HashMap<PathBuf, (FileIdentity, SlotEpochs)>>> = Mutex::new(None);

/// The epoch of each slot of the pool whose state is `file`: empty for a pool that has no state yet, none when the file
/// cannot be read or parsed
fn pool_slots(file: &Path) -> Option<SlotEpochs> {
  let metadata = match std::fs::metadata(file) {
    Ok(metadata) => metadata,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Some(SlotEpochs::default()),
    Err(_) => return None,
  };
  let identity = FileIdentity::of(&metadata);
  if let Some((kept, slots)) = POOLS.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(HashMap::new).get(file)
    && *kept == identity
  {
    return Some(Arc::clone(slots));
  }
  let text = std::fs::read_to_string(file).ok()?;
  let parsed: PoolSlots = serde_json::from_str(&text).ok()?;
  let slots: SlotEpochs = Arc::new(
    parsed
      .slots
      .into_iter()
      .map(|slot| {
        let epoch = match (slot.lease, slot.returned) {
          (Some(lease), _) => lease.id,
          (None, Some(returned)) => format!("returned {returned}"),
          (None, None) => "free".to_owned(),
        };
        (slot.path, epoch)
      })
      .collect(),
  );
  // the identity the parse is of: a write since the stat is read again next time
  POOLS
    .lock()
    .unwrap_or_else(PoisonError::into_inner)
    .get_or_insert_with(HashMap::new)
    .insert(file.to_path_buf(), (identity, Arc::clone(&slots)));
  Some(slots)
}
