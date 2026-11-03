//! The shrink of the pools of this machine below the minimum free space (spec/cli/lease.spec.md, "Get"): a free slot
//! keeps the warm build output of its last lease (cargo keeps a unit while it is among the newest of its group,
//! `lease_clean`), 50 to 59 GiB of `target/` in each free slot of branch-deck on 2026-11-03, and a get reuses the free
//! slot returned last first (`lease::reserve`), so the warm output of the others is rarely used again. Once the trim
//! (`trim`) left the volume below `branchdeck.leaseMinFree`, a get applies the clean rules of each free slot of every
//! pool of the machine with keep 0 and age 0, the ones returned longest ago first, and the slot each pool's next get
//! takes last, until the volume has the minimum and its margin (`lease::shrink_target`): the worktree stays, only what
//! the rules name goes. Only the slots on the volume the get measures are candidates (`free_space::Volume`).
//!
//! A slot is cleaned only while it is free, under the `cleaning` marker that keeps every get off it, as `bd lease clean`
//! cleans one; what the rules remove goes into the trash of its pool and is deleted once the marker is gone, and the
//! free space is measured after each slot (one `statfs`).

use crate::free_space::{AvailableBytes, Volume};
use crate::lease::{Lease, LeaseState, LeaseStore, Pool, SkipReason, SkippedSlot, Slot, busy_processes, release_marker};
use crate::lease_clean::{CleanMode, CleanOverrides, Cleaned, Trash, apply_clean_rules, read_clean_rules};
use crate::trim::{machine_pools, mark_free};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::RepositoryDirs;
use std::cmp::Reverse;
use std::path::Path;
use std::time::Duration;
use sync_utils::paths::display_path;
use sync_utils::process_scan::ProcessScan;
use tracing::{debug, instrument, warn};

/// The parameters of every cargo rule of a shrink: no unit of a free slot is kept, whatever its age
const DEEP: CleanOverrides = CleanOverrides {
  keep: Some(0),
  age: Some(Duration::ZERO),
};

/// What `shrink_core` cleans
pub struct ShrinkParams<'a> {
  /// A checkout of the repository of the get: its pool is shrunk even outside `~/.branch-deck/pool`
  pub repository: &'a RepositoryDirs,
  /// The home directory of the default pools (`~/.branch-deck/pool`), whose pools are the machine's
  pub home: Option<&'a Path>,
  /// The volume whose free space the shrink raises: the pool directory of the get
  pub volume: &'a Path,
  /// The free bytes at which it stops: the minimum and its margin (`lease::shrink_target`)
  pub target: u64,
  /// The free bytes of the volume of a path (`free_space::available_bytes`; a test tells a volume of its own)
  pub available: &'a AvailableBytes<'a>,
  /// The wait for the pool lock, for the `cleaning` marker of a slot
  pub lock_timeout: Duration,
}

/// What a shrink did
#[derive(Debug, Clone, Default)]
pub struct Shrunk {
  /// The free slots it cleaned, in the order it cleaned them
  pub slots: Vec<ShrunkSlot>,
  /// The free slots it left alone because a process works in them
  pub busy: Vec<SkippedSlot>,
  /// Whether the processes were checked (`ProcessScan::Unavailable` on Windows)
  pub process_scan: bool,
}

impl Shrunk {
  /// The bytes on disk the deletes freed
  pub fn freed_bytes(&self) -> u64 {
    self.slots.iter().map(ShrunkSlot::freed_bytes).sum()
  }
}

/// A free slot whose build output a shrink deleted
#[derive(Debug, Clone)]
pub struct ShrunkSlot {
  pub name: String,
  pub path: String,
  /// `$GIT_COMMON_DIR` of its repository, canonical
  pub git_common_dir: String,
  /// What each of its rules did
  pub cleaned: Vec<Cleaned>,
  /// What its trash kept: a path that could not be deleted
  pub trash_left: Vec<String>,
}

impl ShrunkSlot {
  /// The bytes on disk its rules freed
  pub fn freed_bytes(&self) -> u64 {
    self.cleaned.iter().map(|cleaned| cleaned.freed_bytes).sum()
  }
}

/// Clean the free slots of every pool of this machine (the pool of `params.repository` and each pool under
/// `~/.branch-deck/pool`) on the volume of `params.volume` (`free_space::Volume`) by their own clean rules with keep 0
/// and age 0, in the order of `shrink_order`, until the volume has `params.target` free: before each slot one `statfs`,
/// so the shrink stops at the first slot past it. A free slot on another volume frees nothing on this one and is no
/// candidate. A free slot with a process in it stays (`busy`), as `bd lease clean` skips it; one without rules or whose
/// rules can't be read frees nothing and is passed over; one leased meanwhile is passed over. A failed rule never stops
/// the others
#[instrument(skip_all, fields(repository = %params.repository.top_level, target = params.target))]
pub fn shrink_core(git_executor: &GitCommandExecutor, params: &ShrinkParams<'_>) -> Shrunk {
  let pools: Vec<(String, LeaseStore)> = machine_pools(params.repository, params.home)
    .into_iter()
    .filter_map(|pool| match Pool::new(&pool.git_common_dir).read() {
      Ok(store) => Some((display_path(&pool.git_common_dir), store)),
      Err(error) => {
        warn!(pool = %pool.git_common_dir.display(), error = %format!("{error:#}"), "cannot read the state of a pool to shrink it");
        None
      }
    })
    .collect();
  let scan = ProcessScan::scan();
  let mut shrunk = Shrunk {
    process_scan: scan.is_available(),
    ..Shrunk::default()
  };
  let volume = Volume::of(params.volume);
  let present = |slot: &Slot| slot.lease.is_none() && Path::new(&slot.path).is_dir() && volume.holds(Path::new(&slot.path));
  for (_, store) in &pools {
    for slot in store.slots.iter().filter(|slot| present(slot)) {
      let processes = scan.inside(Path::new(&slot.path));
      if !processes.is_empty() {
        shrunk.busy.push(SkippedSlot {
          name: slot.name.clone(),
          path: slot.path.clone(),
          reason: SkipReason::Busy {
            processes: busy_processes(&processes),
          },
        });
      }
    }
  }
  let slots: Vec<&[Slot]> = pools.iter().map(|(_, store)| store.slots.as_slice()).collect();
  let order = shrink_order(&slots, |slot| present(slot) && !shrunk.busy.iter().any(|busy| busy.path == slot.path));
  let marker = Lease::marker("shrink", "bd lease get (shrink)", LeaseState::Cleaning);
  for (index, slot) in order {
    if (params.available)(params.volume).is_none_or(|available| available >= params.target) {
      break;
    }
    let git_common_dir = &pools[index].0;
    let rules = match read_clean_rules(Path::new(&slot.path)) {
      Ok(rules) if !rules.is_empty() => DEEP.apply(rules),
      Ok(_) => continue,
      Err(error) => {
        warn!(slot = %slot.path, error = %format!("{error:#}"), "cannot read the clean rules of a free slot to shrink it");
        continue;
      }
    };
    let pool = Pool::new(Path::new(git_common_dir));
    match mark_free(&pool, params.lock_timeout, &slot.path, &marker) {
      Ok(true) => {}
      Ok(false) => {
        debug!(slot = %slot.path, "a slot the shrink chose was leased meanwhile");
        continue;
      }
      Err(error) => {
        warn!(slot = %slot.path, error = %format!("{error:#}"), "cannot mark a slot for the shrink");
        continue;
      }
    }
    let trash = Trash::new(&pool.trash_dir());
    let mut cleaned = apply_clean_rules(git_executor, &slot.path, &rules, CleanMode::Trash(&trash));
    release_marker(&pool, params.lock_timeout, &slot.path, &marker.id);
    // the slot is free again: its delete keeps no get waiting
    let trash_left = trash.empty(&mut cleaned);
    let cleaned_slot = ShrunkSlot {
      name: slot.name.clone(),
      path: slot.path.clone(),
      git_common_dir: git_common_dir.clone(),
      cleaned,
      trash_left,
    };
    debug!(slot = %slot.path, freed = cleaned_slot.freed_bytes(), "the build output of a free slot is deleted");
    shrunk.slots.push(cleaned_slot);
  }
  shrunk
}

/// The free slots of `pools` that `usable` admits (its directory is there, no process works in it), in the order a
/// shrink takes them: the ones returned longest ago first across every pool (`Slot::returned_at`, then `returned` for a
/// slot a bd before that time returned; one never returned first of all), then, once no other is left, the slot of each
/// pool that its next get would take (clean before dirty, returned last first, as `lease::reserve` orders them), whose
/// warm output that get reuses. A slot with a lease of any state is no candidate. Each with the index of its pool
pub fn shrink_order<'a>(pools: &[&'a [Slot]], usable: impl Fn(&Slot) -> bool) -> Vec<(usize, &'a Slot)> {
  let mut first: Vec<(usize, &Slot)> = Vec::new();
  let mut last: Vec<(usize, &Slot)> = Vec::new();
  for (pool, slots) in pools.iter().enumerate() {
    let free: Vec<&Slot> = slots.iter().filter(|slot| slot.lease.is_none() && usable(slot)).collect();
    // the first of the minimum, as the stable sort of `reserve` takes it
    let next = free.iter().copied().min_by_key(|slot| (slot.dirty, Reverse(slot.returned)));
    for slot in free {
      if next.is_some_and(|next| std::ptr::eq(next, slot)) {
        last.push((pool, slot));
      } else {
        first.push((pool, slot));
      }
    }
  }
  // stable: the slots of the same time in the order of their pools and of the state
  first.sort_by_key(|(_, slot)| (slot.returned_at, slot.returned));
  last.sort_by_key(|(_, slot)| (slot.returned_at, slot.returned));
  first.extend(last);
  first
}

#[cfg(test)]
#[path = "shrink_test.rs"]
mod shrink_test;
