//! `pool-disk`: what the pools of this machine hold on disk (`brd lease disk`, the Disk window of the app; openspec/specs/cli/lease/spec.md,
//! "Disk"). A `du` of a pool takes minutes (two for the pools of the user's Mac, 1.25 TB) and says little: the tracked
//! tree of a slot is an APFS clone that shares its blocks with the others. What a slot holds of its own is what a delete
//! of it frees: the private size of its worktree (`walk`: the blocks no clone shares, as `getattrlistbulk` tells them),
//! the directories its clean rules name (`target/`, `out/`) and its Bazel output bases outside it (`output_bases`),
//! each walked apart, all side by side, and the bytes of a slot are kept in `disk.json` beside the state of its pool: a free slot does not change until a get leases it, so its bytes hold while its `returned` does;
//! a leased slot is measured again after `LEASED_STALE`.

use anyhow::Result;
use fs_walk::changes::{Changes, Mark, changed_since, mark};
use fs_walk::{NodeBytes, Walk, walk_tree_with_callback};
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
  /// Read `disk.json` only, without measuring any path
  pub cached: bool,
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

/// A report ready to show before or during a disk walk
#[derive(Debug, Clone)]
pub enum DiskUsageProgress {
  Cached {
    usage: DiskUsage,
    measuring: u32,
  },
  Planning {
    measuring: u32,
  },
  Slot {
    usage: DiskUsage,
    pool_index: usize,
    slot_index: usize,
    measured: u32,
    measuring: u32,
  },
}

type ChangesOverride<'a> = dyn FnMut(&[&Path]) -> Changes + 'a;

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
  #[serde(default, skip_serializing_if = "Option::is_none")]
  trash_bytes: Option<MeasuredBase>,
  #[serde(default)]
  orphans: BTreeMap<String, MeasuredBase>,
}

/// The bytes of an output base of the main worktree as a walk found them
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
  #[serde(default, skip_serializing_if = "Option::is_none")]
  mark: Option<SavedMark>,
  #[serde(default)]
  roots: Vec<CachedRoot>,
  #[serde(default)]
  nodes: Vec<CachedNode>,
  /// `SlotUsage::worktree`; `None` in the file of a brd that did not walk the worktree, which is stale then
  #[serde(default, skip_serializing_if = "Option::is_none")]
  worktree: Option<u64>,
  build: Vec<DirUsage>,
  output_bases: Vec<BaseUsage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct SavedMark {
  event_id: u64,
  device: u64,
}

impl From<Mark> for SavedMark {
  fn from(mark: Mark) -> Self {
    Self {
      event_id: mark.event_id,
      device: mark.device,
    }
  }
}

impl From<SavedMark> for Mark {
  fn from(mark: SavedMark) -> Self {
    Self {
      event_id: mark.event_id,
      device: mark.device,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CachedNode {
  root: String,
  path: String,
  bytes: u64,
  #[serde(default)]
  has_hardlinks: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CachedRoot {
  path: String,
  bytes: u64,
  #[serde(default)]
  has_hardlinks: bool,
}

impl CachedNode {
  fn of(root: &str, node: NodeBytes) -> Self {
    Self {
      root: root.to_owned(),
      path: node.path,
      bytes: node.bytes,
      has_hardlinks: node.has_hardlinks,
    }
  }
}

struct RefreshingSlot {
  pool: usize,
  slot: usize,
  roots: Vec<CachedRoot>,
  mark: Option<SavedMark>,
  nodes: Vec<CachedNode>,
  incremental: bool,
  build: Vec<DirUsage>,
  bases: Vec<BaseUsage>,
}

struct PendingCheck {
  pool: usize,
  slot: usize,
  path: String,
  walks: Vec<(Walk, Target)>,
  root_keys: Vec<String>,
  mark: Mark,
  previous: Measured,
  build: Vec<DirUsage>,
  bases: Vec<BaseUsage>,
}

struct SlotRefreshPlan<'a> {
  pool: usize,
  slot: usize,
  slot_walks: &'a [(Walk, Target)],
  root_keys: &'a [String],
  requests: Vec<WalkRequest>,
  previous: Option<&'a Measured>,
  current_mark: Option<Mark>,
  incremental: bool,
  build: Vec<DirUsage>,
  bases: Vec<BaseUsage>,
}

/// What the pools of this machine hold: every slot of each pool with the directories its clean rules name and its
/// output bases, each with its bytes (from `disk.json` while they hold, else walked, all walks of the call side by
/// side), the output bases of the main worktrees and the orphans. One `config` of the repository for its minimum, one
/// per pool for its limit; no `status`
#[instrument(skip_all, fields(repository = %params.repository.top_level))]
pub fn disk_usage_core(git_executor: &GitCommandExecutor, params: &DiskParams<'_>) -> Result<DiskUsage> {
  disk_usage_core_inner(git_executor, params, None, None)
}

/// The same report, notifying the caller of its cached state and each completed slot
#[instrument(skip_all, fields(repository = %params.repository.top_level))]
pub fn disk_usage_core_with_progress<F>(git_executor: &GitCommandExecutor, params: &DiskParams<'_>, mut on_progress: F) -> Result<DiskUsage>
where
  F: FnMut(DiskUsageProgress) + Send,
{
  disk_usage_core_inner(git_executor, params, Some(&mut on_progress), None)
}

#[cfg(test)]
fn disk_usage_core_with_changes<F, G>(git_executor: &GitCommandExecutor, params: &DiskParams<'_>, mut on_progress: F, mut changes: G) -> Result<DiskUsage>
where
  F: FnMut(DiskUsageProgress) + Send,
  G: FnMut(&[&Path]) -> Changes,
{
  disk_usage_core_inner(git_executor, params, Some(&mut on_progress), Some(&mut changes))
}

fn disk_usage_core_inner(
  git_executor: &GitCommandExecutor,
  params: &DiskParams<'_>,
  mut on_progress: Option<&mut (dyn FnMut(DiskUsageProgress) + Send)>,
  mut changes_override: Option<&mut ChangesOverride<'_>>,
) -> Result<DiskUsage> {
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
  let mut refreshing_slots: Vec<RefreshingSlot> = Vec::new();
  let mut pending_checks: Vec<PendingCheck> = Vec::new();
  let mut files: Vec<(StateFile, DiskFile, BTreeMap<String, SavedMark>)> = Vec::new();
  for (pool_index, pool) in machine.pools.iter().enumerate() {
    let (statuses, _) = lease_status_core(&pool.git_common_dir, false)?;
    let limit = max_slots(git_executor, None, &pool.git_common_dir).0;
    let trims_next: HashSet<String> = {
      let slots: Vec<Slot> = statuses.iter().map(|status| status.slot.clone()).collect();
      let (present, free) = trim_order(&slots);
      free.into_iter().take(present.len().saturating_sub(limit)).map(|slot| slot.path.clone()).collect()
    };
    let file = disk_file(&pool.git_common_dir);
    let cached = file.read::<DiskFile>().unwrap_or_default();
    let trash_bytes = cached.trash_bytes.map_or(0, |measured| measured.bytes);
    let mut mark_updates = BTreeMap::new();
    let repository = main_worktree(&pool.git_common_dir);
    let mut report = PoolUsage {
      git_common_dir: display_path(&pool.git_common_dir),
      repository: repository.as_deref().map(display_path),
      max_slots: limit,
      slots: Vec::new(),
      checkout_bases: Vec::new(),
      trash_bytes,
    };
    if let Some(repository) = &repository {
      for base in machine.bases.iter().filter(|base| base.is_in(repository) && !in_any(base, &machine.pool_dirs)) {
        let mut usage = base_usage(base);
        let measured = cached.checkout_bases.get(&usage.path);
        if params.cached {
          usage.bytes = measured.map_or(0, |measured| measured.bytes);
        } else if !params.fresh && measured.is_some_and(|measured| now.saturating_sub(measured.at) < LEASED_STALE.as_secs()) {
          usage.bytes = measured.map_or(0, |measured| measured.bytes);
        } else {
          usage.bytes = measured.map_or(0, |measured| measured.bytes);
          walks.push((Walk::of(&base.path), Target::CheckoutBase(pool_index, report.checkout_bases.len())));
        }
        report.checkout_bases.push(usage);
      }
    }
    for status in statuses.into_iter().filter(|status| !status.missing) {
      let slot = status.slot;
      let leased = slot.lease.is_some();
      let holder = slot.lease.as_ref().map(|lease| lease.holder.clone());
      let slot_index = report.slots.len();
      let root = PathBuf::from(&slot.path);
      let build: Vec<DirUsage> = build_dirs(&root).into_iter().map(|path| DirUsage { path, bytes: 0 }).collect();
      let bases: Vec<BaseUsage> = machine.bases.iter().filter(|base| base.is_in(&root)).map(base_usage).collect();
      let mut slot_walks = Vec::new();
      for (index, dir) in build.iter().enumerate() {
        let root_index = slot_walks.len();
        slot_walks.push((Walk::of(root.join(&dir.path)), Target::Build(pool_index, slot_index, index, root_index)));
      }
      // the worktree without its build directories, which count apart
      let skip = build.iter().map(|dir| root.join(&dir.path)).collect();
      let worktree_root_index = slot_walks.len();
      slot_walks.push((Walk { root: root.clone(), skip }, Target::Worktree(pool_index, slot_index, worktree_root_index)));
      for (index, base) in bases.iter().enumerate() {
        let root_index = slot_walks.len();
        slot_walks.push((Walk::of(&base.path), Target::SlotBase(pool_index, slot_index, index, root_index)));
      }
      let slot_root_walks: Vec<Walk> = slot_walks.iter().map(|(walk, _)| walk.clone()).collect();
      let root_keys: Vec<String> = slot_walks.iter().map(|(walk, _)| root_key(&walk.root)).collect();
      let root_paths: Vec<&Path> = slot_walks.iter().map(|(walk, _)| walk.root.as_path()).collect();
      let current_mark = if params.cached { None } else { mark(&root) };
      let previous = cached.slots.get(&slot.path).cloned();
      let cache_candidate = !params.cached
        && !params.fresh
        && previous.as_ref().is_some_and(|measured| {
          measured.worktree.is_some()
            && measured.returned == slot.returned
            && measured.leased == leased
            && measured.roots.iter().map(|root| root.path.as_str()).eq(root_keys.iter().map(String::as_str))
            && root_keys.iter().all(|root| !root.is_empty())
            && if cfg!(target_os = "macos") {
              measured.mark.is_some() && current_mark.is_some()
            } else {
              !leased || now.saturating_sub(measured.at) < LEASED_STALE.as_secs()
            }
        });
      let defer_check = cache_candidate && cfg!(target_os = "macos") && changes_override.is_none();
      let changes = if params.cached || defer_check {
        Changes::Paths(Vec::new())
      } else if !cache_candidate {
        Changes::All
      } else if let Some(changes) = changes_override.as_deref_mut() {
        changes(&root_paths)
      } else if cfg!(target_os = "macos") {
        previous
          .as_ref()
          .and_then(|measured| measured.mark)
          .map_or(Changes::All, |mark| changed_since(&root_paths, &mark.into()))
      } else {
        Changes::Paths(Vec::new())
      };
      let requests = walk_requests(
        &changes,
        &slot_root_walks,
        &root_keys,
        previous.as_ref().map_or(&[], |measured| measured.roots.as_slice()),
        previous.as_ref().map_or(&[], |measured| measured.nodes.as_slice()),
      );
      let unchanged = (params.cached && previous.is_some()) || (cache_candidate && !defer_check && requests.is_empty());
      let incremental = cache_candidate && !unchanged && matches!(changes, Changes::Paths(_));
      let (worktree, shown_build, output_bases, measured_at, from_file) = if defer_check {
        let measured = previous.as_ref().expect("a pending history check has cached bytes");
        pending_checks.push(PendingCheck {
          pool: pool_index,
          slot: slot_index,
          path: slot.path.clone(),
          walks: slot_walks,
          root_keys,
          mark: current_mark.expect("a cache candidate has a current mark"),
          previous: measured.clone(),
          build,
          bases,
        });
        (
          measured.worktree.unwrap_or_default(),
          measured.build.clone(),
          measured.output_bases.clone(),
          measured.at,
          true,
        )
      } else if unchanged {
        if !params.cached
          && let Some(mark) = current_mark
        {
          mark_updates.insert(slot.path.clone(), mark.into());
        }
        let measured = previous.as_ref().expect("an unchanged slot has cached bytes");
        (
          measured.worktree.unwrap_or_default(),
          measured.build.clone(),
          measured.output_bases.clone(),
          measured.at,
          true,
        )
      } else if params.cached {
        (0, build, bases, 0, false)
      } else {
        let display = previous.as_ref().map_or_else(
          || (0, build.clone(), bases.clone(), 0, false),
          |measured| {
            (
              measured.worktree.unwrap_or_default(),
              measured.build.clone(),
              measured.output_bases.clone(),
              measured.at,
              true,
            )
          },
        );
        schedule_slot_refresh(
          &mut walks,
          &mut refreshing_slots,
          SlotRefreshPlan {
            pool: pool_index,
            slot: slot_index,
            slot_walks: &slot_walks,
            root_keys: &root_keys,
            requests,
            previous: previous.as_ref(),
            current_mark,
            incremental,
            build,
            bases,
          },
        );
        display
      };
      report.slots.push(SlotUsage {
        name: slot.name.clone(),
        path: slot.path.clone(),
        state: status.state,
        holder,
        returned: slot.returned,
        worktree,
        build: shown_build,
        output_bases,
        measured_at,
        cached: from_file,
        trims_next: trims_next.contains(&slot.path),
      });
    }
    usage.pools.push(report);
    files.push((file, cached, mark_updates));
  }
  let cached_orphans: BTreeMap<String, MeasuredBase> = files
    .iter()
    .flat_map(|(_, file, _)| file.orphans.iter().map(|(path, measured)| (path.clone(), *measured)))
    .collect();
  for base in orphans(&machine.bases, &machine.pool_dirs) {
    let orphan_index = usage.orphans.len();
    let mut orphan = base_usage(base);
    if params.cached {
      orphan.bytes = cached_orphans.get(&orphan.path).map_or(0, |measured| measured.bytes);
    } else {
      walks.push((Walk::of(&base.path), Target::Orphan(orphan_index)));
    }
    usage.orphans.push(orphan);
  }

  let measuring = u32::try_from(refreshing_slots.len() + pending_checks.len()).unwrap_or(u32::MAX);
  if let Some(progress) = on_progress.as_deref_mut() {
    progress(DiskUsageProgress::Cached { usage: usage.clone(), measuring });
  }
  if !params.cached {
    for (pool_index, pool) in machine.pools.iter().enumerate() {
      let bytes = Pool::new(&pool.git_common_dir).trash_bytes();
      usage.pools[pool_index].trash_bytes = bytes;
      files[pool_index].1.trash_bytes = Some(MeasuredBase { at: now, bytes });
    }
  }
  let planning = !pending_checks.is_empty();
  for pending in pending_checks {
    let roots: Vec<&Path> = pending.walks.iter().map(|(walk, _)| walk.root.as_path()).collect();
    let slot_walks: Vec<Walk> = pending.walks.iter().map(|(walk, _)| walk.clone()).collect();
    let changes = if let Some(changes) = changes_override.as_deref_mut() {
      changes(&roots)
    } else {
      changed_since(&roots, &Mark::from(pending.previous.mark.expect("a pending check has a saved mark")))
    };
    let requests = walk_requests(&changes, &slot_walks, &pending.root_keys, &pending.previous.roots, &pending.previous.nodes);
    if requests.is_empty() {
      files[pending.pool].2.insert(pending.path, pending.mark.into());
      continue;
    }
    let incremental = matches!(changes, Changes::Paths(_));
    schedule_slot_refresh(
      &mut walks,
      &mut refreshing_slots,
      SlotRefreshPlan {
        pool: pending.pool,
        slot: pending.slot,
        slot_walks: &pending.walks,
        root_keys: &pending.root_keys,
        requests,
        previous: Some(&pending.previous),
        current_mark: Some(pending.mark),
        incremental,
        build: pending.build,
        bases: pending.bases,
      },
    );
  }
  if planning && let Some(progress) = on_progress.as_deref_mut() {
    progress(DiskUsageProgress::Planning {
      measuring: u32::try_from(refreshing_slots.len()).unwrap_or(u32::MAX),
    });
  }
  for refreshing in &refreshing_slots {
    if !refreshing.incremental {
      let slot = &mut usage.pools[refreshing.pool].slots[refreshing.slot];
      slot.worktree = 0;
      slot.build.clone_from(&refreshing.build);
      slot.output_bases.clone_from(&refreshing.bases);
    }
  }
  for (_, target) in &walks {
    match *target {
      Target::CheckoutBase(pool, base) => usage.pools[pool].checkout_bases[base].bytes = 0,
      Target::Orphan(base) => usage.orphans[base].bytes = 0,
      Target::Worktree(..) | Target::Build(..) | Target::SlotBase(..) | Target::Node(..) => {}
    }
  }

  let roots: Vec<Walk> = walks.iter().map(|(walk, _)| walk.clone()).collect();
  let targets: Vec<Target> = walks.iter().map(|(_, target)| target.clone()).collect();
  let mut remaining_walks: Vec<Vec<usize>> = usage.pools.iter().map(|pool| vec![0; pool.slots.len()]).collect();
  for target in &targets {
    match target {
      Target::Worktree(pool, slot, _) | Target::Build(pool, slot, _, _) | Target::SlotBase(pool, slot, _, _) | Target::Node(pool, slot, ..) => {
        remaining_walks[*pool][*slot] += 1;
      }
      Target::CheckoutBase(..) | Target::Orphan(..) => {}
    }
  }
  let mut measured_slots = 0;
  let _trees = walk_tree_with_callback(&roots, 3, |index, tree| {
    let target = &targets[index];
    let slot_target = match target {
      Target::Worktree(pool, slot, root) => Some((*pool, *slot, *root, SlotRootKind::Worktree, tree)),
      Target::Build(pool, slot, dir, root) => Some((*pool, *slot, *root, SlotRootKind::Build(*dir), tree)),
      Target::SlotBase(pool, slot, base, root) => Some((*pool, *slot, *root, SlotRootKind::Base(*base), tree)),
      Target::Node(pool, slot, root, kind, path, old_bytes) => {
        let refreshing = refreshing_slots
          .iter_mut()
          .find(|refreshing| refreshing.pool == *pool && refreshing.slot == *slot)
          .expect("the slot is being measured");
        let root_key = refreshing.roots[*root].path.clone();
        let cached_root = &mut refreshing.roots[*root];
        adjust_slot_bytes(&mut usage.pools[*pool].slots[*slot], *kind, *old_bytes, tree.bytes);
        cached_root.bytes = cached_root.bytes.saturating_sub(*old_bytes).saturating_add(tree.bytes);
        cached_root.has_hardlinks |= tree.has_hardlinks;
        if let Some(node) = refreshing.nodes.iter_mut().find(|node| node.root == root_key && node.path == *path) {
          node.bytes = tree.bytes;
          node.has_hardlinks = tree.has_hardlinks;
        }
        Some((*pool, *slot, *root, *kind, tree))
      }
      Target::CheckoutBase(pool, base) => {
        usage.pools[*pool].checkout_bases[*base].bytes = tree.bytes;
        None
      }
      Target::Orphan(base) => {
        usage.orphans[*base].bytes = tree.bytes;
        None
      }
    };
    if let Some((pool_index, slot_index, root_index, kind, tree)) = slot_target {
      let refreshing = refreshing_slots
        .iter_mut()
        .find(|refreshing| refreshing.pool == pool_index && refreshing.slot == slot_index)
        .expect("the slot is being measured");
      let root_key = refreshing.roots[root_index].path.clone();
      if !matches!(target, Target::Node(..)) {
        let old_bytes = refreshing.roots[root_index].bytes;
        adjust_slot_bytes(&mut usage.pools[pool_index].slots[slot_index], kind, old_bytes, tree.bytes);
        refreshing.roots[root_index].bytes = tree.bytes;
        refreshing.roots[root_index].has_hardlinks = tree.has_hardlinks;
        refreshing.nodes.retain(|node| node.root != root_key);
        refreshing.nodes.extend(tree.nodes.into_iter().map(|node| CachedNode::of(&root_key, node)));
      }
      remaining_walks[pool_index][slot_index] -= 1;
      if remaining_walks[pool_index][slot_index] == 0 {
        let slot = &mut usage.pools[pool_index].slots[slot_index];
        slot.measured_at = now;
        slot.cached = false;
        measured_slots += 1;
        if let Some(progress) = on_progress.as_deref_mut() {
          progress(DiskUsageProgress::Slot {
            usage: usage.clone(),
            pool_index,
            slot_index,
            measured: measured_slots,
            measuring,
          });
        }
      }
    }
  });
  let walked_bases: HashSet<&Path> = walks
    .iter()
    .filter(|(_, target)| matches!(target, Target::CheckoutBase(..)))
    .map(|(walk, _)| walk.root.as_path())
    .collect();
  let orphan_updates: BTreeMap<String, MeasuredBase> = usage.orphans.iter().map(|base| (base.path.clone(), MeasuredBase { at: now, bytes: base.bytes })).collect();
  for (pool_index, (pool, (file, cached, mark_updates))) in usage.pools.iter().zip(&files).enumerate() {
    let measured: BTreeMap<String, Measured> = pool
      .slots
      .iter()
      .enumerate()
      .filter(|(_, slot)| !params.cached && !slot.cached)
      .map(|(slot_index, slot)| {
        let refreshing = refreshing_slots
          .iter()
          .find(|refreshing| refreshing.pool == pool_index && refreshing.slot == slot_index)
          .expect("a measured slot has a walk");
        (
          slot.path.clone(),
          Measured {
            returned: slot.returned,
            leased: slot.state != SlotState::Free,
            at: slot.measured_at,
            mark: refreshing.mark,
            roots: refreshing.roots.clone(),
            nodes: refreshing.nodes.clone(),
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
    if params.cached && measured.is_empty() && bases.is_empty() && mark_updates.is_empty() {
      continue;
    }
    let paths: HashSet<&str> = pool.slots.iter().map(|slot| slot.path.as_str()).collect();
    let base_paths: HashSet<&str> = pool.checkout_bases.iter().map(|base| base.path.as_str()).collect();
    let written = file.update(params.lock_timeout, |disk: &mut DiskFile| {
      // the slots that left the pool and the output bases that are gone leave the file
      disk.slots.retain(|path, _| paths.contains(path.as_str()));
      disk.slots.extend(measured);
      for (path, mark) in mark_updates {
        if let Some(measured) = disk.slots.get_mut(path) {
          measured.mark = Some(*mark);
        }
      }
      disk.checkout_bases.retain(|path, _| base_paths.contains(path.as_str()));
      disk.checkout_bases.extend(bases);
      if !params.cached {
        disk.trash_bytes = cached.trash_bytes;
        disk.orphans.clone_from(&orphan_updates);
      }
      Ok(())
    });
    if let Err(error) = written {
      debug!(error = %format!("{error:#}"), "cannot write the bytes of the slots");
    }
  }
  usage.elapsed = started.elapsed();
  Ok(usage)
}

/// A root of a slot whose bytes are cached separately
#[derive(Clone, Copy)]
enum SlotRootKind {
  Worktree,
  Build(usize),
  Base(usize),
}

/// Where the bytes of a walk go
#[derive(Clone)]
enum Target {
  Worktree(usize, usize, usize),
  Build(usize, usize, usize, usize),
  SlotBase(usize, usize, usize, usize),
  Node(usize, usize, usize, SlotRootKind, String, u64),
  CheckoutBase(usize, usize),
  Orphan(usize),
}

struct WalkRequest {
  root_index: usize,
  node: Option<(String, u64)>,
}

fn root_key(path: &Path) -> String {
  fs_walk::changes::canonical_path(path).and_then(|path| path.to_str().map(str::to_owned)).unwrap_or_default()
}

fn slot_root_kind(target: &Target) -> SlotRootKind {
  match target {
    Target::Worktree(..) => SlotRootKind::Worktree,
    Target::Build(_, _, dir, _) => SlotRootKind::Build(*dir),
    Target::SlotBase(_, _, base, _) => SlotRootKind::Base(*base),
    Target::Node(_, _, _, kind, _, _) => *kind,
    Target::CheckoutBase(..) | Target::Orphan(..) => unreachable!("a slot root has a slot target"),
  }
}

fn adjust_slot_bytes(slot: &mut SlotUsage, kind: SlotRootKind, old_bytes: u64, new_bytes: u64) {
  let bytes = match kind {
    SlotRootKind::Worktree => &mut slot.worktree,
    SlotRootKind::Build(index) => &mut slot.build[index].bytes,
    SlotRootKind::Base(index) => &mut slot.output_bases[index].bytes,
  };
  *bytes = bytes.saturating_sub(old_bytes).saturating_add(new_bytes);
}

fn schedule_slot_refresh(walks: &mut Vec<(Walk, Target)>, refreshing_slots: &mut Vec<RefreshingSlot>, plan: SlotRefreshPlan<'_>) {
  let SlotRefreshPlan {
    pool,
    slot,
    slot_walks,
    root_keys,
    requests,
    previous,
    current_mark,
    incremental,
    build,
    bases,
  } = plan;
  let roots: Vec<CachedRoot> = slot_walks
    .iter()
    .enumerate()
    .map(|(index, _)| {
      let previous_root = previous.and_then(|measured| measured.roots.iter().find(|root| root.path == root_keys[index]));
      CachedRoot {
        path: root_keys[index].clone(),
        bytes: if incremental { previous_root.map_or(0, |root| root.bytes) } else { 0 },
        has_hardlinks: incremental && previous_root.is_some_and(|root| root.has_hardlinks),
      }
    })
    .collect();
  let nodes = if incremental {
    previous.map_or_else(Vec::new, |measured| measured.nodes.clone())
  } else {
    Vec::new()
  };
  for request in requests {
    let (root_walk, root_target) = &slot_walks[request.root_index];
    if let Some((path, bytes)) = request.node {
      let target = Target::Node(pool, slot, request.root_index, slot_root_kind(root_target), path.clone(), bytes);
      walks.push((Walk::of(root_walk.root.join(path)), target));
    } else {
      walks.push((root_walk.clone(), root_target.clone()));
    }
  }
  refreshing_slots.push(RefreshingSlot {
    pool,
    slot,
    roots,
    mark: current_mark.map(Into::into),
    nodes,
    incremental,
    build,
    bases,
  });
}

fn walk_requests(changes: &Changes, roots: &[Walk], root_keys: &[String], cached_roots: &[CachedRoot], nodes: &[CachedNode]) -> Vec<WalkRequest> {
  match changes {
    Changes::All => (0..roots.len()).map(|root_index| WalkRequest { root_index, node: None }).collect(),
    Changes::Paths(paths) => {
      let mut full = HashSet::new();
      let mut partial = BTreeMap::new();
      let mut full_slot = false;
      for path in paths {
        let Some(root_index) = roots
          .iter()
          .enumerate()
          .filter(|(index, _)| path.starts_with(Path::new(&root_keys[*index])))
          .max_by_key(|(index, _)| Path::new(&root_keys[*index]).components().count())
          .map(|(index, _)| index)
        else {
          continue;
        };
        let relative = path.strip_prefix(Path::new(&root_keys[root_index])).ok();
        let node = relative.and_then(|relative| {
          nodes
            .iter()
            .filter(|node| node.root == root_keys[root_index] && relative.starts_with(Path::new(&node.path)))
            .max_by_key(|node| Path::new(&node.path).components().count())
        });
        if let Some(node) = node {
          if node.has_hardlinks {
            full_slot = true;
            break;
          }
          if !full.contains(&root_index) {
            partial.insert((root_index, node.path.clone()), node.bytes);
          }
        } else {
          if cached_roots[root_index].has_hardlinks {
            full_slot = true;
            break;
          }
          full.insert(root_index);
          partial.retain(|(index, _), _| *index != root_index);
        }
      }
      if full_slot {
        return (0..roots.len()).map(|root_index| WalkRequest { root_index, node: None }).collect();
      }
      let mut requests: Vec<WalkRequest> = full.into_iter().map(|root_index| WalkRequest { root_index, node: None }).collect();
      requests.extend(partial.into_iter().map(|((root_index, path), bytes)| WalkRequest {
        root_index,
        node: Some((path, bytes)),
      }));
      requests.sort_by_key(|request| request.root_index);
      requests
    }
  }
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
#[cfg(test)]
#[path = "walk_test.rs"]
mod walk_test;

#[cfg(test)]
mod disk_test;
