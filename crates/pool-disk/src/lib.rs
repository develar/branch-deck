//! `pool-disk`: what the lease pools hold on disk (`brd lease disk`: the pool of its repository, with `--all` every pool of
//! this machine; the Disk window of the app: every pool; openspec/specs/cli/lease/spec.md, "Disk",
//! openspec/specs/repository/disk-usage/spec.md). A `du` of a pool takes minutes and says little: the tracked tree of a
//! slot is an APFS clone that shares its blocks with the others, and a slot holds more files than the vnode cache, so a
//! walk of it is cold (a minute for idea-1). What a slot holds of its own is what a delete of it frees: the private size
//! of its worktree (`fs_walk`: the blocks no clone shares), the directories its clean rules name (`target/`, `out/`) and
//! its Bazel output bases outside it, each a root of its own. The output bases of the main worktrees, the orphans and
//! the trash of a pool are measured the same way.
//!
//! A measure is kept by its parts (`fs_walk::TreeBytes`: subtrees of at most `NODE_ENTRIES` entries and the directories
//! above them) in `$GIT_COMMON_DIR/branch-deck/disk/`, a file a slot (written as the slot ends, so a run that stops
//! keeps what it finished) and `pool.json` for the rest. On macOS one replay of the `FSEvents` history for all measures
//! (`fs_walk::changes`) names the parts that changed since the mark of each: those are walked again, a directory above
//! the frontier is read alone; a replay that comes late keeps the saved bytes with their age. Elsewhere, without a
//! history, a free slot keeps its bytes while its `returned` and its reclaimed lease hold, a leased one and the rest for
//! `LEASED_STALE`. The walks run in one budget (`WALK_BUDGET`) unless `fresh`: the changed parts first, then the full
//! walks, the least recently measured first; what the budget did not reach keeps its saved bytes for the next report.
//! One report of a pool walks at a time (`disk/walk.lock`); another one shows the saved bytes.

use anyhow::Result;
use fs_walk::changes::{Change, HistoryQuery, HistoryRefusal, Mark, changed_since, mark};
use fs_walk::{PartBytes, TreeBytes, Walk, dir_listing, walk_tree_with_callback};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::{lease_config, state_dir};
use git_ops::revs::RepositoryDirs;
use lease_pool::free_space::available_bytes;
use lease_pool::lease::{Pool, Slot, SlotState, lease_status_core};
use lease_pool::lease_clean::{CleanRule, read_clean_rules};
use lease_pool::lease_notes::config_pool_dir;
use lease_pool::output_bases::{OutputBase, orphans};
use lease_pool::trim::{Machine, main_worktree, max_slots, trim_order};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::paths::display_path;
use sync_utils::size::parse_size;
use sync_utils::span::in_current_span;
use tracing::{debug, instrument};

/// The directory of the saved measures, beside the state of the pool
const DISK_DIR: &str = "disk";
/// The saved measures of the output bases of the main worktree, the orphans and the trash
const POOL_FILE: &str = "pool.json";
/// Held by the report that walks the pool
const WALK_LOCK: &str = "walk.lock";
/// The one file of the measures of brd before 1.20, which it still writes: read for a slot that `disk/` has not saved
const OLD_DISK_FILE: &str = "disk.json";
/// How long saved bytes hold without a history (Linux, Windows): a leased slot, the output bases of the main worktree
/// and the orphans, which builds write at any time
pub const LEASED_STALE: Duration = Duration::from_mins(15);
/// The most entries of a saved part: a change inside one walks at most this many entries again
const NODE_ENTRIES: u64 = 20_000;
/// How long the replay of the `FSEvents` history may take. A quiet Mac replays hours of it in a second; on the user's Mac
/// with agents building in 15 slots (2026-11-21) an hour of history took 60 s and 20 minutes of it did not end in 30 s,
/// so a longer wait buys nothing there: a late measure keeps its bytes and the budget walks the oldest
const HISTORY_DEADLINE: Duration = Duration::from_secs(10);
/// How long the walks of a report may take unless `fresh`
const WALK_BUDGET: Duration = Duration::from_secs(90);
/// How long a measure may stay unchecked because the history came late: past it the measure is walked again
const LATE_LIMIT: Duration = Duration::from_hours(24);

/// Which pools `disk_usage_core` reports
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskScope {
  /// The pool of the repository that asks, with its orphans: `brd lease disk`
  Repository,
  /// The pool of the repository and every pool under `~/.branch-deck/pool` of `DiskParams::home`, with every orphan of
  /// them, as the trim finds them: `brd lease disk --all`, the Disk window of the app, the agent of `brd-host`
  Machine,
}

/// What `disk_usage_core` reports
pub struct DiskParams<'a> {
  /// A checkout of the repository that asks: its pool is reported wherever it lives, and its config gives the minimum
  pub repository: &'a RepositoryDirs,
  /// The home directory of the default pools (`~/.branch-deck/pool`)
  pub home: Option<&'a Path>,
  /// The pool of the repository only, or every pool of the machine
  pub scope: DiskScope,
  /// Walk every measure again in full, without a budget, whatever `disk/` holds
  pub fresh: bool,
  /// Read `disk/` only, without measuring any path
  pub cached: bool,
}

/// Where the bytes of a slot come from, and why a slot was walked in full
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SlotWalk {
  /// The saved bytes, unchecked: `--cached`, or none saved yet
  Saved,
  /// The saved bytes: the history names no change since they were measured
  Unchanged,
  /// The saved bytes: another report walks the pool now
  Busy,
  /// The saved bytes: the history of the file system did not come in time
  HistoryLate,
  /// The saved bytes: the walk budget of the report ended first
  Budget,
  /// The parts that changed were walked again, the rest is saved
  Parts,
  /// Walked in full: nothing was saved
  NoSaved,
  /// Walked in full: `--fresh`
  Fresh,
  /// Walked in full: without a history, the saved bytes were too old or of another lease or return
  Stale,
  /// Walked in full: the history lost events under the slot
  HistoryLost,
  /// Walked in full: the saved bytes have no mark of the history, or one of another volume or boot
  NoMark,
  /// Walked in full: the history has not been read for `LATE_LIMIT`
  HistoryLateLong,
  /// Walked in full in the budget left after the other walks: the history of the file system did not come in time
  HistoryLateWalked,
  /// Walked in full: the history could not be read
  NoHistory,
}

impl SlotWalk {
  /// The bytes come from `disk/`, not from a walk of this call
  pub const fn is_saved(self) -> bool {
    matches!(self, Self::Saved | Self::Unchanged | Self::Busy | Self::HistoryLate | Self::Budget)
  }
}

/// What the pools of the report hold: the pool of the repository, or every pool of the machine (`DiskScope`)
#[derive(Debug, Clone, Default)]
pub struct DiskUsage {
  /// The free bytes of the volume of the repository; `None` where the system does not tell
  pub free_bytes: Option<u64>,
  /// `branchdeck.leaseMinFree` of the repository, in bytes
  pub min_free: Option<u64>,
  pub pools: Vec<PoolUsage>,
  /// The output bases whose slot is gone, which a trim deletes
  pub orphans: Vec<BaseUsage>,
  /// How long the report took
  pub elapsed: Duration,
  /// How long the replay of the history took; `None` without one
  pub history: Option<Duration>,
}

/// A report ready to show before or during a disk walk
#[derive(Debug, Clone)]
pub enum DiskUsageProgress {
  /// The saved bytes, before any check: `measuring` slots may be walked
  Cached { usage: DiskUsage, measuring: u32 },
  /// The history was read: `measuring` slots are walked, `late` keep their bytes because it came late
  Planning { measuring: u32, late: u32, history: Option<Duration> },
  /// The walks of a slot ended
  Slot {
    usage: DiskUsage,
    pool_index: usize,
    slot_index: usize,
    measured: u32,
    measuring: u32,
  },
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
  /// What the notes of its leases hold (`lease_pool::lease_notes`): a trim deletes those of returned leases past
  /// `branchdeck.leaseNotesKeep`
  pub notes_bytes: u64,
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
  /// The lease that a reclaim or a takeover took from it (`Slot::reclaimed`)
  pub reclaimed: Option<String>,
  /// What its worktree holds of its own beside its build directories: the blocks its files share with no clone (a slot
  /// cloned from a template shares most of them until moves rewrite its files)
  pub worktree: u64,
  /// The directories its clean rules name, each with its bytes
  pub build: Vec<DirUsage>,
  pub output_bases: Vec<BaseUsage>,
  /// Unix seconds of the walk the bytes come from; 0 when never measured
  pub measured_at: u64,
  /// The bytes come from `disk/`, not from a walk of this call
  pub cached: bool,
  /// Where the bytes come from, and why it was walked in full
  pub walk: SlotWalk,
  /// The parts walked or read again for `SlotWalk::Parts`
  pub parts: u32,
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

/// What a root of a measure is to a slot
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RootKind {
  Worktree,
  Build,
  Base,
  /// The root of a measure that is not a slot
  Whole,
}

/// A saved measure: of a slot (a file of `disk/`), an output base, an orphan, the trash (`pool.json`)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Saved {
  /// The slot, the output base or the trash it measures
  path: String,
  /// `returned` of the slot when it was measured
  #[serde(default, skip_serializing_if = "Option::is_none")]
  returned: Option<u64>,
  /// The lease that a reclaim or a takeover took from the slot then
  #[serde(default, skip_serializing_if = "Option::is_none")]
  reclaimed: Option<String>,
  /// The slot was leased then
  #[serde(default)]
  leased: bool,
  /// Unix seconds of the measure
  at: u64,
  /// Unix seconds of the last check that found it current: the measure, or a history that named no change
  #[serde(default)]
  checked: u64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  mark: Option<SavedMark>,
  roots: Vec<SavedRoot>,
}

/// The parts of a root: their bytes add up to the root
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SavedRoot {
  /// The canonical path
  key: String,
  kind: RootKind,
  bytes: u64,
  /// The subtrees, by the path relative to the root, with their bytes
  #[serde(default)]
  nodes: Vec<(String, u64)>,
  /// The directories above the subtrees, with the bytes of their own files
  #[serde(default)]
  dirs: Vec<(String, u64)>,
  /// The directories the walk skipped, canonical: the build directories of a slot inside its worktree, which count
  /// apart; another set makes the parts of no use
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  skip: Vec<String>,
}

impl SavedRoot {
  fn of(key: String, kind: RootKind, tree: &TreeBytes) -> Self {
    let mut root = Self {
      key,
      kind,
      bytes: 0,
      nodes: Vec::new(),
      dirs: Vec::new(),
      skip: Vec::new(),
    };
    root.insert("", tree);
    root
  }

  /// The parts of `tree`, the walk of the part at `at`, in place of every part at or under `at`
  fn insert(&mut self, at: &str, tree: &TreeBytes) {
    self.remove(at);
    let join = |part: &PartBytes| (join_relative(at, &part.path), part.bytes);
    self.nodes.extend(tree.nodes.iter().map(join));
    self.dirs.extend(tree.dirs.iter().map(join));
    self.sum();
  }

  /// Every part at or under `at`
  fn remove(&mut self, at: &str) {
    self.nodes.retain(|(path, _)| !is_under(path, at));
    self.dirs.retain(|(path, _)| !is_under(path, at));
    self.sum();
  }

  /// A root saved with its bytes alone, from the file of brd before 1.20
  const fn bytes_only(key: String, kind: RootKind, bytes: u64) -> Self {
    Self {
      key,
      kind,
      bytes,
      nodes: Vec::new(),
      dirs: Vec::new(),
      skip: Vec::new(),
    }
  }

  fn sum(&mut self) {
    self.bytes = self.nodes.iter().chain(&self.dirs).map(|(_, bytes)| bytes).sum();
  }
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

/// `pool.json`: the measures of a pool that are not of a slot
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct PoolFile {
  /// The output bases of the main worktree, by path
  #[serde(default)]
  checkout_bases: BTreeMap<String, Saved>,
  /// The orphans the last report of this pool measured, by path: each report writes those of its scope into the file
  /// of each pool it walked, so the latest measure of an orphan wins
  #[serde(default)]
  orphans: BTreeMap<String, Saved>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  trash: Option<Saved>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  notes_bytes: Option<u64>,
}

/// `disk.json` of brd before 1.20, as far as a report reads it: the bytes of each root, which a slot shows until its first
/// walk, without parts or a mark, so the history cannot check them
#[derive(Debug, Default, Deserialize)]
struct OldDiskFile {
  #[serde(default)]
  slots: BTreeMap<String, OldMeasured>,
  #[serde(default)]
  checkout_bases: BTreeMap<String, OldBase>,
}

#[derive(Debug, Deserialize)]
struct OldMeasured {
  #[serde(default)]
  returned: Option<u64>,
  #[serde(default)]
  reclaimed: Option<String>,
  #[serde(default)]
  leased: bool,
  at: u64,
  #[serde(default)]
  roots: Vec<OldRoot>,
}

#[derive(Debug, Deserialize)]
struct OldRoot {
  path: String,
  bytes: u64,
}

#[derive(Debug, Deserialize)]
struct OldBase {
  at: u64,
  bytes: u64,
}

impl OldMeasured {
  /// The saved measure of a slot with `roots`: the bytes of the roots it names, without parts or a mark
  fn saved(&self, path: &str, roots: &[UnitRoot]) -> Saved {
    Saved {
      path: path.to_owned(),
      returned: self.returned,
      reclaimed: self.reclaimed.clone(),
      leased: self.leased,
      at: self.at,
      checked: self.at,
      mark: None,
      roots: self
        .roots
        .iter()
        .filter_map(|old| {
          roots
            .iter()
            .find(|root| root.key == old.path)
            .map(|root| SavedRoot::bytes_only(old.path.clone(), root.kind, old.bytes))
        })
        .collect(),
    }
  }
}

/// The saved measures of a pool: `disk/` of its state directory
struct DiskStore {
  dir: PathBuf,
  state_dir: PathBuf,
}

impl DiskStore {
  fn of(git_common_dir: &Path) -> Self {
    let state_dir = state_dir(git_common_dir);
    Self {
      dir: state_dir.join(DISK_DIR),
      state_dir,
    }
  }

  fn slot_file(&self, path: &str) -> PathBuf {
    self.dir.join(format!("slot-{:016x}.json", fnv1a(path.as_bytes())))
  }

  /// The saved measure of the slot at `path`, if its file names that slot
  fn read_slot(&self, path: &str) -> Option<Saved> {
    read_json::<Saved>(&self.slot_file(path)).filter(|saved| saved.path == path)
  }

  fn read_pool(&self) -> PoolFile {
    read_json(&self.dir.join(POOL_FILE)).unwrap_or_default()
  }

  /// `disk.json` of brd before 1.20, if it is still there
  fn read_old(&self) -> OldDiskFile {
    read_json(&self.state_dir.join(OLD_DISK_FILE)).unwrap_or_default()
  }

  /// The saved measure of every slot, by its path
  fn read_slots(&self) -> Vec<Saved> {
    let Ok(entries) = std::fs::read_dir(&self.dir) else { return Vec::new() };
    entries
      .flatten()
      .filter(|entry| entry.file_name().to_string_lossy().starts_with("slot-"))
      .filter_map(|entry| read_json::<Saved>(&entry.path()))
      .collect()
  }

  fn write(&self, path: &Path, value: &impl Serialize) {
    let written = std::fs::create_dir_all(&self.dir)
      .and_then(|()| serde_json::to_vec(value).map_err(std::io::Error::other))
      .and_then(|bytes: Vec<u8>| write_atomically(path, &bytes, AtomicWrite::default()));
    if let Err(error) = written {
      debug!(path = %path.display(), %error, "cannot write the measure");
    }
  }

  /// The slot files of slots that left the pool
  fn prune(&self, slots: &HashSet<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(&self.dir) {
      for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with("slot-") && !slots.contains(&path) {
          let _ = sync_utils::fs_retry::remove_file(&path);
        }
      }
    }
  }

  /// The lock of the walks of the pool, at once or `None` while another report holds it
  fn try_walk_lock(&self) -> Option<std::fs::File> {
    std::fs::create_dir_all(&self.dir).ok()?;
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(self.dir.join(WALK_LOCK)).ok()?;
    match file.try_lock() {
      Ok(()) => Some(file),
      Err(std::fs::TryLockError::WouldBlock) => None,
      // a file system without locks: the report walks as if it held the lock
      Err(std::fs::TryLockError::Error(error)) => {
        debug!(%error, "cannot lock the walks of the pool");
        Some(file)
      }
    }
  }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
  let bytes = std::fs::read(path).ok()?;
  serde_json::from_slice(&bytes)
    .inspect_err(|error| debug!(path = %path.display(), %error, "cannot read the measure"))
    .ok()
}

/// FNV-1a: the name of the file of a slot, the same in every process
fn fnv1a(bytes: &[u8]) -> u64 {
  bytes
    .iter()
    .fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3))
}

/// The tuning of a report: the deadline of the history and the budget of the walks, apart for the tests
#[derive(Debug, Clone, Copy)]
struct Tuning {
  history_deadline: Duration,
  walk_budget: Duration,
  node_entries: u64,
}

impl Default for Tuning {
  fn default() -> Self {
    Self {
      history_deadline: HISTORY_DEADLINE,
      walk_budget: WALK_BUDGET,
      node_entries: NODE_ENTRIES,
    }
  }
}

/// The history of the file system for one query, the seam of the tests: the changed paths, or the refusal
type History<'a> = dyn FnMut(&HistoryQuery) -> std::result::Result<Vec<PathBuf>, HistoryRefusal> + 'a;

/// What a report measures: a slot, an output base of a main worktree, an orphan, the trash of a pool
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnitOf {
  Slot { pool: usize, slot: usize },
  CheckoutBase { pool: usize, base: usize },
  Orphan(usize),
  Trash(usize),
}

/// A root of a unit
struct UnitRoot {
  walk: Walk,
  key: String,
  kind: RootKind,
  /// The canonical paths of `walk.skip`
  skip: Vec<String>,
}

/// A part of a root to walk or read again
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Target {
  /// The whole root
  Root(usize),
  /// A saved subtree
  Node(usize, String),
  /// A saved directory above the subtrees: its own files and its directories
  Dir(usize, String),
  /// A directory that no part holds
  New(usize, String),
}

struct Unit {
  of: UnitOf,
  /// The pool whose `disk/` keeps it, and whose walk lock it needs; `None` for an orphan, kept by every pool
  pool: Option<usize>,
  roots: Vec<UnitRoot>,
  saved: Option<Saved>,
  /// The mark of now, taken before the history is read
  mark: Option<Mark>,
  /// What the report does with it
  walk: SlotWalk,
  targets: BTreeSet<Target>,
  /// The measure as it is being refreshed
  result: Vec<Option<SavedRoot>>,
  /// The walks of it not ended yet
  pending: usize,
  parts: u32,
  /// The slot state when measured: `returned`, `reclaimed`, leased
  state: (Option<u64>, Option<String>, bool),
  /// The history is asked about it
  queried: bool,
  /// Its saved measure comes from the file of brd before 1.20: it is written into `disk/` unless walked
  migrated: bool,
}

impl Unit {
  fn age_key(&self) -> u64 {
    self.saved.as_ref().map_or(0, |saved| saved.at)
  }

  /// The target of a changed `path`: the deepest root it lies in, then the deepest saved part
  fn target_of(&self, path: &Path, lookup: &[PartLookup], change: Change) -> Option<Target> {
    let path = path.to_str()?;
    let (index, root) = self
      .roots
      .iter()
      .enumerate()
      .filter(|(_, root)| !root.key.is_empty() && is_under(path, &root.key))
      .max_by_key(|(_, root)| root.key.len())?;
    let relative = path[root.key.len()..].trim_start_matches(std::path::MAIN_SEPARATOR).replace(std::path::MAIN_SEPARATOR, "/");
    let lookup = &lookup[index];
    if !lookup.saved {
      return Some(Target::Root(index));
    }
    let mut at = relative.as_str();
    loop {
      if lookup.nodes.contains(at) {
        return Some(Target::Node(index, at.to_owned()));
      }
      if change == Change::Path && lookup.dirs.contains(at) {
        return Some(Target::Dir(index, at.to_owned()));
      }
      // a lost subtree that no saved subtree holds is walked whole, in place of every part under it
      if change == Change::Subtree && at.is_empty() {
        return Some(if relative.is_empty() { Target::Root(index) } else { Target::New(index, relative) });
      }
      if at.is_empty() {
        return Some(Target::Root(index));
      }
      at = at.rfind('/').map_or("", |slash| &at[..slash]);
    }
  }
}

/// The saved parts of a root, for the target of a changed path
#[derive(Default)]
struct PartLookup {
  saved: bool,
  nodes: HashSet<String>,
  dirs: HashSet<String>,
}

/// What a report does to the pools of `params.scope`, before its walks
#[instrument(skip_all, fields(repository = %params.repository.top_level))]
pub fn disk_usage_core(git_executor: &GitCommandExecutor, params: &DiskParams<'_>) -> Result<DiskUsage> {
  disk_usage_core_inner(git_executor, params, None, None, Tuning::default())
}

/// The same report, notifying the caller of its saved state, of its plan after the history and of each slot that ends
#[instrument(skip_all, fields(repository = %params.repository.top_level))]
pub fn disk_usage_core_with_progress<F>(git_executor: &GitCommandExecutor, params: &DiskParams<'_>, mut on_progress: F) -> Result<DiskUsage>
where
  F: FnMut(DiskUsageProgress) + Send,
{
  disk_usage_core_inner(git_executor, params, Some(&mut on_progress), None, Tuning::default())
}

/// The same report with `history` as the history of the file system, the seam of the tests: it answers each query where
/// `changed_since` would, on every platform, with a mark where the platform has none; and with the budget of the walks
/// and the size of the parts. It waits for no `FSEvents`, which logs a write late on a loaded machine
#[cfg(test)]
fn disk_usage_core_with_history<F, G>(git_executor: &GitCommandExecutor, params: &DiskParams<'_>, mut on_progress: F, mut history: G, tuning: Tuning) -> Result<DiskUsage>
where
  F: FnMut(DiskUsageProgress) + Send,
  G: FnMut(&HistoryQuery) -> std::result::Result<Vec<PathBuf>, HistoryRefusal>,
{
  disk_usage_core_inner(git_executor, params, Some(&mut on_progress), Some(&mut history), tuning)
}

#[expect(clippy::too_many_lines, reason = "the phases of one report over shared state: saved, history, plan, walks, writes")]
fn disk_usage_core_inner(
  git_executor: &GitCommandExecutor,
  params: &DiskParams<'_>,
  mut on_progress: Option<&mut (dyn FnMut(DiskUsageProgress) + Send)>,
  history_override: Option<&mut History<'_>>,
  tuning: Tuning,
) -> Result<DiskUsage> {
  let started = Instant::now();
  // without a home `Machine::read` takes the pool of the repository alone, with the output bases of its checkouts
  let home = match params.scope {
    DiskScope::Repository => None,
    DiskScope::Machine => params.home,
  };
  let machine = Machine::read(params.repository, home);
  let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
  let config = lease_config(git_executor, &params.repository.top_level)?;
  let mut usage = DiskUsage {
    free_bytes: available_bytes(Path::new(&params.repository.top_level)),
    min_free: config.min_free.as_deref().and_then(|value| parse_size(value).ok()),
    ..DiskUsage::default()
  };
  let stores: Vec<DiskStore> = machine.pools.iter().map(|pool| DiskStore::of(&pool.git_common_dir)).collect();
  let mut pool_files: Vec<PoolFile> = stores.iter().map(DiskStore::read_pool).collect();
  // read only for a measure that `disk/` has not saved: 26 MB for idea-1
  let old_files: Vec<std::cell::OnceCell<OldDiskFile>> = stores.iter().map(|_| std::cell::OnceCell::new()).collect();
  let old_file = |pool: usize| old_files[pool].get_or_init(|| stores[pool].read_old());
  // the walk lock of each pool: without it a report reads the saved bytes and writes nothing
  let locks: Vec<Option<std::fs::File>> = stores.iter().map(|store| if params.cached { None } else { store.try_walk_lock() }).collect();
  let mut units: Vec<Unit> = Vec::new();
  for (pool_index, pool) in machine.pools.iter().enumerate() {
    let (statuses, _) = lease_status_core(&pool.git_common_dir, false)?;
    let limit = max_slots(git_executor, None, &pool.git_common_dir).0;
    let trims_next: HashSet<String> = {
      let slots: Vec<Slot> = statuses.iter().map(|status| status.slot.clone()).collect();
      let (present, free) = trim_order(&slots);
      free.into_iter().take(present.len().saturating_sub(limit)).map(|slot| slot.path.clone()).collect()
    };
    let repository = main_worktree(&pool.git_common_dir);
    let mut report = PoolUsage {
      git_common_dir: display_path(&pool.git_common_dir),
      repository: repository.as_deref().map(display_path),
      max_slots: limit,
      slots: Vec::new(),
      checkout_bases: Vec::new(),
      trash_bytes: 0,
      notes_bytes: pool_files[pool_index].notes_bytes.unwrap_or_default(),
    };
    if let Some(repository) = &repository {
      for base in machine.bases.iter().filter(|base| base.is_in(repository) && !in_any(base, &machine.pool_dirs)) {
        let usage = base_usage(base);
        let saved = pool_files[pool_index].checkout_bases.get(&usage.path).cloned();
        let roots = vec![UnitRoot::new(Walk::of(&base.path), RootKind::Whole)];
        let old = saved.is_none().then(|| old_file(pool_index).checkout_bases.get(&usage.path)).flatten().map(|old| Saved {
          path: usage.path.clone(),
          at: old.at,
          checked: old.at,
          roots: vec![SavedRoot::bytes_only(roots[0].key.clone(), RootKind::Whole, old.bytes)],
          ..Saved::default()
        });
        let mut unit = Unit::new(
          UnitOf::CheckoutBase {
            pool: pool_index,
            base: report.checkout_bases.len(),
          },
          Some(pool_index),
          roots,
          saved.or_else(|| old.clone()),
          (None, None, false),
        );
        unit.migrated = old.is_some();
        units.push(unit);
        report.checkout_bases.push(usage);
      }
    }
    let trash = Pool::new(&pool.git_common_dir).trash_dir();
    units.push(Unit::new(
      UnitOf::Trash(pool_index),
      Some(pool_index),
      vec![UnitRoot::new(Walk::of(&trash), RootKind::Whole)],
      pool_files[pool_index].trash.clone(),
      (None, None, false),
    ));
    for status in statuses.into_iter().filter(|status| !status.missing) {
      let slot = status.slot;
      let root = PathBuf::from(&slot.path);
      let build: Vec<String> = build_dirs(&root);
      let bases: Vec<BaseUsage> = machine.bases.iter().filter(|base| base.is_in(&root)).map(base_usage).collect();
      let mut roots: Vec<UnitRoot> = build.iter().map(|dir| UnitRoot::new(Walk::of(root.join(dir)), RootKind::Build)).collect();
      // the worktree without its build directories, which count apart
      let skip = build.iter().map(|dir| root.join(dir)).collect();
      roots.push(UnitRoot::new(Walk { root, skip }, RootKind::Worktree));
      roots.extend(bases.iter().map(|base| UnitRoot::new(Walk::of(&base.path), RootKind::Base)));
      let saved = stores[pool_index].read_slot(&slot.path);
      let old = saved
        .is_none()
        .then(|| old_file(pool_index).slots.get(&slot.path))
        .flatten()
        .map(|old| old.saved(&slot.path, &roots));
      let state = (slot.returned, slot.reclaimed.as_ref().map(|reclaimed| reclaimed.lease_id.clone()), slot.lease.is_some());
      let slot_index = report.slots.len();
      report.slots.push(SlotUsage {
        name: slot.name.clone(),
        path: slot.path.clone(),
        state: status.state,
        holder: slot.lease.as_ref().map(|lease| lease.holder.clone()),
        returned: slot.returned,
        reclaimed: state.1.clone(),
        worktree: 0,
        build: build.into_iter().map(|path| DirUsage { path, bytes: 0 }).collect(),
        output_bases: bases,
        measured_at: 0,
        cached: true,
        walk: SlotWalk::Saved,
        parts: 0,
        trims_next: trims_next.contains(&slot.path),
      });
      let mut unit = Unit::new(
        UnitOf::Slot {
          pool: pool_index,
          slot: slot_index,
        },
        Some(pool_index),
        roots,
        saved.or_else(|| old.clone()),
        state,
      );
      unit.migrated = old.is_some();
      units.push(unit);
    }
    usage.pools.push(report);
  }
  // a report writes the orphans of its scope into the file of each pool it walks, so the file of another pool can keep
  // an older measure of an orphan: the latest measure wins
  let mut saved_orphans: BTreeMap<String, Saved> = BTreeMap::new();
  for (path, saved) in pool_files.iter().flat_map(|file| &file.orphans) {
    if saved_orphans.get(path).is_none_or(|kept| saved.at > kept.at) {
      saved_orphans.insert(path.clone(), saved.clone());
    }
  }
  for base in orphans(&machine.bases, &machine.pool_dirs) {
    let orphan = base_usage(base);
    let saved = saved_orphans.get(&orphan.path).cloned();
    units.push(Unit::new(
      UnitOf::Orphan(usage.orphans.len()),
      None,
      vec![UnitRoot::new(Walk::of(&base.path), RootKind::Whole)],
      saved,
      (None, None, false),
    ));
    usage.orphans.push(orphan);
  }
  for unit in &units {
    show(&mut usage, unit, unit.saved.as_ref(), SlotWalk::Saved, 0);
  }
  debug!(units = units.len(), ms = started.elapsed().as_millis(), "read the saved measures");

  // what to do with each unit before the history; the seam of the tests has a history on every platform, and a mark
  let seamed = history_override.is_some();
  let any_lock = locks.iter().any(Option::is_some);
  let mut queries: Vec<(usize, HistoryQuery)> = Vec::new();
  for (index, unit) in units.iter_mut().enumerate() {
    let locked = unit.pool.map_or(any_lock, |pool| locks[pool].is_some());
    unit.walk = if params.cached {
      SlotWalk::Saved
    } else if !locked {
      SlotWalk::Busy
    } else if params.fresh {
      SlotWalk::Fresh
    } else if unit.saved.is_none() {
      SlotWalk::NoSaved
    } else if cfg!(target_os = "macos") || seamed {
      let saved = unit.saved.as_ref().expect("a saved measure");
      match saved.mark {
        None => SlotWalk::NoMark,
        Some(mark) => {
          let roots: Vec<PathBuf> = saved
            .roots
            .iter()
            .filter(|root| unit.roots.iter().any(|current| current.holds(root)))
            .map(|root| PathBuf::from(&root.key))
            .collect();
          queries.push((index, HistoryQuery { roots, mark: mark.into() }));
          unit.queried = true;
          SlotWalk::Unchanged
        }
      }
    } else if unit.stale_without_history(now) {
      SlotWalk::Stale
    } else {
      SlotWalk::Unchanged
    };
    if !unit.walk.is_saved() || unit.walk == SlotWalk::Unchanged {
      unit.mark = unit.mark_now().or_else(|| seamed.then_some(Mark { event_id: 1, device: 0 }));
    }
  }
  let measuring = |units: &[Unit]| u32::try_from(units.iter().filter(|unit| matches!(unit.of, UnitOf::Slot { .. }) && unit.may_walk()).count()).unwrap_or(u32::MAX);
  if let Some(progress) = on_progress.as_deref_mut() {
    progress(DiskUsageProgress::Cached {
      usage: usage.clone(),
      measuring: measuring(&units),
    });
  }
  if !params.cached {
    for (pool_index, pool) in machine.pools.iter().enumerate() {
      if locks[pool_index].is_some() {
        // the pool directory of the config only of the pool of the repository, whose config the report read; the
        // others by the slots of their state
        let pool_dir = (pool_index == 0).then(|| config_pool_dir(&config, &pool.git_common_dir, params.home).ok()).flatten();
        let bytes = Pool::new(&pool.git_common_dir).notes_roots(pool_dir.as_deref(), params.home).bytes();
        usage.pools[pool_index].notes_bytes = bytes;
        pool_files[pool_index].notes_bytes = Some(bytes);
      }
    }
  }

  // the history: the changed parts of each unit it was asked for
  if !queries.is_empty() {
    let history_started = Instant::now();
    let lookups: Vec<Vec<PartLookup>> = units.iter().map(Unit::lookup).collect();
    let mut changed: Vec<BTreeSet<Target>> = vec![BTreeSet::new(); units.len()];
    let answers: Vec<std::result::Result<(), HistoryRefusal>> = if let Some(history) = history_override {
      queries
        .iter()
        .map(|(index, query)| {
          history(query).map(|paths| {
            for path in paths {
              if let Some(target) = units[*index].target_of(&path, &lookups[*index], Change::Path) {
                changed[*index].insert(target);
              }
            }
          })
        })
        .collect()
    } else {
      let only: Vec<HistoryQuery> = queries.iter().map(|(_, query)| query.clone()).collect();
      changed_since(&only, tuning.history_deadline, &mut |query, path, change| {
        let index = queries[query].0;
        if let Some(target) = units[index].target_of(path, &lookups[index], change) {
          changed[index].insert(target);
        }
      })
    };
    for ((index, _), answer) in queries.iter().zip(answers) {
      let unit = &mut units[*index];
      debug!(unit = ?unit.of, ?answer, changed = changed[*index].len(), "the history of a measure");
      unit.walk = match answer {
        Ok(()) => {
          // a root that no saved measure holds is walked whole
          for (root_index, lookup) in lookups[*index].iter().enumerate() {
            if !lookup.saved {
              changed[*index].insert(Target::Root(root_index));
            }
          }
          unit.targets = std::mem::take(&mut changed[*index]);
          if unit.targets.is_empty() { SlotWalk::Unchanged } else { SlotWalk::Parts }
        }
        Err(HistoryRefusal::Late) => {
          let checked = unit.saved.as_ref().map_or(0, |saved| saved.checked.max(saved.at));
          if now.saturating_sub(checked) > LATE_LIMIT.as_secs() {
            SlotWalk::HistoryLateLong
          } else {
            SlotWalk::HistoryLate
          }
        }
        Err(HistoryRefusal::Dropped) => SlotWalk::HistoryLost,
        Err(HistoryRefusal::OtherDevice | HistoryRefusal::NoMark | HistoryRefusal::Future) => SlotWalk::NoMark,
        Err(HistoryRefusal::Unavailable) => SlotWalk::NoHistory,
      };
    }
    usage.history = Some(history_started.elapsed());
  }

  // the plan: the reads of the saved directories now, the walks in their order
  for unit in &mut units {
    unit.plan();
    if !unit.walk.is_saved() {
      let roots = unit.targets.iter().filter(|target| matches!(target, Target::Root(_))).count();
      debug!(unit = ?unit.of, walk = ?unit.walk, targets = unit.targets.len(), roots, parts = unit.parts, "the plan of a measure");
    }
  }
  let mut order: Vec<usize> = (0..units.len()).filter(|&index| units[index].pending_targets() > 0).collect();
  // the changed parts first, then the full walks, then the measures whose history came late, which keep their saved
  // bytes unless the budget reaches them; each the least recently measured first
  order.sort_by_key(|&index| {
    let class = match units[index].walk {
      SlotWalk::Parts => 0,
      SlotWalk::HistoryLate => 2,
      _ => 1,
    };
    (class, units[index].age_key())
  });
  let mut walks: Vec<Walk> = Vec::new();
  let mut walk_targets: Vec<(usize, Target)> = Vec::new();
  for &index in &order {
    let unit = &mut units[index];
    for target in unit.targets.clone() {
      if let Some(walk) = unit.walk_of(&target) {
        walks.push(walk);
        walk_targets.push((index, target));
        unit.pending += 1;
      }
    }
  }
  let measuring_now = u32::try_from(
    units
      .iter()
      .filter(|unit| matches!(unit.of, UnitOf::Slot { .. }) && (!unit.walk.is_saved() || unit.pending > 0))
      .count(),
  )
  .unwrap_or(u32::MAX);
  if usage.history.is_some()
    && let Some(progress) = on_progress.as_deref_mut()
  {
    let late = units
      .iter()
      .filter(|unit| matches!(unit.of, UnitOf::Slot { .. }) && unit.walk == SlotWalk::HistoryLate)
      .count();
    progress(DiskUsageProgress::Planning {
      measuring: measuring_now,
      late: u32::try_from(late).unwrap_or(u32::MAX),
      history: usage.history,
    });
  }

  // the units that need no walk end now: the unchanged ones with their new mark, the ones read alone
  let (writes, written) = mpsc::channel::<(PathBuf, Vec<u8>)>();
  let measured = Mutex::new(0u32);
  for unit in &mut units {
    if unit.pending == 0 && (unit.walk == SlotWalk::Unchanged || !unit.walk.is_saved()) {
      end_unit(unit, &mut usage, &stores, &mut pool_files, now, &writes);
      if matches!(unit.of, UnitOf::Slot { .. }) && !unit.walk.is_saved() {
        let count = {
          let mut measured = measured.lock().unwrap_or_else(PoisonError::into_inner);
          *measured += 1;
          *measured
        };
        if let (Some(progress), UnitOf::Slot { pool, slot }) = (on_progress.as_deref_mut(), unit.of) {
          progress(DiskUsageProgress::Slot {
            usage: usage.clone(),
            pool_index: pool,
            slot_index: slot,
            measured: count,
            measuring: measuring_now,
          });
        }
      }
    }
  }

  // the walks, in one budget unless fresh; each unit is written as its last walk ends
  let stop = AtomicBool::new(!params.fresh && tuning.walk_budget.is_zero());
  let state = Mutex::new((&mut units, &mut usage, &mut pool_files, on_progress));
  let (done, finished) = mpsc::channel::<()>();
  std::thread::scope(|scope| {
    let writer = scope.spawn(in_current_span(move || {
      for (path, bytes) in written {
        if let Err(error) = std::fs::create_dir_all(path.parent().unwrap_or(&path)).and_then(|()| write_atomically(&path, &bytes, AtomicWrite::default())) {
          debug!(path = %path.display(), %error, "cannot write the measure");
        }
      }
    }));
    if !params.fresh && !walks.is_empty() {
      let stop = &stop;
      let budget = tuning.walk_budget;
      scope.spawn(in_current_span(move || {
        if finished.recv_timeout(budget).is_err() {
          stop.store(true, Ordering::Relaxed);
        }
      }));
    }
    walk_tree_with_callback(&walks, tuning.node_entries, &stop, |walk, tree| {
      let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
      let (units, usage, pool_files, on_progress) = &mut *state;
      let (index, target) = &walk_targets[walk];
      let unit = &mut units[*index];
      unit.apply(target, tree);
      unit.pending -= 1;
      if unit.pending == 0 {
        if unit.walk == SlotWalk::HistoryLate {
          unit.walk = SlotWalk::HistoryLateWalked;
        }
        end_unit(unit, usage, &stores, pool_files, now, &writes);
        if let UnitOf::Slot { pool, slot } = unit.of {
          let count = {
            let mut measured = measured.lock().unwrap_or_else(PoisonError::into_inner);
            *measured += 1;
            *measured
          };
          if let Some(progress) = on_progress.as_deref_mut() {
            progress(DiskUsageProgress::Slot {
              usage: usage.clone(),
              pool_index: pool,
              slot_index: slot,
              measured: count,
              measuring: measuring_now,
            });
          }
        }
      }
    });
    let _ = done.send(());
    drop(writes);
    let _ = writer.join();
  });
  // what the budget did not reach keeps its saved bytes, as what another report walks or a late history left
  for unit in &mut units {
    if unit.pending > 0 && unit.walk != SlotWalk::HistoryLate {
      debug!(unit = ?unit.of, "the walk budget ended before the walks of a measure");
      unit.walk = SlotWalk::Budget;
    }
    if matches!(unit.walk, SlotWalk::Budget | SlotWalk::Busy | SlotWalk::HistoryLate) {
      show(&mut usage, unit, unit.saved.as_ref(), unit.walk, 0);
    }
  }
  if !params.cached {
    let orphans: BTreeMap<String, Saved> = units
      .iter()
      .filter(|unit| matches!(unit.of, UnitOf::Orphan(_)))
      .filter_map(|unit| unit.saved.clone().map(|saved| (saved.path.clone(), saved)))
      .collect();
    // a measure of the file of brd before 1.20 that no walk replaced goes into `disk/`: that file is of another brd
    for unit in units.iter().filter(|unit| unit.migrated && unit.walk.is_saved()) {
      let (Some(pool), Some(saved)) = (unit.pool, &unit.saved) else { continue };
      if locks[pool].is_none() {
        continue;
      }
      match unit.of {
        UnitOf::Slot { .. } => stores[pool].write(&stores[pool].slot_file(&saved.path), saved),
        UnitOf::CheckoutBase { .. } => {
          pool_files[pool].checkout_bases.entry(saved.path.clone()).or_insert_with(|| saved.clone());
        }
        UnitOf::Orphan(_) | UnitOf::Trash(_) => {}
      }
    }
    for (pool_index, store) in stores.iter().enumerate() {
      if locks[pool_index].is_none() {
        continue;
      }
      pool_files[pool_index].orphans.clone_from(&orphans);
      let base_paths: HashSet<&str> = usage.pools[pool_index].checkout_bases.iter().map(|base| base.path.as_str()).collect();
      pool_files[pool_index].checkout_bases.retain(|path, _| base_paths.contains(path.as_str()));
      store.write(&store.dir.join(POOL_FILE), &pool_files[pool_index]);
      let slots: HashSet<PathBuf> = usage.pools[pool_index].slots.iter().map(|slot| store.slot_file(&slot.path)).collect();
      store.prune(&slots);
    }
  }
  drop(locks);
  usage.elapsed = started.elapsed();
  Ok(usage)
}

impl UnitRoot {
  fn new(walk: Walk, kind: RootKind) -> Self {
    let key = root_key(&walk.root);
    let mut skip: Vec<String> = walk.skip.iter().map(|path| root_key(path)).collect();
    skip.sort();
    Self { walk, key, kind, skip }
  }

  /// The saved root of this one: its key, and the directories it skips
  fn holds(&self, saved: &SavedRoot) -> bool {
    saved.key == self.key && saved.skip == self.skip
  }

  /// Its walk `tree`, to save
  fn saved(&self, tree: &TreeBytes) -> SavedRoot {
    SavedRoot {
      skip: self.skip.clone(),
      ..SavedRoot::of(self.key.clone(), self.kind, tree)
    }
  }
}

impl Unit {
  fn new(of: UnitOf, pool: Option<usize>, roots: Vec<UnitRoot>, saved: Option<Saved>, state: (Option<u64>, Option<String>, bool)) -> Self {
    let result = vec![None; roots.len()];
    Self {
      of,
      pool,
      roots,
      saved,
      mark: None,
      walk: SlotWalk::Saved,
      targets: BTreeSet::new(),
      result,
      pending: 0,
      parts: 0,
      state,
      queried: false,
      migrated: false,
    }
  }

  /// It may be walked in this report: one to walk, or one the history checks
  const fn may_walk(&self) -> bool {
    !self.walk.is_saved() || self.queried
  }

  /// The mark of the volume of its roots now: of the worktree of a slot, else of the first root that exists
  fn mark_now(&self) -> Option<Mark> {
    let worktree = self.roots.iter().find(|root| root.kind == RootKind::Worktree);
    worktree.into_iter().chain(&self.roots).find_map(|root| mark(&root.walk.root))
  }

  /// Without a history: the saved bytes are of another return, reclaim or lease than now, or older than `LEASED_STALE`
  /// for a leased slot and for what is not a slot
  fn stale_without_history(&self, now: u64) -> bool {
    let Some(saved) = &self.saved else { return true };
    let young = now.saturating_sub(saved.at) < LEASED_STALE.as_secs();
    let roots_hold = saved.roots.len() == self.roots.len() && saved.roots.iter().zip(&self.roots).all(|(saved, root)| root.holds(saved));
    match self.of {
      UnitOf::Slot { .. } => {
        let (returned, reclaimed, leased) = &self.state;
        !roots_hold || saved.returned != *returned || saved.reclaimed != *reclaimed || saved.leased != *leased || (*leased && !young)
      }
      UnitOf::Trash(_) => true,
      UnitOf::CheckoutBase { .. } | UnitOf::Orphan(_) => !roots_hold || !young,
    }
  }

  /// The saved parts of each root
  fn lookup(&self) -> Vec<PartLookup> {
    self
      .roots
      .iter()
      .map(|root| {
        self
          .saved
          .as_ref()
          .and_then(|saved| saved.roots.iter().find(|saved| root.holds(saved)))
          .map_or_else(PartLookup::default, |saved| PartLookup {
            saved: true,
            nodes: saved.nodes.iter().map(|(path, _)| path.clone()).collect(),
            dirs: saved.dirs.iter().map(|(path, _)| path.clone()).collect(),
          })
      })
      .collect()
  }

  /// The refresh of a unit to walk: its targets, and the saved roots it starts from; the directories above the
  /// subtrees are read now, one read each
  fn plan(&mut self) {
    match self.walk {
      SlotWalk::Parts => {
        for (index, root) in self.roots.iter().enumerate() {
          self.result[index] = self.saved.as_ref().and_then(|saved| saved.roots.iter().find(|saved| root.holds(saved)).cloned());
        }
        self.dedupe();
        let dirs: Vec<Target> = self.targets.iter().filter(|target| matches!(target, Target::Dir(..))).cloned().collect();
        for target in dirs {
          self.targets.remove(&target);
          self.read_dir(&target);
        }
        // the reads add the new directories they found, or a whole root
        self.dedupe();
        self.parts += u32::try_from(self.targets.len()).unwrap_or(u32::MAX);
      }
      SlotWalk::HistoryLate => {
        self.targets = (0..self.roots.len()).map(Target::Root).collect();
      }
      walk if walk.is_saved() => {}
      _ => {
        self.targets = (0..self.roots.len()).map(Target::Root).collect();
      }
    }
  }

  /// No target lies in another: a whole root takes the place of its parts, a subtree walked whole the place of the parts
  /// at or under it, so no walk counts what another walk counts
  fn dedupe(&mut self) {
    let whole: HashSet<usize> = self
      .targets
      .iter()
      .filter_map(|target| if let Target::Root(index) = target { Some(*index) } else { None })
      .collect();
    self.targets.retain(|target| matches!(target, Target::Root(_)) || !whole.contains(&target.root()));
    let subtrees: Vec<(usize, String)> = self
      .targets
      .iter()
      .filter_map(|target| if let Target::New(index, at) = target { Some((*index, at.clone())) } else { None })
      .collect();
    self.targets.retain(|target| match target {
      Target::New(index, at) => !subtrees.iter().any(|(root, subtree)| root == index && subtree != at && is_under(at, subtree)),
      Target::Node(index, at) | Target::Dir(index, at) => !subtrees.iter().any(|(root, subtree)| root == index && is_under(at, subtree)),
      Target::Root(_) => true,
    });
  }

  const fn pending_targets(&self) -> usize {
    self.targets.len()
  }

  /// A saved directory above the subtrees read again: its own files, the parts under it that are gone, the
  /// directories in it that no part holds, which are walked
  fn read_dir(&mut self, target: &Target) {
    let Target::Dir(index, at) = target else { return };
    self.parts += 1;
    let root = &self.roots[*index];
    let path = join_path(&root.walk.root, at);
    let Some(saved) = self.result[*index].as_mut() else {
      self.targets.insert(Target::Root(*index));
      return;
    };
    match dir_listing(&path) {
      Ok(listing) => {
        if let Some(dir) = saved.dirs.iter_mut().find(|(dir, _)| dir == at) {
          dir.1 = listing.bytes;
        }
        let names: HashSet<String> = listing
          .dirs
          .iter()
          .filter(|name| !root.walk.skip.contains(&path.join(name)))
          .map(|name| name.to_string_lossy().into_owned())
          .collect();
        let children: HashSet<String> = saved
          .nodes
          .iter()
          .chain(&saved.dirs)
          .filter(|(part, _)| !part.is_empty() && part != at && parent_of(part) == at)
          .map(|(part, _)| part.rsplit('/').next().unwrap_or(part).to_owned())
          .collect();
        for gone in children.difference(&names) {
          saved.remove(&join_relative(at, gone));
        }
        for new in names.difference(&children) {
          self.targets.insert(Target::New(*index, join_relative(at, new)));
        }
        saved.sum();
      }
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => saved.remove(at),
      Err(_) => {
        self.targets.insert(Target::Root(*index));
      }
    }
  }

  /// The walk of a target
  fn walk_of(&self, target: &Target) -> Option<Walk> {
    let root = &self.roots[target.root()];
    let at = match target {
      Target::Root(_) => return Some(root.walk.clone()),
      Target::Node(_, at) | Target::New(_, at) => at,
      Target::Dir(..) => return None,
    };
    let path = join_path(&root.walk.root, at);
    Some(Walk {
      skip: root.walk.skip.iter().filter(|skip| skip.starts_with(&path)).cloned().collect(),
      root: path,
    })
  }

  /// The walk of a target ended with `tree`
  fn apply(&mut self, target: &Target, tree: &TreeBytes) {
    let index = target.root();
    let root = &self.roots[index];
    match target {
      Target::Root(_) => self.result[index] = Some(root.saved(tree)),
      Target::Node(_, at) | Target::New(_, at) => {
        let saved = self.result[index].get_or_insert_with(|| root.saved(&TreeBytes::default()));
        saved.insert(at, tree);
      }
      Target::Dir(..) => {}
    }
  }
}

impl Target {
  const fn root(&self) -> usize {
    match self {
      Self::Root(index) | Self::Node(index, _) | Self::Dir(index, _) | Self::New(index, _) => *index,
    }
  }
}

/// A unit whose walks ended, or that needs none: its new measure saved and shown
fn end_unit(unit: &mut Unit, usage: &mut DiskUsage, stores: &[DiskStore], pool_files: &mut [PoolFile], now: u64, writes: &mpsc::Sender<(PathBuf, Vec<u8>)>) {
  let saved = if unit.walk == SlotWalk::Unchanged {
    // without a history nothing changes in the saved measure
    if unit.mark.is_none() {
      show(usage, unit, unit.saved.as_ref(), SlotWalk::Unchanged, 0);
      return;
    }
    unit.saved.clone().map(|mut saved| {
      // a root that left the measure leaves the file: a build dir whose rule is gone
      saved.roots.retain(|root| unit.roots.iter().any(|current| current.holds(root)));
      saved.checked = now;
      saved.mark = unit.mark.map(Into::into).or(saved.mark);
      (saved.returned, saved.reclaimed, saved.leased) = unit.state.clone();
      saved
    })
  } else {
    let path = match unit.of {
      UnitOf::Slot { pool, slot } => usage.pools[pool].slots[slot].path.clone(),
      UnitOf::CheckoutBase { pool, base } => usage.pools[pool].checkout_bases[base].path.clone(),
      UnitOf::Orphan(base) => usage.orphans[base].path.clone(),
      UnitOf::Trash(_) => unit.roots[0].key.clone(),
    };
    Some(Saved {
      path,
      returned: unit.state.0,
      reclaimed: unit.state.1.clone(),
      leased: unit.state.2,
      at: now,
      checked: now,
      mark: unit.mark.map(Into::into),
      roots: unit
        .roots
        .iter()
        .zip(&unit.result)
        .map(|(root, result)| result.clone().unwrap_or_else(|| root.saved(&TreeBytes::default())))
        .collect(),
    })
  };
  let Some(saved) = saved else { return };
  match unit.of {
    UnitOf::Slot { pool, .. } => {
      let file = stores[pool].slot_file(&saved.path);
      if let Ok(bytes) = serde_json::to_vec(&saved) {
        let _ = writes.send((file, bytes));
      }
    }
    UnitOf::CheckoutBase { pool, .. } => {
      pool_files[pool].checkout_bases.insert(saved.path.clone(), saved.clone());
      send_pool(stores, pool_files, pool, writes);
    }
    UnitOf::Trash(pool) => {
      pool_files[pool].trash = Some(saved.clone());
      send_pool(stores, pool_files, pool, writes);
    }
    UnitOf::Orphan(_) => {}
  }
  let walk = unit.walk;
  let parts = unit.parts;
  show(usage, unit, Some(&saved), walk, parts);
  unit.saved = Some(saved);
}

fn send_pool(stores: &[DiskStore], pool_files: &[PoolFile], pool: usize, writes: &mpsc::Sender<(PathBuf, Vec<u8>)>) {
  if let Ok(bytes) = serde_json::to_vec(&pool_files[pool]) {
    let _ = writes.send((stores[pool].dir.join(POOL_FILE), bytes));
  }
}

/// The bytes of `saved` into the report of its unit
fn show(usage: &mut DiskUsage, unit: &Unit, saved: Option<&Saved>, walk: SlotWalk, parts: u32) {
  let bytes_of = |key: &str| saved.and_then(|saved| saved.roots.iter().find(|root| root.key == key)).map_or(0, |root| root.bytes);
  let all = || unit.roots.iter().map(|root| bytes_of(&root.key)).sum::<u64>();
  match unit.of {
    UnitOf::Slot { pool, slot } => {
      let report = &mut usage.pools[pool].slots[slot];
      let mut build = report.build.iter_mut();
      let mut bases = report.output_bases.iter_mut();
      for root in &unit.roots {
        let bytes = bytes_of(&root.key);
        match root.kind {
          RootKind::Worktree => report.worktree = bytes,
          RootKind::Build => {
            if let Some(dir) = build.next() {
              dir.bytes = bytes;
            }
          }
          RootKind::Base => {
            if let Some(base) = bases.next() {
              base.bytes = bytes;
            }
          }
          RootKind::Whole => {}
        }
      }
      report.measured_at = saved.map_or(0, |saved| saved.at);
      report.cached = walk.is_saved();
      report.walk = walk;
      report.parts = parts;
    }
    UnitOf::CheckoutBase { pool, base } => usage.pools[pool].checkout_bases[base].bytes = all(),
    UnitOf::Orphan(base) => usage.orphans[base].bytes = all(),
    UnitOf::Trash(pool) => usage.pools[pool].trash_bytes = all(),
  }
}

/// The bytes of the build dirs of each slot of the pool of `git_common_dir` as `disk/` last measured them, of any age,
/// by slot path: a read of the files without a lock or a walk, for the order of the leased slots in a shrink
/// (`lease_pool::shrink::ShrinkParams::cached_build`). Empty when there are none
pub fn cached_build_bytes(git_common_dir: &Path) -> HashMap<String, u64> {
  DiskStore::of(git_common_dir)
    .read_slots()
    .into_iter()
    .map(|saved| {
      let bytes = saved.roots.iter().filter(|root| root.kind == RootKind::Build).map(|root| root.bytes).sum();
      (saved.path, bytes)
    })
    .collect()
}

fn root_key(path: &Path) -> String {
  fs_walk::changes::canonical_path(path).and_then(|path| path.to_str().map(str::to_owned)).unwrap_or_default()
}

/// `path` is `at` or lies under it (`/` between names, `""` the root); `at` is a path of the file system or relative
fn is_under(path: &str, at: &str) -> bool {
  at.is_empty() || path == at || (path.starts_with(at) && matches!(path.as_bytes().get(at.len()), Some(b'/' | b'\\')))
}

fn parent_of(part: &str) -> &str {
  part.rfind('/').map_or("", |slash| &part[..slash])
}

fn join_relative(at: &str, path: &str) -> String {
  match (at.is_empty(), path.is_empty()) {
    (true, _) => path.to_owned(),
    (false, true) => at.to_owned(),
    (false, false) => format!("{at}/{path}"),
  }
}

fn join_path(root: &Path, relative: &str) -> PathBuf {
  if relative.is_empty() {
    root.to_path_buf()
  } else {
    relative.split('/').fold(root.to_path_buf(), |path, name| path.join(name))
  }
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
pub mod step;
pub mod trim_report;
#[cfg(test)]
#[path = "walk_test.rs"]
mod walk_test;

#[cfg(test)]
mod disk_test;
