//! The trim of the pools of this machine (openspec/specs/cli/lease/spec.md, "Trim"): a pool only grows, a get adds a slot when
//! none is free, and each slot keeps its build outputs, `target/` inside it and its Bazel output bases outside it
//! (`output_bases`), tens of gigabytes. Once the volume has less free space than `branchdeck.leaseMinFree`, a get
//! deletes the free slots past `branchdeck.leaseMaxSlots` of each pool of the machine, the ones returned longest ago
//! first, with their output bases, and the output bases of the workspaces of a pool that are gone; `brd lease trim`
//! does it on demand.
//!
//! A slot is deleted only while it is free, under the `cleaning` marker that keeps every get off it: its Bazel servers
//! stopped (another process in it skips it as busy), its directory renamed into the trash of its pool (`Trash`), the
//! admin directory dropped (`git_ops::lease_slot::forget_slot`), its entry dropped from the state, its output bases
//! renamed into the trash too. The deletes run after every pool is done, side by side, so the slot of the next get
//! never waits for them.
//!
//! A get that the trim and the shrink (`shrink`) leave below the minimum evicts free slots below the limit as well
//! (`evict_core`): the same removal of a slot, in the order of the shrink across every pool, each deleted before the
//! next one is chosen, until the volume has the minimum; only the slots on the volume of the get.

use crate::bazel_servers::child_dirs;
use crate::free_space::{AvailableBytes, Volume};
use crate::lease::{
  Lease, LeaseState, LeaseStore, Pool, ServerStop, SkipReason, SkippedSlot, Slot, busy_processes, canonical_path, release_marker, settle_abandoned, slot_servers, stop_servers,
};
use crate::lease_clean::{Trash, remove_measured};
use crate::output_bases::{OutputBase, orphans, output_bases, output_user_roots};
use crate::shrink::{next_free, shrink_order};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::{forget_slot, lease_config};
use git_ops::revs::RepositoryDirs;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use sync_utils::fs_retry;
use sync_utils::paths::display_path;
use sync_utils::process_scan::ProcessScan;
use sync_utils::span::in_current_span;
use tracing::{debug, instrument, warn};

/// How many slots a pool keeps once the free space is low, without `branchdeck.leaseMaxSlots`
pub const DEFAULT_MAX_SLOTS: usize = 10;

/// How many deletes of the trim run side by side: an output base of Bazel holds hundreds of thousands of files, and one
/// thread deletes one at a time
const DELETE_WORKERS: usize = 8;

/// What `trim_core` trims
pub struct TrimParams<'a> {
  /// A checkout of the repository that runs the trim: its pool is trimmed even outside `~/.branch-deck/pool`
  pub repository: &'a RepositoryDirs,
  /// The home directory of the default pools (`~/.branch-deck/pool`), whose pools are the machine's
  pub home: Option<&'a Path>,
  /// The limit of every pool over `branchdeck.leaseMaxSlots` of each repository (`--max`)
  pub max_slots: Option<usize>,
  /// Delete; false (`--plan`) only says what would go
  pub apply: bool,
  /// How long the trim waits for the Bazel servers of a slot it asked to exit
  pub server_timeout: Duration,
  /// The program of `bazel shutdown` on Windows; `None`: `bazel` of the `PATH`
  pub bazel: Option<&'a OsStr>,
  /// The wait for the pool lock and for the lock of the `worktree` commands
  pub lock_timeout: Duration,
}

/// What a trim did, or with a plan would do
#[derive(Debug, Clone, Default)]
pub struct Trimmed {
  pub pools: Vec<TrimmedPool>,
  /// The output bases of workspaces of a pool that are gone
  pub orphans: Vec<RemovedBase>,
  /// Whether the processes were checked (`ProcessScan::Unavailable` on Windows)
  pub process_scan: bool,
}

impl Trimmed {
  /// How many slots it deleted (with a plan: would delete)
  pub fn removed_slots(&self) -> usize {
    self.pools.iter().map(|pool| pool.removed.len()).sum()
  }

  /// The bytes on disk the deletes freed; 0 for a plan, which measures nothing
  pub fn freed_bytes(&self) -> u64 {
    let slots: u64 = self.pools.iter().flat_map(|pool| &pool.removed).map(RemovedSlot::freed_bytes).sum();
    slots + self.orphans.iter().map(|base| base.freed_bytes).sum::<u64>()
  }

  /// Whether it deleted (with a plan: would delete) anything
  pub fn is_empty(&self) -> bool {
    self.removed_slots() == 0 && self.orphans.is_empty()
  }
}

/// A pool of the machine and what the trim did in it
#[derive(Debug, Clone)]
pub struct TrimmedPool {
  /// `$GIT_COMMON_DIR` of its repository, canonical
  pub git_common_dir: String,
  /// The limit: `--max`, else `branchdeck.leaseMaxSlots` of the repository, else `DEFAULT_MAX_SLOTS`
  pub max_slots: usize,
  /// Its slots before the trim
  pub slots: usize,
  pub removed: Vec<RemovedSlot>,
  /// The free slots past the limit that stay: a process works in one, or its Bazel server did not exit
  pub skipped: Vec<SkippedSlot>,
  /// Why the limit of the repository could not be read: the limit is the default then
  pub config_error: Option<String>,
}

/// A slot the trim deleted
#[derive(Debug, Clone)]
pub struct RemovedSlot {
  pub name: String,
  pub path: String,
  /// The number of its last return in the pool (`Slot::returned`)
  pub returned: Option<u64>,
  /// The directory deleted with it: the number directory of the pool that holds it, else the slot itself
  pub dir: String,
  pub output_bases: Vec<RemovedBase>,
  /// The bytes on disk its directory freed
  pub dir_freed_bytes: u64,
  /// What failed after the slot left the state: the drop of its admin directory, a delete
  pub errors: Vec<String>,
}

impl RemovedSlot {
  /// The bytes its directory and its output bases freed
  pub fn freed_bytes(&self) -> u64 {
    self.dir_freed_bytes + self.output_bases.iter().map(|base| base.freed_bytes).sum::<u64>()
  }

  /// What the delete of `what`, moved into the trash at `path`, freed or why it failed
  fn record(&mut self, what: &Moved, path: &Path, result: std::io::Result<u64>) {
    match what {
      Moved::Dir => match result {
        Ok(freed) => self.dir_freed_bytes = freed,
        Err(error) => self.errors.push(delete_error(path, &error)),
      },
      Moved::Base(index) => self.output_bases[*index].record(path, result),
    }
  }
}

fn delete_error(path: &Path, error: &std::io::Error) -> String {
  format!("cannot delete {} in the trash: {error}", path.display())
}

/// An output base the trim deleted
#[derive(Debug, Clone)]
pub struct RemovedBase {
  pub path: String,
  /// The workspace it served
  pub workspace: String,
  pub freed_bytes: u64,
  pub error: Option<String>,
}

impl RemovedBase {
  /// What the delete of the output base, moved into the trash at `path`, freed or why it failed; an error of its move
  /// comes first
  pub(crate) fn record(&mut self, path: &Path, result: std::io::Result<u64>) {
    match result {
      Ok(freed) => self.freed_bytes = freed,
      Err(error) => self.error = self.error.take().or_else(|| Some(delete_error(path, &error))),
    }
  }

  pub(crate) fn of(base: &OutputBase) -> Self {
    Self {
      path: display_path(&base.path),
      workspace: display_path(&base.workspace),
      freed_bytes: 0,
      error: None,
    }
  }
}

/// A pool of this machine, by the common dir of its repository
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachinePool {
  pub git_common_dir: PathBuf,
}

/// The pools of this machine with the directories their slots live in and the Bazel output bases of every root the
/// checkouts of their repositories link into: what the trim and the disk report (`disk`) both start from, no git
pub struct Machine {
  pub pools: Vec<MachinePool>,
  pub pool_dirs: Vec<PathBuf>,
  pub bases: Vec<OutputBase>,
}

impl Machine {
  /// The pools of the repository of `repository` and of `~/.branch-deck/pool` of `home`
  pub fn read(repository: &RepositoryDirs, home: Option<&Path>) -> Self {
    let pools = machine_pools(repository, home);
    let pool_dirs = pool_dirs(home, &pools);
    // the roots of every checkout that may still have a `bazel-out` link: the source checkouts and every slot
    let mut checkouts: Vec<PathBuf> = vec![PathBuf::from(&repository.top_level)];
    for pool in &pools {
      if let Some(main) = main_worktree(&pool.git_common_dir) {
        checkouts.push(main);
      }
      if let Ok(store) = Pool::new(&pool.git_common_dir).read() {
        checkouts.extend(store.slots.iter().map(|slot| PathBuf::from(&slot.path)));
      }
    }
    let roots = output_user_roots(checkouts.iter().map(PathBuf::as_path));
    let bases = output_bases(&roots);
    debug!(pools = pools.len(), roots = roots.len(), bases = bases.len(), "the pools of the machine");
    Self { pools, pool_dirs, bases }
  }
}

/// What the trim moved into a trash and deletes at the end, with the report it adds the freed bytes to
enum Trashed {
  /// What the removal of the slot `slot` of the pool `pool` moved
  Slot { pool: usize, slot: usize, what: Moved },
  /// The orphan `index`
  Orphan(usize),
}

/// Trim every pool of this machine: the pool of `params.repository` and each pool under `~/.branch-deck/pool` whose
/// slots name their repository. In each pool the free slots past its limit go, the ones returned longest ago first and
/// the ones never returned before them (the last a get would take), each with the output bases of its workspaces;
/// then the output bases of workspaces of a pool that are gone. A slot with a process in it, or whose Bazel server
/// does not exit, stays and is named. Without `apply` nothing is written and only the state is read
#[instrument(skip_all, fields(repository = %params.repository.top_level))]
pub fn trim_core(git_executor: &GitCommandExecutor, params: &TrimParams<'_>) -> Result<Trimmed> {
  let Machine { pools, pool_dirs, bases } = Machine::read(params.repository, params.home);
  let scan = ProcessScan::scan();

  let mut trimmed = Trimmed {
    process_scan: scan.is_available(),
    ..Trimmed::default()
  };
  let mut trashed: Vec<(PathBuf, Trashed)> = Vec::new();
  let mut trashes: Vec<Trash> = Vec::new();
  for machine_pool in &pools {
    let pool_index = trimmed.pools.len();
    let pool = Pool::new(&machine_pool.git_common_dir);
    let trash = Trash::new(&pool.trash_dir());
    let (max_slots, config_error) = max_slots(git_executor, params.max_slots, &machine_pool.git_common_dir);
    let store = pool.read()?;
    let (present, free) = trim_order(&store.slots);
    let mut report = TrimmedPool {
      git_common_dir: display_path(&machine_pool.git_common_dir),
      max_slots,
      slots: present.len(),
      removed: Vec::new(),
      skipped: Vec::new(),
      config_error,
    };
    let excess = present.len().saturating_sub(max_slots);
    let removal = SlotRemoval {
      git_common_dir: &machine_pool.git_common_dir,
      pool: &pool,
      trash: &trash,
      bases: &bases,
      scan: &scan,
      params,
    };
    for slot in free {
      if report.removed.len() >= excess {
        break;
      }
      match removal.remove(git_executor, slot) {
        Removal::Removed(removed, moved) => {
          let slot_index = report.removed.len();
          trashed.extend(moved.into_iter().map(|(path, what)| {
            (
              path,
              Trashed::Slot {
                pool: pool_index,
                slot: slot_index,
                what,
              },
            )
          }));
          report.removed.push(*removed);
        }
        Removal::Skipped(skipped) => report.skipped.push(*skipped),
        Removal::Passed => {}
      }
    }
    trimmed.pools.push(report);
    trashes.push(trash);
  }

  // the output bases of workspaces of a pool that are gone, into the trash of the repository of the trim
  let own_trash = Trash::new(&Pool::new(Path::new(&canonical_path(Path::new(&params.repository.git_common_dir)))).trash_dir());
  let with_slots: Vec<&str> = trimmed
    .pools
    .iter()
    .flat_map(|pool| &pool.removed)
    .flat_map(|slot| &slot.output_bases)
    .map(|base| base.path.as_str())
    .collect();
  let orphans: Vec<&OutputBase> = orphans(&bases, &pool_dirs)
    .into_iter()
    .filter(|base| !with_slots.contains(&display_path(&base.path).as_str()))
    .collect();
  for base in orphans {
    if base.has_server() {
      debug!(output_base = %base.path.display(), "an output base of a gone workspace whose server runs stays");
      continue;
    }
    let index = trimmed.orphans.len();
    let mut removed = RemovedBase::of(base);
    if params.apply {
      match own_trash.move_in(&base.path) {
        Ok(moved) => trashed.push((moved, Trashed::Orphan(index))),
        Err(error) => removed.error = Some(format!("cannot move it into the trash: {error}")),
      }
    }
    trimmed.orphans.push(removed);
  }
  trashes.push(own_trash);

  for ((path, target), result) in trashed.iter().zip(remove_all(&trashed.iter().map(|(path, _)| path.as_path()).collect::<Vec<_>>())) {
    match target {
      Trashed::Slot { pool, slot, what } => trimmed.pools[*pool].removed[*slot].record(what, path, result),
      Trashed::Orphan(index) => trimmed.orphans[*index].record(path, result),
    }
  }
  for trash in &trashes {
    // the directory of this run, when every delete emptied it
    let _ = fs_retry::remove_dir(trash.dir());
  }
  Ok(trimmed)
}

/// What `evict_core` deletes
pub struct EvictParams<'a> {
  /// The repository, the home, the timeouts of a trim; `max_slots` is not read, and nothing is deleted without `apply`
  pub trim: TrimParams<'a>,
  /// The volume whose free space the eviction raises: the pool directory of the get
  pub volume: &'a Path,
  /// The free bytes at which it stops: the minimum, without the margin of the shrink
  pub target: u64,
  /// The free bytes of the volume of a path (`free_space::available_bytes`; a test tells a volume of its own)
  pub available: &'a AvailableBytes<'a>,
}

/// Delete free slots of the pools of this machine below their limit, as the trim deletes a slot past it, in the order of
/// the shrink (`shrink::shrink_order`: the ones returned longest ago first across every pool, the slot each pool's next
/// get would take after every other), one at a time until the volume has `params.target` free: each slot is deleted with
/// its output bases before the next `statfs`, so no slot goes that the volume did not need. A get runs it only below the
/// minimum after the trim and the shrink, whose clean rules left the free slots only their worktrees. Only the slots on
/// the volume of `params.volume` are candidates (`free_space::Volume`): the delete of another frees nothing there. A slot
/// with a lease of any state stays, as does one with a process in it but its Bazel servers, which are stopped. So do the
/// free slot the get takes next in its own pool (`shrink::next_free`), which it reuses warm where an eviction would make
/// it clone the same worktree again; a dirty one, whose changes made without a lease may be their only copy; and one
/// never returned, a worktree `brd lease adopt` kept where it was
#[instrument(skip_all, fields(repository = %params.trim.repository.top_level, target = params.target))]
pub fn evict_core(git_executor: &GitCommandExecutor, params: &EvictParams<'_>) -> Trimmed {
  let Machine { pools, bases, .. } = Machine::read(params.trim.repository, params.trim.home);
  let scan = ProcessScan::scan();
  let stores: Vec<(&MachinePool, LeaseStore)> = pools
    .iter()
    .filter_map(|pool| match Pool::new(&pool.git_common_dir).read() {
      Ok(store) => Some((pool, store)),
      Err(error) => {
        warn!(pool = %pool.git_common_dir.display(), error = %format!("{error:#}"), "cannot read the state of a pool to evict its slots");
        None
      }
    })
    .collect();
  let slots: Vec<&[Slot]> = stores.iter().map(|(_, store)| store.slots.as_slice()).collect();
  let volume = Volume::of(params.volume);
  let present = |slot: &Slot| {
    let path = Path::new(&slot.path);
    path.is_dir() && volume.holds(path) && slot_servers(&scan, path).1.is_empty()
  };
  let own = PathBuf::from(canonical_path(Path::new(&params.trim.repository.git_common_dir)));
  let kept = stores
    .iter()
    .find(|(pool, _)| pool.git_common_dir == own)
    .and_then(|(_, store)| next_free(store.slots.iter().filter(|slot| slot.lease.is_none() && present(slot))));
  let order = shrink_order(&slots, |slot| {
    present(slot) && !slot.dirty && slot.returned.is_some() && !kept.is_some_and(|kept| std::ptr::eq(kept, slot))
  });
  // the report of each pool, made as its first slot goes
  let mut reports: Vec<Option<TrimmedPool>> = stores.iter().map(|_| None).collect();
  for (index, slot) in order {
    if (params.available)(params.volume).is_none_or(|available| available >= params.target) {
      break;
    }
    let (machine_pool, store) = &stores[index];
    let pool = Pool::new(&machine_pool.git_common_dir);
    let trash = Trash::new(&pool.trash_dir());
    let report = reports[index].get_or_insert_with(|| {
      let (max_slots, config_error) = max_slots(git_executor, None, &machine_pool.git_common_dir);
      TrimmedPool {
        git_common_dir: display_path(&machine_pool.git_common_dir),
        max_slots,
        slots: trim_order(&store.slots).0.len(),
        removed: Vec::new(),
        skipped: Vec::new(),
        config_error,
      }
    });
    let removal = SlotRemoval {
      git_common_dir: &machine_pool.git_common_dir,
      pool: &pool,
      trash: &trash,
      bases: &bases,
      scan: &scan,
      params: &params.trim,
    };
    match removal.remove(git_executor, slot) {
      Removal::Removed(mut removed, moved) => {
        let paths: Vec<&Path> = moved.iter().map(|(path, _)| path.as_path()).collect();
        for ((path, what), result) in moved.iter().zip(remove_all(&paths)) {
          removed.record(what, path, result);
        }
        let _ = fs_retry::remove_dir(trash.dir());
        debug!(slot = %removed.path, freed = removed.freed_bytes(), "a free slot below the limit is deleted");
        report.removed.push(*removed);
      }
      Removal::Skipped(skipped) => report.skipped.push(*skipped),
      Removal::Passed => {}
    }
  }
  Trimmed {
    pools: reports.into_iter().flatten().collect(),
    orphans: Vec::new(),
    process_scan: scan.is_available(),
  }
}

/// What the removal of a free slot moved into the trash of its pool
enum Moved {
  /// Its directory (`slot_dir`)
  Dir,
  /// Its output base of this index (`RemovedSlot::output_bases`)
  Base(usize),
}

/// What became of a free slot the trim or the eviction chose
enum Removal {
  /// It left the pool: its report, and what went into the trash, which the caller deletes
  Removed(Box<RemovedSlot>, Vec<(PathBuf, Moved)>),
  /// A process works in it, its Bazel server did not exit, or its directory could not be moved: it stays, free
  Skipped(Box<SkippedSlot>),
  /// Leased meanwhile, or the marker could not be set: it stays as it is
  Passed,
}

/// The removal of the free slots of one pool: its state, its trash, the output bases of the machine and the processes
struct SlotRemoval<'a> {
  git_common_dir: &'a Path,
  pool: &'a Pool,
  trash: &'a Trash,
  bases: &'a [OutputBase],
  scan: &'a ProcessScan,
  params: &'a TrimParams<'a>,
}

impl SlotRemoval<'_> {
  /// Remove the free `slot` under the `cleaning` marker: its Bazel servers stopped, its directory and its output bases
  /// moved into the trash, its admin directory dropped, its entry dropped from the state. Without `apply` it only says
  /// what would go
  fn remove(&self, git_executor: &GitCommandExecutor, slot: &Slot) -> Removal {
    let busy = |processes| {
      Box::new(SkippedSlot {
        name: slot.name.clone(),
        path: slot.path.clone(),
        reason: SkipReason::Busy { processes },
      })
    };
    let (servers, others) = slot_servers(self.scan, Path::new(&slot.path));
    if !others.is_empty() {
      return Removal::Skipped(busy(busy_processes(&others)));
    }
    let slot_bases: Vec<&OutputBase> = self.bases.iter().filter(|base| base.is_in(Path::new(&slot.path))).collect();
    let dir = slot_dir(Path::new(&slot.path));
    let mut removed = RemovedSlot {
      name: slot.name.clone(),
      path: slot.path.clone(),
      returned: slot.returned,
      dir: display_path(&dir),
      output_bases: slot_bases.iter().map(|base| RemovedBase::of(base)).collect(),
      dir_freed_bytes: 0,
      errors: Vec::new(),
    };
    if !self.params.apply {
      return Removal::Removed(Box::new(removed), Vec::new());
    }
    let (pool, timeout) = (self.pool, self.params.lock_timeout);
    let marker = Lease::marker("trim", "brd lease trim", LeaseState::Cleaning);
    match mark_free(pool, timeout, &slot.path, &marker) {
      Ok(true) => {}
      Ok(false) => {
        debug!(slot = %slot.path, "a slot the trim chose was leased meanwhile");
        return Removal::Passed;
      }
      Err(error) => {
        warn!(slot = %slot.path, error = %format!("{error:#}"), "cannot mark a slot for the trim");
        return Removal::Passed;
      }
    }
    let servers: Vec<_> = servers.iter().collect();
    let ServerStop { running, .. } = stop_servers(&servers, self.params.bazel, self.params.server_timeout);
    if !running.is_empty() {
      release_marker(pool, timeout, &slot.path, &marker.id);
      return Removal::Skipped(busy(running));
    }
    let mut moved = match self.trash.move_in(&dir) {
      Ok(moved) => vec![(moved, Moved::Dir)],
      Err(error) => {
        // the slot stays as it was, free
        release_marker(pool, timeout, &slot.path, &marker.id);
        warn!(slot = %slot.path, error = %error, "cannot move a slot into the trash");
        return Removal::Skipped(busy(Vec::new()));
      }
    };
    let git_common_dir = display_path(self.git_common_dir);
    if let Err(error) = pool.worktree_command(timeout, || forget_slot(git_executor, &git_common_dir, Path::new(&slot.path))) {
      // the directory is gone: git's own `worktree prune` drops the admin directory later
      removed.errors.push(format!("{error:#}"));
    }
    if let Err(error) = drop_slot(pool, timeout, &slot.path, &marker.id) {
      removed.errors.push(format!("{error:#}"));
    }
    moved.extend(
      trash_bases(self.trash, &slot_bases, &mut removed.output_bases)
        .into_iter()
        .map(|(index, path)| (path, Moved::Base(index))),
    );
    Removal::Removed(Box::new(removed), moved)
  }
}

/// Move `bases`, the output bases of the workspaces of one slot, into `trash`: the paths in the trash, each with the
/// index of its base in `reports` (`RemovedBase::of` of each, in the order of `bases`), whose `error` says why one
/// stayed. The Bazel servers of the slot are stopped before (`stop_servers`): the trim of a free slot
/// (`SlotRemoval::remove`) and the keeper's relief of an idle host slot (`host_slots::relieve_host_slots`) both take
/// it, and both delete what moved with `remove_all`
pub(crate) fn trash_bases(trash: &Trash, bases: &[&OutputBase], reports: &mut [RemovedBase]) -> Vec<(usize, PathBuf)> {
  let mut moved = Vec::new();
  for (index, base) in bases.iter().enumerate() {
    match trash.move_in(&base.path) {
      Ok(path) => moved.push((index, path)),
      Err(error) => reports[index].error = Some(format!("cannot move it into the trash: {error}")),
    }
  }
  moved
}

/// The slots of a pool as a trim counts them (`present`: a lease, or a directory that is there) and its free slots in
/// the order the trim deletes them: the last a get would take first, the never returned ones in reverse order of
/// joining, then the ones returned longest ago (`reserve` takes the one returned last first, the never returned ones
/// last). The disk report marks the first of them past the limit by it too (`pool_disk`)
pub fn trim_order(slots: &[Slot]) -> (Vec<&Slot>, Vec<&Slot>) {
  let present: Vec<&Slot> = slots.iter().filter(|slot| slot.lease.is_some() || Path::new(&slot.path).is_dir()).collect();
  let mut free: Vec<&Slot> = present.iter().copied().filter(|slot| slot.lease.is_none() && Path::new(&slot.path).is_dir()).collect();
  free.reverse();
  free.sort_by_key(|slot| slot.returned);
  (present, free)
}

/// The pools of this machine: the one of `params.repository`, then each repository whose slots live under
/// `~/.branch-deck/pool/<pool>/<n>/<name>`, told by the `.git` file of a slot (`linked_worktree`), each once
pub(crate) fn machine_pools(repository: &RepositoryDirs, home: Option<&Path>) -> Vec<MachinePool> {
  let mut pools = vec![MachinePool {
    git_common_dir: PathBuf::from(canonical_path(Path::new(&repository.git_common_dir))),
  }];
  let Some(root) = home.map(|home| home.join(".branch-deck").join("pool")) else {
    return pools;
  };
  for pool_dir in child_dirs(&root) {
    // a pool names one repository: the first slot that tells it is enough; its common dir in the form of
    // `canonical_path`, as the pool of the repository above: the canonical path of Windows is verbatim (`\\?\C:\`), and
    // the pool of the repository would come twice, each free slot shrunk twice
    let common = child_dirs(&pool_dir)
      .into_iter()
      .flat_map(|number| child_dirs(&number))
      .find_map(|slot| git_ops::lease_slot::linked_worktree(&slot).map(|worktree| PathBuf::from(display_path(&worktree.git_common_dir))));
    if let Some(git_common_dir) = common
      && !pools.iter().any(|pool| pool.git_common_dir == git_common_dir)
    {
      pools.push(MachinePool { git_common_dir });
    }
  }
  pools
}

/// The directories the slots of `pools` live in: each pool directory under `~/.branch-deck/pool`, and the one of each
/// slot of the state (`<pool>/<n>/<name>`), for a pool configured elsewhere (`branchdeck.leasePool`)
fn pool_dirs(home: Option<&Path>, pools: &[MachinePool]) -> Vec<PathBuf> {
  let mut dirs: Vec<PathBuf> = home.map_or_else(Vec::new, |home| child_dirs(&home.join(".branch-deck").join("pool")));
  for pool in pools {
    let Ok(store) = Pool::new(&pool.git_common_dir).read() else { continue };
    for slot in &store.slots {
      if let Some(dir) = Path::new(&slot.path).parent().and_then(Path::parent)
        && !dirs.iter().any(|known| known == dir)
      {
        dirs.push(dir.to_path_buf());
      }
    }
  }
  dirs.iter().map(|dir| PathBuf::from(canonical_path(dir))).collect()
}

/// The main worktree of the repository of `git_common_dir`: its parent for a `.git` directory; `None` for a bare one
pub fn main_worktree(git_common_dir: &Path) -> Option<PathBuf> {
  (git_common_dir.file_name()? == ".git").then(|| git_common_dir.parent().map(Path::to_path_buf)).flatten()
}

/// The limit of the pool of `git_common_dir`: `max`, else its `branchdeck.leaseMaxSlots` (one `config` in the git dir),
/// else `DEFAULT_MAX_SLOTS`; with why the config could not be read or parsed
pub fn max_slots(git_executor: &GitCommandExecutor, max: Option<usize>, git_common_dir: &Path) -> (usize, Option<String>) {
  if let Some(max) = max {
    return (max, None);
  }
  match lease_config(git_executor, &display_path(git_common_dir)) {
    Ok(config) => match config.max_slots {
      None => (DEFAULT_MAX_SLOTS, None),
      Some(value) => match parse_max_slots(&value) {
        Ok(max) => (max, None),
        Err(reason) => (DEFAULT_MAX_SLOTS, Some(format!("branchdeck.leaseMaxSlots {value:?}: {reason}"))),
      },
    },
    Err(error) => (DEFAULT_MAX_SLOTS, Some(format!("{error:#}"))),
  }
}

/// `branchdeck.leaseMaxSlots`: a whole number of slots, 0 included (every free slot goes once the space is low)
pub fn parse_max_slots(value: &str) -> Result<usize, String> {
  value.trim().parse::<usize>().map_err(|error| format!("not a whole number of slots: {error}"))
}

/// What the trim deletes of the slot at `slot`: `<pool>/<n>`, the number directory a get made for it, when the slot is
/// its only directory (what else is there, a log an agent left beside it, is pool property too); else the slot
fn slot_dir(slot: &Path) -> PathBuf {
  let Some(number) = slot.parent() else { return slot.to_path_buf() };
  let numbered = number.file_name().is_some_and(|name| name.to_string_lossy().parse::<u32>().is_ok());
  let only = std::fs::read_dir(number).is_ok_and(|entries| {
    entries
      .flatten()
      .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_dir()))
      .all(|entry| entry.path() == slot)
  });
  if numbered && only { number.to_path_buf() } else { slot.to_path_buf() }
}

/// Under the pool lock: put the `cleaning` `marker` on the slot `path` while it is free (a lease whose process is gone
/// counts as free); false when it has a lease now or left the state
pub(crate) fn mark_free(pool: &Pool, timeout: Duration, path: &str, marker: &Lease) -> Result<bool> {
  pool.update(timeout, |store| {
    let Some(slot) = store.slots.iter_mut().find(|slot| slot.path == path) else {
      return Ok(false);
    };
    settle_abandoned(slot);
    if slot.lease.is_some() {
      return Ok(false);
    }
    slot.lease = Some(marker.clone());
    Ok(true)
  })
}

/// Under the pool lock: drop the slot `path` that the trim's `marker_id` holds from the state
fn drop_slot(pool: &Pool, timeout: Duration, path: &str, marker_id: &str) -> Result<()> {
  pool.update(timeout, |store| {
    let before = store.slots.len();
    store
      .slots
      .retain(|slot| !(slot.path == path && slot.lease.as_ref().is_some_and(|lease| lease.id == marker_id)));
    if store.slots.len() == before {
      return Err(anyhow!("{path} left the state of the pool while the trim deleted it"));
    }
    Ok(())
  })
}

/// Delete each of `paths` with what is below it, by `DELETE_WORKERS` threads that take the entries right in each path
/// one by one, then the paths themselves: the bytes on disk each freed, or why it could not be deleted
pub(crate) fn remove_all(paths: &[&Path]) -> Vec<std::io::Result<u64>> {
  // the entries right in each path, with the index of the path
  let mut entries: Vec<(usize, PathBuf)> = Vec::new();
  for (index, path) in paths.iter().enumerate() {
    if path.symlink_metadata().is_ok_and(|metadata| metadata.is_dir()) {
      entries.extend(std::fs::read_dir(path).into_iter().flatten().flatten().map(|entry| (index, entry.path())));
    }
  }
  let freed: Vec<AtomicU64> = paths.iter().map(|_| AtomicU64::new(0)).collect();
  let errors: Mutex<BTreeMap<usize, std::io::Error>> = Mutex::default();
  let next = AtomicUsize::new(0);
  std::thread::scope(|scope| {
    for _ in 0..DELETE_WORKERS.min(entries.len()) {
      scope.spawn(in_current_span(|| {
        while let Some((index, entry)) = entries.get(next.fetch_add(1, Ordering::Relaxed)) {
          match remove_measured(entry) {
            Ok(bytes) => {
              freed[*index].fetch_add(bytes, Ordering::Relaxed);
            }
            Err(error) => {
              errors.lock().unwrap_or_else(std::sync::PoisonError::into_inner).entry(*index).or_insert(error);
            }
          }
        }
      }));
    }
  });
  let mut errors = errors.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
  paths
    .iter()
    .enumerate()
    .map(|(index, path)| {
      if let Some(error) = errors.remove(&index) {
        return Err(error);
      }
      let own = remove_measured(path)?;
      Ok(freed[index].load(Ordering::Relaxed) + own)
    })
    .collect()
}

#[cfg(test)]
#[path = "trim_test.rs"]
mod trim_test;
