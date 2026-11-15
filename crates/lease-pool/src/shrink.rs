//! The shrink of the pools of this machine below the minimum free space (openspec/specs/cli/lease/spec.md, "Get"): a free slot
//! keeps the warm build output of its last lease (cargo keeps a unit while it is among the newest of its group,
//! `lease_clean`), 50 to 59 GiB of `target/` in each free slot of branch-deck on 2026-11-03, and a get reuses the free
//! slot returned last first (`lease::reserve`), so the warm output of the others is rarely used again. Once the trim
//! (`trim`) left the volume below `branchdeck.leaseMinFree`, a get applies the clean rules of each free slot of every
//! pool of the machine with keep 0 and age 0, the ones returned longest ago first, and the slot each pool's next get
//! takes last, until the volume has the minimum and its margin (`lease::shrink_target`): the worktree stays, only what
//! the rules name goes. Only the slots on the volume the get measures are candidates (`free_space::Volume`).
//!
//! First the shrink prunes the slots with a `ready` lease of an agent, by the cargo rules of each lease with their own
//! `keep` and `age` (`lease::prune_leased_slot`): a slot stays hot, as the units its builds read stay, and a long session
//! piled up 48 to 120 GB of stale units and objects in a slot on 2026-11-15. The largest build output goes first, as
//! `disk.json` of `pool-disk` last measured it (`ShrinkParams::cached_build`); a prune stamps the lease, and a slot is
//! pruned again only after the smallest `age` of its rules. Then the free slots, by their rules with keep 0 and age 0.
//!
//! A free slot is cleaned only while it is free, under the `cleaning` marker that keeps every get off it, as `brd lease clean`
//! cleans one; what the rules remove goes into the trash of its pool and is deleted once the marker is gone, and the
//! free space is measured after each slot (one `statfs`).
//!
//! The shrink and the eviction (`trim::evict_core`) of a get run under one lock of the machine (`lock_pressure`): each
//! measures only before its own next slot, so N gets below the minimum at once, as the threads of a `brd-host` fill
//! start, cleaned or deleted up to N - 1 slots more than the volume needed. The turn of a get ends its wait once the
//! volume has the minimum (`canceled`, one `statfs` a poll): the slots the holder deletes free it as they go, and a
//! holder that deletes a slot of idea-1 (1.4 million files, minutes under load) keeps no get of another repository
//! that needs nothing more behind it.

use crate::free_space::{AvailableBytes, Volume};
use crate::lease::{Lease, LeaseState, LeaseStore, Pool, SkipReason, SkippedSlot, Slot, busy_processes, lease_rules_dir, prune_leased_slot, release_marker};
use crate::lease_clean::{CleanMode, CleanOverrides, CleanRule, Cleaned, Trash, apply_clean_rules, read_clean_rules};
use crate::trim::{machine_pools, mark_free};
use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::RepositoryDirs;
use history_rewrite::repository_lock::{LockHolder, RepositoryLock, lock_file_while_held};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sync_utils::paths::display_path;
use sync_utils::process_scan::ProcessScan;
use tracing::{debug, instrument, warn};

/// The lock of the shrink and the eviction of the gets of this machine, under `~/.branch-deck`
const PRESSURE_LOCK_FILE: &str = "pressure.lock";

/// Take the lock of the shrink and the eviction of the gets of this machine: `~/.branch-deck/pressure.lock` of `home`,
/// the pools the shrink and the eviction walk, else (no home) beside the state of `pool`. The wait lasts while the
/// record of the holder names a live process (`lock_file_while_held`), as the shrink of another get takes minutes; one
/// no record names a live holder of refuses after `unknown_timeout`. The threads of one process take it in turn too.
/// `on_holder` hears the holder while the wait lasts; the wait ends with `LockWaitCanceled` once `canceled` holds (a
/// get: the volume has its minimum, `lease::pressure_turn`)
pub fn lock_pressure(home: Option<&Path>, pool: &Pool, unknown_timeout: Duration, on_holder: &mut dyn FnMut(&LockHolder), canceled: &dyn Fn() -> bool) -> Result<RepositoryLock> {
  let dir = home.map_or_else(|| pool.state_dir().to_path_buf(), |home| home.join(".branch-deck"));
  std::fs::create_dir_all(&dir).with_context(|| format!("Cannot create {}", dir.display()))?;
  lock_file_while_held(dir.join(PRESSURE_LOCK_FILE), unknown_timeout, on_holder, canceled)
}

/// The parameters of every cargo rule of a shrink: no unit of a free slot is kept, whatever its age
pub(crate) const DEEP: CleanOverrides = CleanOverrides {
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
  /// The free bytes at which it stops, also within the delete of a slot: the minimum and its margin
  /// (`lease::shrink_target`)
  pub target: u64,
  /// The free bytes of the volume of a path (`free_space::available_bytes`; a test tells a volume of its own)
  pub available: &'a AvailableBytes<'a>,
  /// The wait for the pool lock, for the `cleaning` marker of a slot
  pub lock_timeout: Duration,
  /// The bytes of the build dirs of each slot of the pool of a common dir as `disk.json` last measured them, by slot
  /// path, read without a walk (`pool_disk::cached_build_bytes`); `None`: the leased slots go by `leased_at` alone
  pub cached_build: Option<&'a CachedBuild<'a>>,
}

/// The bytes of the build dirs of each slot of the pool of a common dir, by slot path (`ShrinkParams::cached_build`)
pub type CachedBuild<'a> = dyn Fn(&Path) -> HashMap<String, u64> + 'a;

/// What a shrink did
#[derive(Debug, Clone, Default)]
pub struct Shrunk {
  /// The leased slots it pruned by the cargo rules of their leases, in the order it pruned them
  pub leased: Vec<ShrunkSlot>,
  /// The leased slots it left as they are, as cargo builds in them
  pub building: Vec<BuildingSlot>,
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
    self.leased.iter().chain(&self.slots).map(ShrunkSlot::freed_bytes).sum()
  }
}

/// A leased slot that a shrink left as it is: the lock of cargo of every profile dir its cargo rules found is held
#[derive(Debug, Clone)]
pub struct BuildingSlot {
  pub name: String,
  pub path: String,
  pub holder: String,
  /// The profile dirs whose lock is held, relative to the slot: `target/debug`
  pub profiles: Vec<String>,
}

/// A free slot whose build output a shrink deleted
#[derive(Debug, Clone)]
pub struct ShrunkSlot {
  pub name: String,
  pub path: String,
  /// The holder of the lease of a leased slot; `None` for a free slot
  pub holder: Option<String>,
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
/// so the shrink stops at the first slot past it. The delete of a slot asks one `statfs` every few hundred entries and
/// stops once the volume has it too, the rest left in the trash (`Trash::empty_until`): a slot of idea-1 is 1.4 million
/// files, minutes to delete, and the volume that has the target needs none of them. A free slot on another volume frees nothing on this one and is no
/// candidate. A free slot with a process in it stays (`busy`), as `brd lease clean` skips it; one without rules or whose
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
  // a delete under way stops at the target too: the rest waits in the trash
  let enough = || (params.available)(params.volume).is_some_and(|available| available >= params.target);
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
  if !prune_leased_slots(git_executor, params, (&pools, &volume), &enough, &mut shrunk) {
    return shrunk;
  }
  let slots: Vec<&[Slot]> = pools.iter().map(|(_, store)| store.slots.as_slice()).collect();
  let order = shrink_order(&slots, |slot| present(slot) && !shrunk.busy.iter().any(|busy| busy.path == slot.path));
  let marker = Lease::marker("shrink", "brd lease get (shrink)", LeaseState::Cleaning);
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
    // the slot is free again: its delete keeps no get waiting, and stops once the volume has the minimum
    let (trash_left, complete) = trash.empty_until(&mut cleaned, &enough);
    let cleaned_slot = ShrunkSlot {
      name: slot.name.clone(),
      path: slot.path.clone(),
      holder: None,
      git_common_dir: git_common_dir.clone(),
      cleaned,
      trash_left,
    };
    debug!(slot = %slot.path, freed = cleaned_slot.freed_bytes(), complete, "the build output of a free slot is deleted");
    shrunk.slots.push(cleaned_slot);
    if !complete {
      break;
    }
  }
  shrunk
}

/// Prune the leased slots of `pools` on `volume` by the cargo rules of their leases (`lease::prune_leased_slot`), in the
/// order of `leased_order`, until the volume has the target: before each slot one `statfs`, and the delete stops at it
/// too. A slot whose prune of the same lease is younger than the smallest `age` of its cargo rules is passed over, so is
/// a host slot and a lease in another state than `ready`. Whether the shrink goes on to the free slots: false once the
/// volume has the target or a delete stopped
fn prune_leased_slots(
  git_executor: &GitCommandExecutor,
  params: &ShrinkParams<'_>,
  (pools, volume): (&[(String, LeaseStore)], &Volume),
  enough: &dyn Fn() -> bool,
  shrunk: &mut Shrunk,
) -> bool {
  let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
  let mut candidates: Vec<LeasedCandidate<'_>> = Vec::new();
  for (index, (git_common_dir, store)) in pools.iter().enumerate() {
    let sizes = params.cached_build.map_or_default(|cached| cached(Path::new(git_common_dir)));
    for slot in &store.slots {
      let Some(lease) = slot.lease.as_ref().filter(|lease| lease.is_agent_ready()) else {
        continue;
      };
      if !Path::new(&slot.path).is_dir() || !volume.holds(Path::new(&slot.path)) {
        continue;
      }
      if let Some(pruned_at) = lease.pruned_at
        && now.saturating_sub(pruned_at) < smallest_cargo_age(lease, &slot.path).as_secs()
      {
        debug!(slot = %slot.path, "a leased slot was pruned within the age of its cargo rules");
        continue;
      }
      candidates.push(LeasedCandidate {
        pool: index,
        slot,
        lease,
        build_bytes: sizes.get(&slot.path).copied(),
      });
    }
  }
  for candidate in leased_order(candidates) {
    if (params.available)(params.volume).is_none_or(|available| available >= params.target) {
      return false;
    }
    let git_common_dir = &pools[candidate.pool].0;
    let pool = Pool::new(Path::new(git_common_dir));
    let trash = Trash::new(&pool.trash_dir());
    let pruned = match prune_leased_slot(git_executor, &pool, params.lock_timeout, (&candidate.slot.path, candidate.lease), CleanMode::Trash(&trash)) {
      Ok(Some(pruned)) => pruned,
      Ok(None) => {
        debug!(slot = %candidate.slot.path, "the lease of a slot the shrink chose changed");
        continue;
      }
      Err(error) => {
        warn!(slot = %candidate.slot.path, error = %format!("{error:#}"), "cannot prune a leased slot");
        continue;
      }
    };
    if !pruned.building.is_empty() {
      shrunk.building.push(BuildingSlot {
        name: candidate.slot.name.clone(),
        path: candidate.slot.path.clone(),
        holder: candidate.lease.holder.clone(),
        profiles: pruned.building,
      });
      continue;
    }
    let mut cleaned = pruned.cleaned;
    let (trash_left, complete) = trash.empty_until(&mut cleaned, enough);
    let pruned_slot = ShrunkSlot {
      name: candidate.slot.name.clone(),
      path: candidate.slot.path.clone(),
      holder: Some(candidate.lease.holder.clone()),
      git_common_dir: git_common_dir.clone(),
      cleaned,
      trash_left,
    };
    debug!(slot = %candidate.slot.path, freed = pruned_slot.freed_bytes(), complete, "the stale build output of a leased slot is pruned");
    shrunk.leased.push(pruned_slot);
    if !complete {
      return false;
    }
  }
  true
}

/// The smallest `age` of the cargo rules of `lease` of the slot `path`; zero without one or with rules that can't be
/// read, so the slot is never passed over for its stamp then
fn smallest_cargo_age(lease: &Lease, path: &str) -> Duration {
  read_clean_rules(lease_rules_dir(lease, path))
    .unwrap_or_default()
    .iter()
    .filter_map(|rule| match rule {
      Ok(CleanRule::Cargo { age, .. }) => Some(*age),
      _ => None,
    })
    .min()
    .unwrap_or_default()
}

/// A leased slot the shrink may prune
pub struct LeasedCandidate<'a> {
  /// The index of its pool
  pub pool: usize,
  pub slot: &'a Slot,
  pub lease: &'a Lease,
  /// The bytes of its build dirs as `disk.json` last measured them, of any age
  pub build_bytes: Option<u64>,
}

/// The order of the leased slots a shrink prunes: the largest build output first, then the ones `disk.json` has no
/// size of, leased longest ago first. Before it, the order by `leased_at` alone walked the oldest lease first, which
/// can hold the smallest target
pub fn leased_order(mut candidates: Vec<LeasedCandidate<'_>>) -> Vec<LeasedCandidate<'_>> {
  candidates.sort_by_key(|candidate| (candidate.build_bytes.is_none(), Reverse(candidate.build_bytes.unwrap_or(0)), candidate.lease.leased_at));
  candidates
}

/// The free slots of `pools` that `usable` admits (its directory is there, no process works in it), in the order a
/// shrink takes them: the ones returned longest ago first across every pool (`Slot::returned_at`, then `returned` for a
/// slot a brd before that time returned; one never returned first of all), then, once no other is left, the slot of each
/// pool that its next get would take (clean before dirty, returned last first, as `lease::reserve` orders them), whose
/// warm output that get reuses. A slot with a lease of any state is no candidate. Each with the index of its pool
pub fn shrink_order<'a>(pools: &[&'a [Slot]], usable: impl Fn(&Slot) -> bool) -> Vec<(usize, &'a Slot)> {
  let mut first: Vec<(usize, &Slot)> = Vec::new();
  let mut last: Vec<(usize, &Slot)> = Vec::new();
  for (pool, slots) in pools.iter().enumerate() {
    let free: Vec<&Slot> = slots.iter().filter(|slot| slot.lease.is_none() && usable(slot)).collect();
    let next = next_free(free.iter().copied());
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

/// Of the free slots `free`, the one a get takes first, as `lease::reserve` orders them: clean before dirty, returned
/// last first, then the first never returned (the first of the minimum, as the stable sort of `reserve` takes it)
pub fn next_free<'a>(free: impl Iterator<Item = &'a Slot>) -> Option<&'a Slot> {
  free.min_by_key(|slot| (slot.dirty, Reverse(slot.returned)))
}

#[cfg(test)]
#[path = "shrink_test.rs"]
mod shrink_test;
