//! `pool-disk`: what the pools of this machine hold on disk (`brd lease disk`, the Disk window of the app; spec/cli/lease.spec.md,
//! "Disk"). A `du` of a pool takes minutes (two for the pools of the user's Mac, 1.25 TB) and says little: the tracked
//! tree of a slot is an APFS clone that shares its blocks with the others. What a slot holds of its own is what a delete
//! of it frees: the private size of its worktree (`walk`: the blocks no clone shares, as `getattrlistbulk` tells them),
//! the directories its clean rules name (`target/`, `out/`) and its Bazel output bases outside it (`output_bases`),
//! each walked apart, all side by side, and the bytes of a slot are kept in `disk.json` beside the state of its pool: a free slot does not change until a get leases it, so its bytes hold while its `returned` does;
//! a leased slot is measured again after `LEASED_STALE`.

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::lease_config;
use git_ops::revs::RepositoryDirs;
use lease_pool::free_space::available_bytes;
use lease_pool::lease::{Pool, Slot, SlotState, lease_status_core};
use lease_pool::lease_clean::{CleanRule, read_clean_rules};
use lease_pool::output_bases::{OutputBase, orphans};
use lease_pool::state_file::StateFile;
use lease_pool::trim::{Machine, main_worktree, max_slots, trim_order};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sync_utils::paths::display_path;
use sync_utils::size::parse_size;
use tracing::{debug, instrument};
use walk::{Walk, walk_bytes};

/// The file of the bytes of each slot, beside the state of the pool
const DISK_FILE: &str = "disk.json";
/// Its lock
const DISK_LOCK: &str = "disk.lock";
/// How long the bytes of a leased slot hold: its builds write while it is leased
pub const LEASED_STALE: Duration = Duration::from_mins(15);

/// What `disk_usage_core` reports
pub struct DiskParams<'a> {
  /// A checkout of the repository that asks: its pool is reported wherever it lives, and its config gives the minimum
  pub repository: &'a RepositoryDirs,
  /// The home directory of the default pools (`~/.branch-deck/pool`)
  pub home: Option<&'a Path>,
  /// Measure every slot again, whatever `disk.json` holds
  pub fresh: bool,
  /// The wait for the lock of `disk.json`
  pub lock_timeout: Duration,
}

/// What the pools of this machine hold
#[derive(Debug, Clone, Default)]
pub struct DiskUsage {
  /// The free bytes of the volume of the repository; `None` where the system does not tell
  pub free_bytes: Option<u64>,
  /// `branchdeck.leaseMinFree` of the repository, in bytes
  pub min_free: Option<u64>,
  pub pools: Vec<PoolUsage>,
  /// The output bases whose slot is gone, which a trim deletes
  pub orphans: Vec<BaseUsage>,
  /// How long the walks took
  pub elapsed: Duration,
}

/// A pool and what its slots hold
#[derive(Debug, Clone)]
pub struct PoolUsage {
  /// `$GIT_COMMON_DIR` of its repository, canonical
  pub git_common_dir: String,
  /// The main worktree of the repository; `None` for a bare one
  pub repository: Option<String>,
  /// How many slots a trim keeps (`branchdeck.leaseMaxSlots`, else 10)
  pub max_slots: usize,
  pub slots: Vec<SlotUsage>,
  /// The output bases of the main worktree and its workspaces: no trim deletes them, `bazel clean --expunge` there does
  pub checkout_bases: Vec<BaseUsage>,
  /// What the trash of the pool holds: what a run killed before its delete left
  pub trash_bytes: u64,
}

impl PoolUsage {
  /// The bytes of its slots
  pub fn bytes(&self) -> u64 {
    self.slots.iter().map(SlotUsage::bytes).sum()
  }

  /// The free slots a trim would delete now (`SlotUsage::trims_next`)
  pub fn past_limit(&self) -> usize {
    self.slots.iter().filter(|slot| slot.trims_next).count()
  }
}

/// A slot and the bytes of its own
#[derive(Debug, Clone)]
pub struct SlotUsage {
  pub name: String,
  pub path: String,
  pub state: SlotState,
  /// The holder of its lease
  pub holder: Option<String>,
  /// The number of its last return in the pool
  pub returned: Option<u64>,
  /// What its worktree holds of its own beside its build directories: the blocks its files share with no clone (a slot
  /// cloned from a template shares most of them until moves rewrite its files)
  pub worktree: u64,
  /// The directories its clean rules name, each with its bytes
  pub build: Vec<DirUsage>,
  pub output_bases: Vec<BaseUsage>,
  /// Unix seconds of the walk the bytes come from
  pub measured_at: u64,
  /// The bytes come from `disk.json`, not from a walk of this call
  pub cached: bool,
  /// A free slot past the limit that a trim deletes now, by its order (`lease_pool::trim::trim_order`), unless a
  /// process works in it then
  pub trims_next: bool,
}

impl SlotUsage {
  pub fn bytes(&self) -> u64 {
    self.worktree + self.build.iter().map(|dir| dir.bytes).sum::<u64>() + self.output_bases.iter().map(|base| base.bytes).sum::<u64>()
  }
}

/// A directory of a slot and its bytes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirUsage {
  /// Relative to the slot, as the clean rule names it
  pub path: String,
  pub bytes: u64,
}

/// An output base and its bytes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseUsage {
  pub path: String,
  pub workspace: String,
  pub bytes: u64,
}

/// `disk.json`: the bytes of each slot, by path
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct DiskFile {
  #[serde(default)]
  slots: BTreeMap<String, Measured>,
  /// The output bases of the main worktree, by path: its builds write at any time, so they hold for `LEASED_STALE`
  #[serde(default)]
  checkout_bases: BTreeMap<String, MeasuredBase>,
}

/// The bytes of an output base of the main worktree as a walk found them
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MeasuredBase {
  at: u64,
  bytes: u64,
}

/// The bytes of a slot as a walk found them
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Measured {
  /// `returned` of the slot when it was walked: a later return or a get makes it stale
  returned: Option<u64>,
  /// Whether it was leased then
  leased: bool,
  at: u64,
  /// `SlotUsage::worktree`; `None` in the file of a brd that did not walk the worktree, which is stale then
  #[serde(default, skip_serializing_if = "Option::is_none")]
  worktree: Option<u64>,
  build: Vec<DirUsage>,
  output_bases: Vec<BaseUsage>,
}

/// What the pools of this machine hold: every slot of each pool with the directories its clean rules name and its
/// output bases, each with its bytes (from `disk.json` while they hold, else walked, all walks of the call side by
/// side), the output bases of the main worktrees and the orphans. One `config` of the repository for its minimum, one
/// per pool for its limit; no `status`
#[instrument(skip_all, fields(repository = %params.repository.top_level))]
pub fn disk_usage_core(git_executor: &GitCommandExecutor, params: &DiskParams<'_>) -> Result<DiskUsage> {
  let started = Instant::now();
  let machine = Machine::read(params.repository, params.home);
  let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
  let config = lease_config(git_executor, &params.repository.top_level)?;
  let mut usage = DiskUsage {
    free_bytes: available_bytes(Path::new(&params.repository.top_level)),
    min_free: config.min_free.as_deref().and_then(|value| parse_size(value).ok()),
    ..DiskUsage::default()
  };
  // every directory to walk, with where its bytes go
  let mut walks: Vec<(Walk, Target)> = Vec::new();
  let mut files: Vec<(StateFile, DiskFile)> = Vec::new();
  for (pool_index, pool) in machine.pools.iter().enumerate() {
    let (statuses, _) = lease_status_core(&pool.git_common_dir, false)?;
    let limit = max_slots(git_executor, None, &pool.git_common_dir).0;
    let trims_next: HashSet<String> = {
      let slots: Vec<Slot> = statuses.iter().map(|status| status.slot.clone()).collect();
      let (present, free) = trim_order(&slots);
      free.into_iter().take(present.len().saturating_sub(limit)).map(|slot| slot.path.clone()).collect()
    };
    let file = disk_file(&pool.git_common_dir);
    let cached = if params.fresh {
      DiskFile::default()
    } else {
      file.read::<DiskFile>().unwrap_or_default()
    };
    let repository = main_worktree(&pool.git_common_dir);
    let mut report = PoolUsage {
      git_common_dir: display_path(&pool.git_common_dir),
      repository: repository.as_deref().map(display_path),
      max_slots: limit,
      slots: Vec::new(),
      checkout_bases: Vec::new(),
      trash_bytes: Pool::new(&pool.git_common_dir).trash_bytes(),
    };
    if let Some(repository) = &repository {
      for base in machine.bases.iter().filter(|base| base.is_in(repository) && !in_any(base, &machine.pool_dirs)) {
        let mut usage = base_usage(base);
        match cached
          .checkout_bases
          .get(&usage.path)
          .filter(|measured| now.saturating_sub(measured.at) < LEASED_STALE.as_secs())
        {
          Some(measured) => usage.bytes = measured.bytes,
          None => walks.push((Walk::of(&base.path), Target::CheckoutBase(pool_index, report.checkout_bases.len()))),
        }
        report.checkout_bases.push(usage);
      }
    }
    for status in statuses.into_iter().filter(|status| !status.missing) {
      let slot = status.slot;
      let leased = slot.lease.is_some();
      let holder = slot.lease.as_ref().map(|lease| lease.holder.clone());
      let fresh_enough = |measured: &&Measured| {
        measured.worktree.is_some() && measured.returned == slot.returned && measured.leased == leased && (!leased || now.saturating_sub(measured.at) < LEASED_STALE.as_secs())
      };
      let slot_index = report.slots.len();
      let (worktree, build, output_bases, measured_at, from_file) = if let Some(measured) = cached.slots.get(&slot.path).filter(fresh_enough) {
        (
          measured.worktree.unwrap_or_default(),
          measured.build.clone(),
          measured.output_bases.clone(),
          measured.at,
          true,
        )
      } else {
        let root = PathBuf::from(&slot.path);
        let build: Vec<DirUsage> = build_dirs(&root).into_iter().map(|path| DirUsage { path, bytes: 0 }).collect();
        for (index, dir) in build.iter().enumerate() {
          walks.push((Walk::of(root.join(&dir.path)), Target::Build(pool_index, slot_index, index)));
        }
        // the worktree without its build directories, which count apart
        let skip = build.iter().map(|dir| root.join(&dir.path)).collect();
        walks.push((Walk { root: root.clone(), skip }, Target::Worktree(pool_index, slot_index)));
        let bases: Vec<BaseUsage> = machine.bases.iter().filter(|base| base.is_in(&root)).map(base_usage).collect();
        for (index, base) in bases.iter().enumerate() {
          walks.push((Walk::of(&base.path), Target::SlotBase(pool_index, slot_index, index)));
        }
        (0, build, bases, now, false)
      };
      report.slots.push(SlotUsage {
        name: slot.name.clone(),
        path: slot.path.clone(),
        state: status.state,
        holder,
        returned: slot.returned,
        worktree,
        build,
        output_bases,
        measured_at,
        cached: from_file,
        trims_next: trims_next.contains(&slot.path),
      });
    }
    usage.pools.push(report);
    files.push((file, cached));
  }
  for base in orphans(&machine.bases, &machine.pool_dirs) {
    walks.push((Walk::of(&base.path), Target::Orphan(usage.orphans.len())));
    usage.orphans.push(base_usage(base));
  }

  let roots: Vec<Walk> = walks.iter().map(|(walk, _)| walk.clone()).collect();
  for ((_, target), bytes) in walks.iter().zip(walk_bytes(&roots)) {
    match *target {
      Target::Worktree(pool, slot) => usage.pools[pool].slots[slot].worktree = bytes,
      Target::Build(pool, slot, dir) => usage.pools[pool].slots[slot].build[dir].bytes = bytes,
      Target::SlotBase(pool, slot, base) => usage.pools[pool].slots[slot].output_bases[base].bytes = bytes,
      Target::CheckoutBase(pool, base) => usage.pools[pool].checkout_bases[base].bytes = bytes,
      Target::Orphan(base) => usage.orphans[base].bytes = bytes,
    }
  }
  let walked_bases: HashSet<&Path> = walks
    .iter()
    .filter(|(_, target)| matches!(target, Target::CheckoutBase(..)))
    .map(|(walk, _)| walk.root.as_path())
    .collect();
  for (pool, (file, _)) in usage.pools.iter().zip(&files) {
    let measured: BTreeMap<String, Measured> = pool
      .slots
      .iter()
      .filter(|slot| !slot.cached)
      .map(|slot| {
        (
          slot.path.clone(),
          Measured {
            returned: slot.returned,
            leased: slot.state != SlotState::Free,
            at: slot.measured_at,
            worktree: Some(slot.worktree),
            build: slot.build.clone(),
            output_bases: slot.output_bases.clone(),
          },
        )
      })
      .collect();
    let bases: BTreeMap<String, MeasuredBase> = pool
      .checkout_bases
      .iter()
      .filter(|base| walked_bases.contains(Path::new(&base.path)))
      .map(|base| (base.path.clone(), MeasuredBase { at: now, bytes: base.bytes }))
      .collect();
    if measured.is_empty() && bases.is_empty() {
      continue;
    }
    let paths: HashSet<&str> = pool.slots.iter().map(|slot| slot.path.as_str()).collect();
    let base_paths: HashSet<&str> = pool.checkout_bases.iter().map(|base| base.path.as_str()).collect();
    let written = file.update(params.lock_timeout, |disk: &mut DiskFile| {
      // the slots that left the pool and the output bases that are gone leave the file
      disk.slots.retain(|path, _| paths.contains(path.as_str()));
      disk.slots.extend(measured);
      disk.checkout_bases.retain(|path, _| base_paths.contains(path.as_str()));
      disk.checkout_bases.extend(bases);
      Ok(())
    });
    if let Err(error) = written {
      debug!(error = %format!("{error:#}"), "cannot write the bytes of the slots");
    }
  }
  usage.elapsed = started.elapsed();
  Ok(usage)
}

/// Where the bytes of a walk go
enum Target {
  Worktree(usize, usize),
  Build(usize, usize, usize),
  SlotBase(usize, usize, usize),
  CheckoutBase(usize, usize),
  Orphan(usize),
}

fn disk_file(git_common_dir: &Path) -> StateFile {
  StateFile::new(git_common_dir, DISK_FILE, DISK_LOCK, "the disk usage of the pool")
}

fn base_usage(base: &OutputBase) -> BaseUsage {
  BaseUsage {
    path: display_path(&base.path),
    workspace: display_path(&base.workspace),
    bytes: 0,
  }
}

fn in_any(base: &OutputBase, dirs: &[PathBuf]) -> bool {
  dirs.iter().any(|dir| base.is_in(dir))
}

/// The directories of `slot` that its clean rules name, the outermost of nested ones only (`out`, not `out/dev-run`),
/// relative and sorted: what its builds write
fn build_dirs(slot: &Path) -> Vec<String> {
  let mut dirs: Vec<String> = read_clean_rules(slot)
    .unwrap_or_default()
    .into_iter()
    .flatten()
    .map(|rule| match rule {
      CleanRule::Cargo { dir, .. } => dir,
      CleanRule::Delete { path, .. } => path,
    })
    .collect();
  dirs.sort();
  dirs.dedup();
  let all = dirs.clone();
  dirs.retain(|dir| !all.iter().any(|other| other != dir && Path::new(dir).starts_with(other)));
  dirs
}

pub mod report;
pub mod trim_report;
pub mod walk;

#[cfg(test)]
mod disk_test;
