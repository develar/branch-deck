//! The lease pool of a repository (`brd lease`, openspec/specs/cli/lease/spec.md, "Lease" in docs/design/git-operations.md): linked
//! worktrees that agent sessions lease, one session per slot. The state in `$GIT_COMMON_DIR/branch-deck/leases.json` is
//! the authority: a free slot is pool property, and a get replaces its content, so no ancestry proves anything. The
//! pool has a lock of its own (`leases.lock`), held only while the state is read and written, never across a git that
//! writes a slot: a get that reuses a slot never waits for another's checkout, and no get waits for an update. A get
//! that adds a slot holds the clones lock of the pool (`clones.lock`, `Pool::lock_clones`) for the whole clone and its
//! checkout, so the pool adds one slot at a time, and across the checks of each slot that its reserve under the lock
//! gives it (`end_checks_under_lock`). The gets that find no free slot wait in a queue of the state (`get_queue`): only its first waiting get waits
//! for the clones lock, and a free slot or an idle lease goes to the first waiting get that has not tried it. The
//! `worktree` commands of the pool, which git can't run two at once,
//! take a lock of their own (`worktrees.lock`, `Pool::worktree_command`). The order is `clones.lock`, `leases.lock`,
//! `worktrees.lock`: a get takes the other two under the clones lock, one at a time; nothing is taken under the pool
//! lock or the lock of the `worktree` commands (the move of an adopt holds only the latter), and a return or a clean
//! never takes the clones lock. The
//! lock of the lend table of the host slots (`host-slots.lock`, `host_slots`) comes before all three and is a leaf: it is
//! never held while another is taken. The repository lock (`branch-deck.lock`) is never held across a lock of the pool:
//! `brd lease pick --return` releases it with the end of its pick, before its return takes `leases.lock`.

use crate::bazel_servers::{servers_by_output_base, shutdown};
use crate::clone_dir::{CloneSkips, clone_directory};
use crate::free_space::{SpaceOf, Volume, VolumeSpace, volume_space};
use crate::get_queue::{Behind, Gate, QueuePlace, QueueTicket, QueueTiming, Turn, settle_queue};
use crate::host_slots::{HOST_HOLDER_PREFIX, RelievedHostSlot, host_slot_number};
use crate::idle_changes::{ChangesVerdict, Preserve, PreserveConfig, Preserved, preserve_changes, preserves, withdraw_preserved};
use crate::landed_work::{LandedCheck, LandedCommit, Purpose, landed_work};
use crate::lease_clean::{
  CleanMode, CleanOptions, CleanOverrides, Cleaned, Trash, apply_clean_rules, apply_clean_rules_with, clone_skips, describe_size, holds_gone_runs, read_clean_rules,
  remove_together, sweep_gone_until,
};
use crate::lease_notes::{LeaseNotes, NotesRoots, close_notes, new_notes_root, open_notes};
use crate::shrink::{CachedBuild, ShrinkParams, Shrunk, lock_pressure, shrink_core};
use crate::state_file::StateFile;
use crate::trim::{EvictParams, RemovedSlot, TrimParams, Trimmed, evict_core, machine_pools, remove_unfinished_slots, trim_core};
use anyhow::{Context, Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::{
  AgentLanding, CheckoutWorkers, LeaseConfig, LfsFiles, NOTES_LINK, SlotMove, UnlandedCommit, UntrackedFiles, attach_new_branch, checkout_slot, clean_cloned_slot,
  detach_slot_head, discard_slot_changes, ignored_paths, is_valid_branch_name, lease_config, linked_worktree, move_clean_slot, move_slot, refresh_cloned_index, register_slot,
  slot_changes, start_deferred_maintenance, unlock_slot,
};
use git_ops::lfs::{checkout_lfs_files, pull_lfs_files, skips_download_errors};
use git_ops::revs::{RepositoryDirs, read_head, ref_exists, resolve_commit, short_id};
use git_ops::update_resolve::session_ref;
use history_rewrite::repository_lock::{LockHolder, LockWaitCanceled, RepositoryLock, lock_file_while_held};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fmt;
use std::fmt::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sync_core::lease_state::forget_lease_state;
use sync_utils::fs_retry;
use sync_utils::list_cut::kept;
use sync_utils::paths::display_path;
use sync_utils::process_scan::{Ancestors, ProcessInfo, ProcessScan, is_running, process_started, terminate};
use sync_utils::size::parse_size;
use sync_utils::span::in_current_span;
use tracing::{debug, instrument, warn};

/// The state of the pool in `branch-deck/` of the common dir
pub(crate) const POOL_FILE: &str = "leases.json";
const LOCK_FILE: &str = "leases.lock";
/// The trash of the returns and the cleans, in `branch-deck/` of the common dir (`lease_clean::Trash`)
const TRASH_DIR: &str = "trash";
/// How many characters of the argv of a busy process an answer names
const COMMAND_LIMIT: usize = 200;
/// How long a return waits for a `HEAD.lock` of the slot that git's background maintenance holds (`pack-refs`,
/// `reflog expire` of a `gc --auto`)
const HEAD_LOCK_WAIT: Duration = Duration::from_secs(10);
/// The lock of the `worktree` commands of the pool (`Pool::worktree_command`)
const WORKTREES_LOCK_FILE: &str = "worktrees.lock";
/// The lock of the gets that add a slot (`Pool::lock_clones`)
const CLONES_LOCK_FILE: &str = "clones.lock";
/// How often a get that waits for the clones lock tells the same holder again (`GetStep::WaitForClone`)
const CLONE_WAIT_TICK: Duration = Duration::from_mins(1);

/// How often a get that waits for the shrink or the eviction of another tells its wait again (`GetStep::WaitForPressure`)
const PRESSURE_WAIT_TICK: Duration = Duration::from_mins(1);
/// The file at the root of the source that names the ignored files a get copies into the slot
pub const WORKTREE_INCLUDE: &str = ".worktreeinclude";

/// The state of the pool: every slot, free or leased
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseStore {
  #[serde(default)]
  pub slots: Vec<Slot>,
  /// The highest `Slot::returned` the pool gave. A reclaim numbers its slot below the other free slots, so the slot of
  /// the highest number can hold a lower one later. A return takes one past this (`stamp_returned`), so no free slot
  /// gets the number of an earlier free period again. `None` in a state of a brd before it: the highest number of the
  /// slots counts then
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub last_returned: Option<u64>,
  /// The gets that wait for a slot (`get_queue`), in no order: each ticket names its place
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub queue: Vec<QueueTicket>,
}

impl LeaseStore {
  /// The highest number a return of the pool gave: `last_returned`, or a higher number of a slot
  fn top_returned(&self) -> u64 {
    self.slots.iter().filter_map(|slot| slot.returned).chain(self.last_returned).max().unwrap_or(0)
  }
}

/// A linked worktree of the pool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
  /// The worktree id (its directory under `$GIT_COMMON_DIR/worktrees`); empty while a new slot is being added
  #[serde(default)]
  pub name: String,
  /// The top level, canonical
  pub path: String,
  /// A get found local changes in the free slot and skipped it; the next gets try it last
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub dirty: bool,
  /// The `status` that found it dirty, while `dirty`: a later get skips the slot without another while it holds
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub dirty_check: Option<DirtyCheck>,
  /// The number of its last return in the pool, higher for a later one, below every other free slot for a reclaim
  /// (`stamp_reclaimed`): the order of the free slots of one pool. A get takes the highest first, the trim the lowest
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub returned: Option<u64>,
  /// Unix seconds of its last return, never before an earlier return of the pool (`stamp_returned`), so it orders the
  /// free slots of one pool as `returned` does: `returned` counts within one pool, and the shrink of a get below the
  /// minimum free space orders the free slots of every pool of the machine by this (`shrink::shrink_order`). `None` for
  /// a slot never returned, for one a brd before this field returned, and for a reclaimed slot that comes before such a
  /// slot
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub returned_at: Option<u64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub lease: Option<Lease>,
  /// The lease a takeover or `brd lease reclaim` took from its dead holder: a command of another session in the slot is
  /// refused (`reclaimed_refusal`) until the next get of the free slot
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reclaimed: Option<Reclaimed>,
}

/// How far a reclaim moves up the numbers of the other free slots of its pool when no number is left below them
/// (`stamp_reclaimed`). The move gives each of them a new epoch, and `pool-disk` measures it again, so it leaves room
/// for many reclaims
pub(crate) const RECLAIM_ROOM: u64 = 1_000;

/// Record a return of the slot `index` of `store` at `now` (Unix seconds): one past the highest number the pool gave,
/// and a time not before the last return of the pool. So a step back of the clock leaves the order of the pool by time,
/// which the shrink across pools reads, the same as its order by number
pub(crate) fn stamp_returned(store: &mut LeaseStore, index: usize, now: u64) {
  let number = store.top_returned() + 1;
  let last_time = store.slots.iter().filter_map(|slot| slot.returned_at).max().unwrap_or(0);
  let slot = &mut store.slots[index];
  slot.returned = Some(number);
  slot.returned_at = Some(now.max(last_time));
  store.last_returned = Some(number);
}

/// Record the return of the slot `index` of `store` by a reclaim, its lease quiet since `quiet_at` (Unix seconds), as
/// the coldest free slot of the pool. Its number is below that of every other free slot, so no other free slot changes
/// its epoch or its measure of `pool-disk`. That number can be one that the slot had in an earlier free period, so the
/// epoch and the measure of a reclaimed slot also name the reclaimed lease (`Reclaimed::lease_id`). The number is not
/// its number before the lease either: a brd that compares only the number most likely kept that measure. Its time is
/// not after that of any other free slot (`None` before a slot without one). So every brd orders it last for a get and
/// first for the trim and the shrink, by the number alone (a brd before `returned_at`) or by the time first, and no
/// clock moves it. With no number left below, the other free slots move up by `RECLAIM_ROOM` first. With no other free
/// slot that was returned, it gets one past the highest number of the pool and `quiet_at`. `last_returned` keeps the
/// highest number, which the slot can give up here
pub(crate) fn stamp_reclaimed(store: &mut LeaseStore, index: usize, quiet_at: u64) {
  let top = store.top_returned();
  let slots = &mut store.slots;
  let own = slots[index].returned;
  let other_free = |(other, slot): &(usize, &Slot)| *other != index && slot.lease.is_none() && slot.returned.is_some();
  let lowest = slots.iter().enumerate().filter(other_free).filter_map(|(_, slot)| slot.returned).min();
  let Some(lowest) = lowest else {
    slots[index].returned = Some(top + 1);
    slots[index].returned_at = Some(quiet_at);
    store.last_returned = Some(top + 1);
    return;
  };
  let time = slots
    .iter()
    .enumerate()
    .filter(other_free)
    .map(|(_, slot)| slot.returned_at)
    .chain([Some(quiet_at)])
    .min()
    .flatten();
  // the two highest numbers below `lowest`: one of them is not its own
  let below = |lowest: u64| (0..lowest).rev().take(2).find(|&number| Some(number) != own);
  let number = below(lowest).unwrap_or_else(|| {
    for (other, slot) in slots.iter_mut().enumerate() {
      if other != index
        && slot.lease.is_none()
        && let Some(number) = slot.returned.as_mut()
      {
        *number += RECLAIM_ROOM;
      }
    }
    below(lowest + RECLAIM_ROOM).unwrap_or(lowest + RECLAIM_ROOM - 1)
  });
  slots[index].returned = Some(number);
  slots[index].returned_at = time;
  store.last_returned = Some(store.top_returned().max(top));
}

/// A lease that was taken from its dead holder because its work had landed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reclaimed {
  pub lease_id: String,
  pub holder: String,
  /// Unix seconds of its get
  pub leased_at: u64,
  /// The branch its get created: it stays, with the commits of the lease
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub branch: Option<String>,
  /// Where its commits landed, the home of the lease
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub source: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub source_branch: Option<String>,
  /// Unix seconds of the takeover
  pub at: u64,
  /// The lease of the get that took it over; none for `brd lease reclaim`, which left the slot free
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by_lease_id: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by_holder: Option<String>,
  /// The local changes of the lease, preserved as patches in its notes before its slot was discarded (`idle_changes`)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub preserved: Option<Preserved>,
}

/// The last check of an idle lease that found work (`check_idle`): a later get checks it again only when what the
/// verdict rests on moved
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "IdleCheckRecord")]
pub struct IdleCheck {
  /// Unix seconds
  pub at: u64,
  /// The slot as its files tell it, no git: a commit, a reset or a checkout changes it
  pub mark: SlotMark,
  /// The tip of the home branch: a commit that lands there may prove the commits
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub home_tip: Option<String>,
  /// The commits no proof found on the home branch, newest first
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub commits: Vec<WorkCommit>,
  /// The local changes, the first `PATH_LIMIT` of them (`cut_paths`)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub paths: Vec<String>,
  /// How many local changes `paths` leaves out
  #[serde(default, skip_serializing_if = "is_zero")]
  pub paths_omitted: usize,
  /// A git operation in progress (`rebase`, `merge`, `cherry-pick`, `revert`, `bisect`) or an open resolve session
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub operation: Option<String>,
  /// The first `DIRTY_PROOF_PATHS` of `paths` with their stat then, as `DirtyCheck::proof`: a change restored by hand
  /// moves them, and the next get checks the lease again
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub proof: Vec<ProofPath>,
  /// The two ignore files of the slot then (`ignore_stats`), with `proof`
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub ignores: Vec<FileStat>,
  /// When the local changes, the only work, are quiet (Unix seconds, `idle_changes`): the verdict holds until then, and a
  /// get or a reclaim after it preserves them and takes the slot. `None`: never, or `not_preserved` says why
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub quiet_at: Option<u64>,
  /// Why the local changes, quiet, stay in the slot as they are (`git_ops::lease_slot::PreserveRefusal`, a lease
  /// without notes): the verdict holds by `proof`, the path that refuses where one does, and `preserve_config`
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub not_preserved: Option<String>,
  /// The keys of the preserve the check ran with (`PreserveConfig::fingerprint`): another value ends the verdict
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub preserve_config: Option<String>,
  /// Why the last preserve failed (an edit meanwhile, a process in the slot): the next one waits for `quiet_at`
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub preserve_error: Option<String>,
  /// The stat of the index of the slot with `not_preserved`: a refusal of the index (skip-worktree, an unmerged entry)
  /// ends with a write of the index, which moves no file
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub index: Option<FileStat>,
}

/// An `IdleCheck` as the state holds it, its paths cut as it is read: a brd before the cut kept them all
#[derive(Deserialize)]
struct IdleCheckRecord {
  at: u64,
  mark: SlotMark,
  #[serde(default)]
  home_tip: Option<String>,
  #[serde(default)]
  commits: Vec<WorkCommit>,
  #[serde(default)]
  paths: Vec<String>,
  #[serde(default)]
  paths_omitted: usize,
  #[serde(default)]
  operation: Option<String>,
  #[serde(default)]
  proof: Vec<ProofPath>,
  #[serde(default)]
  ignores: Vec<FileStat>,
  #[serde(default)]
  quiet_at: Option<u64>,
  #[serde(default)]
  not_preserved: Option<String>,
  #[serde(default)]
  preserve_config: Option<String>,
  #[serde(default)]
  preserve_error: Option<String>,
  #[serde(default)]
  index: Option<FileStat>,
}

impl From<IdleCheckRecord> for IdleCheck {
  fn from(record: IdleCheckRecord) -> Self {
    let (paths, paths_omitted) = cut_paths(record.paths, record.paths_omitted);
    Self {
      at: record.at,
      mark: record.mark,
      home_tip: record.home_tip,
      commits: record.commits,
      paths,
      paths_omitted,
      operation: record.operation,
      proof: record.proof,
      ignores: record.ignores,
      quiet_at: record.quiet_at,
      not_preserved: record.not_preserved,
      preserve_config: record.preserve_config,
      preserve_error: record.preserve_error,
      index: record.index,
    }
  }
}

/// How many paths of a list of local changes the state keeps (`DirtyCheck`, `IdleCheck`) and an answer of `brd` names;
/// the rest is a count. A slot whose tree is half written lists every tracked file as deleted: 2 million paths of
/// idea-1 made `leases.json` 380 MB, which every change of the pool read and wrote under its lock, longer than the
/// wait of a return for that lock
pub const PATH_LIMIT: usize = 100;

/// `paths` cut at `PATH_LIMIT` (`list_cut::kept`: never a count of one), and `omitted` with what the cut left out. A list
/// cut once stays as it is
#[must_use]
pub fn cut_paths(mut paths: Vec<String>, omitted: usize) -> (Vec<String>, usize) {
  let kept = kept(paths.len(), PATH_LIMIT);
  let omitted = omitted.saturating_add(paths.len() - kept);
  paths.truncate(kept);
  (paths, omitted)
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature of `skip_serializing_if`")]
const fn is_zero(count: &usize) -> bool {
  *count == 0
}

impl IdleCheck {
  /// The work, in words: "2 commit(s) its home branch lacks", "3 local change(s)", "a rebase in progress"
  pub fn describe(&self) -> String {
    let mut work = Vec::new();
    if !self.commits.is_empty() {
      work.push(format!("{} commit(s) its home branch lacks", self.commits.len()));
    }
    if !self.paths.is_empty() {
      work.push(format!("{} local change(s)", self.paths.len() + self.paths_omitted));
    }
    if let Some(operation) = &self.operation {
      work.push(format!("a {operation} in progress"));
    }
    work.join(", ")
  }

  /// The check of an idle lease whose only work is the local changes `paths` of `slot`, found now with the slot and the
  /// home tip of `state` under `config`: with their stat proof, and when they are quiet or why they stay (`verdict`).
  /// The proof of a verdict that a path refuses is that path alone, so its fix by hand ends the verdict
  pub(crate) fn of_changes(slot: &Path, (mark, home_tip): (SlotMark, Option<String>), paths: Vec<String>, config: &PreserveConfig, verdict: ChangesVerdict) -> Self {
    let (quiet_at, not_preserved, refusing) = match verdict {
      ChangesVerdict::Quiet(quiet_at) => (quiet_at, None, None),
      ChangesVerdict::Kept { reason, path } => (None, Some(reason), path),
    };
    // a verdict that a path refuses holds while that path has its stat: its fix by hand ends it, past the first changes
    let proof = proof_of(slot, refusing.as_ref().map_or(&paths[..], std::slice::from_ref));
    let index = not_preserved.is_some().then(|| index_stat(slot));
    let (paths, paths_omitted) = cut_paths(paths, 0);
    Self {
      at: now(),
      mark,
      home_tip,
      commits: Vec::new(),
      paths,
      paths_omitted,
      operation: None,
      proof,
      ignores: ignore_stats(slot),
      quiet_at,
      not_preserved,
      preserve_config: Some(config.fingerprint()),
      preserve_error: None,
      index,
    }
  }

  /// This check after a preserve that failed with `error` at `now`: the next one waits `RECLAIM_RETRY`, as a reclaim
  /// that failed does, rather than run again at each get
  pub(crate) fn failed_preserve(&self, error: &anyhow::Error, now: u64) -> Self {
    Self {
      at: now,
      quiet_at: Some(now.saturating_add(RECLAIM_RETRY.as_secs())),
      preserve_error: Some(format!("{error:#}")),
      ..self.clone()
    }
  }

  /// Whether the verdict holds at `now` for the slot at `slot` as `mark` and the home tip `home_tip` tell it: the slot
  /// did not change (its holder is gone); the local changes while one of their proof paths has its stat of then (as
  /// `DirtyCheck::holds`), they are not quiet yet and the keys of the preserve are `config` still (`None`: not read); an operation as long as that; the commits while the home branch
  /// did not move either
  fn holds(&self, slot: &Path, (mark, home_tip): (&SlotMark, Option<&str>), now: u64, config: Option<&str>) -> bool {
    if self.mark != *mark {
      return false;
    }
    if !self.paths.is_empty() {
      return proof_holds(slot, &self.proof, &self.ignores)
        && self.index.as_ref().is_none_or(|index| *index == index_stat(slot))
        && self.quiet_at.is_none_or(|at| now < at)
        && config.is_none_or(|config| self.preserve_config.as_deref() == Some(config));
    }
    self.operation.is_some() || self.home_tip.as_deref() == home_tip
  }
}

/// How many of the dirty paths a `DirtyCheck` keeps with their stat: one of them unchanged proves the slot dirty still
const DIRTY_PROOF_PATHS: usize = 8;

/// The `status` that found a free slot dirty (`Slot::dirty_check`), and what proves the slot dirty still without
/// another: a `status` of a slot of idea-1 takes 2 to 40 s, and every get that found no clean free slot paid it for
/// each dirty one before it added a slot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "DirtyCheckRecord")]
pub struct DirtyCheck {
  /// The slot as its files told it then, no git: a commit, a reset or a checkout changes it
  pub mark: SlotMark,
  /// The local changes it found, which a get that skips the slot names: the first `PATH_LIMIT` of them (`cut_paths`)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub paths: Vec<String>,
  /// How many local changes `paths` leaves out
  #[serde(default, skip_serializing_if = "is_zero")]
  pub paths_omitted: usize,
  /// The first `DIRTY_PROOF_PATHS` of them with their stat then
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub proof: Vec<ProofPath>,
  /// The top-level `.gitignore` of the slot and `info/exclude` of its repository: an untracked file they come to
  /// ignore is no change
  #[serde(default)]
  pub ignores: Vec<FileStat>,
}

/// A `DirtyCheck` as the state holds it, its paths cut as it is read: a brd before the cut kept them all
#[derive(Deserialize)]
struct DirtyCheckRecord {
  mark: SlotMark,
  #[serde(default)]
  paths: Vec<String>,
  #[serde(default)]
  paths_omitted: usize,
  #[serde(default)]
  proof: Vec<ProofPath>,
  #[serde(default)]
  ignores: Vec<FileStat>,
}

impl From<DirtyCheckRecord> for DirtyCheck {
  fn from(record: DirtyCheckRecord) -> Self {
    let (paths, paths_omitted) = cut_paths(record.paths, record.paths_omitted);
    Self {
      mark: record.mark,
      paths,
      paths_omitted,
      proof: record.proof,
      ignores: record.ignores,
    }
  }
}

/// A dirty path of a `DirtyCheck`, relative to the slot, with its stat
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofPath {
  pub path: String,
  pub stat: FileStat,
}

/// A file as its metadata tells it, no read: gone, or its length, its modification time and its inode (Unix). A file
/// whose stat did not move kept its content
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStat {
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub exists: bool,
  #[serde(default)]
  pub len: u64,
  /// Nanoseconds since the epoch
  #[serde(default)]
  pub modified: u64,
  /// 0 where the system has none
  #[serde(default)]
  pub inode: u64,
}

impl FileStat {
  /// The stat of `path` itself, a link not followed
  pub fn of(path: &Path) -> Self {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
      return Self::default();
    };
    Self {
      exists: true,
      len: metadata.len(),
      modified: metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| u64::try_from(since.as_nanos()).unwrap_or(u64::MAX)),
      inode: inode(&metadata),
    }
  }
}

#[cfg(unix)]
fn inode(metadata: &std::fs::Metadata) -> u64 {
  use std::os::unix::fs::MetadataExt;
  metadata.ino()
}

#[cfg(not(unix))]
const fn inode(_metadata: &std::fs::Metadata) -> u64 {
  0
}

impl DirtyCheck {
  /// The verdict of a `status` that found `paths` changed in the slot at `slot`, read right after it: a free slot has no
  /// process in it (a busy one is skipped), so nothing writes the paths meanwhile
  pub(crate) fn of(slot: &Path, paths: Vec<String>) -> Self {
    let proof = proof_of(slot, &paths);
    let (paths, paths_omitted) = cut_paths(paths, 0);
    Self {
      mark: SlotMark::of(slot),
      proof,
      ignores: ignore_stats(slot),
      paths,
      paths_omitted,
    }
  }

  /// Whether the slot at `slot` is dirty still, no git: it did not move, its ignore files did not change, and one dirty
  /// path has its stat of then. A path cleaned by hand is touched (a stray file deleted, an edit checked out), so a
  /// slot cleaned so gets its `status` again
  fn holds(&self, slot: &Path) -> bool {
    self.mark == SlotMark::of(slot) && proof_holds(slot, &self.proof, &self.ignores)
  }
}

/// The first `DIRTY_PROOF_PATHS` of the changed `paths` of `slot` with their stat now
fn proof_of(slot: &Path, paths: &[String]) -> Vec<ProofPath> {
  paths
    .iter()
    .take(DIRTY_PROOF_PATHS)
    .map(|path| ProofPath {
      path: path.clone(),
      stat: FileStat::of(&slot.join(path)),
    })
    .collect()
}

/// Whether `slot` has its changes still by their proof, no git: its ignore files are `ignores` and one path of `proof`
/// has its stat of then
fn proof_holds(slot: &Path, proof: &[ProofPath], ignores: &[FileStat]) -> bool {
  ignores == ignore_stats(slot).as_slice() && proof.iter().any(|proof| FileStat::of(&slot.join(&proof.path)) == proof.stat)
}

/// The stat of the index of the slot at `slot`, in its git dir
fn index_stat(slot: &Path) -> FileStat {
  linked_worktree(slot).map_or_else(FileStat::default, |worktree| FileStat::of(&worktree.git_dir.join("index")))
}

/// The stat of the top-level `.gitignore` of the slot at `slot` and of `info/exclude` of its repository (in the common
/// dir, which the worktrees share)
fn ignore_stats(slot: &Path) -> Vec<FileStat> {
  let exclude = linked_worktree(slot).map_or_else(FileStat::default, |worktree| FileStat::of(&worktree.git_common_dir.join("info").join("exclude")));
  vec![FileStat::of(&slot.join(".gitignore")), exclude]
}

/// A commit of a lease that its home branch lacks
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkCommit {
  pub id: String,
  pub subject: String,
}

/// What the files of the git dir of a slot tell of its HEAD, without git: the content of `HEAD`, the length and the
/// modification time of its reflog. A commit, a reset or a checkout in the slot changes it
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotMark {
  pub head: String,
  pub log_len: u64,
  /// Unix seconds
  pub log_modified: u64,
}

impl SlotMark {
  /// The mark of the slot at `path`; empty when it is no linked worktree
  pub fn of(path: &Path) -> Self {
    let Some(worktree) = linked_worktree(path) else {
      return Self::default();
    };
    let log = std::fs::metadata(worktree.git_dir.join("logs").join("HEAD")).ok();
    Self {
      head: std::fs::read_to_string(worktree.git_dir.join("HEAD")).unwrap_or_default().trim().to_string(),
      log_len: log.as_ref().map_or(0, std::fs::Metadata::len),
      log_modified: log.and_then(|log| log.modified().ok()).map_or(0, unix_seconds),
    }
  }
}

fn unix_seconds(time: SystemTime) -> u64 {
  time.duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

fn now() -> u64 {
  unix_seconds(SystemTime::now())
}

/// The lease of a slot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
  /// A UUID
  pub id: String,
  pub holder: String,
  /// Unix seconds
  pub leased_at: u64,
  pub state: LeaseState,
  /// The process of the get, and when it started (`process_started`); of the return while `returning`, of the clean
  /// while `cleaning`: a lease that is not `ready` and whose process is gone is free
  pub pid: u32,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub process_started: Option<u64>,
  /// The commit of the source that the slot was moved to
  pub source_head: String,
  /// The source checkout of the get, its top level
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub source: String,
  /// The branch checked out in the source at the get; `None` for a detached HEAD. The return asks whether this branch
  /// has every change of the slot (`landed_work`)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub source_branch: Option<String>,
  /// The branch the get created at `source_head`
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub branch: Option<String>,
  /// The paths the get copied from the source (`.worktreeinclude`), relative, `/`-separated: the return removes them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub seeded: Vec<String>,
  /// Whether the get moved a free slot rather than adding a new one
  #[serde(default)]
  pub reused: bool,
  /// The process the get ran for: the nearest ancestor that is no shell (`ProcessScan::session`, the coding agent or the
  /// terminal), and when it started, on Windows too (from a snapshot of the process list). `pid` is of the get itself,
  /// which exits at once; `None` when the system named no ancestors, or of a brd that recorded none on Windows
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub holder_pid: Option<u32>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub holder_started: Option<u64>,
  /// The idle lease this `preparing` lease of a get takes over, while the get checks its work: a failed check, and a
  /// get gone meanwhile, put it back (`settle_abandoned`)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub taking_over: Option<Box<Self>>,
  /// The last check of the idle lease that found work
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub idle_check: Option<IdleCheck>,
  /// When a reclaim last failed after its check in full (unix seconds): the check failed, the lease changed, a Bazel
  /// server did not exit, or the return failed. The gets pass over the lease for `RECLAIM_RETRY` and then check it after
  /// the idle leases checked before that time (`Lease::idle_checked_at`)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reclaim_failed_at: Option<u64>,
  /// When a clean last pruned the stale build output of the slot by the cargo rules of the lease (`prune_leased_slot`),
  /// unix seconds: the shrink passes over the slot within the smallest `age` of those rules
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub pruned_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseState {
  /// The get recorded the lease and moves the slot
  Preparing,
  /// The slot is at `source_head`, seeded, the holder's
  Ready,
  /// A free slot that a get clones into a new slot: no get takes it until the clone ends
  Templating,
  /// The return of the lease runs (`lease_return_core`): no get takes the slot and no other return acts on it
  Returning,
  /// A free slot that `brd lease clean` cleans: no get takes it until the clean ends
  Cleaning,
}

impl Lease {
  /// A lease of this process that only keeps the gets off a slot while a command acts on it (`<kind>-<uuid>`): the
  /// `cleaning` marker of a clean, the `preparing` entry of the path an adopt moves a worktree to
  pub(crate) fn marker(kind: &str, holder: &str, state: LeaseState) -> Self {
    let pid = std::process::id();
    Self {
      id: format!("{kind}-{}", uuid::Uuid::new_v4()),
      holder: holder.to_string(),
      leased_at: now(),
      state,
      pid,
      process_started: process_started(pid),
      source_head: String::new(),
      source: String::new(),
      source_branch: None,
      branch: None,
      seeded: Vec::new(),
      reused: false,
      holder_pid: None,
      holder_started: None,
      taking_over: None,
      idle_check: None,
      reclaim_failed_at: None,
      pruned_at: None,
    }
  }

  /// Whether the lease is not `ready` (`preparing`, `templating`, `returning`, `cleaning`) and its process is gone: the
  /// next get takes the slot as free
  pub fn is_abandoned(&self) -> bool {
    self.state != LeaseState::Ready && !is_running(self.pid, self.process_started)
  }

  /// Whether the process the lease was got for still runs; `None` when the get recorded none. A gone holder alone does
  /// not free a lease: the slot may hold unlanded work, and a resumed session runs as another process; a get takes it
  /// over only once its work landed (`is_idle`)
  pub fn holder_alive(&self) -> Option<bool> {
    self.holder_pid.map(|pid| is_running(pid, self.holder_started))
  }

  /// Whether the lease is idle: `ready`, its holder gone, and no host slot (idle between runs by design; only
  /// `brd-host slots --local --shrink` gives one back)
  pub fn is_idle(&self) -> bool {
    self.is_agent_ready() && self.holder_alive() == Some(false)
  }

  /// Whether the lease is `ready` and of an agent, no host slot: `is_idle` once its holder is gone, and a reclaim names
  /// it when no holder was recorded (`IdleSkip::HolderUnknown`)
  pub(crate) fn is_agent_ready(&self) -> bool {
    self.state == LeaseState::Ready && !self.holder.starts_with(HOST_HOLDER_PREFIX)
  }

  /// When a reclaim last looked at the idle lease in full (unix seconds, 0 for never): its last check that found work or
  /// its last failed reclaim. The reclaim of a get checks the idle leases looked at longest ago first
  pub(crate) fn idle_checked_at(&self) -> u64 {
    self.idle_check.as_ref().map_or(0, |check| check.at).max(self.reclaim_failed_at.unwrap_or(0))
  }

  /// Whether the reclaim of a get passes over the idle lease at `now`: a reclaim of it failed less than `RECLAIM_RETRY`
  /// ago
  pub(crate) fn reclaim_waits(&self, now: u64) -> bool {
    self.reclaim_failed_at.is_some_and(|at| now.saturating_sub(at) < RECLAIM_RETRY.as_secs())
  }
}

/// How long the reclaim of a get passes over an idle lease whose reclaim failed after its check in full. Each try costs
/// a `status` (2 to 40 s in a slot of idea-1) and the wait for its Bazel servers, and the next get would try it again
/// first
pub const RECLAIM_RETRY: Duration = Duration::from_hours(1);

/// How long an idle lease must have been quiet before a get takes it over: its get and the last move of its HEAD. A
/// holder recorded as a wrapper that exits at once (`cargo run`, `pnpm`) must not lose the lease its agent just began
pub const IDLE_QUIET: Duration = Duration::from_hours(1);

/// Whether the idle lease of `slot` was quiet for `IDLE_QUIET`: since its get and since the last move of its HEAD (the
/// reflog of the slot, no git)
fn is_quiet(slot: &Slot, lease: &Lease, now: u64) -> bool {
  let quiet = IDLE_QUIET.as_secs();
  now.saturating_sub(lease.leased_at) >= quiet && now.saturating_sub(SlotMark::of(Path::new(&slot.path)).log_modified) >= quiet
}

/// A lease that is not `ready` and whose process is gone: the idle lease it was taking over comes back, else the slot
/// is free. The one rule of every reader that acts on such a lease: the reserve of a get, the mark of a clean, the
/// return, the status
pub(crate) fn settle_abandoned(slot: &mut Slot) {
  if let Some(lease) = slot.lease.as_mut().filter(|lease| lease.is_abandoned()) {
    warn!(slot = %slot.path, "freeing a lease whose process is gone");
    slot.lease = lease.taking_over.take().map(|old| *old);
  }
}

/// `settle_abandoned` of a get gone while it took an idle lease over, only: the idle lease comes back. A return and the
/// status act on another abandoned lease as before (a return whose process died is returned by its retry)
fn settle_abandoned_takeover(slot: &mut Slot) {
  if slot.lease.as_ref().is_some_and(|lease| lease.is_abandoned() && lease.taking_over.is_some()) {
    settle_abandoned(slot);
  }
}

/// A process that works in a slot, as a refusal and the outputs name it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusyProcess {
  pub pid: u32,
  /// The command name, as `ps -o comm` shows it
  pub name: String,
  /// The arguments as one line, at most 200 characters
  pub command: String,
  /// The workspace of a Bazel server: a return stops it, a get stops it in a free slot
  pub bazel_workspace: Option<String>,
  /// Why a Bazel server that still runs was not asked to exit: its `bazel shutdown` could not run (no `bazel` on the
  /// `PATH`) or failed (Windows)
  pub stop_error: Option<String>,
  /// A command of the session that holds the lease, seen by a return that this session runs (`HolderProcesses`):
  /// `--stop-own` ends it
  pub holder: bool,
}

impl BusyProcess {
  pub fn of(process: &ProcessInfo) -> Self {
    Self {
      pid: process.pid,
      name: process.name.clone(),
      command: process.command_line(COMMAND_LIMIT),
      bazel_workspace: process.bazel_workspace().map(|workspace| display_path(&workspace)),
      stop_error: None,
      holder: false,
    }
  }
}

pub(crate) fn busy_processes(processes: &[&ProcessInfo]) -> Vec<BusyProcess> {
  processes.iter().map(|process| BusyProcess::of(process)).collect()
}

/// A lease whose holder process is gone (`Lease::holder_alive`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdleLease {
  pub path: String,
  pub holder: String,
  pub lease_id: String,
  /// The last check that found work in it
  pub check: Option<IdleCheck>,
}

/// The `ready` leases of `store` whose holder process is gone
fn idle_leases(store: &LeaseStore) -> Vec<IdleLease> {
  store
    .slots
    .iter()
    .filter_map(|slot| {
      let lease = slot.lease.as_ref()?;
      // a host slot is idle between runs by design: `brd-host slots --local --shrink` gives it back, never a hand return
      lease.is_idle().then(|| IdleLease {
        path: slot.path.clone(),
        holder: lease.holder.clone(),
        lease_id: lease.id.clone(),
        check: lease.idle_check.clone(),
      })
    })
    .collect()
}

/// What the pool refuses; `brd` gives each its error code
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LeaseRefusal {
  #[error("{0} holds no lease of this repository's pool")]
  NoLease(String),
  #[error("The slot {path} is being prepared or cloned by the process {pid}")]
  Preparing { path: String, pid: u32 },
  #[error("The lease of {path} is being returned by the process {pid}")]
  Returning { path: String, pid: u32 },
  #[error("The lease of {path} is {actual}, not {expected}")]
  LeaseMismatch { path: String, expected: String, actual: String },
  #[error(
    "Processes work in {path}: {}; wait until they are done{}{}",
    describe_processes(.processes),
    holders_note(.processes),
    servers_note(.processes)
  )]
  SlotBusy { path: String, processes: Vec<BusyProcess> },
  #[error("Bazel servers of {path} did not exit on {SERVER_STOP}: {}; wait, or stop them with `bazel shutdown` in their workspace", describe_processes(.processes))]
  ServersRunning { path: String, processes: Vec<BusyProcess> },
  #[error(
    "The volume of the pool {pool} has {} free, less than the {} a lease needs (`branchdeck.leaseMinFree`, `--min-free`); {free_slots} free slot(s){}{}{}{}{}{}{}. Tell the user (`brd lease clean --free`, `brd lease reclaim --idle`, a lower `branchdeck.leaseMaxSlots` and `brd lease trim` free space); never pass `--min-free 0` on your own",
    describe_size(*.available),
    describe_size(*.required),
    describe_reclaimed_idle(.freed.reclaim),
    describe_trimmed(.freed.trim),
    describe_past_the_volume(*.total, *.required),
    describe_shrunk(.freed.shrink),
    describe_evicted(.freed.evict),
    describe_idle(.idle),
    describe_trash(*.trash)
  )]
  LowDiskSpace {
    pool: String,
    available: u64,
    required: u64,
    /// The bytes the volume holds in all: a minimum past it deletes no build output and no slot below the limit
    total: u64,
    free_slots: usize,
    idle: Vec<IdleLease>,
    /// The trash a process still running keeps
    trash: u64,
    /// What the reclaim, the trim, the shrink and the eviction of the get freed first
    freed: Box<FreedBy>,
  },
  #[error("{key} {value:?}: {reason}")]
  InvalidSize { key: String, value: String, reason: String },
  #[error(
    "HEAD of {path} has {} commit(s) that {}; land them first (`brd -C {path} lease pick`); --confirm-preserved gives them up, only when the user says they are kept elsewhere",
    .commits.len(),
    .check.describe()
  )]
  UnlandedCommits { path: String, commits: Vec<UnlandedCommit>, check: LandedCheck },
  #[error("{path} has local changes; commit or land them; --confirm-preserved discards them, only when the user says they are kept elsewhere")]
  DirtyWorktree { path: String, paths: Vec<String> },
  #[error(
    "{path} has a resolve session open ({session}); finish it with `brd -C {path} update --continue` and land the commits with `brd -C {path} lease pick`, or close it with `brd -C {path} update --abort`"
  )]
  ResolveInProgress { path: String, session: String },
  #[error("{path} has local changes; a slot of the pool must be clean")]
  DirtyCandidate { path: String, paths: Vec<String> },
  #[error("The branch {0} exists")]
  BranchExists(String),
  #[error("{0:?} is not a valid branch name")]
  InvalidBranch(String),
  #[error("{path} is not a slot: {reason}")]
  NotASlot { path: String, reason: String },
  #[error("The pool holds {0} already")]
  InPool(String),
  #[error("The repository at {0} has no commits yet")]
  NoCommits(String),
  #[error("{path} is on another volume than the pool {pool}: a move would copy every file")]
  OtherVolume { path: String, pool: String },
  #[error("Cannot move {path} into the pool: {reason}")]
  CannotMove { path: String, reason: String },
  #[error(
    "{path} is host slot {n}, which the agent of `brd-host` on this host lends to the runs of other machines; `brd-host slots --local --shrink <count>` on this host gives the host slots past the count back to the pool"
  )]
  HostSlot { path: String, n: u32 },
  #[error("The holder {0:?} is reserved: the host slots of `brd-host` are `{HOST_HOLDER_PREFIX}<n>`")]
  ReservedHolder(String),
  #[error("{}", describe_reclaimed(.path, .reclaimed, *.in_progress))]
  LeaseReclaimed {
    path: String,
    reclaimed: Box<Reclaimed>,
    /// The get still checks the work of the lease, which may come back
    in_progress: bool,
  },
}

fn describe_reclaimed(path: &str, reclaimed: &Reclaimed, in_progress: bool) -> String {
  let Reclaimed {
    lease_id,
    holder,
    branch,
    source_branch,
    by_lease_id,
    by_holder,
    preserved,
    ..
  } = reclaimed;
  let home = source_branch.as_deref().unwrap_or("its home branch");
  let by = match (by_lease_id, by_holder) {
    (Some(id), Some(by)) => format!("the lease {id} of {by}"),
    _ => "`brd lease reclaim`, which left it free".to_string(),
  };
  let branch = branch.as_ref().map_or_else(String::new, |branch| format!("; its branch {branch} stays"));
  if let Some(preserved) = preserved {
    let staged = if preserved.staged { " and staged.patch, the index" } else { "" };
    return format!(
      "The lease {lease_id} of {holder} in {path} was taken over by {by}: its holder process was gone, every commit of it was on {home}, and its {} local change(s) were quiet, so they are preserved in {}: changes.patch on {}{staged}{branch}. This worktree is not yours; get a lease of your own and apply the patch there",
      preserved.paths,
      preserved.dir,
      short_id(&preserved.base)
    );
  }
  if in_progress {
    format!(
      "The lease {lease_id} of {holder} in {path} is being taken over by {by}, which checks whether its work landed on {home}: this worktree is not yours now{branch}. Wait for `brd lease status`, then get a lease of your own"
    )
  } else {
    format!(
      "The lease {lease_id} of {holder} in {path} was taken over by {by}: its holder process was gone and every commit of it was on {home}{branch}. This worktree is not yours; get a lease of your own and work there"
    )
  }
}

fn describe_processes(processes: &[BusyProcess]) -> String {
  processes
    .iter()
    .map(|process| match &process.bazel_workspace {
      Some(workspace) => match &process.stop_error {
        Some(error) => format!("{} ({}, the Bazel server of {workspace}: {error})", process.name, process.pid),
        None => format!("{} ({}, the Bazel server of {workspace})", process.name, process.pid),
      },
      None if process.holder => format!("{} ({}, a command of your own session: {})", process.name, process.pid, process.command),
      None => format!("{} ({}: {})", process.name, process.pid, process.command),
    })
    .collect::<Vec<_>>()
    .join(", ")
}

/// What a return can do about the commands of the session that holds the lease, among the processes of a busy slot
fn holders_note(processes: &[BusyProcess]) -> &'static str {
  let holders = processes.iter().any(|process| process.holder);
  let others = processes.iter().any(|process| !process.holder && process.bazel_workspace.is_none());
  match (holders, others) {
    (true, false) => "; once you no longer need the output of the commands of your own session, `brd lease return --stop-own` ends them",
    (true, true) => "; `--stop-own` ends only the commands of your own session, never those of another session",
    (false, _) => "",
  }
}

/// What a return does about the Bazel servers among `processes`: each serves only its slot, so the return stops it
fn servers_note(processes: &[BusyProcess]) -> &'static str {
  if processes.iter().any(|process| process.bazel_workspace.is_some()) {
    "; the return stops the Bazel servers of the slot itself once no other process is there"
  } else {
    ""
  }
}

fn describe_idle(idle: &[IdleLease]) -> String {
  if idle.is_empty() {
    return String::new();
  }
  let leases: Vec<String> = idle
    .iter()
    .map(|lease| match &lease.check {
      Some(check) => format!("{} ({}, with {})", lease.path, lease.holder, check.describe()),
      None => format!("{} ({})", lease.path, lease.holder),
    })
    .collect();
  format!("; leases whose holder process is gone: {}", leases.join(", "))
}

/// What a get below the minimum free space freed before it refused (`LowDiskSpace`), in every pool of the machine
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FreedBy {
  /// The idle leases whose work landed that it returned first, and what their clean rules freed (`reclaim_for_get`)
  pub reclaim: Freed,
  /// The output bases of gone workspaces it deleted first (`trim` without its slots); no slot
  pub trim: Freed,
  /// The slots whose build output it deleted after the trim, the leased and the free ones that freed bytes (`shrink`,
  /// `Shrunk::freed_slots`)
  pub shrink: Freed,
  /// The free slots it deleted after the shrink (`trim::evict_core`)
  pub evict: Freed,
}

/// How many slots a step of a get below the minimum free space took and the bytes on disk it freed
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Freed {
  pub slots: usize,
  pub bytes: u64,
}

fn describe_reclaimed_idle(Freed { slots, bytes }: Freed) -> String {
  if slots == 0 {
    String::new()
  } else {
    format!(
      "; the get returned {slots} idle lease(s) whose work landed, of the pools of this machine, and their clean rules freed {}",
      describe_size(bytes)
    )
  }
}

fn describe_trimmed(Freed { bytes, .. }: Freed) -> String {
  if bytes == 0 {
    String::new()
  } else {
    format!("; the get deleted the Bazel output bases of gone workspaces first, {}", describe_size(bytes))
  }
}

fn describe_past_the_volume(total: u64, required: u64) -> String {
  if required <= total {
    String::new()
  } else {
    format!(
      "; the volume holds {} in all, less than the minimum, so the get deleted no build output and no free slot for it",
      describe_size(total)
    )
  }
}

fn describe_shrunk(Freed { slots, bytes }: Freed) -> String {
  if slots == 0 {
    String::new()
  } else {
    format!(
      "; the get cleaned the build output of {slots} slot(s) of the pools of this machine, the stale output of the leased ones by their cargo rules and the free ones by their clean rules with keep 0, {}",
      describe_size(bytes)
    )
  }
}

fn describe_evicted(Freed { slots, bytes }: Freed) -> String {
  if slots == 0 {
    String::new()
  } else {
    format!(
      "; then it deleted {slots} free slot(s), returned longest ago first, with their Bazel output bases, {}",
      describe_size(bytes)
    )
  }
}

fn describe_trash(trash: u64) -> String {
  if trash == 0 {
    String::new()
  } else {
    format!("; {} in the trash of a run that still runs", describe_size(trash))
  }
}

/// The pool state of one repository: its file and its lock in `$GIT_COMMON_DIR/branch-deck/`
pub struct Pool {
  state: StateFile,
  git_common_dir: PathBuf,
}

impl Pool {
  /// The pool of the repository whose `$GIT_COMMON_DIR` is `git_common_dir`
  pub fn new(git_common_dir: &Path) -> Self {
    Self {
      state: StateFile::new(git_common_dir, POOL_FILE, LOCK_FILE, "the lease state"),
      git_common_dir: git_common_dir.to_path_buf(),
    }
  }

  /// The state, without the lock: the file is replaced by a rename, so a read sees one whole state. A missing file is an
  /// empty pool
  pub fn read(&self) -> Result<LeaseStore> {
    self.state.read()
  }

  /// The directory of its state and its locks, `$GIT_COMMON_DIR/branch-deck`
  pub fn state_dir(&self) -> &Path {
    self.state.dir()
  }

  /// The trash of the returns and the cleans (`lease_clean::Trash`)
  pub fn trash_dir(&self) -> PathBuf {
    self.state.dir().join(TRASH_DIR)
  }

  /// Every place the notes of the leases of the pool may be in (`lease_notes::NotesRoots`), by the slots of its state
  /// (none where it can't be read), `notes` of `pool_dir` (the pool directory of the config) and the pool directories of
  /// `home`, where each is known
  pub fn notes_roots(&self, pool_dir: Option<&Path>, home: Option<&Path>) -> NotesRoots {
    NotesRoots::of(&self.git_common_dir, &self.read().unwrap_or_default(), pool_dir, home)
  }

  /// The bytes on disk of what the trash holds: what a run killed before its delete left behind
  pub fn trash_bytes(&self) -> u64 {
    std::fs::read_dir(self.trash_dir()).map_or(0, |entries| entries.flatten().map(|entry| fs_walk::tree_size(&entry.path())).sum())
  }

  /// Run `change` on the state under the pool lock and write the state it leaves; the lock is released at once
  pub fn update<T>(&self, timeout: Duration, change: impl FnOnce(&mut LeaseStore) -> Result<T>) -> Result<T> {
    self.state.update(timeout, change)
  }

  /// Run the `worktree` command `command` under the lock of the pool's `worktree` commands (`worktrees.lock`): git
  /// writes the admin directory of a new worktree in several files, and every `worktree` command reads the admin
  /// directories of all worktrees, so one beside a registration read a file the registration had created and not yet
  /// written (an empty `commondir`) and failed. The registration of a new slot, its unlock and the move of an adopt are
  /// a moment each, none a checkout. Not the pool lock, which a git process under load would hold for seconds against
  /// every get and return; the move of an adopt holds no pool lock across it either, and nothing takes the pool lock
  /// inside this one
  pub fn worktree_command<T>(&self, timeout: Duration, command: impl FnOnce() -> Result<T>) -> Result<T> {
    let _lock = self.state.lock_at(WORKTREES_LOCK_FILE, timeout, "waiting for the lock of the pool's worktree commands")?;
    command()
  }

  /// The lock of the gets that add a slot (`clones.lock`), which a get holds from the reserve of its new slot until the
  /// slot is checked out and clean, or given back, not across the seed, and across the checks of each slot that its
  /// reserve under the lock gives it (`end_checks_under_lock`); only the first waiting get of the queue (`get_queue`) waits for it: on 2026-10-24 fourteen gets cloned the idea-1 checkout at once, several ran past the
  /// deadline of their caller, and the disk stalled for 1.5 h; one after the other, at 40 s each, they would have
  /// finished. Beside the state, as the pool lock: the reserve under it reads that state, and the pool directory is
  /// only config. The wait lasts while the record of the holder names a live process (`lock_file_while_held`): a holder
  /// killed meanwhile frees the lock with its process, and a lock no record names a live holder of refuses after
  /// `unknown_timeout` (`RepositoryLocked`). `on_holder` hears the holder when it changes and about once a second, and
  /// `canceled`, asked every 50 ms, ends the wait without the lock (`LockWaitCanceled`): a get whose wait found a slot
  /// freed meanwhile reserves that one
  pub fn lock_clones(&self, unknown_timeout: Duration, on_holder: &mut dyn FnMut(&LockHolder), canceled: &dyn Fn() -> bool) -> Result<RepositoryLock> {
    let dir = self.state.dir();
    std::fs::create_dir_all(dir).with_context(|| format!("Cannot create {}", dir.display()))?;
    lock_file_while_held(dir.join(CLONES_LOCK_FILE), unknown_timeout, on_holder, canceled)
  }
}

/// `path` canonical, as the state names a slot (`display_path`: no `\\?\` on Windows); `path` itself when it is gone
pub fn canonical_path(path: &Path) -> String {
  std::fs::canonicalize(path).map_or_else(|_| display_path(path), |path| display_path(&path))
}

/// The name of the repository: the directory of the main worktree (`idea-1` of `idea-1/.git`), or of a bare one without
/// `.git`. A slot's leaf directory keeps it, since tools derive the identity of a checkout from it
pub fn repository_name(git_common_dir: &Path) -> String {
  let leaf = git_common_dir.file_name().map_or_default(|name| name.to_string_lossy().into_owned());
  if leaf == ".git" {
    return git_common_dir
      .parent()
      .and_then(Path::file_name)
      .map_or_else(|| "repository".to_string(), |name| name.to_string_lossy().into_owned());
  }
  leaf.strip_suffix(".git").unwrap_or(&leaf).to_string()
}

/// The directory of the new slots: `branchdeck.leasePool` of `config` (a relative one under the repository), else
/// `~/.branch-deck/pool/<repository name>-<8 hex digits of the SHA-256 of the canonical common dir>`
pub fn pool_dir(config: &LeaseConfig, repository: &str, git_common_dir: &Path, home: Option<&Path>) -> Result<PathBuf> {
  if let Some(configured) = &config.pool {
    return Ok(Path::new(repository).join(configured));
  }
  let home = home.ok_or_else(|| anyhow!("No home directory for the lease pool; set `git config branchdeck.leasePool <dir>`"))?;
  let digest = Sha256::digest(canonical_path(git_common_dir).as_bytes());
  let hash = digest.iter().take(4).fold(String::new(), |mut hash, byte| {
    let _ = write!(hash, "{byte:02x}");
    hash
  });
  Ok(home.join(".branch-deck").join("pool").join(format!("{}-{hash}", repository_name(git_common_dir))))
}

/// A slot a get passed over
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedSlot {
  pub name: String,
  pub path: String,
  pub reason: SkipReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
  /// Local changes: the slot stays free and marked dirty, and the next gets try it last
  /// `paths` the first `PATH_LIMIT` of them, `omitted` the count of the rest
  Dirty { paths: Vec<String>, omitted: usize },
  /// Processes have their working directory in it, or a Bazel server of it did not exit
  Busy { processes: Vec<BusyProcess> },
  /// An idle lease, its holder gone, with work its home branch lacks: the get leaves it to the user
  IdleWithWork { holder: String, lease_id: String, check: Box<IdleCheck> },
}

pub struct LeaseGetParams<'a> {
  /// The checkout whose HEAD the slot is moved to; a leased slot itself may be the source
  pub source: &'a RepositoryDirs,
  /// The home directory of the default pool directory
  pub home: Option<&'a Path>,
  /// Default: `agent-<uuid>`
  pub holder: Option<String>,
  /// A new branch at the source HEAD, checked out in the slot
  pub branch: Option<String>,
  /// The free space the volume of the pool must have, over `branchdeck.leaseMinFree` (`--min-free`; 0 checks nothing)
  pub min_free: Option<u64>,
  /// How long the get waits for the Bazel servers of a free slot it asked to exit
  pub server_timeout: Duration,
  /// The program of `bazel shutdown`, which stops the servers on Windows; `None`: `bazel` of the `PATH`
  pub bazel: Option<&'a OsStr>,
  /// The wait for the pool lock and for the lock of the `worktree` commands, and for a clones lock no record names a
  /// live holder of
  pub lock_timeout: Duration,
  /// Hears each step as it starts
  pub progress: Option<&'a dyn Fn(GetStep)>,
  /// The space of the volume of a path; `None`: `free_space::volume_space`. A test tells a volume of its own
  pub space: Option<&'a SpaceOf<'a>>,
  /// The bytes of the build dirs of the slots of a pool as `disk.json` last measured them, for the order of the leased
  /// slots in a shrink (`ShrinkParams::cached_build`); `None`: by `leased_at` alone
  pub cached_build: Option<&'a CachedBuild<'a>>,
  /// The heartbeat of the get while it waits in the queue (`get_queue`), and the age of a stale one
  pub queue: QueueTiming,
}

impl LeaseGetParams<'_> {
  /// The space of the volume of `path` (`space`)
  fn space(&self, path: &Path) -> Option<VolumeSpace> {
    self.space.map_or_else(|| volume_space(path), |space| space(path))
  }

  /// The free bytes of the volume of `path` (`space`)
  fn available(&self, path: &Path) -> Option<u64> {
    self.space(path).map(|space| space.available)
  }
}

/// A step of a get, told as it starts (`LeaseGetParams::progress`): in a large repository a get takes minutes, and
/// whoever waits for it sees where it is
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetStep {
  /// Under the pool lock, a free slot; `adding`: under the clones lock, a slot freed meanwhile, else a new one
  Reserve { adding: bool },
  /// No free slot, and another get holds the clones lock while it adds a slot: told when the wait starts, when the
  /// holder or the slot it adds changes, and once a minute
  WaitForClone(CloneHolder),
  /// No free slot, and gets that asked before this one wait in the queue of the pool (`get_queue`): told when the wait
  /// starts, when the get right ahead changes, and once a minute
  WaitBehind(Behind),
  /// Under the clones lock, no free slot, and the return or the clean of the slot `slot` by the process `pid` runs: the
  /// get waits for it to end rather than add a slot (`clean`: a clean, else a return)
  WaitForSlot { slot: String, pid: u32, clean: bool },
  /// Below the minimum free space: the trash of the runs that are gone is deleted, without the clones lock
  DeleteTrash,
  /// Below the minimum free space, after the trash, while a pool of the machine holds an idle lease on the volume of
  /// the get: the idle leases whose work landed are returned (`lease_reclaim_core`), without the clones lock
  Reclaim,
  /// Below the minimum free space, after the reclaim: the Bazel output bases of the gone workspaces of every pool of the
  /// machine are deleted and their expired notes (`trim` without its slots), without the clones lock; no slot goes for
  /// `branchdeck.leaseMaxSlots`, a warm slot being worth more than its bytes
  Trim,
  /// Below the minimum free space, after the trim: the stale build output of the leased slots of every pool of the
  /// machine is pruned by the cargo rules of their leases, the largest first, then the build output of the free slots is
  /// deleted by their clean rules with keep 0, the ones returned longest ago first (`shrink`), without the clones lock
  Shrink,
  /// Below the minimum free space, after the shrink: free slots of every pool of the machine are deleted with their
  /// Bazel output bases, the ones returned longest ago first, one at a time (`trim::evict_core`), without the clones
  /// lock
  Evict,
  /// The shrink or the eviction waits for the one of another get of the machine (`shrink::lock_pressure`), told when
  /// its holder changes and once a minute, with how long that one holds it and the free space of the volume now, so a
  /// wait of minutes reads as the deletes of disk pressure, never as a hang; after its turn, or once the volume has the
  /// minimum, the get measures again
  WaitForPressure {
    holder: LockHolder,
    /// The free bytes of the volume now, `None` where the system does not tell them
    free: Option<u64>,
    /// The free bytes the get needs: `branchdeck.leaseMinFree`
    minimum: u64,
  },
  /// Under the clones lock: the number directories that gets killed during their clone left, `<n>/<repository>.cloning`
  /// in no state of the machine, are moved into the trash (`sweep_clone_leftovers`)
  SweepCloneLeftovers { dirs: Vec<String> },
  /// Under the clones lock: the slots whose add never finished (`is_unfinished`) are removed, their trees moved into the
  /// trash (`trim::remove_unfinished_slots`)
  RemoveUnfinished { slots: Vec<String> },
  /// The Bazel servers of the reserved free slot are asked to exit
  StopServers { slot: String, servers: usize },
  /// The `status` of the slot: of a reserved free slot, or of the clean of a clone after its checkout
  /// (`clean_cloned_slot`), which looks for the untracked files and the local changes at once
  Status { slot: String },
  /// The APFS clone of `template` into the new slot, and its registration
  Clone { template: String, slot: String },
  /// The refresh of the index copied from the template
  Refresh { slot: String },
  /// The move to the source HEAD: by the diff for a reused slot (`move_clean_slot`), every file of a clone or of a
  /// registered new slot (`checkout_slot`)
  Checkout { slot: String, commit: String },
  /// git refused the move of a reused slot by the diff, for `reason` (the first line of its words): every file is
  /// checked out instead
  CheckoutEveryFile { slot: String, reason: String },
  /// The download of the files of Git LFS of a new slot and their content in place of the pointers that its checkout
  /// wrote, after the release of the clones lock (`git_ops::lfs::pull_lfs_files`)
  LfsPull { slot: String },
  /// The copy of the paths that `.worktreeinclude` names
  Seed { slot: String, paths: usize },
  /// The check of the work of an idle lease, whose holder is gone: landed in its home branch, a clean tree
  TakeOver { slot: String, holder: String },
  /// The return of the idle lease the get took over: its seeded paths, its branch detached, the clean rules
  CleanTakenOver { slot: String },
  /// The local changes of the idle lease are its only work and quiet: they become patches in its notes, without the
  /// clones lock, before the get takes the slot (`idle_changes`)
  PreserveChanges { slot: String, holder: String, paths: usize },
}

impl fmt::Display for GetStep {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Reserve { adding: false } => f.write_str("reserving a free slot"),
      Self::Reserve { adding: true } => f.write_str("reserving a slot freed meanwhile, else a new one"),
      Self::WaitForClone(holder) => write!(f, "no free slot; {holder}"),
      Self::WaitBehind(behind) => write!(f, "no free slot; {behind}"),
      Self::WaitForSlot { slot, pid, clean } => write!(
        f,
        "no free slot; waiting for the {} of {slot} (pid {pid}): the slot it frees is leased rather than a new one",
        if *clean { "clean" } else { "return" }
      ),
      Self::DeleteTrash => f.write_str("deleting the trash of runs that are gone: the free space is below the minimum"),
      Self::Reclaim => f.write_str("returning the idle leases whose work landed, in every pool of this machine: the free space is below the minimum"),
      Self::Trim => {
        f.write_str("deleting the Bazel output bases of gone workspaces and the expired notes of the pools of this machine: the free space is below the minimum")
      }
      Self::Shrink => f.write_str(
        "pruning the stale build output of the leased slots of the pools of this machine by their cargo rules, then deleting the build output of the free slots, returned longest ago first, by their clean rules with keep 0: the free space is below the minimum",
      ),
      Self::Evict => f.write_str(
        "deleting free slots of the pools of this machine, returned longest ago first, with their Bazel output bases: the free space is below the minimum after the shrink",
      ),
      Self::SweepCloneLeftovers { dirs } => write!(
        f,
        "moving what {} killed clone(s) left into the trash: {}",
        dirs.len(),
        dirs.join(", ")
      ),
      Self::RemoveUnfinished { slots } => write!(
        f,
        "removing {} slot(s) whose add never finished, their trees moved into the trash: {}",
        slots.len(),
        slots.join(", ")
      ),
      Self::WaitForPressure { holder, free, minimum } => {
        let free = free.map_or_else(|| "an unknown free space".to_string(), describe_size);
        write!(
          f,
          "waiting for the shrink or the eviction of another get, {holder}, the volume at {free} of the {} this get needs: the free space is measured again after it, or once it has the minimum",
          describe_size(*minimum)
        )
      }
      Self::StopServers { slot, servers } => write!(f, "stopping {servers} Bazel server(s) of {slot}"),
      Self::Status { slot } => write!(f, "looking for local changes in {slot}"),
      Self::Clone { template, slot } => write!(f, "cloning {template} into {slot}"),
      Self::Refresh { slot } => write!(f, "refreshing the index of {slot}"),
      Self::Checkout { slot, commit } => write!(f, "checking out {} in {slot}", short_id(commit)),
      Self::CheckoutEveryFile { slot, reason } => write!(f, "checking out every file of {slot}, because git refused the diff: {reason}"),
      Self::LfsPull { slot } => write!(f, "downloading the files of Git LFS into {slot}"),
      Self::Seed { slot, paths } => write!(f, "copying {paths} path(s) that {WORKTREE_INCLUDE} names into {slot}"),
      Self::TakeOver { slot, holder } => write!(f, "checking whether the work of the idle lease of {holder} in {slot} landed"),
      Self::CleanTakenOver { slot } => write!(f, "returning the idle lease of {slot}: its seeded paths, its branch and the clean rules"),
      Self::PreserveChanges { slot, holder, paths } => write!(
        f,
        "preserving the {paths} quiet local change(s) of the idle lease of {holder} in {slot} as patches in its notes"
      ),
    }
  }
}

/// Who holds the clones lock, as a get that waits for it tells
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloneHolder {
  /// The process, as the record of the lock names it: its command, its pid and since when it holds the lock
  pub process: LockHolder,
  /// The new slot it adds, once it reserved one
  pub adding: Option<AddingSlot>,
}

/// The new slot that the holder of the clones lock adds
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddingSlot {
  pub slot: String,
  /// The holder of its lease
  pub holder: String,
  /// The free slot it clones, while the clone of the directory runs
  pub template: Option<String>,
}

impl fmt::Display for CloneHolder {
  /// "waiting for `brd lease get --holder lane-3` (pid 4242, for 35 s), which adds /pool/8/idea-1 for lane-3: …"
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "waiting for {}", self.process)?;
    match &self.adding {
      Some(adding) => {
        write!(f, ", which adds {} for {}", adding.slot, adding.holder)?;
        if let Some(template) = &adding.template {
          write!(f, " (a clone of {template})")?;
        }
      }
      None => f.write_str(", which adds a slot")?,
    }
    f.write_str(": the pool adds one slot at a time, and a slot freed meanwhile is leased instead")
  }
}

/// How a get added a new slot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreatedBy {
  /// An APFS clone of a template (`clone_dir`), then the checkout of the diff
  Clone,
  /// `git worktree add --no-checkout`, then a full checkout
  Checkout,
}

/// What a get leased
#[derive(Debug, Clone)]
pub struct LeaseGot {
  /// The slot with its `ready` lease
  pub slot: Slot,
  /// The notes of the lease, which `notes.local` of the slot links to (`lease_notes`); `None` for a host slot, and where
  /// they could not be made
  pub notes: Option<LeaseNotes>,
  /// How a new slot was added; `None` for a reused one
  pub created_by: Option<CreatedBy>,
  /// The free slot or the source checkout a new slot was cloned from
  pub cloned_from: Option<String>,
  pub pool_dir: PathBuf,
  pub skipped: Vec<SkippedSlot>,
  /// The Bazel servers of the free slot that exited on the SIGTERM of the get
  pub stopped_servers: Vec<StoppedServer>,
  /// The free space of the volume of the pool before the get, measured again under the clones lock for a new slot;
  /// `None` where the system does not tell
  pub free_bytes: Option<u64>,
  /// With a new slot: the leases whose holder process is gone, which keep their slots from the pool
  pub idle: Vec<IdleLease>,
  /// The idle lease the get took over, its work landed
  pub took_over: Option<TookOver>,
  /// Whether the processes were checked (`ProcessScan::Unavailable` on Windows)
  pub process_scan: bool,
  /// How long the get waited for the clones lock, which other gets held while they added slots; `None` when it waited
  /// for none
  pub waited_for_clone: Option<Duration>,
  /// Under the clones lock: the slots whose add never finished, which the get removed (`trim::remove_unfinished_slots`)
  pub removed_unfinished: Vec<RemovedSlot>,
  /// Below the minimum free space: the idle leases of the pools of the machine whose work landed, which the get
  /// returned before the trim (`reclaim_for_get`)
  pub reclaimed: Vec<ReclaimedLease>,
  /// Below the minimum free space: the output bases of gone workspaces that the trim of the pools of the machine deleted,
  /// no slot (`trim_for_get`)
  pub trimmed: Option<Trimmed>,
  /// Below the minimum free space after the trim: the free slots of the pools of the machine whose build output the
  /// get deleted (`shrink::shrink_core`)
  pub shrunk: Option<Shrunk>,
  /// Below the minimum free space after the shrink: the free slots that the get deleted (`trim::evict_core`)
  pub evicted: Option<Trimmed>,
  /// `branchdeck.agentPullRequest` of the source, from the get's one read of the pool config: the `next` of the get
  pub agent_landing: AgentLanding,
}

/// The slot a get reserved under the pool lock
enum Reserved {
  /// With the Bazel servers of the slot, the only processes in it: the get stops them before the checkout
  Free { path: String, name: String, servers: Vec<ProcessInfo> },
  /// An idle lease, which the get takes over once its work landed (`check_idle`); `old` is that lease, which the
  /// `preparing` one of the get keeps as `taking_over` meanwhile, and `servers` the Bazel servers of the slot
  TakeOver {
    path: String,
    name: String,
    servers: Vec<ProcessInfo>,
    old: Box<Lease>,
  },
  /// A new slot at `path`; `template` is a free slot marked `templating` for its clone
  New { path: PathBuf, template: Option<String> },
  /// Nothing reserved: the slot `path` is free in a moment, once the return or the clean of `lease_id` by the process
  /// `pid` ends. The get waits for it under the clones lock rather than add a slot
  Freeing { path: String, lease_id: String, pid: u32, clean: bool },
  /// No free slot, and the get does not hold the clones lock yet: nothing is reserved
  NoFreeSlot,
}

impl Reserved {
  /// What the reserve makes of the ticket of the get in the queue
  const fn turn(&self) -> Turn {
    match self {
      Self::NoFreeSlot | Self::Freeing { .. } => Turn::Wait,
      Self::New { .. } => Turn::Final,
      Self::Free { .. } | Self::TakeOver { .. } => Turn::Busy,
    }
  }
}

/// Lease a slot at the HEAD of the source. First the free space of the volume of the pool, when a minimum is set
/// (`LowDiskSpace`). Under the pool lock: a free slot of this repository (a linked worktree of the same common dir, no
/// process in it but its Bazel servers, clean slots before dirty ones), its lease recorded as `preparing`. Then,
/// without the lock: its Bazel servers stopped (one that does not exit skips the slot as busy), one `status` (a dirty
/// slot is marked, skipped, and the next one tried), `checkout --force` to the source HEAD (only the diff is written),
/// the seed. No free slot: the get waits at its place in the queue of the pool (`get_queue`), the first of it for the
/// clones lock (`Pool::lock_clones`) while another get adds a slot; a slot freed or an idle lease that turned quiet
/// during the wait ends the wait of the first waiting get that the queue gives it (`claimable`), which reserves that
/// one without the lock. After a wait the minimum and the processes
/// as they are now; the free space again (a trash to delete releases the lock for the delete), and the reserve again,
/// where a slot freed meanwhile wins, and a slot whose return or clean runs is waited for. Else, under the lock until
/// the slot is ready, a clone of a template without what the `delete` rules delete (`add_cloned_slot`), else a
/// registration and the only full checkout. A failure after the lease was recorded gives the slot back
///
/// The holders `brd-host/<n>` are the host slots' (`host_slots`), which only `host_slots::fill` gets: another get with
/// such a holder is refused (`ReservedHolder`)
#[instrument(skip_all, fields(source = %params.source.top_level))]
pub fn lease_get_core(git_executor: &GitCommandExecutor, params: &LeaseGetParams<'_>) -> Result<LeaseGot> {
  if let Some(holder) = params.holder.as_ref().filter(|holder| holder.starts_with(HOST_HOLDER_PREFIX)) {
    bail!(LeaseRefusal::ReservedHolder(holder.clone()));
  }
  get_lease(git_executor, params, TakeOver::Idle)
}

/// The get of a host slot (`host_slots::fill`), with its holder `brd-host/<n>`: it takes over no idle lease, as the
/// trash of a takeover is named by the process of the long-lived agent, which no later return deletes while it runs
pub(crate) fn get_host_slot(git_executor: &GitCommandExecutor, params: &LeaseGetParams<'_>) -> Result<LeaseGot> {
  get_lease(git_executor, params, TakeOver::None)
}

/// Which idle leases a get takes over
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TakeOver {
  /// Those whose work landed (`check_idle`), before it adds a slot
  Idle,
  None,
}

/// How many idle leases one get checks in full (`landed_work`, a `status`): one whose verdict still holds costs one
/// `rev-parse` and counts not
const IDLE_CHECKS_PER_GET: usize = 2;

fn get_lease(git_executor: &GitCommandExecutor, params: &LeaseGetParams<'_>, take_over: TakeOver) -> Result<LeaseGot> {
  let report = |step: GetStep| {
    if let Some(progress) = params.progress {
      progress(step);
    }
  };
  let source = &params.source.top_level;
  let git_common_dir = PathBuf::from(canonical_path(Path::new(&params.source.git_common_dir)));
  let Some((source_head, source_branch)) = read_head(git_executor, source)? else {
    bail!(LeaseRefusal::NoCommits(source.clone()));
  };
  if let Some(branch) = &params.branch {
    if !is_valid_branch_name(git_executor, source, branch)? {
      bail!(LeaseRefusal::InvalidBranch(branch.clone()));
    }
    if ref_exists(git_executor, source, &format!("refs/heads/{branch}")) {
      bail!(LeaseRefusal::BranchExists(branch.clone()));
    }
  }
  let config = lease_config(git_executor, source)?;
  let pool_dir = pool_dir(&config, source, &git_common_dir, params.home)?;
  std::fs::create_dir_all(&pool_dir).with_context(|| format!("Cannot create the pool directory {}", pool_dir.display()))?;
  let pool_dir = PathBuf::from(canonical_path(&pool_dir));
  let pool = Pool::new(&git_common_dir);
  let mut min_free = required_free(params, &config)?;
  // what the get did below the minimum free space
  let mut relief = Relief::default();
  let mut free_bytes = relief.raise(git_executor, params, &pool, (&pool_dir, min_free), &report)?;
  let seeds = seed_paths(git_executor, source)?;
  // the rules of the source leave out of a cloned slot what a return would delete; a file that can't be read clones all
  // and the notes of the lease of a template slot, which the get links anew
  let mut skipped_paths = clone_skips(&read_clean_rules(Path::new(source)).unwrap_or_else(|error| {
    warn!(source = %source, error = %format!("{error:#}"), "cannot read the clean rules; a clone copies everything");
    Vec::new()
  }));
  skipped_paths.push((NOTES_LINK.to_string(), Vec::new()));
  let skips = CloneSkips::new(&skipped_paths);
  let mut scan = ProcessScan::scan();
  let holder = scan.session();
  let pid = std::process::id();
  let lease = Lease {
    id: uuid::Uuid::new_v4().to_string(),
    holder: params.holder.clone().unwrap_or_else(|| format!("agent-{}", uuid::Uuid::new_v4())),
    leased_at: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs()),
    state: LeaseState::Preparing,
    pid,
    process_started: process_started(pid),
    source_head: source_head.clone(),
    source: source.clone(),
    source_branch,
    branch: params.branch.clone(),
    seeded: Vec::new(),
    reused: false,
    holder_pid: holder.as_ref().map(|process| process.pid),
    holder_started: holder.as_ref().and_then(|process| process.started),
    taking_over: None,
    idle_check: None,
    reclaim_failed_at: None,
    pruned_at: None,
  };

  let mut skipped = Vec::new();
  let mut tried = HashSet::new();
  // the returns and the cleans the get waited for (`Reserved::Freeing`), each once
  let mut awaited = HashSet::new();
  let mut created_by = None;
  let mut cloned_from = None;
  let mut stopped_servers = Vec::new();
  let mut idle = Vec::new();
  let mut took_over = None;
  let mut idle_checks = 0;
  // held from the reserve of a new slot until the slot is ready or given back, and across the checks of each slot
  // reserved under it (dropped on return)
  let mut clones_lock: Option<RepositoryLock> = None;
  let mut waited_for_clone: Option<Duration> = None;
  let mut removed_unfinished = Vec::new();
  // the ticket of the get in the queue, from its first wait for a slot; its drop takes it out of the state
  let mut place = QueuePlace::new(&pool, params.lock_timeout, params.queue, &lease);
  loop {
    let adding = clones_lock.is_some();
    report(GetStep::Reserve { adding });
    let takes_over = take_over == TakeOver::Idle && idle_checks < IDLE_CHECKS_PER_GET;
    let reserved = pool.update(params.lock_timeout, |store| {
      settle_queue(&mut store.queue);
      let gate = place.gate(store);
      let reserved = reserve(
        store,
        &git_common_dir,
        &pool_dir,
        (&scan, &gate),
        &mut tried,
        &mut awaited,
        &mut skipped,
        &lease,
        adding,
        takes_over,
      );
      if matches!(reserved, Reserved::New { .. }) {
        idle = idle_leases(store);
      }
      // the phase of the get in the queue changes in the write of its reserve
      place.reserved(store, reserved.turn(), &tried, takes_over);
      Ok(reserved)
    })?;
    if matches!(reserved, Reserved::New { .. }) {
      place.left();
    }
    let (path, reused) = match reserved {
      Reserved::NoFreeSlot => match wait_turn(&pool, params.lock_timeout, &mut place, (&tried, takes_over.then_some(git_common_dir.as_path())), &report)? {
        CloneTurn::SlotFreed { waited } => {
          if let Some(waited) = waited {
            *waited_for_clone.get_or_insert_default() += waited;
          }
          // the slot freed meanwhile is reserved like any free slot, without the lock, with the processes as they are now
          scan = ProcessScan::scan();
          continue;
        }
        CloneTurn::Locked { lock, waited } => {
          if let Some(waited) = waited {
            *waited_for_clone.get_or_insert_default() += waited;
            // the turn came minutes later perhaps: the minimum of the config and the processes in the slots as they are
            // now; a get that took the lock at once read them a moment ago
            min_free = required_free(params, &lease_config(git_executor, source)?)?;
            scan = ProcessScan::scan();
            // and the trash, which the gets before it may have filled
            relief.swept = false;
          }
          // under the clones lock no add of the pool runs: a slot whose add never finished is no slot, and neither is
          // what a killed clone left; the slots first, so the `.cloning` beside a removed one goes in the same sweep
          let (removed, busy) = remove_unfinished_slots(git_executor, &trim_params(params, true), &scan, &|slots| report(GetStep::RemoveUnfinished { slots }));
          removed_unfinished.extend(removed);
          // a turn again after the get gave the lock up for the free space names a busy slot once
          for busy in busy {
            if !skipped.iter().any(|skipped| skipped.path == busy.path) {
              skipped.push(busy);
            }
          }
          sweep_clone_leftovers(&pool, params, (&pool_dir, &git_common_dir), &report);
          // a queue of gets never adds slots past the minimum: each measures what the ones before it left
          match free_space(params, &pool, &pool_dir, min_free, &relief)? {
            FreeSpace::Enough(available) => {
              free_bytes = available;
              clones_lock = Some(lock);
            }
            low => {
              // the delete of a trash takes minutes for a Bazel output base of tens of GB, and so do the deletes of a
              // trim and a shrink, which mark the slots of this pool under its lock: the gets behind this one go on
              // meanwhile, the queue passing this one, and it waits at its place again after
              drop(lock);
              place.busy();
              relief.relieve(git_executor, params, &pool, (&pool_dir, min_free), &low, &report);
            }
          }
          continue;
        }
      },
      Reserved::Freeing { path, lease_id, pid, clean } => {
        report(GetStep::WaitForSlot { slot: path.clone(), pid, clean });
        // the get waits at the head of the queue, its heartbeat written, so no get behind it takes the slot it waits for
        wait_while_freeing(&pool, &path, &lease_id, &mut |store| place.keep(store))?;
        continue;
      }
      Reserved::Free { path, name, servers } => {
        // a get that holds the clones lock keeps it across the check: a busy or a dirty slot leaves it the next reserve
        // under the lock, where it adds a slot without another wait
        if let Some(busy) = stop_reserved_servers(&pool, params, (&name, &path), (&lease.id, None), &servers, &mut stopped_servers) {
          skipped.push(busy);
          continue;
        }
        report(GetStep::Status { slot: path.clone() });
        let changes = slot_changes(git_executor, &path, config.untracked).map_err(|error| give_back(&pool, params.lock_timeout, &path, &lease.id, false, error))?;
        if !changes.is_empty() {
          debug!(slot = %path, changes = changes.len(), "skipping a dirty slot");
          // the slot is the get's still: nothing writes the paths before their stat is read
          let check = DirtyCheck::of(Path::new(&path), changes);
          pool.update(params.lock_timeout, |store| {
            if let Some(slot) = leased_slot(store, &path, &lease.id) {
              slot.lease = None;
              slot.dirty = true;
              slot.dirty_check = Some(check.clone());
            }
            Ok(())
          })?;
          skipped.push(SkippedSlot {
            name,
            path,
            reason: SkipReason::Dirty {
              paths: check.paths,
              omitted: check.paths_omitted,
            },
          });
          continue;
        }
        end_checks_under_lock(&mut clones_lock);
        move_leased_slot(git_executor, &pool, params, &path, &lease.id, &source_head, config.workers)?;
        (path, true)
      }
      Reserved::TakeOver { path, name, servers, old } => {
        // the clones lock stays across the check, as for a free slot
        report(GetStep::TakeOver {
          slot: path.clone(),
          holder: old.holder.clone(),
        });
        let preserve = PreserveConfig::of(&config);
        let verdict = check_idle(git_executor, (&path, &name), &old, (config.untracked, preserve));
        let notes_roots = pool.notes_roots(Some(&pool_dir), params.home);
        let kept_with = |check: IdleCheck, skipped: &mut Vec<SkippedSlot>| {
          let mut kept = (*old).clone();
          kept.idle_check = Some(check.clone());
          skipped.push(SkippedSlot {
            name: name.clone(),
            path: path.clone(),
            reason: SkipReason::IdleWithWork {
              holder: kept.holder.clone(),
              lease_id: kept.id.clone(),
              check: Box::new(check),
            },
          });
          keep_idle(&pool, params.lock_timeout, &path, &lease.id, kept);
        };
        let (landed, preserved) = match verdict {
          Ok(IdleVerdict::Landed { landed, mark }) if commit_takeover(&pool, params.lock_timeout, &path, &lease, &mark)? => (landed, None),
          Ok(IdleVerdict::Landed { .. }) => {
            debug!(slot = %path, "the idle slot moved while the get checked it");
            keep_idle(&pool, params.lock_timeout, &path, &lease.id, *old);
            idle_checks += 1;
            continue;
          }
          Ok(IdleVerdict::Work { check, fresh, landed }) if preserves(&check, now()) => {
            idle_checks += usize::from(fresh);
            // the patches of a large slot take minutes: the get gives the clones lock up, its ticket stays busy, and it
            // waits at its place again after a preserve that did not take the slot
            end_checks_under_lock(&mut clones_lock);
            // before the commit point a Bazel server that does not exit gives the idle lease back
            if let Some(busy) = stop_reserved_servers(&pool, params, (&name, &path), (&lease.id, Some(&old)), &servers, &mut stopped_servers) {
              skipped.push(busy);
              continue;
            }
            report(GetStep::PreserveChanges {
              slot: path.clone(),
              holder: old.holder.clone(),
              paths: check.paths.len() + check.paths_omitted,
            });
            match preserve_changes(git_executor, &path, (&old, &check), preserve, &notes_roots, true) {
              Ok(Preserve::Ready { preserved, dirty }) if commit_takeover_with(&pool, params.lock_timeout, (&path, &lease, &check.mark), Some((&preserved, &dirty)))? => {
                (landed, Some(preserved))
              }
              Ok(Preserve::Ready { .. }) => {
                debug!(slot = %path, "the idle slot moved while the get preserved its changes");
                withdraw_preserved(&notes_roots, &old.id);
                keep_idle(&pool, params.lock_timeout, &path, &lease.id, *old);
                continue;
              }
              Ok(Preserve::Kept(check)) => {
                kept_with(check, &mut skipped);
                continue;
              }
              Ok(Preserve::Planned(_)) => unreachable!("a get preserves, it plans no preserve"),
              Err(error) => {
                warn!(slot = %path, error = %format!("{error:#}"), "cannot preserve the local changes of an idle lease");
                kept_with(check.failed_preserve(&error, now()), &mut skipped);
                continue;
              }
            }
          }
          // the work stays: the get reserves again with the clones lock it holds, so it adds a slot without another wait
          Ok(IdleVerdict::Work { check, fresh, .. }) => {
            idle_checks += usize::from(fresh);
            kept_with(*check, &mut skipped);
            continue;
          }
          Err(error) => {
            warn!(slot = %path, error = %format!("{error:#}"), "cannot check the work of an idle lease");
            keep_idle(&pool, params.lock_timeout, &path, &lease.id, *old);
            idle_checks += 1;
            continue;
          }
        };
        // the lease is the get's now: what fails from here frees the slot, whose work landed and whose changes are in the
        // notes of the lease. A Bazel server that does not exit leaves the get the next reserve under the clones lock, as
        // for a free slot; the servers of a preserve were stopped before it
        if preserved.is_none()
          && let Some(busy) = stop_reserved_servers(&pool, params, (&name, &path), (&lease.id, None), &servers, &mut stopped_servers)
        {
          skipped.push(busy);
          continue;
        }
        end_checks_under_lock(&mut clones_lock);
        report(GetStep::CleanTakenOver { slot: path.clone() });
        // the trash stays: the next return or clean deletes it as the trash of a gone run, and the get goes on
        let trash = Trash::new(&pool.trash_dir());
        // no wait for a lock of cargo, which only another clean holds: the get goes on, and a later clean prunes what stays.
        // The stale objects it defers stay in place for that clean too. The discard of preserved changes comes after the
        // close of the notes, whose directory its `clean` would delete
        finish_return(
          git_executor,
          &path,
          &old,
          preserved.as_ref().map(|_| config.untracked),
          (Some(CleanOverrides::default()), Duration::ZERO),
          (&trash, &notes_roots),
        )
        .map_err(|error| give_back(&pool, params.lock_timeout, &path, &lease.id, false, error))?;
        move_leased_slot(git_executor, &pool, params, &path, &lease.id, &source_head, config.workers)?;
        took_over = Some(TookOver { lease: *old, landed, preserved });
        (path, true)
      }
      Reserved::New { path, template } => {
        let text = display_path(&path);
        let template_dir = template.clone().unwrap_or_else(|| source.clone());
        let release_template = || {
          if let Some(template) = &template {
            release_marker(&pool, params.lock_timeout, template, &template_marker(&lease.id));
          }
        };
        let request = NewSlot {
          pool: &pool,
          lock_timeout: params.lock_timeout,
          path: &path,
          source: params.source,
          template: &template_dir,
          commit: &source_head,
          branch: params.branch.as_deref(),
          workers: config.workers,
          lfs: if config.lfs.smudge { LfsFiles::Pointers } else { LfsFiles::Content },
          untracked: config.untracked,
          skips: &skips,
          progress: &report,
        };
        let added = path
          .parent()
          .map_or(Ok(()), std::fs::create_dir_all)
          .map_err(|error| Added::NotRegistered(error.into()))
          .and_then(|()| add_new_slot(git_executor, &request, &release_template));
        // a clone that failed early releases the template itself; this one is for any later error
        release_template();
        match added {
          Ok(how) => {
            created_by = Some(how);
            if how == CreatedBy::Clone {
              cloned_from = Some(template_dir);
            }
          }
          Err(Added::NotRegistered(error)) => {
            // the number directory, when the add left it empty
            if let Some(parent) = path.parent() {
              let _ = fs_retry::remove_dir(parent);
            }
            return Err(give_back(&pool, params.lock_timeout, &text, &lease.id, true, error));
          }
          Err(Added::Registered(error)) => return Err(give_back(&pool, params.lock_timeout, &text, &lease.id, false, error)),
        }
        (text, false)
      }
    };
    // a new slot is registered, checked out and clean: the seed, a copy of ignored files that can take minutes, holds
    // no get that adds a slot (`clones.lock`), and neither does the download of the files of Git LFS, which its checkout
    // left as pointers
    drop(clones_lock.take());
    if !reused && config.lfs.smudge && config.lfs.objects(&git_common_dir).is_dir() {
      report(GetStep::LfsPull { slot: path.clone() });
      match pull_lfs_files(git_executor, &path) {
        Ok(()) => {}
        // the repository lives on pointers of what it can not download, as its smudge writes them; what the pull got,
        // or the store had, becomes content with no network, the index refreshed, as a pull that ended early left them
        Err(error) if skips_download_errors(git_executor, &path) => {
          warn!(slot = %path, error = %format!("{error:#}"), "cannot pull the files of Git LFS; lfs.skipdownloaderrors: the slot keeps the pointers of what it lacks");
          if let Err(error) = checkout_lfs_files(git_executor, &path) {
            forget_slot_index(&path);
            return Err(give_back(&pool, params.lock_timeout, &path, &lease.id, false, error));
          }
        }
        Err(error) => {
          forget_slot_index(&path);
          return Err(give_back(&pool, params.lock_timeout, &path, &lease.id, false, error));
        }
      }
    }
    if !seeds.is_empty() {
      report(GetStep::Seed {
        slot: path.clone(),
        paths: seeds.len(),
      });
    }
    let seeded = seed(Path::new(source), Path::new(&path), &seeds).map_err(|error| give_back(&pool, params.lock_timeout, &path, &lease.id, false, error))?;
    // the notes of an agent's lease, outside the worktree (`lease_notes`); a host slot has none. They never fail the get
    let notes = (!lease.holder.starts_with(HOST_HOLDER_PREFIX))
      .then(|| {
        open_notes(&new_notes_root(Path::new(&path), &pool_dir, params.home), Path::new(&path), &git_common_dir, &lease)
          .inspect_err(|error| warn!(slot = %path, error = %error, "cannot make the notes of the lease"))
          .ok()
      })
      .flatten();
    let name = linked_worktree(Path::new(&path)).map_or_default(|worktree| worktree.id);
    let slot = pool.update(params.lock_timeout, |store| {
      let slot = leased_slot(store, &path, &lease.id).ok_or_else(|| anyhow!("The lease of {path} was taken back while it was being prepared"))?;
      slot.name = name;
      slot.dirty = false;
      slot.dirty_check = None;
      if let Some(lease) = slot.lease.as_mut() {
        lease.state = LeaseState::Ready;
        lease.seeded = seeded;
        lease.reused = reused;
      }
      let slot = slot.clone();
      place.leave_in(store);
      Ok(slot)
    })?;
    place.left();
    return Ok(LeaseGot {
      slot,
      notes,
      created_by,
      cloned_from,
      pool_dir,
      skipped,
      stopped_servers,
      free_bytes,
      idle,
      took_over,
      process_scan: scan.is_available(),
      waited_for_clone,
      removed_unfinished,
      reclaimed: relief.reclaimed.unwrap_or_default(),
      trimmed: relief.trimmed,
      shrunk: relief.shrunk,
      evicted: relief.evicted,
      agent_landing: config.agent_landing,
    });
  }
}

/// The end of the checks of a slot that the get reserved. A get that took the clones lock to add a slot holds it across
/// them, so it keeps its turn: a busy or a dirty slot, or an idle lease with work, leaves it the next reserve under the
/// lock. Under the lock run one stop of the Bazel servers and one `status` for each free slot the get is given, and at
/// most `IDLE_CHECKS_PER_GET` full checks of an idle lease, each with the stop of the servers of its slot. Once the
/// slot is the get's, the lock goes. The next get in the queue then adds its slot while this one moves the slot and
/// cleans the lease it took over
fn end_checks_under_lock(clones_lock: &mut Option<RepositoryLock>) {
  drop(clones_lock.take());
}

/// Stop the Bazel servers of the slot `(name, path)` that the get of `lease_id` reserved, added to `stopped`: with one
/// that did not exit, the slot is given back, free or to its `idle` lease before the commit point of a takeover, and
/// skipped as busy
fn stop_reserved_servers(
  pool: &Pool,
  params: &LeaseGetParams<'_>,
  (name, path): (&str, &str),
  (lease_id, idle): (&str, Option<&Lease>),
  servers: &[ProcessInfo],
  stopped: &mut Vec<StoppedServer>,
) -> Option<SkippedSlot> {
  if servers.is_empty() {
    return None;
  }
  if let Some(progress) = params.progress {
    progress(GetStep::StopServers {
      slot: path.to_string(),
      servers: servers.len(),
    });
  }
  let servers: Vec<&ProcessInfo> = servers.iter().collect();
  let ServerStop { stopped: exited, running } = stop_servers(&servers, params.bazel, params.server_timeout);
  stopped.extend(exited);
  if running.is_empty() {
    return None;
  }
  debug!(slot = %path, servers = running.len(), "skipping a slot whose Bazel servers did not exit");
  match idle {
    Some(idle) => keep_idle(pool, params.lock_timeout, path, lease_id, idle.clone()),
    None => release_marker(pool, params.lock_timeout, path, lease_id),
  }
  Some(SkippedSlot {
    name: name.to_string(),
    path: path.to_string(),
    reason: SkipReason::Busy { processes: running },
  })
}

/// Move the reserved slot `path` to `commit` (`move_reused_slot`), onto the new branch of the get; a failure gives the
/// slot back
fn move_leased_slot(git_executor: &GitCommandExecutor, pool: &Pool, params: &LeaseGetParams<'_>, path: &str, lease_id: &str, commit: &str, workers: CheckoutWorkers) -> Result<()> {
  let report = |step: GetStep| {
    if let Some(progress) = params.progress {
      progress(step);
    }
  };
  report(GetStep::Checkout {
    slot: path.to_string(),
    commit: commit.to_string(),
  });
  move_reused_slot(git_executor, path, commit, params.branch.as_deref(), workers, &report).map_err(|error| give_back(pool, params.lock_timeout, path, lease_id, false, error))
}

/// The free space the volume of the pool must have: `--min-free`, else `branchdeck.leaseMinFree` of `config`, else none
fn required_free(params: &LeaseGetParams<'_>, config: &LeaseConfig) -> Result<u64, LeaseRefusal> {
  match (params.min_free, &config.min_free) {
    (Some(min_free), _) => Ok(min_free),
    (None, Some(value)) => parse_size(value).map_err(|reason| LeaseRefusal::InvalidSize {
      key: "branchdeck.leaseMinFree".to_string(),
      value: value.clone(),
      reason,
    }),
    (None, None) => Ok(0),
  }
}

/// What the free space of the volume of the pool lets a get do (`free_space`)
enum FreeSpace {
  /// At least the minimum: the free bytes, `None` where the system does not tell them
  Enough(Option<u64>),
  /// Below the minimum, and the trash holds what runs that are gone left (what the clean rules of a takeover moved
  /// there, which only a later return or clean deletes): the get deletes it (`delete_trash`), never under the clones
  /// lock, and measures again
  Trash,
  /// Below the minimum after the trash, and the get reclaimed no idle lease yet: the idle leases of every pool of the
  /// machine whose work landed are returned (`reclaim_for_get`), never under the clones lock, and the get measures
  /// again. A reclaim deletes no slot, so it runs for a minimum past the size of the volume too
  Reclaim,
  /// Below the minimum after the reclaim, and the get trimmed no pool yet: the output bases of the gone workspaces of
  /// every pool of the machine go and their expired notes, no slot (`trim_for_get`), never under the clones lock, and
  /// the get measures again
  Trim,
  /// Below the minimum after the trim, and the get shrank no pool yet: the build output of the free slots of every pool
  /// of the machine goes, the ones returned longest ago first, until the volume has the minimum and its margin
  /// (`shrink_for_get`), never under the clones lock
  Shrink,
  /// Below the minimum after the shrink, and the get evicted no slot yet: free slots of every pool go, one at a time,
  /// the ones returned longest ago first, until the volume has the minimum (`evict_for_get`), never under the clones
  /// lock
  Evict,
}

/// What a get below the minimum free space did to raise it: the trash once per wait for the clones lock, the reclaim,
/// the trim, the shrink and the eviction once per get
#[derive(Default)]
struct Relief {
  /// The relief of the keeper (`relieve_core`), which checks every idle lease in full: a get checks at most
  /// `IDLE_CHECKS_PER_GET` (`reclaim_for_get`)
  keeper: bool,
  /// The get deleted the trash since its last wait for the clones lock
  swept: bool,
  /// The idle leases it returned; `Some` once the reclaim ran
  reclaimed: Option<Vec<ReclaimedLease>>,
  trimmed: Option<Trimmed>,
  shrunk: Option<Shrunk>,
  evicted: Option<Trimmed>,
}

impl Relief {
  /// The free bytes of the volume of the pool once it has `min_free`: each step of `free_space` that it still lacks,
  /// one after the other (`relieve`), measured again after each. `LowDiskSpace` below it after all
  fn raise(
    &mut self,
    git_executor: &GitCommandExecutor,
    params: &LeaseGetParams<'_>,
    pool: &Pool,
    (pool_dir, min_free): (&Path, u64),
    report: &dyn Fn(GetStep),
  ) -> Result<Option<u64>> {
    loop {
      match free_space(params, pool, pool_dir, min_free, self)? {
        FreeSpace::Enough(available) => return Ok(available),
        low => self.relieve(git_executor, params, pool, (pool_dir, min_free), &low, report),
      }
    }
  }

  /// The step `low` of `free_space` for the get of `params` whose pool directory is `pool_dir` and whose minimum is
  /// `min_free`, never under the clones lock: the get measures again after it
  fn relieve(
    &mut self,
    git_executor: &GitCommandExecutor,
    params: &LeaseGetParams<'_>,
    pool: &Pool,
    (pool_dir, min_free): (&Path, u64),
    low: &FreeSpace,
    report: &dyn Fn(GetStep),
  ) {
    match low {
      FreeSpace::Enough(_) => {}
      FreeSpace::Trash => {
        delete_trash(pool, report);
        self.swept = true;
      }
      FreeSpace::Reclaim => {
        let full_checks = (!self.keeper).then_some(IDLE_CHECKS_PER_GET);
        if let Some(reclaimed) = reclaim_for_get(git_executor, params, pool, (pool_dir, min_free), full_checks, report) {
          self.reclaimed = Some(reclaimed);
        } else {
          // no idle lease on the volume: nothing moved, and the trim, the next step, runs in the same turn
          self.reclaimed = Some(Vec::new());
          self.relieve(git_executor, params, pool, (pool_dir, min_free), &FreeSpace::Trim, report);
        }
      }
      FreeSpace::Trim => self.trimmed = Some(trim_for_get(git_executor, params, report)),
      FreeSpace::Shrink => {
        report(GetStep::Shrink);
        // the volume got its minimum while the get waited: measured again, nothing to shrink
        if let PressureTurn::Taken(_pressure) = pressure_turn(params, pool, (pool_dir, min_free), report) {
          self.shrunk = Some(shrink_for_get(git_executor, params, (pool_dir, min_free)));
        }
      }
      FreeSpace::Evict => {
        report(GetStep::Evict);
        if let PressureTurn::Taken(_pressure) = pressure_turn(params, pool, (pool_dir, min_free), report) {
          self.evicted = Some(evict_for_get(git_executor, params, (pool_dir, min_free)));
        }
      }
    }
  }
}

/// What `relieve_core` did on a volume below the minimum free space, as a get does it
#[derive(Debug)]
pub struct Relieved {
  /// The minimum free space (`branchdeck.leaseMinFree`)
  pub required: u64,
  /// The free bytes before
  pub before: u64,
  /// The free bytes after, `None` where the system does not tell them; below `required` when every step was taken and
  /// the volume still lacks it
  pub after: Option<u64>,
  /// The trash of gone runs was deleted
  pub swept: bool,
  /// The idle leases of the pools of the machine whose work landed, returned before the trim (`reclaim_for_get`)
  pub reclaimed: Vec<ReclaimedLease>,
  pub trimmed: Option<Trimmed>,
  pub shrunk: Option<Shrunk>,
  pub evicted: Option<Trimmed>,
  /// The idle host slots of the repository relieved after these steps, used longest ago first, until the volume had its
  /// minimum (`host_slots::relieve_host_slots`); empty for a get, which lends no host slot
  pub host_slots: Vec<RelievedHostSlot>,
  /// The prune ahead of the minimum, when the volume had the minimum but less than one and a half of it: then only the
  /// left trash and the shrink ran, up to that threshold, and the shrink kept the free slot each pool's next get takes
  /// (`ShrinkParams::keep_next`); `None` for a relief below the minimum
  pub ahead: Option<Ahead>,
}

/// What the keeper's prune ahead of the minimum did besides the shrink (`Relieved::ahead`, `prune_ahead`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ahead {
  /// The free bytes below which the keeper prunes ahead of the minimum, and up to which it prunes (`ahead_threshold`)
  pub threshold: u64,
  /// The bytes of the trash that earlier deletes left in the pools of the machine on the volume (`Trash::abandon`, the
  /// trash of gone runs), deleted before the shrink
  pub trash_freed: u64,
}

/// The volumes that one round of the keeper took already (`relieve_core`): a relief raises the free space of a volume
/// for every pool on it, and its steps take every pool of the machine, so the next pool of the round on that volume
/// measures nothing and deletes nothing
#[derive(Debug, Default)]
pub struct ReliefRound {
  /// The pool directory of each volume taken, as `free_space::Volume` tells it
  pools: Vec<PathBuf>,
  /// The pool directory of each volume that had the minimum at its measure in this round: no host slot of it is
  /// relieved in this round (`host_slots::relieve_host_slots`), though a prune ahead ended below the minimum; the next
  /// round runs the whole relief
  ahead: Vec<PathBuf>,
}

impl ReliefRound {
  /// Whether a pool of the round lives on the volume of `pool_dir`
  fn covers(&self, pool_dir: &Path) -> bool {
    self.pools.iter().any(|pool| Volume::of(pool).holds(pool_dir))
  }

  /// Whether the round measured the volume of `path` above the minimum (`ReliefRound::ahead`)
  pub(crate) fn had_minimum(&self, path: &Path) -> bool {
    self.ahead.iter().any(|pool| Volume::of(pool).holds(path))
  }
}

/// The free space of the volume of the pool of `params.source` raised to its minimum without a get, by the steps a get
/// takes below it (the trash, the reclaim, the trim, the shrink, the eviction; `Relieved::after`). `None` without a
/// minimum, where the system tells no free space, and on a volume that has one and a half of it: then it measures only.
/// Below it after all, it answers what it did, not the `LowDiskSpace` of a get, whose advice is for an agent that asked
/// for a slot. The keeper of the host slots calls it (`host_slots::keep_free`), so a host that only runs keeps its
/// minimum. A volume that `round` took already is `None` too: once per volume and round.
///
/// A volume that has the minimum but less than `ahead_threshold` is pruned ahead of it (`prune_ahead`), so a get rarely
/// meets the minimum while agents build: `None` when another holds the lock of the shrink of the machine, and without
/// `ahead`. Such a volume is marked in `round` (`ReliefRound::had_minimum`): a prune ahead that ends below the minimum
/// leaves the whole relief to the next round
pub(crate) fn relieve_core(git_executor: &GitCommandExecutor, params: &LeaseGetParams<'_>, round: &mut ReliefRound, ahead: bool) -> Result<Option<Relieved>> {
  let source = &params.source.top_level;
  let config = lease_config(git_executor, source)?;
  let min_free = required_free(params, &config)?;
  if min_free == 0 {
    return Ok(None);
  }
  let git_common_dir = PathBuf::from(canonical_path(Path::new(&params.source.git_common_dir)));
  let pool_dir = PathBuf::from(canonical_path(&pool_dir(&config, source, &git_common_dir, params.home)?));
  if round.covers(&pool_dir) {
    return Ok(None);
  }
  round.pools.push(pool_dir.clone());
  let threshold = ahead_threshold(min_free);
  let Some(before) = params.available(&pool_dir).filter(|available| *available < threshold) else {
    return Ok(None);
  };
  let pool = Pool::new(&git_common_dir);
  if before >= min_free {
    round.ahead.push(pool_dir.clone());
    if !ahead {
      return Ok(None);
    }
    return Ok(prune_ahead(git_executor, params, &pool, (&pool_dir, threshold)).map(|(trash_freed, shrunk)| Relieved {
      required: min_free,
      before,
      after: params.available(&pool_dir),
      swept: false,
      reclaimed: Vec::new(),
      trimmed: None,
      shrunk,
      evicted: None,
      host_slots: Vec::new(),
      ahead: Some(Ahead { threshold, trash_freed }),
    }));
  }
  let mut relief = Relief {
    keeper: true,
    ..Relief::default()
  };
  let after = match relief.raise(git_executor, params, &pool, (&pool_dir, min_free), &|_| {}) {
    Ok(after) => after,
    Err(error) if matches!(error.downcast_ref::<LeaseRefusal>(), Some(LeaseRefusal::LowDiskSpace { .. })) => params.available(&pool_dir),
    Err(error) => return Err(error),
  };
  Ok(Some(Relieved {
    required: min_free,
    before,
    after,
    swept: relief.swept,
    reclaimed: relief.reclaimed.unwrap_or_default(),
    trimmed: relief.trimmed,
    shrunk: relief.shrunk,
    evicted: relief.evicted,
    host_slots: Vec::new(),
    ahead: None,
  }))
}

/// How far below one and a half of the minimum free space the keeper starts its prune (`relieve_core`): on 2026-11-24
/// a dozen leases building at once took the user's Mac from 139 to 82 GiB free in an hour, below its `100G`, where
/// every get then shrank first. Ahead of the minimum, the keeper prunes once a minute what the cargo rules of the leases name
/// stale and cleans the free slots no next get takes, so the get mostly finds the minimum. The minimum is what the user chose
/// for the machine, so the threshold scales with it, and no setting is added
const AHEAD_DIVISOR: u64 = 2;

/// The free bytes below which the keeper prunes ahead of the minimum `min_free` (`AHEAD_DIVISOR`)
pub const fn ahead_threshold(min_free: u64) -> u64 {
  min_free.saturating_add(min_free / AHEAD_DIVISOR)
}

/// The keeper's prune ahead of the minimum on the volume of `pool_dir` up to `threshold`: first the trash that earlier
/// deletes left (`delete_left_trash`), then, below `threshold` still, the shrink of the pools of the machine
/// (`shrink::shrink_core`): the leased slots by the cargo rules of their leases, then the free slots but the one each
/// pool's next get takes (`ShrinkParams::keep_next`). A shrink stops its delete at `threshold` and leaves the rest in the
/// trash, so without the first step each round would make one more slot cold and leave its output in the trash. It
/// takes the lock of the shrink of the machine only when no get holds it (`shrink::lock_pressure`): a get that shrinks
/// frees the volume anyway, and the keeper's next round measures again. `None` then, and for a threshold past the size
/// of the volume, where every free slot would go; else the bytes of the trash it deleted and the shrink, if it ran
fn prune_ahead(git_executor: &GitCommandExecutor, params: &LeaseGetParams<'_>, pool: &Pool, (pool_dir, threshold): (&Path, u64)) -> Option<(u64, Option<Shrunk>)> {
  if params.space(pool_dir).is_none_or(|space| threshold > space.total) {
    return None;
  }
  let _pressure = match lock_pressure(params.home, pool, params.lock_timeout, &mut |_| {}, &|| true) {
    Ok(lock) => lock,
    Err(error) if error.downcast_ref::<LockWaitCanceled>().is_some() => {
      debug!("a get holds the lock of the shrink of the machine: the keeper prunes ahead at its next round");
      return None;
    }
    Err(error) => {
      warn!(error = %format!("{error:#}"), "cannot take the lock of the shrink of the machine to prune ahead of the minimum");
      return None;
    }
  };
  let available = |path: &Path| params.available(path);
  let enough = || available(pool_dir).is_some_and(|free| free >= threshold);
  let trash_freed = delete_left_trash(params, pool_dir, &enough);
  if enough() {
    return Some((trash_freed, None));
  }
  let shrunk = shrink_core(
    git_executor,
    &ShrinkParams {
      repository: params.source,
      home: params.home,
      volume: pool_dir,
      target: threshold,
      available: &available,
      lock_timeout: params.lock_timeout,
      cached_build: params.cached_build,
      keep_next: true,
    },
  );
  Some((trash_freed, Some(shrunk)))
}

/// Delete the trash that earlier deletes left in each pool of the machine (`trim::machine_pools`) on the volume of
/// `pool_dir`: the directories that a stopped delete left (`Trash::abandon`) and those of gone runs, until `enough`
/// holds (`lease_clean::sweep_gone_until`). The trash of a live run stays. The bytes it freed
fn delete_left_trash(params: &LeaseGetParams<'_>, pool_dir: &Path, enough: &dyn Fn() -> bool) -> u64 {
  let volume = Volume::of(pool_dir);
  let mut freed = 0;
  for machine_pool in machine_pools(params.source, params.home) {
    let trash = Pool::new(&machine_pool.git_common_dir).trash_dir();
    if !volume.holds(&trash) {
      continue;
    }
    let (bytes, complete) = sweep_gone_until(&trash, enough);
    freed += bytes;
    if !complete {
      break;
    }
  }
  freed
}

/// The turn of a get at the shrink and the eviction of the machine (`pressure_turn`)
enum PressureTurn {
  /// Its turn, the lock held while the guard lives; `None`: a lock that can't be taken, and the get goes on without it,
  /// as the gets did before the lock
  Taken(Option<RepositoryLock>),
  /// The volume got the minimum while the get waited, from the deletes of the get whose turn it was: the get needs no
  /// turn and measures again
  Enough,
}

/// The turn of the get at the shrink and the eviction of the machine (`shrink::lock_pressure`), which the core measures
/// after first: a get that waited for one beside it usually finds the volume above its target and deletes nothing. The
/// wait ends without the turn once the volume of `pool_dir` has `min_free` (one `statfs` a poll of the wait): the
/// holder frees the volume slot by slot, and the delete of a slot of a large repository takes minutes, so a get that
/// needs no more than the minimum goes on as soon as it is there, not when the whole shrink or eviction of the other
/// ended. The wait is told once per holder (`GetStep::WaitForPressure`)
fn pressure_turn(params: &LeaseGetParams<'_>, pool: &Pool, (pool_dir, min_free): (&Path, u64), report: &dyn Fn(GetStep)) -> PressureTurn {
  // the holder told last, and when
  let mut told: Option<(u32, Instant)> = None;
  let mut on_holder = |holder: &LockHolder| {
    if told.is_none_or(|(pid, at)| pid != holder.pid || at.elapsed() >= PRESSURE_WAIT_TICK) {
      told = Some((holder.pid, Instant::now()));
      report(GetStep::WaitForPressure {
        holder: holder.clone(),
        free: params.available(pool_dir),
        minimum: min_free,
      });
    }
  };
  let enough = || params.available(pool_dir).is_some_and(|available| available >= min_free);
  match lock_pressure(params.home, pool, params.lock_timeout, &mut on_holder, &enough) {
    Ok(lock) => PressureTurn::Taken(Some(lock)),
    Err(error) if error.downcast_ref::<LockWaitCanceled>().is_some() => PressureTurn::Enough,
    Err(error) => {
      warn!(error = %format!("{error:#}"), "cannot take the lock of the shrink and the eviction of the machine");
      PressureTurn::Taken(None)
    }
  }
}

/// The free space of the volume of the pool, at least `min_free`, measured only: one `statfs`
/// (`LeaseGetParams::space`), and below the minimum the names in the trash. Below it with no trash of a gone run, or
/// one the get deleted since its last wait (`swept`), the reclaim of the idle leases, the trim of the pools, their
/// shrink, then the eviction of their free slots, each once; the shrink and the eviction only for a minimum the volume
/// can hold (`VolumeSpace::total`): past it every free slot of the volume would go and the get refuse anyway. Below it
/// after all: `LowDiskSpace`, naming what the trim and the eviction deleted, what the shrink freed and what the trash
/// keeps
fn free_space(params: &LeaseGetParams<'_>, pool: &Pool, pool_dir: &Path, min_free: u64, relief: &Relief) -> Result<FreeSpace> {
  let space = params.space(pool_dir);
  let Some(VolumeSpace { available, total }) = space.filter(|space| space.available < min_free) else {
    return Ok(FreeSpace::Enough(space.map(|space| space.available)));
  };
  if !relief.swept && holds_gone_runs(&pool.trash_dir()) {
    return Ok(FreeSpace::Trash);
  }
  let Some(reclaimed) = &relief.reclaimed else {
    return Ok(FreeSpace::Reclaim);
  };
  let Some(trimmed) = &relief.trimmed else {
    return Ok(FreeSpace::Trim);
  };
  let reachable = min_free <= total;
  if reachable && relief.shrunk.is_none() {
    return Ok(FreeSpace::Shrink);
  }
  if reachable && relief.evicted.is_none() {
    return Ok(FreeSpace::Evict);
  }
  let shrink = relief.shrunk.as_ref().map_or_else(Freed::default, |shrunk| Freed {
    slots: shrunk.freed_slots(),
    bytes: shrunk.freed_bytes(),
  });
  let evict = relief.evicted.as_ref().map_or_else(Freed::default, |evicted| Freed {
    slots: evicted.removed_slots(),
    bytes: evicted.freed_bytes(),
  });
  let store = pool.read()?;
  bail!(LeaseRefusal::LowDiskSpace {
    pool: display_path(pool_dir),
    available,
    required: min_free,
    total,
    free_slots: store.slots.iter().filter(|slot| slot.lease.is_none()).count(),
    idle: idle_leases(&store),
    trash: pool.trash_bytes(),
    freed: Box::new(FreedBy {
      reclaim: Freed {
        slots: reclaimed.len(),
        bytes: reclaimed.iter().map(ReclaimedLease::freed_bytes).sum(),
      },
      trim: Freed {
        slots: trimmed.removed_slots(),
        bytes: trimmed.freed_bytes(),
      },
      shrink,
      evict,
    }),
  });
}

/// The trim of the pools of the machine below the minimum free space (`trim::trim_core`) without its slots: the output
/// bases of gone workspaces and the expired notes. A free slot past `branchdeck.leaseMaxSlots` stays: with more leases
/// than the limit every free slot is past it, and a trim of them deleted the slot the get was about to reuse, so it
/// cloned one, 25 minutes in the idea-1 pool on 2026-11-18; the shrink and the eviction free the space by the bytes
/// instead, the next free slot kept. Told as a step; a trim that fails deletes nothing more, and the get measures again
fn trim_for_get(git_executor: &GitCommandExecutor, params: &LeaseGetParams<'_>, report: &dyn Fn(GetStep)) -> Trimmed {
  report(GetStep::Trim);
  trim_core(git_executor, &trim_params(params, false)).unwrap_or_else(|error| {
    warn!(error = %format!("{error:#}"), "cannot trim the pools of the machine");
    Trimmed::default()
  })
}

/// The removals of a get as a trim runs them, which deletes and stops the Bazel servers; `slots`: the free slots may go
const fn trim_params<'a>(params: &LeaseGetParams<'a>, slots: bool) -> TrimParams<'a> {
  TrimParams {
    repository: params.source,
    home: params.home,
    max_slots: None,
    slots,
    apply: true,
    server_timeout: params.server_timeout,
    bazel: params.bazel,
    lock_timeout: params.lock_timeout,
  }
}

/// The reclaim of the idle leases of the pools of the machine below the minimum free space (`lease_reclaim_core`, the
/// rule of `brd lease reclaim --idle`), only of the slots on the volume of `pool_dir`: an idle lease of another volume
/// frees nothing on it. Told as a step and run only while a pool holds an idle lease there (one read of each state, no
/// git), under the turn of the shrink of the machine (`pressure_turn`): two gets do not check the same slots at once,
/// and a get whose volume got its minimum meanwhile reclaims nothing. A recorded check that holds is kept for one
/// `rev-parse`; with `full_checks` at most that many leases are checked in full, the pools whose idle lease was checked
/// longest ago first, so later gets check the rest, and a lease whose reclaim failed within `RECLAIM_RETRY` counts as
/// none. A pool whose reclaim fails is passed over. `None` when no pool holds such a lease
fn reclaim_for_get(
  git_executor: &GitCommandExecutor,
  params: &LeaseGetParams<'_>,
  pool: &Pool,
  (pool_dir, min_free): (&Path, u64),
  full_checks: Option<usize>,
  report: &dyn Fn(GetStep),
) -> Option<Vec<ReclaimedLease>> {
  let volume = Volume::of(pool_dir);
  let now = now();
  let mut pools: Vec<(u64, PathBuf)> = machine_pools(params.source, params.home)
    .into_iter()
    .filter_map(|machine| {
      let store = Pool::new(&machine.git_common_dir).read().ok()?;
      let checked_longest_ago = store
        .slots
        .iter()
        .filter_map(|slot| {
          slot
            .lease
            .as_ref()
            .filter(|lease| lease.is_idle() && !(full_checks.is_some() && lease.reclaim_waits(now)) && volume.holds(Path::new(&slot.path)))
        })
        .map(Lease::idle_checked_at)
        .min()?;
      Some((checked_longest_ago, machine.git_common_dir))
    })
    .collect();
  if pools.is_empty() {
    return None;
  }
  pools.sort_by_key(|(checked, _)| *checked);
  report(GetStep::Reclaim);
  let PressureTurn::Taken(_pressure) = pressure_turn(params, pool, (pool_dir, min_free), report) else {
    return Some(Vec::new());
  };
  let full_checks = full_checks.map(Cell::new);
  let mut reclaimed = Vec::new();
  for (_, git_common_dir) in pools {
    let reclaim = LeaseReclaimParams {
      git_common_dir: &git_common_dir,
      apply: true,
      clean: CleanOverrides::default(),
      reuse_checks: true,
      volume: Some(&volume),
      full_checks: full_checks.as_ref(),
      server_timeout: params.server_timeout,
      bazel: params.bazel,
      lock_timeout: params.lock_timeout,
    };
    match lease_reclaim_core(git_executor, &reclaim) {
      Ok(answer) => reclaimed.extend(answer.reclaimed),
      Err(error) => warn!(pool = %git_common_dir.display(), error = %format!("{error:#}"), "cannot reclaim the idle leases of a pool"),
    }
  }
  Some(reclaimed)
}

/// How far above the minimum free space a shrink goes once the volume is below it: a tenth of the minimum (20 GiB of
/// the `200G` of the user's Mac). Without a margin every get after the first would find the volume just below the
/// minimum again, once the lease it leased had built, and shrink one more slot; a margin of a whole slot (50 to 59 GiB
/// of `target/` in a slot of branch-deck on 2026-11-03) would deep-clean two slots where one lifts the volume over the
/// minimum. The minimum is what the user chose for the machine, so its margin scales with it, and no setting is added
const SHRINK_MARGIN_DIVISOR: u64 = 10;

/// The free bytes at which a shrink below the minimum `min_free` stops (`SHRINK_MARGIN_DIVISOR`)
pub const fn shrink_target(min_free: u64) -> u64 {
  min_free.saturating_add(min_free / SHRINK_MARGIN_DIVISOR)
}

/// The shrink of the pools of the machine below the minimum free space (`shrink::shrink_core`), told as a step, up to
/// the minimum and its margin (`shrink_target`) of the volume of `pool_dir`
fn shrink_for_get(git_executor: &GitCommandExecutor, params: &LeaseGetParams<'_>, (pool_dir, min_free): (&Path, u64)) -> Shrunk {
  let available = |path: &Path| params.available(path);
  shrink_core(
    git_executor,
    &ShrinkParams {
      repository: params.source,
      home: params.home,
      volume: pool_dir,
      target: shrink_target(min_free),
      available: &available,
      lock_timeout: params.lock_timeout,
      cached_build: params.cached_build,
      keep_next: false,
    },
  )
}

/// The eviction of free slots of the pools of the machine (`trim::evict_core`), told as
/// a step, up to the minimum of the volume of `pool_dir`, without the margin of the shrink: a slot is a whole worktree,
/// and with slots of 15 GiB and a margin of 20 GiB a margin would delete one more slot after the volume is already
/// above the minimum. After the shrink a free slot holds little but its worktree, which a later get clones again; a
/// refusal instead would leave the user to delete slots by hand
fn evict_for_get(git_executor: &GitCommandExecutor, params: &LeaseGetParams<'_>, (pool_dir, min_free): (&Path, u64)) -> Trimmed {
  let available = |path: &Path| params.available(path);
  evict_core(
    git_executor,
    &EvictParams {
      trim: trim_params(params, true),
      volume: pool_dir,
      target: min_free,
      available: &available,
    },
  )
}

/// Delete the trash of the runs that are gone (`Trash::sweep`), told as a step
fn delete_trash(pool: &Pool, report: &dyn Fn(GetStep)) {
  report(GetStep::DeleteTrash);
  Trash::new(&pool.trash_dir()).sweep();
}

/// The turn of a get at the clones lock (`wait_turn`); `waited` is how long the get waited for it, when the wait was
/// told (`GetStep::WaitForClone`, `GetStep::WaitBehind`)
enum CloneTurn {
  Locked {
    lock: RepositoryLock,
    waited: Option<Duration>,
  },
  /// A free slot or an idle lease that the queue gives the get is there (`claimable`): the get reserves it without the
  /// lock
  SlotFreed {
    waited: Option<Duration>,
  },
}

/// How often a get that waits behind others in the queue looks at the state again: a slot freed meanwhile ends the
/// wait within about a second
const QUEUE_LOOK: Duration = Duration::from_millis(200);

/// The state of `pool` without the lock; an error of the read is an empty state, which frees no slot
fn read_state(pool: &Pool) -> LeaseStore {
  pool.read().unwrap_or_else(|error| {
    debug!(error = %format!("{error:#}"), "cannot read the state while waiting for a slot");
    LeaseStore::default()
  })
}

/// Whether `store` has a slot that the queue gives the get of `gate` and that it has not tried (`tried`): a free slot,
/// or with `idle` (the common dir of a get that takes idle leases over) an idle lease that its reserve takes over
/// (`is_idle_candidate`)
fn claimable(store: &LeaseStore, gate: &Gate, (tried, idle): (&HashSet<String>, Option<&Path>)) -> bool {
  let now = now();
  store.slots.iter().any(|slot| {
    (reservable(slot, tried) && gate.lets_free(&slot.path)) || idle.is_some_and(|git_common_dir| is_idle_candidate(slot, git_common_dir, tried, now) && gate.lets_idle(&slot.path))
  })
}

/// The wait of a get that found no free slot, at its place in the queue (`get_queue`): it ends once a free slot or an
/// idle lease that the queue gives the get is there (`claimable`), or with the clones lock, which only the first
/// waiting get waits for (`lock_clones`). A get behind others looks at the state every `QUEUE_LOOK` and tells whom it waits behind
/// (`GetStep::WaitBehind`) when the wait starts, when the get right ahead changes, and once a minute. Each look writes
/// the heartbeat of the get when it is due (`QueuePlace::keep`)
fn wait_turn(pool: &Pool, unknown_timeout: Duration, place: &mut QueuePlace<'_>, tried: (&HashSet<String>, Option<&Path>), report: &dyn Fn(GetStep)) -> Result<CloneTurn> {
  let started = Instant::now();
  let mut waited = false;
  // the get ahead told last, and when
  let mut told: Option<(Behind, Instant)> = None;
  loop {
    let store = read_state(pool);
    place.keep(&store);
    let gate = place.gate(&store);
    if claimable(&store, &gate, tried) {
      return Ok(CloneTurn::SlotFreed {
        waited: waited.then(|| started.elapsed()),
      });
    }
    let Some(behind) = gate.behind() else {
      let (turn, told_holder) = lock_clones(pool, unknown_timeout, place, tried, report)?;
      waited |= told_holder;
      let waited_for = waited.then(|| started.elapsed());
      match turn {
        ClonesWait::Locked(lock) => return Ok(CloneTurn::Locked { lock, waited: waited_for }),
        ClonesWait::SlotFreed => return Ok(CloneTurn::SlotFreed { waited: waited_for }),
        ClonesWait::Behind => continue,
      }
    };
    if told.as_ref().is_none_or(|(last, at)| *last != behind || at.elapsed() >= CLONE_WAIT_TICK) {
      report(GetStep::WaitBehind(behind.clone()));
      told = Some((behind, Instant::now()));
    }
    waited = true;
    std::thread::sleep(QUEUE_LOOK);
  }
}

/// How the wait of the first get of the queue for the clones lock ended (`lock_clones`)
enum ClonesWait {
  Locked(RepositoryLock),
  /// A free slot or an idle lease that the queue gives the get is there
  SlotFreed,
  /// Another get waits ahead of it again: a ticket that was busy or stale
  Behind,
}

/// The clones lock of `pool` (`Pool::lock_clones`) for the first waiting get of the queue, or the end of the wait for it
/// once a slot that the queue gives the get is there (`claimable`), or once another get waits ahead of
/// it: the state it reads for the holder anyway (`clone_holder`, about once a second) tells both, and an error of that
/// read is no slot. Not the modification time of the state: two writes within one tick of the clock look like none.
/// The wait is told (`GetStep::WaitForClone`) when it starts, when the holder or the slot it adds changes (a holder that
/// took the lock a moment ago reserves its slot next), and once a minute; the answer says whether it was told
fn lock_clones(
  pool: &Pool,
  unknown_timeout: Duration,
  place: &mut QueuePlace<'_>,
  tried: (&HashSet<String>, Option<&Path>),
  report: &dyn Fn(GetStep),
) -> Result<(ClonesWait, bool)> {
  // the holder told last, and when
  let mut told: Option<(CloneHolder, Instant)> = None;
  let freed = Cell::new(false);
  let behind = Cell::new(false);
  let lock = pool.lock_clones(
    unknown_timeout,
    &mut |process| {
      let store = read_state(pool);
      place.keep(&store);
      let gate = place.gate(&store);
      if claimable(&store, &gate, tried) {
        freed.set(true);
        return;
      }
      if !gate.is_first() {
        behind.set(true);
        return;
      }
      let holder = clone_holder(&store, process);
      if told.as_ref().is_none_or(|(last, at)| *last != holder || at.elapsed() >= CLONE_WAIT_TICK) {
        report(GetStep::WaitForClone(holder.clone()));
        told = Some((holder, Instant::now()));
      }
    },
    &|| freed.get() || behind.get(),
  );
  let told = told.is_some();
  match lock {
    Ok(lock) => Ok((ClonesWait::Locked(lock), told)),
    Err(error) if error.is::<LockWaitCanceled>() && freed.get() => Ok((ClonesWait::SlotFreed, told)),
    Err(error) if error.is::<LockWaitCanceled>() && behind.get() => Ok((ClonesWait::Behind, told)),
    Err(error) => Err(error),
  }
}

/// The holder of the clones lock with the new slot it adds, as `store` (read without the lock) tells it: the slot whose
/// `preparing` lease is of its process and that has no worktree id yet, and the free slot that lease clones
fn clone_holder(store: &LeaseStore, process: &LockHolder) -> CloneHolder {
  let adding = store.slots.iter().find_map(|slot| {
    let lease = slot
      .lease
      .as_ref()
      .filter(|lease| slot.name.is_empty() && lease.state == LeaseState::Preparing && lease.pid == process.pid && lease.process_started == process.process_started)?;
    let marker = template_marker(&lease.id);
    let template = store
      .slots
      .iter()
      .find(|template| template.lease.as_ref().is_some_and(|template_lease| template_lease.id == marker))
      .map(|template| template.path.clone());
    Some(AddingSlot {
      slot: slot.path.clone(),
      holder: lease.holder.clone(),
      template,
    })
  });
  CloneHolder { process: process.clone(), adding }
}

/// Under the pool lock: free the leases whose get died while preparing, drop the free slots whose directory is gone, and
/// reserve a free slot (clean ones first, the one returned last first, then in the order they joined the pool) or, with
/// `may_add` (the get holds the clones lock), a new one, unless the return or the clean of a slot runs (`Freeing`). The slot returned last has the warmest index, fsmonitor daemon
/// and build caches and the commit closest to the source; the order they joined left the other slots idle for days,
/// cold and far behind. A free slot with a process in it is skipped as busy; one with only its Bazel servers in it is
/// reserved with them, for the get to stop them
#[expect(clippy::too_many_arguments, reason = "the state of one get and its pool, passed to each of its reserves")]
fn reserve(
  store: &mut LeaseStore,
  git_common_dir: &Path,
  pool_dir: &Path,
  (scan, gate): (&ProcessScan, &Gate),
  tried: &mut HashSet<String>,
  awaited: &mut HashSet<String>,
  skipped: &mut Vec<SkippedSlot>,
  lease: &Lease,
  may_add: bool,
  take_over: bool,
) -> Reserved {
  for slot in &mut store.slots {
    settle_abandoned(slot);
  }
  store.slots.retain(|slot| slot.lease.is_some() || Path::new(&slot.path).is_dir());
  for dirty in [false, true] {
    // a slot that a get ahead in the queue has not tried is that get's
    let mut order: Vec<usize> = (0..store.slots.len())
      .filter(|&index| reservable(&store.slots[index], tried) && store.slots[index].dirty == dirty && gate.lets_free(&store.slots[index].path))
      .collect();
    // stable: the slots never returned come last, in the order they joined
    order.sort_by_key(|&index| std::cmp::Reverse(store.slots[index].returned));
    for index in order {
      let slot = &mut store.slots[index];
      // tried before any check: the wait for the clones lock ends only for a slot this get has not looked at
      tried.insert(slot.path.clone());
      if !is_worktree_of(Path::new(&slot.path), git_common_dir) {
        continue;
      }
      // a dirty slot that its files still prove dirty is skipped without another `status`
      if let Some(check) = slot.dirty_check.as_ref().filter(|check| slot.dirty && check.holds(Path::new(&slot.path))) {
        skipped.push(SkippedSlot {
          name: slot.name.clone(),
          path: slot.path.clone(),
          reason: SkipReason::Dirty {
            paths: check.paths.clone(),
            omitted: check.paths_omitted,
          },
        });
        continue;
      }
      // a Bazel server serves only its slot, and a free slot is the pool's: the get stops it. A `--watchfs` server could
      // miss the events of the checkout and build stale
      let Some(servers) = servers_alone(scan, slot, skipped) else {
        continue;
      };
      slot.lease = Some(lease.clone());
      // the record of a takeover lasts until the free slot is leased again
      slot.reclaimed = None;
      return Reserved::Free {
        path: slot.path.clone(),
        name: slot.name.clone(),
        servers,
      };
    }
  }
  if take_over && let Some(reserved) = reserve_idle(store, git_common_dir, (scan, gate), tried, skipped, lease) {
    return reserved;
  }
  if !may_add {
    return Reserved::NoFreeSlot;
  }
  // a slot whose return or clean runs (a dead one's lease was settled above) is free in a moment, a clone takes 40 s and
  // more in idea-1: the get waits for it, once for each (`awaited`), as a failed return leaves the lease `ready`
  let freeing = store.slots.iter().find_map(|slot| {
    let lease = slot
      .lease
      .as_ref()
      .filter(|lease| matches!(lease.state, LeaseState::Returning | LeaseState::Cleaning) && !awaited.contains(&lease.id))?;
    Some((slot.path.clone(), lease.id.clone(), lease.pid, lease.state == LeaseState::Cleaning))
  });
  if let Some((path, lease_id, pid, clean)) = freeing {
    awaited.insert(lease_id.clone());
    return Reserved::Freeing { path, lease_id, pid, clean };
  }
  let template = CLONE_AVAILABLE.then(|| mark_template(store, git_common_dir, (scan, gate), lease)).flatten();
  let path = next_slot_path(store, pool_dir, git_common_dir);
  store.slots.push(Slot {
    name: String::new(),
    path: display_path(&path),
    dirty: false,
    dirty_check: None,
    returned: None,
    returned_at: None,
    lease: Some(lease.clone()),
    reclaimed: None,
  });
  Reserved::New { path, template }
}

/// Whether a get that looked at the slots of `tried` may reserve `slot` as free: not tried yet, and no lease, or one
/// that `settle_abandoned` frees (its process gone, no idle lease to give back). The candidates of `reserve`, and the
/// slot freed meanwhile that ends the wait for the clones lock (`lock_clones`)
fn reservable(slot: &Slot, tried: &HashSet<String>) -> bool {
  !tried.contains(&slot.path) && is_free(slot) && !is_unfinished(slot)
}

/// Whether `slot` is free as the next writer settles it (`settle_abandoned`): no lease, or one whose process is gone with
/// no idle lease to give back
fn is_free(slot: &Slot) -> bool {
  slot.lease.as_ref().is_none_or(|lease| lease.taking_over.is_none() && lease.is_abandoned())
}

/// Whether `slot` is one whose add never finished: free (`is_free`), never returned, and without the name that only the
/// end of a get writes (`get_lease`; an adopt writes it in its first record). A get that failed after the registration
/// gave it back free (`give_back`), a killed one left it `preparing`, and its tree can be half written: on 2026-11-22
/// two slots of idea-1 lacked hundreds of thousands of tracked files. No lease had it, so it holds no work. No get
/// reserves it as free, whose `status` would take minutes and find it dirty, and none clones it as a template; the next
/// get that adds a slot removes it (`trim::remove_unfinished_slots`)
pub(crate) fn is_unfinished(slot: &Slot) -> bool {
  slot.name.is_empty() && slot.returned.is_none() && is_free(slot)
}

/// The Bazel servers of `slot` when they are the only processes in it, which the get stops itself; else `None`, and the
/// slot skipped as busy
fn servers_alone(scan: &ProcessScan, slot: &Slot, skipped: &mut Vec<SkippedSlot>) -> Option<Vec<ProcessInfo>> {
  let (servers, others) = slot_servers(scan, Path::new(&slot.path));
  if !others.is_empty() {
    skipped.push(SkippedSlot {
      name: slot.name.clone(),
      path: slot.path.clone(),
      reason: SkipReason::Busy {
        processes: busy_processes(&others),
      },
    });
    return None;
  }
  Some(servers)
}

/// The processes in the slot at `path`: its Bazel servers, which serve only it and which a return, a get and a reclaim
/// stop, and every other process, which keeps it busy. Without a scan (Windows) no other process is known, and the
/// servers are told by the pid file of their output base (`servers_by_output_base`)
pub(crate) fn slot_servers<'a>(scan: &'a ProcessScan, path: &Path) -> (Vec<ProcessInfo>, Vec<&'a ProcessInfo>) {
  if !scan.is_available() {
    return (servers_by_output_base(path), Vec::new());
  }
  let (servers, others): (Vec<&ProcessInfo>, Vec<&ProcessInfo>) = scan.inside(path).into_iter().partition(|process| process.bazel_workspace().is_some());
  (servers.into_iter().cloned().collect(), others)
}

/// Whether the get that tried `tried` may take over the lease of `slot` at `now`: an idle lease (`Lease::is_idle`),
/// quiet (`is_quiet`), of a slot of the repository of `git_common_dir`
fn is_idle_candidate(slot: &Slot, git_common_dir: &Path, tried: &HashSet<String>, now: u64) -> bool {
  slot.lease.as_ref().is_some_and(|idle| idle.is_idle() && is_quiet(slot, idle, now)) && !tried.contains(&slot.path) && is_worktree_of(Path::new(&slot.path), git_common_dir)
}

/// Under the pool lock: the idle lease (`Lease::is_idle`) that was quiet longest (`is_quiet`), of a slot of this
/// repository with no process in it but its Bazel servers, taken by the `preparing` lease of the get, which keeps the
/// idle one as `taking_over`; a busy slot is skipped
fn reserve_idle(
  store: &mut LeaseStore,
  git_common_dir: &Path,
  (scan, gate): (&ProcessScan, &Gate),
  tried: &mut HashSet<String>,
  skipped: &mut Vec<SkippedSlot>,
  lease: &Lease,
) -> Option<Reserved> {
  let now = now();
  let mut order: Vec<usize> = (0..store.slots.len())
    .filter(|&index| {
      let slot = &store.slots[index];
      is_idle_candidate(slot, git_common_dir, tried, now) && gate.lets_idle(&slot.path)
    })
    .collect();
  order.sort_by_key(|&index| store.slots[index].lease.as_ref().map_or(0, |idle| idle.leased_at));
  for index in order {
    let slot = &mut store.slots[index];
    tried.insert(slot.path.clone());
    let Some(servers) = servers_alone(scan, slot, skipped) else {
      continue;
    };
    let old = Box::new(slot.lease.take()?);
    slot.lease = Some(Lease {
      taking_over: Some(old.clone()),
      ..lease.clone()
    });
    return Some(Reserved::TakeOver {
      path: slot.path.clone(),
      name: slot.name.clone(),
      servers,
      old,
    });
  }
  None
}

/// What the check of an idle lease found
enum IdleVerdict {
  /// Every commit of the lease is in its home branch and the tree is clean: `mark` is the slot as the check found it
  Landed { landed: Vec<LandedCommit>, mark: SlotMark },
  /// Work the get leaves alone; `fresh` when this check found it, not the one before (its verdict still holds). With
  /// local changes alone, `landed` holds the commits of the lease that its home branch has, for a preserve that takes
  /// the slot (`idle_changes`)
  Work { check: Box<IdleCheck>, fresh: bool, landed: Vec<LandedCommit> },
}

/// Whether the work of the idle lease `old` of the slot at `path` (the worktree `name`) landed, cheap checks first:
/// the verdict of the check before while it holds (`IdleCheck::holds`, one `rev-parse` of the home tip), then the check
/// in full (`check_idle_work`)
fn check_idle(git_executor: &GitCommandExecutor, (path, name): (&str, &str), old: &Lease, (untracked, preserve): (UntrackedFiles, PreserveConfig)) -> Result<IdleVerdict> {
  let state = IdleState::of(git_executor, path, old)?;
  match state.held(path, old, Some(&preserve.fingerprint())) {
    Some(verdict) => Ok(verdict),
    None => check_idle_work(git_executor, (path, name), old, (untracked, preserve), state),
  }
}

/// The slot of an idle lease and the tip of its home branch, as a check reads them first
struct IdleState {
  mark: SlotMark,
  home_tip: Option<String>,
}

impl IdleState {
  /// The slot at `path` (no git) and the home tip of the idle lease `old` (one `rev-parse`)
  fn of(git_executor: &GitCommandExecutor, path: &str, old: &Lease) -> Result<Self> {
    let home_tip = match &old.source_branch {
      Some(branch) => resolve_commit(git_executor, path, &format!("refs/heads/{branch}"))?,
      None => None,
    };
    Ok(Self {
      mark: SlotMark::of(Path::new(path)),
      home_tip,
    })
  }

  /// The verdict of the check of `old` of the slot at `path` before, while it holds (`IdleCheck::holds`)
  fn held(&self, path: &str, old: &Lease, config: Option<&str>) -> Option<IdleVerdict> {
    old
      .idle_check
      .as_ref()
      .filter(|check| check.holds(Path::new(path), (&self.mark, self.home_tip.as_deref()), now(), config))
      .map(|check| IdleVerdict::Work {
        check: Box::new(check.clone()),
        fresh: false,
        landed: Vec::new(),
      })
  }
}

/// The check in full of the idle lease `old` of the slot `(path, name)` as `state` found it: the commits on git objects
/// only (`landed_work`), a git operation or a resolve session in progress (no git but one `show-ref`), the local
/// changes (one `status`), with their stat proof and when they are quiet by `preserve` (`idle_changes`)
fn check_idle_work(
  git_executor: &GitCommandExecutor,
  (path, name): (&str, &str),
  old: &Lease,
  (untracked, preserve): (UntrackedFiles, PreserveConfig),
  state: IdleState,
) -> Result<IdleVerdict> {
  let IdleState { mark, home_tip } = state;
  let work = |commits: Vec<WorkCommit>, paths: Vec<String>, operation: Option<String>| {
    let (paths, paths_omitted) = cut_paths(paths, 0);
    IdleVerdict::Work {
      check: Box::new(IdleCheck {
        at: now(),
        mark: mark.clone(),
        home_tip: home_tip.clone(),
        commits,
        paths,
        paths_omitted,
        operation,
        proof: Vec::new(),
        ignores: Vec::new(),
        quiet_at: None,
        not_preserved: None,
        preserve_config: None,
        preserve_error: None,
        index: None,
      }),
      fresh: true,
      landed: Vec::new(),
    }
  };
  let landed = landed_work(git_executor, path, &old.source_head, old.source_branch.as_deref(), Purpose::TakeOver, &[])?;
  if !landed.unlanded.is_empty() {
    let commits = landed
      .unlanded
      .into_iter()
      .map(|commit| WorkCommit {
        id: commit.id,
        subject: commit.subject,
      })
      .collect();
    return Ok(work(commits, Vec::new(), None));
  }
  if let Some(operation) = operation_in_progress(git_executor, path, name) {
    return Ok(work(Vec::new(), Vec::new(), Some(operation)));
  }
  let paths = slot_changes(git_executor, path, untracked)?;
  if !paths.is_empty() {
    let slot = Path::new(path);
    // by the paths of the `status` in the mode of the repository: the preserve lists each untracked file again
    let quiet_at = preserve.quiet_at(crate::idle_changes::latest_change(slot, &paths));
    return Ok(IdleVerdict::Work {
      check: Box::new(IdleCheck::of_changes(slot, (mark, home_tip), paths, &preserve, ChangesVerdict::Quiet(quiet_at))),
      fresh: true,
      landed: landed.landed,
    });
  }
  Ok(IdleVerdict::Landed { landed: landed.landed, mark })
}

/// A git operation in progress in the slot at `path` (its git dir, which `status` does not show), or its resolve
/// session (`refs/branch-deck/resolve/<name>`): the next lease would find it half done
fn operation_in_progress(git_executor: &GitCommandExecutor, path: &str, name: &str) -> Option<String> {
  const OPERATIONS: [(&str, &str); 7] = [
    ("rebase-merge", "rebase"),
    ("rebase-apply", "rebase"),
    ("MERGE_HEAD", "merge"),
    ("CHERRY_PICK_HEAD", "cherry-pick"),
    ("REVERT_HEAD", "revert"),
    ("sequencer", "cherry-pick"),
    ("BISECT_LOG", "bisect"),
  ];
  let git_dir = linked_worktree(Path::new(path))?.git_dir;
  if let Some((_, operation)) = OPERATIONS.iter().find(|(file, _)| git_dir.join(file).exists()) {
    return Some((*operation).to_string());
  }
  let session = session_ref(name);
  ref_exists(git_executor, path, &session).then(|| format!("resolve session {session}"))
}

/// The commit point of a takeover, under the pool lock: the slot still has the lease of the get and is as the check
/// found it (`mark`: no commit of a resumed session meanwhile), and the idle lease leaves for the record of the slot
/// (`Slot::reclaimed`). From here a get that dies leaves the slot free, as its work landed. False when the check no
/// longer holds
pub(crate) fn commit_takeover(pool: &Pool, timeout: Duration, path: &str, lease: &Lease, mark: &SlotMark) -> Result<bool> {
  commit_takeover_with(pool, timeout, (path, lease, mark), None)
}

/// `commit_takeover` of the slot at `path` whose local changes the get preserved (`idle_changes`): the record names
/// the patches, and the slot is dirty by `dirty` in the same write, so a get killed or a Bazel server that does not exit
/// before the discard leaves a slot that no get takes and no eviction deletes; the end of the get clears it
fn commit_takeover_with(pool: &Pool, timeout: Duration, (path, lease, mark): (&str, &Lease, &SlotMark), preserved: Option<(&Preserved, &DirtyCheck)>) -> Result<bool> {
  pool.update(timeout, |store| {
    let Some(slot) = leased_slot(store, path, &lease.id) else {
      return Ok(false);
    };
    if SlotMark::of(Path::new(path)) != *mark {
      return Ok(false);
    }
    let Some(old) = slot.lease.as_mut().and_then(|current| current.taking_over.take()) else {
      return Ok(false);
    };
    slot.reclaimed = Some(Reclaimed {
      preserved: preserved.map(|(preserved, _)| preserved.clone()),
      ..Reclaimed::of(&old, Some(lease))
    });
    if let Some((_, dirty)) = preserved {
      mark_dirty(slot, dirty);
    }
    Ok(true)
  })
}

/// `slot` dirty by `check` (`Slot::dirty_check`)
fn mark_dirty(slot: &mut Slot, check: &DirtyCheck) {
  slot.dirty = true;
  slot.dirty_check = Some(check.clone());
}

/// Give the idle lease `old` back to the slot at `path` that the get of `lease_id` reserved, from the copy of the get: a
/// brd before `taking_over` that wrote the state meanwhile dropped the one in the state. Best effort: a get gone before
/// it gives it back leaves it to `settle_abandoned`
fn keep_idle(pool: &Pool, timeout: Duration, path: &str, lease_id: &str, old: Lease) {
  let kept = pool.update(timeout, |store| {
    if let Some(slot) = leased_slot(store, path, lease_id) {
      slot.lease = Some(old);
    }
    Ok(())
  });
  if let Err(error) = kept {
    warn!(slot = %path, error = %format!("{error:#}"), "cannot give the idle lease back");
  }
}

impl Reclaimed {
  /// The record of `old`, taken over by `by` (none: `brd lease reclaim`)
  fn of(old: &Lease, by: Option<&Lease>) -> Self {
    Self {
      lease_id: old.id.clone(),
      holder: old.holder.clone(),
      leased_at: old.leased_at,
      branch: old.branch.clone(),
      source: old.source.clone(),
      source_branch: old.source_branch.clone(),
      at: now(),
      by_lease_id: by.map(|lease| lease.id.clone()),
      by_holder: by.map(|lease| lease.holder.clone()),
      preserved: None,
    }
  }
}

/// The idle lease a get took over
#[derive(Debug, Clone)]
pub struct TookOver {
  /// As it was before the get
  pub lease: Lease,
  /// Its commits that its home branch was proven to have
  pub landed: Vec<LandedCommit>,
  /// Its local changes, preserved in its notes before the get discarded them
  pub preserved: Option<Preserved>,
}

/// The path of the next new slot: `<pool>/<n>/<repository name>`, the smallest `n` whose directory does not exist and
/// holds no slot of the state
/// The number directories of `pool_dir` that a get killed during its clone left: one that no state of a pool of the
/// machine names (`machine`), holding nothing but `<repository>.cloning` (`add_cloned_slot`). The caller holds the
/// clones lock of the repository, so no clone of it runs: such a directory is no slot, and its number is lost to
/// `next_slot_path` for ever. An empty one costs no space and may be a move of `adopt` on its way, so it stays
pub(crate) fn clone_leftovers(pool_dir: &Path, git_common_dir: &Path, machine: &[LeaseStore]) -> Vec<PathBuf> {
  let cloning = format!("{}.cloning", repository_name(git_common_dir));
  let Ok(entries) = std::fs::read_dir(pool_dir) else {
    return Vec::new();
  };
  let mut leftovers: Vec<PathBuf> = entries
    .filter_map(Result::ok)
    .filter(|entry| entry.file_name().to_str().is_some_and(|name| name.parse::<u32>().is_ok()))
    .map(|entry| entry.path())
    .filter(|dir| dir.is_dir() && !machine.iter().any(|store| store.slots.iter().any(|slot| Path::new(&slot.path).starts_with(dir))))
    .filter(|dir| {
      std::fs::read_dir(dir).is_ok_and(|entries| {
        let names: Vec<_> = entries.filter_map(Result::ok).map(|entry| entry.file_name()).collect();
        names.len() == 1 && names[0].to_str() == Some(cloning.as_str())
      })
    })
    .collect();
  leftovers.sort();
  leftovers
}

/// Move the leftovers of killed clones of the pool (`clone_leftovers`) into the trash of this run, under the clones lock,
/// and leave that trash to a later sweep (`Trash::abandon`): the delete of a clone of idea-1 is 1.4 million files, which
/// the next get below the minimum, a return or a clean deletes outside the lock. Told as a step when there is one; a
/// move that fails leaves its directory and is logged
fn sweep_clone_leftovers(pool: &Pool, params: &LeaseGetParams<'_>, (pool_dir, git_common_dir): (&Path, &Path), report: &dyn Fn(GetStep)) {
  let machine: Vec<LeaseStore> = machine_pools(params.source, params.home)
    .iter()
    .filter_map(|machine| Pool::new(&machine.git_common_dir).read().ok())
    .collect();
  let leftovers = clone_leftovers(pool_dir, git_common_dir, &machine);
  if leftovers.is_empty() {
    return;
  }
  report(GetStep::SweepCloneLeftovers {
    dirs: leftovers.iter().map(|dir| display_path(dir)).collect(),
  });
  let trash = Trash::new(&pool.trash_dir());
  for dir in &leftovers {
    if let Err(error) = trash.move_in(dir) {
      warn!(dir = %dir.display(), error = %error, "cannot move the leftover of a killed clone into the trash");
    }
  }
  trash.abandon();
}

fn next_slot_path(store: &LeaseStore, pool_dir: &Path, git_common_dir: &Path) -> PathBuf {
  let leaf = repository_name(git_common_dir);
  (1..=u32::MAX)
    .map(|number| pool_dir.join(number.to_string()))
    .find(|dir| !dir.exists() && !store.slots.iter().any(|slot| Path::new(&slot.path).starts_with(dir)))
    .map_or_default(|dir| dir.join(&leaf))
}

/// Whether a new slot can be a clone: APFS `clonefile(2)` is macOS only
const CLONE_AVAILABLE: bool = cfg!(target_os = "macos");

/// The id of the `templating` marker of the free slot that the get of `lease_id` clones
fn template_marker(lease_id: &str) -> String {
  format!("{lease_id}-template")
}

/// Under the pool lock: mark a free slot of this repository `templating` for the clone of a new slot, one without a
/// process in it first, and among those a clean one before a dirty one, so the clone starts with the least to undo;
/// `None` without one, and the source checkout is the template. The free slots left here are the ones the get passed
/// over (dirty or busy) and none that a get ahead in the queue has yet to try: the checkout and the clean of the clone
/// make them right
fn mark_template(store: &mut LeaseStore, git_common_dir: &Path, (scan, gate): (&ProcessScan, &Gate), lease: &Lease) -> Option<String> {
  let index = (0..store.slots.len())
    .filter(|&index| {
      let slot = &store.slots[index];
      slot.lease.is_none() && !is_unfinished(slot) && gate.lets_free(&slot.path) && is_worktree_of(Path::new(&slot.path), git_common_dir)
    })
    // the first of the best: no process, then clean
    .min_by_key(|&index| (!scan.inside(Path::new(&store.slots[index].path)).is_empty(), store.slots[index].dirty))?;
  let slot = &mut store.slots[index];
  slot.lease = Some(Lease {
    id: template_marker(&lease.id),
    holder: format!("template of {}", lease.id),
    state: LeaseState::Templating,
    branch: None,
    seeded: Vec::new(),
    ..lease.clone()
  });
  Some(slot.path.clone())
}

/// Free the template `path` of its marker `marker_id`, best effort: an abandoned marker is freed by the next get
pub(crate) fn release_marker(pool: &Pool, timeout: Duration, path: &str, marker_id: &str) {
  let released = pool.update(timeout, |store| {
    if let Some(slot) = leased_slot(store, path, marker_id) {
      slot.lease = None;
    }
    Ok(())
  });
  if let Err(error) = released {
    warn!(template = %path, error = %error, "cannot release the template");
  }
}

/// A new slot to add
struct NewSlot<'a> {
  /// Its `worktree` commands run one at a time (`Pool::worktree_command`)
  pool: &'a Pool,
  lock_timeout: Duration,
  path: &'a Path,
  source: &'a RepositoryDirs,
  /// A free slot or the source checkout
  template: &'a str,
  commit: &'a str,
  branch: Option<&'a str>,
  workers: CheckoutWorkers,
  /// The pointers when the smudge filter of Git LFS downloads (`LfsConfig::smudge`): the checkout runs under the
  /// clones lock, and the get pulls the content after its release where the repository keeps objects of LFS
  lfs: LfsFiles,
  untracked: UntrackedFiles,
  /// What the clone leaves out: the `delete` rules of the source
  skips: &'a CloneSkips,
  /// Hears each step as it starts
  progress: &'a dyn Fn(GetStep),
}

/// The index of the new slot at `path`, whose `git lfs pull` failed, deleted: its files of Git LFS are pointers still,
/// which a later move by the diff would keep as the content. Without an index the next get finds the slot dirty, as
/// after a checkout that failed, and never reuses it
fn forget_slot_index(path: &str) {
  let Some(worktree) = linked_worktree(Path::new(path)) else { return };
  if let Err(error) = fs_retry::remove_file(worktree.git_dir.join("index")) {
    warn!(slot = path, "cannot delete the index of a slot whose files of Git LFS are pointers: {error}");
  }
}

/// Why a new slot failed
enum Added {
  /// Nothing was registered: the slot leaves the state
  NotRegistered(anyhow::Error),
  /// git knows the worktree: the slot stays as a free one, and the next get checks it
  Registered(anyhow::Error),
}

/// Bring the clean reused `slot` to `commit` by the diff of the two commits (`move_clean_slot`), then onto the new
/// `branch`; every file (`checkout_slot`) only when git refused the diff, which `report` tells with git's reason
pub(crate) fn move_reused_slot(
  git_executor: &GitCommandExecutor,
  slot: &str,
  commit: &str,
  branch: Option<&str>,
  workers: CheckoutWorkers,
  report: &dyn Fn(GetStep),
) -> Result<()> {
  // a slot returned by a build that left what a lease keeps in its git directory (`lease_state`: the mark of a sync, the
  // landed state): the new lease is a worktree no sync built anything for
  if let Err(error) = forget_lease_state(Path::new(slot)) {
    warn!(slot, "cannot take what the lease before left out of the slot: {error}");
  }
  match move_clean_slot(git_executor, slot, commit, workers)? {
    SlotMove::Kept | SlotMove::Switched => branch.map_or(Ok(()), |branch| attach_new_branch(git_executor, slot, branch, commit)),
    SlotMove::Refused(reason) => {
      report(GetStep::CheckoutEveryFile {
        slot: slot.to_string(),
        reason: reason.lines().next().unwrap_or_default().to_string(),
      });
      checkout_slot(git_executor, slot, commit, branch, workers, LfsFiles::Content)
    }
  }
}

/// Add the new slot of `request`: a clone of its template when the volume clones (`add_cloned_slot`), else a
/// registration under the lock of the pool's `worktree` commands (`register_slot`), then the parallel checkout of
/// every file without it
fn add_new_slot(git_executor: &GitCommandExecutor, request: &NewSlot<'_>, release_template: &dyn Fn()) -> Result<CreatedBy, Added> {
  if CLONE_AVAILABLE {
    match add_cloned_slot(git_executor, request, release_template) {
      Ok(()) => return Ok(CreatedBy::Clone),
      Err(Cloned::Unavailable(error)) => debug!(template = %request.template, error = %error, "no clone, checking out instead"),
      Err(Cloned::Failed(error)) => return Err(error),
    }
  }
  let repository = &request.source.top_level;
  let slot = display_path(request.path);
  (request.progress)(GetStep::Checkout {
    slot: slot.clone(),
    commit: request.commit.to_string(),
  });
  request
    .pool
    .worktree_command(request.lock_timeout, || register_slot(git_executor, repository, request.path, request.commit, false))
    .map_err(Added::NotRegistered)?;
  checkout_slot(git_executor, &slot, request.commit, request.branch, request.workers, request.lfs).map_err(Added::Registered)?;
  Ok(CreatedBy::Checkout)
}

/// Why a clone failed
enum Cloned {
  /// `clonefile(2)` refused (another volume, no APFS, an unreadable file): check out instead
  Unavailable(std::io::Error),
  Failed(Added),
}

/// A new slot as an APFS clone of `request.template`, then the diff to `request.commit`:
/// 1. `clonefile(2)` of the template to `<path>.cloning`, without what the `delete` rules of the source delete, then the
///    template is released;
/// 2. `worktree add --no-checkout`, its `.git` file moved over the cloned `.git` (a directory when the template is a
///    main checkout), `<path>.cloning` renamed to `<path>`;
/// 3. the template's index copied in, with the stat data of the cloned files, and refreshed (`refresh_cloned_index`):
///    the checkout of step 4 then writes only the files that differ;
/// 4. `checkout --force` to the commit, then its untracked files removed (`clean_cloned_slot`), whose last `status`
///    must find no change: the changes and the untracked files of the template never reach the lease, and its ignored
///    files stay as the warm cache. A tracked file under a skipped path comes back with the checkout
fn add_cloned_slot(git_executor: &GitCommandExecutor, request: &NewSlot<'_>, release_template: &dyn Fn()) -> Result<(), Cloned> {
  let path = request.path;
  let slot = display_path(path);
  let cloning = PathBuf::from(format!("{}.cloning", path.display()));
  let started = Instant::now();
  (request.progress)(GetStep::Clone {
    template: request.template.to_string(),
    slot: slot.clone(),
  });
  let cloned = clone_directory(Path::new(request.template), &cloning, request.skips);
  release_template();
  if let Err(error) = cloned {
    let _ = fs_retry::remove_dir_all(&cloning);
    return Err(Cloned::Unavailable(error));
  }
  debug!(template = %request.template, elapsed_ms = started.elapsed().as_millis(), "cloned the template");
  let failed = |error: anyhow::Error| {
    let _ = fs_retry::remove_dir_all(&cloning);
    Cloned::Failed(Added::NotRegistered(error))
  };
  let repository = &request.source.top_level;
  request
    .pool
    .worktree_command(request.lock_timeout, || register_slot(git_executor, repository, path, request.commit, true))
    .map_err(failed)?;
  let registered = |error: anyhow::Error| Cloned::Failed(Added::Registered(error));
  let swapped = swap_git_link(path, &cloning);
  // the lock of the registration kept a `worktree prune` beside this get (the user's, a gc's) from deleting the admin
  // directory while `path` was away; a slot left as a free one must be prunable again once its directory is gone
  let unlocked = request.pool.worktree_command(request.lock_timeout, || unlock_slot(git_executor, repository, path));
  swapped.map_err(|error| {
    let _ = fs_retry::remove_dir_all(&cloning);
    registered(error)
  })?;
  unlocked.map_err(registered)?;
  let worktree = linked_worktree(path).ok_or_else(|| registered(anyhow!("{slot} is not a linked worktree after its clone")))?;
  let template_git_dir = if request.template == request.source.top_level {
    PathBuf::from(&request.source.git_dir)
  } else {
    linked_worktree(Path::new(request.template)).map_or_else(|| PathBuf::from(&request.source.git_dir), |template| template.git_dir)
  };
  // without the index of the template the checkout writes every file: slow, still right. Its mtime is read before the
  // copy: a write of the template after it only makes more entries racy
  let template_index = template_git_dir.join("index");
  let template_written = template_index.metadata().and_then(|metadata| metadata.modified()).ok();
  match std::fs::copy(&template_index, worktree.git_dir.join("index")) {
    Ok(_) => {
      (request.progress)(GetStep::Refresh { slot: slot.clone() });
      refresh_cloned_index(git_executor, &slot, &worktree.git_dir, template_written, request.commit).map_err(registered)?;
    }
    Err(error) => warn!(template = %request.template, error = %error, "no index of the template to copy"),
  }
  debug!(elapsed_ms = started.elapsed().as_millis(), "refreshed the index of the clone");
  (request.progress)(GetStep::Checkout {
    slot: slot.clone(),
    commit: request.commit.to_string(),
  });
  checkout_slot(git_executor, &slot, request.commit, request.branch, request.workers, request.lfs).map_err(registered)?;
  // the `status` of the clean lists the untracked files and the changes that are left, so no second `status` walks
  // the tree
  (request.progress)(GetStep::Status { slot: slot.clone() });
  let changes = clean_cloned_slot(git_executor, &slot, request.untracked).map_err(registered)?;
  if !changes.is_empty() {
    return Err(registered(anyhow!(
      "The clone of {} kept {} local change(s), e.g. {}",
      request.template,
      changes.len(),
      changes[0]
    )));
  }
  debug!(elapsed_ms = started.elapsed().as_millis(), "checked out the clone");
  Ok(())
}

/// Give the clone at `cloning` the `.git` file that `worktree add --no-checkout` wrote at `path`, then move the clone to
/// `path`. The cloned `.git` of the template goes first: a file of a linked worktree, or the whole git directory of a
/// main checkout (its packs are clones too, so the removal is cheap: 0.2 s for the 2,079 files of idea-1)
fn swap_git_link(path: &Path, cloning: &Path) -> Result<()> {
  let cloned_git = cloning.join(".git");
  match cloned_git.symlink_metadata() {
    Ok(metadata) if metadata.is_dir() => fs_retry::remove_dir_all(&cloned_git)?,
    Ok(_) => fs_retry::remove_file(&cloned_git)?,
    Err(_) => {}
  }
  fs_retry::rename(path.join(".git"), &cloned_git).with_context(|| format!("Cannot move the .git file of {}", path.display()))?;
  fs_retry::remove_dir(path).with_context(|| format!("Cannot remove the empty {}", path.display()))?;
  fs_retry::rename(cloning, path).with_context(|| format!("Cannot move the clone to {}", path.display()))?;
  Ok(())
}

/// Whether `dir` is the top level of a linked worktree whose common dir is `git_common_dir` (canonical)
fn is_worktree_of(dir: &Path, git_common_dir: &Path) -> bool {
  linked_worktree(dir).is_some_and(|worktree| display_path(&worktree.git_common_dir) == display_path(git_common_dir))
}

/// The slot at `path` whose lease is `lease_id`
fn leased_slot<'a>(store: &'a mut LeaseStore, path: &str, lease_id: &str) -> Option<&'a mut Slot> {
  let index = leased_index(store, path, lease_id)?;
  store.slots.get_mut(index)
}

/// The index of the slot of `path` whose lease is `lease_id`
fn leased_index(store: &LeaseStore, path: &str, lease_id: &str) -> Option<usize> {
  store
    .slots
    .iter()
    .position(|slot| slot.path == path && slot.lease.as_ref().is_some_and(|lease| lease.id == lease_id))
}

/// Give the slot of a failed get back: free it, or drop it from the state when it was never added (`remove`). The
/// error says so
fn give_back(pool: &Pool, timeout: Duration, path: &str, lease_id: &str, remove: bool, error: anyhow::Error) -> anyhow::Error {
  let released = pool.update(timeout, |store| {
    if remove {
      store
        .slots
        .retain(|slot| !(slot.path == path && slot.lease.as_ref().is_some_and(|lease| lease.id == lease_id)));
    } else if let Some(slot) = leased_slot(store, path, lease_id) {
      // the idle lease it took over, before the commit point of the takeover
      slot.lease = slot.lease.take().and_then(|lease| lease.taking_over).map(|old| *old);
      return Ok(is_unfinished(slot));
    }
    Ok(false)
  });
  match released {
    Ok(true) => error.context(format!(
      "The lease was rolled back: the add of {path} never finished, and the next get that adds a slot removes it"
    )),
    Ok(false) => error.context(format!("The lease was rolled back: {path} is free again")),
    Err(release_error) => error.context(format!("The lease of {path} could not be rolled back ({release_error:#}): the next get frees it")),
  }
}

/// The ignored paths that `.worktreeinclude` at the root of `source` names (`seed_candidates`), one `check-ignore`: a
/// tracked or not ignored file is never seeded
fn seed_paths(git_executor: &GitCommandExecutor, source: &str) -> Result<Vec<String>> {
  let Ok(text) = std::fs::read_to_string(Path::new(source).join(WORKTREE_INCLUDE)) else {
    return Ok(Vec::new());
  };
  let candidates = seed_candidates(Path::new(source), &text);
  ignored_paths(git_executor, source, &candidates)
}

/// The paths under `root` that the lines of a `.worktreeinclude` name, without a walk of the tree: a literal path
/// (`/.claude/settings.local.json`, a file or a directory), or a glob of `*`, `?` and `[…]` in its last component
/// (`/config/*.local.json`), listed from that one directory. A pattern is always relative to the root, also without a
/// leading `/`: unlike in `.gitignore` it never matches at every depth. `#` comments, `!` negations, `**` and a glob in
/// a directory component are left out
pub fn seed_candidates(root: &Path, text: &str) -> Vec<String> {
  let mut paths = Vec::new();
  for line in text.lines() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
      continue;
    }
    let pattern = line.trim_start_matches('/').trim_end_matches('/');
    if !is_relative_path(pattern) || pattern.contains("**") {
      continue;
    }
    let (dir, leaf) = pattern.rsplit_once('/').unwrap_or(("", pattern));
    if has_glob(dir) {
      continue;
    }
    if !has_glob(leaf) {
      if root.join(pattern).symlink_metadata().is_ok() {
        paths.push(pattern.to_string());
      }
      continue;
    }
    let Ok(entries) = std::fs::read_dir(root.join(dir)) else { continue };
    let mut names: Vec<String> = entries
      .flatten()
      .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
      .filter(|name| name != ".git" && glob_matches(leaf, name))
      .collect();
    names.sort();
    paths.extend(names.into_iter().map(|name| if dir.is_empty() { name } else { format!("{dir}/{name}") }));
  }
  let mut seen = HashSet::new();
  paths.retain(|path| seen.insert(path.clone()));
  paths
}

/// A path of normal components only, none of them `.git`
fn is_relative_path(path: &str) -> bool {
  !path.is_empty() && !path.contains('\\') && Path::new(path).components().all(|component| matches!(component, Component::Normal(name) if name != ".git"))
}

fn has_glob(text: &str) -> bool {
  text.contains(['*', '?', '['])
}

/// Whether `name` matches the glob `pattern`: `*` any run of characters, `?` one, `[abc]`, `[a-z]` and `[!a]` (or
/// `[^a]`) one of a set; a `[` without its `]` is itself
pub fn glob_matches(pattern: &str, name: &str) -> bool {
  let pattern: Vec<char> = pattern.chars().collect();
  let name: Vec<char> = name.chars().collect();
  matches_from(&pattern, &name)
}

fn matches_from(pattern: &[char], name: &[char]) -> bool {
  match pattern.first() {
    None => name.is_empty(),
    Some('*') => (0..=name.len()).any(|skip| matches_from(&pattern[1..], &name[skip..])),
    Some('?') => !name.is_empty() && matches_from(&pattern[1..], &name[1..]),
    Some('[') => match (class_end(pattern), name.first()) {
      (Some(end), Some(&c)) => class_matches(&pattern[1..end], c) && matches_from(&pattern[end + 1..], &name[1..]),
      (Some(_), None) => false,
      (None, _) => name.first() == Some(&'[') && matches_from(&pattern[1..], &name[1..]),
    },
    Some(&c) => name.first() == Some(&c) && matches_from(&pattern[1..], &name[1..]),
  }
}

/// The index of the `]` that closes the class `pattern` starts with; a `]` right after `[` or `[!` belongs to the set
fn class_end(pattern: &[char]) -> Option<usize> {
  let mut index = 1;
  if matches!(pattern.get(index), Some('!' | '^')) {
    index += 1;
  }
  if pattern.get(index) == Some(&']') {
    index += 1;
  }
  pattern[index..].iter().position(|&c| c == ']').map(|offset| index + offset)
}

fn class_matches(class: &[char], c: char) -> bool {
  let (negated, class) = match class.first() {
    Some('!' | '^') => (true, &class[1..]),
    _ => (false, class),
  };
  let mut found = false;
  let mut index = 0;
  while index < class.len() {
    if class.get(index + 1) == Some(&'-') && index + 2 < class.len() {
      found |= (class[index]..=class[index + 2]).contains(&c);
      index += 3;
    } else {
      found |= class[index] == c;
      index += 1;
    }
  }
  found != negated
}

/// Copy each of `paths` from `source` into `slot`: a file, or a directory with its files and directories (symlinks are
/// left out); an existing file is replaced. Returns the paths copied
fn seed(source: &Path, slot: &Path, paths: &[String]) -> Result<Vec<String>> {
  let mut seeded = Vec::new();
  for path in paths {
    let from = source.join(path);
    let Ok(metadata) = from.symlink_metadata() else { continue };
    let to = slot.join(path);
    if metadata.is_dir() {
      copy_dir(&from, &to)?;
    } else if metadata.is_file() {
      if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
      }
      std::fs::copy(&from, &to).with_context(|| format!("Cannot copy {} to {}", from.display(), to.display()))?;
    } else {
      continue;
    }
    seeded.push(path.clone());
  }
  Ok(seeded)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
  std::fs::create_dir_all(to).with_context(|| format!("Cannot create {}", to.display()))?;
  for entry in std::fs::read_dir(from)?.flatten() {
    let file_type = entry.file_type()?;
    let target = to.join(entry.file_name());
    if file_type.is_dir() {
      copy_dir(&entry.path(), &target)?;
    } else if entry.metadata()?.is_file() {
      std::fs::copy(entry.path(), &target).with_context(|| format!("Cannot copy {} to {}", entry.path().display(), target.display()))?;
    }
  }
  Ok(())
}

/// Remove the seeded `paths` from `slot`, each a file or a directory with what is in it, moved into `trash` as a whole
/// (`remove_together`; a link is deleted in place): what a process keeps stays and fails the return, never half removed.
/// A path that is gone already is no error. Returns the paths removed
fn remove_seeded(slot: &Path, paths: &[String], trash: &Trash) -> Result<Vec<String>> {
  let mut removed = Vec::new();
  for path in paths.iter().filter(|path| is_relative_path(path)) {
    let target = slot.join(path);
    if target.symlink_metadata().is_err() {
      continue;
    }
    let mut moved = Vec::new();
    remove_together(std::slice::from_ref(&target), CleanMode::Trash(trash), &mut moved).with_context(|| format!("Cannot remove the seeded {}", target.display()))?;
    trash.adopt(moved);
    removed.push(path.clone());
  }
  Ok(removed)
}

pub struct LeaseReturnParams<'a> {
  /// The slot, as git names its worktree
  pub workspace: &'a RepositoryDirs,
  /// The lease the caller holds; another one refuses
  pub lease_id: Option<&'a str>,
  /// The caller named no worktree (`brd lease return --lease-id` without `--workspace`): when `workspace` holds no lease
  /// or another one, the slot of `lease_id` is returned, so a coordinator returns the lease of another session from any
  /// checkout of the repository
  pub find_by_lease_id: bool,
  /// Give up the unlanded commits and discard the local changes
  pub confirm_preserved: bool,
  /// End the commands of the session that holds the lease in the slot (`HolderProcesses`), once nothing else refuses
  /// the return, and check the slot again without them; a process of another session still refuses as `slot_busy`
  pub stop_own: bool,
  /// How long the return waits for the Bazel servers it asked to exit
  pub server_timeout: Duration,
  /// The program of `bazel shutdown`, which stops the servers on Windows; `None`: `bazel` of the `PATH`
  pub bazel: Option<&'a OsStr>,
  /// Apply the clean rules of `.branchdeck.toml`, with these overrides; `None` (`--no-clean`) applies none
  pub clean: Option<CleanOverrides>,
  pub lock_timeout: Duration,
  /// How long a cargo rule waits for the locks of cargo of a profile dir that another clean holds
  /// (`lease_clean::RETURN_CARGO_LOCK_WAIT`); after it the profile dir stays as it is and the slot is free all the same
  pub cargo_lock_wait: Duration,
  /// The commits of the range of the pick of `brd lease pick --return`, which landed them on the home branch or found
  /// them there in the same process just before: they are landed by that pick (`landed_work`), and no proof runs for
  /// them. Empty for any other return
  pub picked: &'a [String],
}

/// What a return did
#[derive(Debug, Clone)]
pub struct LeaseReturned {
  /// The slot with the lease it had
  pub slot: Slot,
  /// The local changes `--confirm-preserved` discarded
  pub discarded: Vec<String>,
  /// The unlanded commits `--confirm-preserved` gave up; they stay in the reflog of the slot's HEAD
  pub given_up: Vec<UnlandedCommit>,
  /// The commits of the slot no branch, tag or remote holds that the home branch was proven to have (`landed_work`)
  pub landed: Vec<LandedCommit>,
  /// Which check decided what is unlanded; none for a return that joined another before it checked anything
  pub landed_check: Option<LandedCheck>,
  /// Another return of the same lease (a retry) returned it: this one changed nothing, and its lists are empty
  pub joined: bool,
  /// The Bazel servers of the slot that exited when the return asked them (SIGTERM; `bazel shutdown` on Windows)
  pub stopped_servers: Vec<StoppedServer>,
  /// The commands of the session that holds the lease that `--stop-own` ended: the processes of their trees that the
  /// scan saw in the slot
  pub stopped_processes: Vec<BusyProcess>,
  /// git's background maintenance in the slot, which does not keep it busy
  pub maintenance: Vec<u32>,
  /// `branchdeck.leaseMaintenance` is true, and the return started `git maintenance run --auto --detach` once the slot
  /// was free
  pub maintenance_started: bool,
  /// What the trash kept: a path that could not be deleted once the slot was free
  pub trash_left: Vec<String>,
  /// The seeded paths removed
  pub removed: Vec<String>,
  /// The branch the slot was on; it stays
  pub detached_branch: Option<String>,
  /// What each clean rule of `.branchdeck.toml` did
  pub cleaned: Vec<Cleaned>,
  /// Why the rules could not be read; the return succeeded anyway
  pub clean_error: Option<String>,
  /// The notes of the lease, which stay (`lease_notes`); `None` for a lease without them
  pub notes: Option<LeaseNotes>,
  pub process_scan: bool,
}

/// Give the lease of a slot back. Refused while a process other than `brd` and its ancestors is in the slot, while HEAD
/// has commits that no branch, tag or remote branch holds (not counting those of the commit it was leased at), and
/// while the tree has local changes; `confirm_preserved`
/// gives up the commits and discards the changes (`reset --hard`, `clean -fd`: the ignored files stay). Then the seeded
/// paths go, a branch is detached (it stays) and the slot is free; it is not moved, the next get moves it
#[instrument(skip_all, fields(workspace = %params.workspace.top_level))]
///
/// A repeat joins: while another return of the same lease runs (a retry after a timeout), this one waits for it and
/// answers `joined` once it freed the slot, or returns the lease itself once it failed; a return that another return
/// of its lease overtook answers `joined` too and changes nothing
///
/// A host slot (`host_slots`: its holder is `brd-host/<n>`, or the lend table names its lease) is refused (`HostSlot`):
/// the agent of the host lends it to its runs, and only a shrink (`brd-host slots --local --shrink`) gives it back
pub fn lease_return_core(git_executor: &GitCommandExecutor, params: &LeaseReturnParams<'_>) -> Result<LeaseReturned> {
  return_lease(git_executor, params, ReturnAs::Hand)
}

/// The return of a host slot by `host_slots::shrink`, which a hand return is refused. The shrink checked the slot
/// before: `config` is its config and its changes are discarded, so the return runs no `status` of its own. What the
/// return removes goes into `trash` and stays there, in `cleaned`: the shrink deletes it once every slot it returns is
/// free
pub(crate) fn return_host_slot(git_executor: &GitCommandExecutor, params: &LeaseReturnParams<'_>, config: &LeaseConfig, trash: &Trash) -> Result<LeaseReturned> {
  return_lease(git_executor, params, ReturnAs::HostSlot { config, trash })
}

/// Who returns a lease
#[derive(Clone, Copy)]
enum ReturnAs<'a> {
  /// `brd lease return`: a host slot is refused
  Hand,
  /// The shrink of the host slots, with the config it read of the clean slot and the trash it deletes
  HostSlot { config: &'a LeaseConfig, trash: &'a Trash },
}

fn return_lease(git_executor: &GitCommandExecutor, params: &LeaseReturnParams<'_>, returning_as: ReturnAs<'_>) -> Result<LeaseReturned> {
  let mut path = canonical_path(Path::new(&params.workspace.top_level));
  let pool = Pool::new(Path::new(&canonical_path(Path::new(&params.workspace.git_common_dir))));
  if let (true, Some(lease_id)) = (params.find_by_lease_id, params.lease_id)
    && let Some(slot) = slot_of_lease(&pool, &path, lease_id)?
  {
    path = slot;
  }
  // the slot with the lease of the return this one waited for
  let mut waited_for: Option<Slot> = None;
  // what `--stop-own` ended; it ends the commands of the holder once, so a pass after it never ends any
  let mut stopped: Vec<BusyProcess> = Vec::new();
  let mut stops_holders = params.stop_own;
  loop {
    let mut pass = Pass { stops_holders, checked: None };
    let step = match return_once(git_executor, params, returning_as, &pool, &path, waited_for.as_ref(), &mut pass) {
      Ok(step) => step,
      // a check refused the slot after another return of the lease gave it back (a get may have leased it again):
      // the refusal is of the next lease, the lease of this return was returned
      Err(error) => match pass.checked.filter(|slot| !holds_lease(&pool, slot).unwrap_or(true)) {
        Some(slot) => {
          return Ok(LeaseReturned {
            stopped_processes: stopped,
            ..joined_return(slot, None, false)
          });
        }
        None => return Err(stopped_before(error, &stopped)),
      },
    };
    match step {
      ReturnStep::Done(returned) => {
        return Ok(LeaseReturned {
          stopped_processes: stopped,
          ..*returned
        });
      }
      ReturnStep::WaitFor(slot) => {
        if let Some(lease) = &slot.lease {
          wait_while_freeing(&pool, &path, &lease.id, &mut |_| {})?;
        }
        waited_for = Some(*slot);
      }
      ReturnStep::StopHolders { roots, processes } => {
        stop_trees(&roots);
        stopped.extend(processes);
        stops_holders = false;
      }
    }
  }
}

/// What one pass of a return is given, and what it tells the loop of `return_lease`
struct Pass {
  /// Whether it ends the commands of the holder in the slot once nothing else refuses: `--stop-own`, and no pass before
  /// it ended them
  stops_holders: bool,
  /// The slot with the lease it checks, once no other return of it runs
  checked: Option<Slot>,
}

/// A refusal of a pass after `--stop-own` ended the commands of the holder: it names them, as the refusal no longer
/// changed nothing
fn stopped_before(error: anyhow::Error, stopped: &[BusyProcess]) -> anyhow::Error {
  if stopped.is_empty() {
    return error;
  }
  error.context(format!("`--stop-own` ended {}", describe_processes(stopped)))
}

/// Kills each tree of the commands of the holder in the slot, its root last (`kill_running_tree`); a process whose pid
/// another process got since is never signalled. A tree that a kill is not sure of is a warning: the next pass of the
/// return scans the slot again and refuses what is left
fn stop_trees(roots: &[(u32, u64)]) {
  #[cfg(unix)]
  for &(pid, started) in roots {
    let report = sync_utils::process_tree::kill_running_tree(pid, started);
    if report.settled() {
      debug!(pid, rounds = ?report.rounds, "ended a command of the holder in the slot");
    } else {
      warn!(pid, rounds = ?report.rounds, unsettled = ?report.unsettled, left = ?report.left, "a kill of a command of the holder is not sure of its tree");
    }
  }
  // no process scan, so no command of the holder is known in the slot
  #[cfg(not(unix))]
  debug!(roots = roots.len(), "no process scan, so no command of the holder to end in the slot");
}

/// The processes of a slot but its Bazel servers, by whose they are: the commands of the session that holds the lease,
/// seen by a `brd` of that session, and every other process
struct HolderProcesses<'a> {
  /// The commands of the holder: a process that runs below the holder process of the lease (`holder_pid` with
  /// `holder_started`) by the chain of its parents, with a start the scan read, in a tree whose root has no controlling
  /// terminal. A coding agent runs each command in a session of its own without one; an interactive shell of a terminal
  /// tab, of tmux or of the terminal of an IDE has one, and is the user's
  holders: Vec<&'a ProcessInfo>,
  /// The root of each tree of `holders`: no other of them is above it, below the holder
  roots: Vec<&'a ProcessInfo>,
  /// Every other process: of another session, an orphan, or seen by a `brd` that the holder does not run (a coordinator,
  /// a resumed session, which runs as another process)
  others: Vec<&'a ProcessInfo>,
}

impl<'a> HolderProcesses<'a> {
  fn of(scan: &ProcessScan, lease: &Lease, processes: Vec<&'a ProcessInfo>) -> Self {
    let holder = lease
      .holder_pid
      .zip(lease.holder_started)
      .filter(|&(pid, started)| scan.has_ancestor(pid, Some(started)) == Some(true));
    let Some(holder) = holder else {
      return Self {
        holders: Vec::new(),
        roots: Vec::new(),
        others: processes,
      };
    };
    // the processes between each one and the holder, for one below it with a known start
    let paths: Vec<Option<Vec<u32>>> = processes
      .iter()
      .map(|process| {
        process.started?;
        Some(scan.path_to(process.pid, holder)?.iter().map(|between| between.pid).collect())
      })
      .collect();
    let below: HashMap<u32, &ProcessInfo> = processes
      .iter()
      .zip(&paths)
      .filter(|(_, path)| path.is_some())
      .map(|(process, _)| (process.pid, *process))
      .collect();
    let mut split = Self {
      holders: Vec::new(),
      roots: Vec::new(),
      others: Vec::new(),
    };
    for (process, path) in processes.into_iter().zip(paths) {
      let Some(path) = path else {
        split.others.push(process);
        continue;
      };
      // the farthest from it of the processes of the slot below the holder on its chain, or itself
      let root = path.iter().rev().find_map(|pid| below.get(pid).copied()).unwrap_or(process);
      if root.terminal {
        split.others.push(process);
        continue;
      }
      if root.pid == process.pid {
        split.roots.push(process);
      }
      split.holders.push(process);
    }
    split
  }

  /// The processes of the slot as a `slot_busy` names them, the commands of the holder marked
  fn busy(&self, inside: &[&ProcessInfo]) -> Vec<BusyProcess> {
    inside
      .iter()
      .map(|process| BusyProcess {
        holder: self.holders.iter().any(|holder| holder.pid == process.pid),
        ..BusyProcess::of(process)
      })
      .collect()
  }
}

/// The slot that holds the lease `lease_id`, unless the slot of `path` does (or no slot does): the pool is only read
fn slot_of_lease(pool: &Pool, path: &str, lease_id: &str) -> Result<Option<String>> {
  let store = pool.read()?;
  let holds = |slot: &Slot| slot.lease.as_ref().is_some_and(|lease| lease.id == lease_id);
  if store.slots.iter().any(|slot| slot.path == path && holds(slot)) {
    return Ok(None);
  }
  Ok(store.slots.iter().find(|slot| holds(slot)).map(|slot| slot.path.clone()))
}

/// What one pass of a return ends with
enum ReturnStep {
  Done(Box<LeaseReturned>),
  /// Another return of the lease runs: wait for it, then look again
  WaitFor(Box<Slot>),
  /// `--stop-own`, and nothing else refused: end the trees of these roots (pid and start), then look again; `processes`
  /// are the commands of the holder of those trees in the slot
  StopHolders {
    roots: Vec<(u32, u64)>,
    processes: Vec<BusyProcess>,
  },
}

/// The answer of a return whose lease another return of it gave back: nothing changed
const fn joined_return(slot: Slot, landed_check: Option<LandedCheck>, process_scan: bool) -> LeaseReturned {
  LeaseReturned {
    slot,
    discarded: Vec::new(),
    given_up: Vec::new(),
    landed: Vec::new(),
    landed_check,
    joined: true,
    stopped_servers: Vec::new(),
    stopped_processes: Vec::new(),
    maintenance: Vec::new(),
    maintenance_started: false,
    trash_left: Vec::new(),
    removed: Vec::new(),
    detached_branch: None,
    cleaned: Vec::new(),
    clean_error: None,
    notes: None,
    process_scan,
  }
}

/// Whether the slot still has the lease of `slot`
fn holds_lease(pool: &Pool, slot: &Slot) -> Result<bool> {
  let store = pool.read()?;
  Ok(
    store
      .slots
      .iter()
      .any(|current| current.path == slot.path && current.lease.as_ref().map(|lease| &lease.id) == slot.lease.as_ref().map(|lease| &lease.id)),
  )
}

/// Until the return or the clean of `lease_id` in `path` ends: it freed the slot, it failed (`ready` again) or its
/// process died. No lock of the state is held: it is only read, and `look` hears each read
fn wait_while_freeing(pool: &Pool, path: &str, lease_id: &str, look: &mut dyn FnMut(&LeaseStore)) -> Result<()> {
  loop {
    let store = pool.read()?;
    look(&store);
    let returning = store
      .slots
      .iter()
      .filter(|slot| slot.path == path)
      .filter_map(|slot| slot.lease.as_ref())
      .any(|lease| lease.id == lease_id && matches!(lease.state, LeaseState::Returning | LeaseState::Cleaning) && !lease.is_abandoned());
    if !returning {
      return Ok(());
    }
    std::thread::sleep(Duration::from_millis(20));
  }
}

/// One pass of a return (`Pass`)
fn return_once(
  git_executor: &GitCommandExecutor,
  params: &LeaseReturnParams<'_>,
  returning_as: ReturnAs<'_>,
  pool: &Pool,
  path: &str,
  waited_for: Option<&Slot>,
  pass: &mut Pass,
) -> Result<ReturnStep> {
  let path = path.to_string();
  let mut store = pool.read()?;
  // as the next writer settles it: a get gone while it took the lease over gave it back
  store.slots.iter_mut().for_each(settle_abandoned_takeover);
  let current = store
    .slots
    .iter()
    .find_map(|slot| slot.lease.clone().filter(|_| slot.path == path).map(|lease| (slot.clone(), lease)));
  // the return it waited for freed the slot (a get may have leased it again): that return was this one's
  if let Some(waited) = waited_for
    && let Some(waited_lease) = &waited.lease
    && current.as_ref().is_none_or(|(_, lease)| lease.id != waited_lease.id)
  {
    return Ok(ReturnStep::Done(Box::new(joined_return(waited.clone(), None, false))));
  }
  let (slot, lease) = current.ok_or_else(|| LeaseRefusal::NoLease(path.clone()))?;
  if let Some(expected) = params.lease_id
    && expected != lease.id
  {
    bail!(LeaseRefusal::LeaseMismatch {
      path,
      expected: expected.to_string(),
      actual: lease.id,
    });
  }
  if matches!(returning_as, ReturnAs::Hand)
    && let Some(n) = host_slot_number(Path::new(&params.workspace.git_common_dir), &lease)?
  {
    bail!(LeaseRefusal::HostSlot { path, n });
  }
  match refusal_to_return(&path, &lease) {
    Some(LeaseRefusal::Returning { .. }) => return Ok(ReturnStep::WaitFor(Box::new(slot))),
    Some(refusal) => bail!(refusal),
    None => {}
  }
  pass.checked = Some(slot.clone());
  let scan = ProcessScan::scan();
  let inside = scan.inside(Path::new(&path));
  let maintenance = scan.maintenance_inside(Path::new(&path));
  // the Bazel servers of the slot, which serve only it; every other process keeps the slot busy, but the commands of the
  // holder when the pass ends them
  let (servers, others) = slot_servers(&scan, Path::new(&path));
  let split = HolderProcesses::of(&scan, &lease, others);
  if !split.others.is_empty() || (!split.holders.is_empty() && !pass.stops_holders) {
    bail!(LeaseRefusal::SlotBusy {
      path,
      processes: split.busy(&inside),
    });
  }
  // a resolve session of the slot (`brd lease pick` opens one onto its home): the next lease of the slot would find it
  let session = session_ref(&slot.name);
  if ref_exists(git_executor, &path, &session) {
    bail!(LeaseRefusal::ResolveInProgress { path, session });
  }
  let work = landed_work(git_executor, &path, &lease.source_head, lease.source_branch.as_deref(), Purpose::Return, params.picked)?;
  let (unlanded, landed_check) = (work.unlanded, work.check);
  if !unlanded.is_empty() && !params.confirm_preserved {
    bail!(LeaseRefusal::UnlandedCommits {
      path,
      commits: unlanded,
      check: landed_check,
    });
  }
  let read_config;
  let (config, changes) = match returning_as {
    ReturnAs::Hand => {
      read_config = lease_config(git_executor, &path)?;
      let changes = slot_changes(git_executor, &path, read_config.untracked)?;
      (&read_config, changes)
    }
    // the shrink discarded the changes of the slot it checked
    ReturnAs::HostSlot { config, .. } => (config, Vec::new()),
  };
  if !changes.is_empty() && !params.confirm_preserved {
    bail!(LeaseRefusal::DirtyWorktree { path, paths: changes });
  }
  // `--stop-own`, and nothing else refuses: the commands of the holder end now, and the next pass checks the slot again
  // without them, with what they wrote or committed before they ended
  if !split.roots.is_empty() {
    return Ok(ReturnStep::StopHolders {
      roots: split.roots.iter().filter_map(|root| Some((root.pid, root.started?))).collect(),
      processes: split
        .holders
        .iter()
        .map(|process| BusyProcess {
          holder: true,
          ..BusyProcess::of(process)
        })
        .collect(),
    });
  }
  // only now, when nothing else refuses: a server that does not exit keeps the slot busy
  let servers: Vec<&ProcessInfo> = servers.iter().collect();
  let ServerStop {
    stopped: stopped_servers,
    running,
  } = stop_servers(&servers, params.bazel, params.server_timeout);
  if !running.is_empty() {
    bail!(LeaseRefusal::ServersRunning { path, processes: running });
  }
  // from here on the return acts on the slot: reserved under the pool lock, while the lease is still the one it
  // checked, so a retried return beside it, or one that another return overtook and whose slot a get leased meanwhile,
  // changes nothing
  match reserve_return(pool, params.lock_timeout, &path, &lease.id)? {
    Reservation::Reserved => {}
    Reservation::Returning => return Ok(ReturnStep::WaitFor(Box::new(slot))),
    Reservation::Returned => return Ok(ReturnStep::Done(Box::new(joined_return(slot, Some(landed_check), scan.is_available())))),
  }
  let own_trash;
  let (trash, deletes_trash) = match returning_as {
    ReturnAs::Hand => {
      own_trash = Trash::new(&pool.trash_dir());
      (&own_trash, true)
    }
    ReturnAs::HostSlot { trash, .. } => (trash, false),
  };
  let returned = finish_return(
    git_executor,
    &path,
    &lease,
    (!changes.is_empty()).then_some(config.untracked),
    (params.clean, params.cargo_lock_wait),
    (trash, &pool.notes_roots(None, None)),
  );
  let FinishedReturn {
    removed,
    detached_branch,
    mut cleaned,
    clean_error,
    notes,
  } = match returned {
    Ok(finished) => finished,
    Err(error) => {
      // the lease stays the holder's, `ready` again, for a return once the error is fixed
      let restored = pool.update(params.lock_timeout, |store| {
        if let Some(lease) = leased_slot(store, &path, &lease.id).and_then(|slot| slot.lease.as_mut()) {
          lease.state = LeaseState::Ready;
        }
        Ok(())
      });
      if let Err(restore_error) = restored {
        warn!(slot = %path, error = %format!("{restore_error:#}"), "cannot give the lease back after a failed return");
      }
      // what went into the trash before the error goes for good (the seeded paths)
      if deletes_trash {
        trash.empty(&mut []);
      }
      return Err(error);
    }
  };
  pool.update(params.lock_timeout, |store| {
    let index = leased_index(store, &path, &lease.id).ok_or_else(|| anyhow!("The lease of {path} changed while it was returned"))?;
    stamp_returned(store, index, now());
    let slot = &mut store.slots[index];
    slot.lease = None;
    slot.dirty = false;
    slot.dirty_check = None;
    Ok(())
  })?;
  // the slot is free. With `maintenance.auto` false no commit in a slot starts git's maintenance, whose repack held a
  // slot of idea-1 for 2 to 17 minutes: with `branchdeck.leaseMaintenance` the return starts it now, in the common
  // dir, before the long delete of the trash that a timeout of the caller may cut short. It never fails the return
  let maintenance_started = config.return_maintenance
    && start_deferred_maintenance(git_executor, &params.workspace.git_common_dir)
      .inspect_err(|error| warn!(error = %format!("{error:#}"), "cannot start git's maintenance after the return"))
      .is_ok();
  let trash_left = if deletes_trash { trash.empty(&mut cleaned) } else { Vec::new() };
  Ok(ReturnStep::Done(Box::new(LeaseReturned {
    slot,
    discarded: changes,
    given_up: unlanded,
    landed: work.landed,
    landed_check: Some(landed_check),
    joined: false,
    stopped_servers,
    stopped_processes: Vec::new(),
    maintenance,
    maintenance_started,
    trash_left,
    removed,
    detached_branch,
    cleaned,
    clean_error,
    notes,
    process_scan: scan.is_available(),
  })))
}

/// Why `lease` of the slot `path` can't be returned now: its get or another return of it runs
fn refusal_to_return(path: &str, lease: &Lease) -> Option<LeaseRefusal> {
  if lease.state == LeaseState::Ready || lease.is_abandoned() {
    return None;
  }
  let (path, pid) = (path.to_string(), lease.pid);
  Some(match lease.state {
    LeaseState::Returning => LeaseRefusal::Returning { path, pid },
    _ => LeaseRefusal::Preparing { path, pid },
  })
}

/// What the reservation of a return found
enum Reservation {
  /// The lease of `path` is `returning`, of this process
  Reserved,
  /// Another return of the lease runs
  Returning,
  /// The slot has another lease or none: another return of the lease freed it (a get may have leased it again)
  Returned,
}

/// Under the pool lock, with CAS on the lease id: mark the lease of `path` `returning`, of this process, unless another
/// return of it runs or freed it
fn reserve_return(pool: &Pool, timeout: Duration, path: &str, lease_id: &str) -> Result<Reservation> {
  let pid = std::process::id();
  pool.update(timeout, |store| {
    let slot = store.slots.iter_mut().find(|slot| slot.path == path);
    let current = slot.and_then(|slot| {
      settle_abandoned_takeover(slot);
      slot.lease.as_mut()
    });
    let Some(current) = current.filter(|current| current.id == lease_id) else {
      return Ok(Reservation::Returned);
    };
    match refusal_to_return(path, current) {
      Some(LeaseRefusal::Returning { .. }) => return Ok(Reservation::Returning),
      Some(refusal) => bail!(refusal),
      None => {}
    }
    current.state = LeaseState::Returning;
    current.pid = pid;
    current.process_started = process_started(pid);
    Ok(Reservation::Reserved)
  })
}

/// What a reserved return did to the slot
struct FinishedReturn {
  removed: Vec<String>,
  detached_branch: Option<String>,
  cleaned: Vec<Cleaned>,
  clean_error: Option<String>,
  notes: Option<LeaseNotes>,
}

/// What a reserved return does to the slot: discard the changes it may give up (`discard`: the untracked mode of the
/// repository, when it found any), remove the seeded paths, detach HEAD, take the notes of the lease out of the slot
/// into the place of `notes_roots` that has them (`lease_notes::close_notes`) and apply the clean rules, moving what a
/// `delete` removes into `trash`. A cargo rule waits up to `cargo_lock_wait` for a held lock of cargo, then leaves that
/// profile dir as it is
fn finish_return(
  git_executor: &GitCommandExecutor,
  path: &str,
  lease: &Lease,
  discard: Option<UntrackedFiles>,
  (clean, cargo_lock_wait): (Option<CleanOverrides>, Duration),
  (trash, notes_roots): (&Trash, &NotesRoots),
) -> Result<FinishedReturn> {
  // the next lease of the slot is a worktree no sync built anything for (`lease_state`: the mark of a sync, the landed
  // state): a window that opens it shows the Ready to Sync card, never syncs it of itself, and nothing landed is
  // reported of a branch it never had. First, so a return, a takeover or a reclaim that fails after it leaves none
  // either (a takeover that fails gives the slot back, free). As the clean rules, it never fails the return
  if let Err(error) = forget_lease_state(Path::new(path)) {
    warn!(slot = %path, "cannot take what the lease left out of the slot: {error}");
  }
  // the next holder of the slot starts with an empty `notes.local`; the notes stay. Before the discard, whose `clean`
  // would delete a directory of notes that `info/exclude` no longer ignores. Never fails the return
  let notes = close_notes(notes_roots, Path::new(path), &lease.id);
  if let Some(untracked) = discard {
    discard_slot_changes(git_executor, path, untracked)?;
  }
  let removed = remove_seeded(Path::new(path), &lease.seeded, trash)?;
  let detached_branch = read_head(git_executor, path)?.and_then(|(_, branch)| branch);
  if detached_branch.is_some() {
    detach_waiting_for_head_lock(git_executor, path)?;
  }
  // the rules of the source, as the seed is read; the slot's own when the source is gone. They never fail the return
  let (cleaned, clean_error) = match clean.map(|overrides| read_clean_rules(lease_rules_dir(lease, path)).map(|rules| overrides.apply(rules))) {
    None => (Vec::new(), None),
    Some(Ok(rules)) => (
      apply_clean_rules_with(
        git_executor,
        path,
        &rules,
        CleanMode::Trash(trash),
        CleanOptions {
          cargo_lock_wait,
          ..CleanOptions::default()
        },
      ),
      None,
    ),
    Some(Err(error)) => (Vec::new(), Some(format!("{error:#}"))),
  };
  Ok(FinishedReturn {
    removed,
    detached_branch,
    cleaned,
    clean_error,
    notes,
  })
}

/// The checkout whose `.branchdeck.toml` holds the clean rules of `lease` of the slot `path`: the source the lease
/// recorded, as the seed is read; the slot's own when the source is gone
pub(crate) fn lease_rules_dir<'a>(lease: &'a Lease, path: &'a str) -> &'a Path {
  Some(Path::new(&lease.source))
    .filter(|source| !lease.source.is_empty() && source.is_dir())
    .unwrap_or_else(|| Path::new(path))
}

/// What the prune of a leased slot did (`prune_leased_slot`)
#[derive(Debug, Clone)]
pub(crate) struct LeasedPrune {
  /// What each cargo rule did
  pub(crate) cleaned: Vec<Cleaned>,
  /// The profile dirs it left for a held lock of cargo, when it pruned none: cargo builds in the slot
  pub(crate) building: Vec<String>,
}

/// The cargo rules of the source of `lease` (`lease_rules_dir`) on its leased slot `path`, with their own `keep` and
/// `age`, never its `delete` rules, which name the files of its holder (`notes.local`, test results). Each profile dir
/// is pruned under the locks of cargo, taken first, and only while the slot, read then under the pool lock, still holds
/// `lease` in state `ready`: a return or a get that came between them keeps it. No wait for a held lock: cargo builds
/// there. A prune that pruned a profile dir stamps the lease (`Lease::pruned_at`). `None`: the lease changed first
pub(crate) fn prune_leased_slot(
  git_executor: &GitCommandExecutor,
  pool: &Pool,
  lock_timeout: Duration,
  (path, lease): (&str, &Lease),
  mode: CleanMode<'_>,
) -> Result<Option<LeasedPrune>> {
  let rules = read_clean_rules(lease_rules_dir(lease, path))?;
  let still_holds = || {
    pool
      .update(lock_timeout, |store| {
        Ok(leased_slot(store, path, &lease.id).and_then(|slot| slot.lease.as_ref()).is_some_and(Lease::is_agent_ready))
      })
      .unwrap_or_else(|error| {
        warn!(slot = %path, error = %format!("{error:#}"), "cannot read the lease of a slot to prune");
        false
      })
  };
  let options = CleanOptions {
    cargo_only: true,
    still_holds: Some(&still_holds),
    ..CleanOptions::default()
  };
  let cleaned = apply_clean_rules_with(git_executor, path, &rules, mode, options);
  let pruned: usize = cleaned.iter().map(|rule| rule.profiles.pruned).sum();
  if pruned == 0 && cleaned.iter().any(|rule| rule.profiles.changed) {
    return Ok(None);
  }
  if mode.applies() && pruned > 0 {
    let stamped = pool.update(lock_timeout, |store| {
      if let Some(current) = leased_slot(store, path, &lease.id).and_then(|slot| slot.lease.as_mut()) {
        current.pruned_at = Some(now());
      }
      Ok(())
    });
    if let Err(error) = stamped {
      warn!(slot = %path, error = %format!("{error:#}"), "cannot stamp the prune of a leased slot");
    }
  }
  let building = if pruned == 0 {
    cleaned.iter().flat_map(|rule| rule.profiles.held.clone()).collect()
  } else {
    Vec::new()
  };
  Ok(Some(LeasedPrune { cleaned, building }))
}

/// Detach HEAD of the slot; while git's background maintenance holds `HEAD.lock` of the slot (`pack-refs`, `reflog
/// expire` of a `gc --auto`, which the return does not wait for), retried for up to `HEAD_LOCK_WAIT`. The lock is told
/// by its file, never by the words of git's error
fn detach_waiting_for_head_lock(git_executor: &GitCommandExecutor, path: &str) -> Result<()> {
  let head_lock = linked_worktree(Path::new(path)).map(|worktree| worktree.git_dir.join("HEAD.lock"));
  let deadline = Instant::now() + HEAD_LOCK_WAIT;
  loop {
    match detach_slot_head(git_executor, path) {
      Ok(()) => return Ok(()),
      Err(error) if head_lock.as_ref().is_some_and(|lock| lock.exists()) && Instant::now() < deadline => {
        debug!(slot = %path, error = %format!("{error:#}"), "HEAD.lock of the slot is held, retrying the detach");
        std::thread::sleep(Duration::from_millis(100));
      }
      Err(error) => return Err(error),
    }
  }
}

/// A Bazel server of a slot that the return stopped
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoppedServer {
  pub pid: u32,
  /// Its `--workspace_directory`: the slot or a directory in it (`community/`)
  pub workspace: String,
  /// Its `--output_base`, which stays on disk: the next lease of the slot gets the same one, with its disk cache
  pub output_base: Option<String>,
}

/// How a return, a get and a reclaim ask a Bazel server to exit, as their refusal names it
const SERVER_STOP: &str = if cfg!(windows) { "`bazel shutdown`" } else { "SIGTERM" };

/// What `stop_servers` did with the Bazel servers of a slot
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ServerStop {
  /// The servers that are gone
  pub stopped: Vec<StoppedServer>,
  /// The servers that still run, each with the `stop_error` of a `bazel shutdown` that could not run or failed
  pub running: Vec<BusyProcess>,
}

/// Ask each Bazel server to exit and wait up to `timeout` for them. On macOS and Linux with SIGTERM; on Windows, which
/// has no signals, with `bazel shutdown` for its output base (`bazel_servers::shutdown`, through `bazel`, else `bazel`
/// of the `PATH`), the shutdowns side by side, each with the whole `timeout`: one after the other, a slow shutdown left
/// the next one only the rest of it. A `bazel shutdown` that could not run or failed is not waited for:
/// its server, unless gone anyway, runs on with the error as its `stop_error`. No other process is ever signalled: the
/// caller passes only processes with `--workspace_directory` in the slot, and each is asked only while its pid still
/// has the start time of the scan, checked right before its `kill`. Between the scan and the signal a server can exit
/// and the system can give its pid to an unrelated process. A server without a known start time is never signalled and
/// stays running, so the slot stays busy
pub(crate) fn stop_servers(servers: &[&ProcessInfo], bazel: Option<&OsStr>, timeout: Duration) -> ServerStop {
  let deadline = Instant::now() + timeout;
  let running = |server: &ProcessInfo| server.started.is_none_or(|started| is_running(server.pid, Some(started)));
  // the servers to ask, each checked right before it is asked (lazily)
  let asked = || {
    servers.iter().copied().filter(|server| {
      let Some(started) = server.started else {
        debug!(pid = server.pid, "a Bazel server without a start time is not signalled");
        return false;
      };
      let asks = is_running(server.pid, Some(started));
      if asks {
        debug!(pid = server.pid, argv = ?server.argv, "stopping a Bazel server of the slot");
      }
      asks
    })
  };
  // the servers whose `bazel shutdown` could not run or failed, with why
  let failed: Vec<(u32, String)> = if cfg!(windows) {
    std::thread::scope(|scope| {
      #[expect(clippy::needless_collect, reason = "every shutdown starts before the first one is joined")]
      let shutdowns: Vec<_> = asked()
        .map(|server| (server, scope.spawn(in_current_span(move || shutdown(bazel, server, timeout)))))
        .collect();
      shutdowns
        .into_iter()
        .filter_map(|(server, shutdown)| {
          let error = shutdown.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)).err()?;
          warn!(pid = server.pid, argv = ?server.argv, error = %error, "cannot stop a Bazel server of the slot");
          Some((server.pid, error))
        })
        .collect()
    })
  } else {
    for server in asked() {
      terminate(server.pid);
    }
    Vec::new()
  };
  let stop_error = |server: &ProcessInfo| failed.iter().find(|(pid, _)| *pid == server.pid).map(|(_, error)| error.clone());
  while servers.iter().any(|server| stop_error(server).is_none() && running(server)) && Instant::now() < deadline {
    std::thread::sleep(Duration::from_millis(50));
  }
  let (left, gone): (Vec<&ProcessInfo>, Vec<&ProcessInfo>) = servers.iter().copied().partition(|server| running(server));
  ServerStop {
    stopped: gone
      .into_iter()
      .map(|server| StoppedServer {
        pid: server.pid,
        workspace: server.bazel_workspace().map_or_default(|workspace| display_path(&workspace)),
        output_base: server.output_base().map(str::to_string),
      })
      .collect(),
    running: left
      .into_iter()
      .map(|server| BusyProcess {
        stop_error: stop_error(server),
        ..BusyProcess::of(server)
      })
      .collect(),
  }
}

/// How `brd lease status` sees a slot, from the state alone; its JSON is the snake case name (`free`, `ready`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SlotState {
  Free,
  Preparing,
  Ready,
  /// A free slot that a get clones into a new one
  Templating,
  /// Its return runs
  Returning,
  /// A free slot that `brd lease clean` cleans
  Cleaning,
  /// A lease that is not `ready` and whose process is gone: the next get takes the slot as free, or removes it when its
  /// add never finished
  Abandoned,
  /// A free slot whose add never finished (`is_unfinished`): no get takes it, and the next get that adds a slot removes it
  Unfinished,
}

/// A slot of `brd lease status`
#[derive(Debug, Clone)]
pub struct SlotStatus {
  pub slot: Slot,
  pub state: SlotState,
  /// The directory is gone
  pub missing: bool,
  /// With `processes`: the processes in it, except the session of `brd`, git's maintenance and `caffeinate`
  pub processes: Option<Vec<BusyProcess>>,
  /// A `ready` lease of an agent whose holder is gone (`Lease::is_idle`): what it is to the pool
  pub idle: Option<IdleStatus>,
}

/// Which slots `lease_status_core` scans for processes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusScan {
  /// No slot: the state alone
  None,
  /// The slots of the leases whose holder is gone, with one scan only when there is one: a slot in which a process of
  /// another session works is `IdleStatus::Busy`
  Idle,
  /// Every slot (`brd lease status --processes`), the idle ones as `Idle` scans them too
  All,
}

/// What a lease whose holder process is gone is to the pool
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdleStatus {
  /// No check found work in it yet: a get takes it over once its work landed and it was quiet for an hour
  Idle,
  /// A check found work its home branch lacks, which no get touches
  WithWork(IdleCheck),
  /// Processes of another session work in the slot: the session of the lease lives, as a resumed session runs as
  /// another process. Its Bazel servers alone keep no slot busy, as a get stops them itself. `check` is the work a
  /// check found, if any
  Busy { processes: Vec<BusyProcess>, check: Option<IdleCheck> },
}

impl IdleStatus {
  /// The status of an idle lease with the last check that found work in it (`Lease::idle_check`) and the processes of
  /// its slot that are no Bazel server of it (the `others` of `slot_servers`)
  fn of(check: Option<IdleCheck>, others: &[&ProcessInfo]) -> Self {
    match check {
      _ if !others.is_empty() => Self::Busy {
        processes: busy_processes(others),
        check,
      },
      None => Self::Idle,
      Some(check) => Self::WithWork(check),
    }
  }
}

/// Where the commits of a lease go (`brd lease pick`): the source checkout of its get and the branch it had checked out
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseHome {
  /// The source checkout, its top level; empty when the state does not name it (a lease of an older bd)
  pub checkout: String,
  /// The branch checked out there at the get; `None` for a detached HEAD, where no pick can land
  pub branch: Option<String>,
}

impl Lease {
  pub fn home(&self) -> LeaseHome {
    LeaseHome {
      // git writes the source with `/`; the form of a top level, backslashes on Windows
      checkout: if self.source.is_empty() {
        String::new()
      } else {
        canonical_path(Path::new(&self.source))
      },
      branch: self.source_branch.clone(),
    }
  }
}

/// The slot whose top level is `top_level` when its lease is ready, from the state, no git; `None` outside a leased slot
pub fn ready_slot_at(git_common_dir: &Path, top_level: &str) -> Result<Option<Slot>> {
  let top_level = canonical_path(Path::new(top_level));
  let (slots, _) = lease_status_core(git_common_dir, StatusScan::None)?;
  Ok(
    slots
      .into_iter()
      .find(|status| status.state == SlotState::Ready && status.slot.path == top_level && status.slot.lease.is_some())
      .map(|status| status.slot),
  )
}

/// Why a command of `brd` in the worktree `top_level` must not run: the slot's lease was taken over from a dead holder
/// (`Slot::reclaimed`) or is being taken over, and this process is no process of the session that holds it now, as a
/// resumed session of the old holder is not. From the state, no git; a process scan only in such a slot. A command that
/// `mutates` nothing passes but while a get takes the lease over. `lease_id`, the id a command names, passes when it is
/// the current lease's (a coordinator returns it). Not refused: a slot that no takeover touched, and a holder that is
/// gone too or a platform without a scan, where the two sessions can't be told apart
pub fn reclaimed_refusal(git_common_dir: &Path, top_level: &str, mutates: bool, lease_id: Option<&str>) -> Result<Option<LeaseRefusal>> {
  let path = canonical_path(Path::new(top_level));
  let store = Pool::new(Path::new(&canonical_path(git_common_dir))).read()?;
  let Some(slot) = store.slots.into_iter().find(|slot| slot.path == path) else {
    return Ok(None);
  };
  let refusal = |reclaimed: Reclaimed, in_progress: bool| {
    Some(LeaseRefusal::LeaseReclaimed {
      path: path.clone(),
      reclaimed: Box::new(reclaimed),
      in_progress,
    })
  };
  if let Some(lease) = &slot.lease
    && let Some(old) = lease.taking_over.as_ref().filter(|_| !lease.is_abandoned())
  {
    return Ok(refusal(Reclaimed::of(old, Some(lease)), true));
  }
  let Some(reclaimed) = slot.reclaimed else {
    return Ok(None);
  };
  if !mutates {
    return Ok(None);
  }
  match &slot.lease {
    None => Ok(refusal(reclaimed, false)),
    Some(lease) if lease_id == Some(lease.id.as_str()) => Ok(None),
    Some(lease) if lease.state == LeaseState::Ready && lease.holder_alive() == Some(true) => {
      let holder = lease.holder_pid.unwrap_or_default();
      let scan = ProcessScan::scan();
      if scan.has_ancestor(holder, lease.holder_started) == Some(false) {
        Ok(refusal(reclaimed, false))
      } else {
        Ok(None)
      }
    }
    Some(_) => Ok(None),
  }
}

/// The `brd` that may become the holder of the lease of its slot (`rebind_holder`)
pub struct Rebinder<'a> {
  /// The working directory of `brd`: a `brd` that `-C` points into the slot from outside (a coordinator) rebinds
  /// nothing
  pub cwd: &'a Path,
  /// The wait for the pool lock
  pub lock_timeout: Duration,
  /// The ancestors of `brd`, read only once the holder is gone: `Ancestors::of_this_process`, and a list of a test
  pub ancestors: &'a dyn Fn() -> Option<Ancestors>,
}

impl<'a> Rebinder<'a> {
  /// A `brd` in `cwd` among the processes of this system
  pub fn of_system(cwd: &'a Path, lock_timeout: Duration) -> Self {
    Self {
      cwd,
      lock_timeout,
      ancestors: &Ancestors::of_this_process,
    }
  }
}

/// The holder a rebind recorded for a lease
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rebound {
  pub lease_id: String,
  pub holder_pid: u32,
  pub holder_started: u64,
}

/// Whether the lease of `slot` may get a new holder: `ready`, of an agent (no host slot), its holder gone, no get
/// taking it over, and no `reclaimed` record in the slot, as a resumed session of a lease a takeover took would else
/// take the lease of another session and pass `reclaimed_refusal`
fn rebindable(slot: &Slot) -> Option<&Lease> {
  slot
    .lease
    .as_ref()
    .filter(|lease| lease.is_idle() && lease.taking_over.is_none() && slot.reclaimed.is_none())
}

/// The session of the `brd` of `ancestors` that may hold a lease (`Ancestors::session`, with its start), unless a
/// process between them owns a terminal: it leads its session and has a controlling terminal, as the shell that a tab of
/// a terminal, a pane of tmux or the terminal of an IDE starts, so a person runs `brd`. A command shell of an agent that
/// only inherits the terminal of its TUI leads no session, and the session itself may have a terminal
pub fn rebind_session(ancestors: &Ancestors) -> Option<(u32, u64)> {
  if ancestors.below_session()?.iter().any(|process| process.terminal && process.session_leader) {
    return None;
  }
  let session = ancestors.session()?;
  Some((session.pid, session.started?))
}

/// Make the session of the `brd` of `rebinder` the holder of the lease of the slot `top_level` (`holder_pid`,
/// `holder_started`), when its holder is gone (`rebindable`), `brd` runs inside the slot, and its session runs it as
/// an agent runs a command (`rebind_session`): a resumed session runs as another process, and without it the lease
/// stays idle while the session works. The state is read first, and the scan and the pool lock come only for a gone
/// holder; the write is a compare-and-swap of the lease under the lock. `None`: nothing changed
pub fn rebind_holder(git_common_dir: &Path, top_level: &str, rebinder: &Rebinder<'_>) -> Result<Option<Rebound>> {
  let path = canonical_path(Path::new(top_level));
  let pool = Pool::new(Path::new(&canonical_path(git_common_dir)));
  let store = pool.read()?;
  let Some(lease_id) = store.slots.iter().find(|slot| slot.path == path).and_then(rebindable).map(|lease| lease.id.clone()) else {
    return Ok(None);
  };
  if !Path::new(&canonical_path(rebinder.cwd)).starts_with(&path) {
    return Ok(None);
  }
  let Some((holder_pid, holder_started)) = (rebinder.ancestors)().as_ref().and_then(rebind_session) else {
    return Ok(None);
  };
  pool.update(rebinder.lock_timeout, |store| {
    let Some(slot) = store.slots.iter_mut().find(|slot| slot.path == path) else {
      return Ok(None);
    };
    if rebindable(slot).is_none_or(|lease| lease.id != lease_id) {
      return Ok(None);
    }
    let Some(lease) = slot.lease.as_mut() else { return Ok(None) };
    lease.holder_pid = Some(holder_pid);
    lease.holder_started = Some(holder_started);
    Ok(Some(Rebound {
      lease_id,
      holder_pid,
      holder_started,
    }))
  })
}

/// `rebind_holder` before a command of `brd` that writes, by the processes of this system: a failure is a warning, and
/// the command goes on
pub fn rebind_before_write(git_common_dir: &Path, top_level: &str, cwd: &Path, lock_timeout: Duration) {
  match rebind_holder(git_common_dir, top_level, &Rebinder::of_system(cwd, lock_timeout)) {
    Ok(Some(rebound)) => tracing::info!(lease_id = %rebound.lease_id, holder_pid = rebound.holder_pid, "this session became the holder of the lease of its slot"),
    Ok(None) => {}
    Err(error) => warn!("Cannot make this session the holder of the lease: {error:#}"),
  }
}

/// Every slot of the pool from the state, no git, and the process scan that `scan` asks: one scan of the processes at
/// most, and none for `StatusScan::Idle` without an idle lease. The second value says whether the platform has a scan,
/// for `StatusScan::All`
pub fn lease_status_core(git_common_dir: &Path, scan: StatusScan) -> Result<(Vec<SlotStatus>, bool)> {
  lease_status_with(git_common_dir, scan, &ProcessScan::scan)
}

/// `lease_status_core` with the scan of the processes `process_scan`: `ProcessScan::scan`, and a counted one of a test
pub fn lease_status_with(git_common_dir: &Path, scan: StatusScan, process_scan: &dyn Fn() -> ProcessScan) -> Result<(Vec<SlotStatus>, bool)> {
  let mut store = Pool::new(Path::new(&canonical_path(git_common_dir))).read()?;
  // a get gone while it took an idle lease over gave it back: the state as the next writer settles it
  store.slots.iter_mut().for_each(settle_abandoned_takeover);
  let idle: Vec<bool> = store.slots.iter().map(|slot| slot.lease.as_ref().is_some_and(Lease::is_idle)).collect();
  let processes = scan == StatusScan::All;
  let scan = if processes || (scan == StatusScan::Idle && idle.contains(&true)) {
    process_scan()
  } else {
    ProcessScan::Unavailable
  };
  let slots = store
    .slots
    .into_iter()
    .zip(idle)
    .map(|(slot, idle)| {
      let state = match &slot.lease {
        _ if is_unfinished(&slot) => SlotState::Unfinished,
        None => SlotState::Free,
        Some(lease) if lease.is_abandoned() => SlotState::Abandoned,
        Some(lease) if lease.state == LeaseState::Preparing => SlotState::Preparing,
        Some(lease) if lease.state == LeaseState::Templating => SlotState::Templating,
        Some(lease) if lease.state == LeaseState::Returning => SlotState::Returning,
        Some(lease) if lease.state == LeaseState::Cleaning => SlotState::Cleaning,
        Some(_) => SlotState::Ready,
      };
      let missing = !Path::new(&slot.path).is_dir();
      let processes = processes.then(|| busy_processes(&scan.inside(Path::new(&slot.path))));
      // without a scan no other process is known: `slot_servers` would read the pid files of the output bases
      let idle = idle.then(|| {
        let others = if scan.is_available() {
          slot_servers(&scan, Path::new(&slot.path)).1
        } else {
          Vec::new()
        };
        IdleStatus::of(slot.lease.as_ref().and_then(|lease| lease.idle_check.clone()), &others)
      });
      SlotStatus {
        slot,
        state,
        missing,
        processes,
        idle,
      }
    })
    .collect();
  Ok((slots, scan.is_available()))
}

/// What `brd lease clean` cleans
pub struct LeaseCleanParams<'a> {
  /// The checkout brd runs in: its `.branchdeck.toml` holds the rules, and without `free` and `dirs` it is the target
  pub repository: &'a RepositoryDirs,
  /// The free slots of the pool
  pub free: bool,
  /// The leased slots of the pool, each by the cargo rules of its lease (`prune_leased_slot`)
  pub leased: bool,
  /// Worktrees to clean
  pub dirs: &'a [PathBuf],
  /// Delete, the default; false (`--dry-run`) only measures
  pub apply: bool,
  /// The parameters of this call that change the rules of the file
  pub overrides: CleanOverrides,
  /// The wait for the pool lock, for the `cleaning` marker of a slot
  pub lock_timeout: Duration,
}

/// A checkout `brd lease clean` cleaned, or skipped
#[derive(Debug, Clone)]
pub struct CleanTarget {
  pub path: String,
  /// Why it was not cleaned: a lease, a process in it
  pub skipped: Option<String>,
  /// The holder of its lease: only its cargo rules ran, with their own `keep` and `age` (`prune_leased_slot`)
  pub leased_by: Option<String>,
  pub cleaned: Vec<Cleaned>,
  /// What the trash kept: a path that could not be deleted
  pub trash_left: Vec<String>,
}

/// Apply the rules of `.branchdeck.toml` of the repository, read once, to the free slots, to `dirs`, or to the
/// repository itself. A free slot of the pool with a process in it is skipped, as a get would skip it; a checkout that is
/// no slot is cleaned as asked. A slot with a `ready` lease of an agent, named or by `leased`, gets only the cargo rules
/// of its lease with their own `keep` and `age` (`prune_leased_slot`), and is skipped with an override; a slot with
/// another lease is skipped. The rules that can't be read are the error, and nothing is cleaned
pub fn lease_clean_core(git_executor: &GitCommandExecutor, params: &LeaseCleanParams<'_>) -> Result<(Vec<CleanTarget>, bool)> {
  let rules = params.overrides.apply(read_clean_rules(Path::new(&params.repository.top_level))?);
  let pool = Pool::new(Path::new(&canonical_path(Path::new(&params.repository.git_common_dir))));
  let store = pool.read()?;
  let mut paths: Vec<String> = Vec::new();
  let listed = |leased: bool| {
    store
      .slots
      .iter()
      .filter(move |slot| slot.lease.as_ref().map_or(!leased, |lease| leased && lease.is_agent_ready()) && Path::new(&slot.path).is_dir())
      .map(|slot| slot.path.clone())
  };
  if params.free {
    paths.extend(listed(false));
  }
  if params.leased {
    paths.extend(listed(true));
  }
  paths.extend(params.dirs.iter().map(|dir| canonical_path(dir)));
  if !params.free && !params.leased && params.dirs.is_empty() {
    paths.push(canonical_path(Path::new(&params.repository.top_level)));
  }
  let mut seen = HashSet::new();
  paths.retain(|path| seen.insert(path.clone()));
  let scan = ProcessScan::scan();
  let marker = Lease::marker("clean", "brd lease clean", LeaseState::Cleaning);
  let trash = Trash::new(&pool.trash_dir());
  let mode = if params.apply { CleanMode::Trash(&trash) } else { CleanMode::Measure };
  let mut targets: Vec<CleanTarget> = paths
    .into_iter()
    .map(|path| {
      let target = |skipped: Option<String>, leased_by: Option<String>, cleaned: Vec<Cleaned>| CleanTarget {
        path: path.clone(),
        skipped,
        leased_by,
        cleaned,
        trash_left: Vec::new(),
      };
      let leased = store
        .slots
        .iter()
        .find(|slot| slot.path == path)
        .and_then(|slot| slot.lease.clone())
        .filter(Lease::is_agent_ready);
      if let Some(lease) = leased {
        let holder = lease.holder.clone();
        if params.overrides != CleanOverrides::default() {
          return target(Some(format!("leased by {holder}: --keep and --age apply only to a slot without a lease")), None, Vec::new());
        }
        return match prune_leased_slot(git_executor, &pool, params.lock_timeout, (&path, &lease), mode) {
          Ok(Some(pruned)) => target(None, Some(holder), pruned.cleaned),
          Ok(None) => target(Some(format!("leased by {holder}: the lease changed during the clean")), None, Vec::new()),
          Err(error) => target(Some(format!("{error:#}")), Some(holder), Vec::new()),
        };
      }
      let is_slot = store.slots.iter().any(|slot| slot.path == path);
      let processes = busy_processes(&scan.inside(Path::new(&path)));
      // a free slot is cleaned only under the `cleaning` marker, set while it is free, so no get leases it meanwhile; a
      // dry run deletes nothing and only reads the state again
      let leased = if !is_slot {
        Ok(None)
      } else if params.apply && processes.is_empty() {
        mark_for_clean(&pool, params.lock_timeout, &path, &marker)
      } else {
        Ok(
          pool
            .read()
            .ok()
            .and_then(|store| store.slots.into_iter().find(|slot| slot.path == path))
            .and_then(|slot| slot.lease)
            .map(|lease| lease.holder),
        )
      };
      let skipped = match leased {
        Ok(Some(holder)) => Some(format!("leased by {holder}")),
        Ok(None) if is_slot && !processes.is_empty() => Some(format!("processes work in it: {}", describe_processes(&processes))),
        Ok(None) => None,
        Err(error) => Some(format!("{error:#}")),
      };
      let cleaned = if skipped.is_some() {
        Vec::new()
      } else {
        apply_clean_rules(git_executor, &path, &rules, mode)
      };
      if is_slot && params.apply && skipped.is_none() {
        release_marker(&pool, params.lock_timeout, &path, &marker.id);
      }
      target(skipped, None, cleaned)
    })
    .collect();
  // every slot is free again: the delete of what the rules moved into the trash keeps no get waiting
  for target in &mut targets {
    target.trash_left = trash.empty(&mut target.cleaned);
  }
  Ok((targets, scan.is_available()))
}

/// Under the pool lock: mark the slot `path` with the `cleaning` `marker` while it is free (a lease whose process is gone
/// counts as free); the holder of its lease when it has one (a get leased it since the read, or a return runs)
fn mark_for_clean(pool: &Pool, timeout: Duration, path: &str, marker: &Lease) -> Result<Option<String>> {
  pool.update(timeout, |store| {
    // a slot the pool dropped meanwhile (its directory was gone) is a checkout that is no slot
    let Some(slot) = store.slots.iter_mut().find(|slot| slot.path == path) else {
      return Ok(None);
    };
    settle_abandoned(slot);
    if let Some(lease) = &slot.lease {
      return Ok(Some(lease.holder.clone()));
    }
    slot.lease = Some(marker.clone());
    Ok(None)
  })
}

/// What `brd lease reclaim --idle` reclaims, and the reclaim of a get below the minimum free space (`reclaim_for_get`)
pub struct LeaseReclaimParams<'a> {
  /// The common dir of the repository of the pool: its config (`lease_config`, read in it as `trim::max_slots` does, so
  /// a pool of another repository needs no checkout of it)
  pub git_common_dir: &'a Path,
  /// Reclaim, the default; false (`--dry-run`) only checks and writes nothing
  pub apply: bool,
  /// The parameters of the call that change the clean rules of the file
  pub clean: CleanOverrides,
  /// Keep the verdict of a recorded check while it holds (`IdleCheck::holds`, one `rev-parse`): the reclaim of a get
  /// and of the keeper. False: each lease is checked anew (`brd lease reclaim`)
  pub reuse_checks: bool,
  /// Only the slots on this volume, told before any git: an idle lease of another volume frees nothing on the one a get
  /// measures. `None`: every slot
  pub volume: Option<&'a Volume>,
  /// How many more leases may be checked in full, shared by the pools of one get (`IDLE_CHECKS_PER_GET`): a lease past
  /// it whose recorded check does not hold is left to a later get. `None`: no bound
  pub full_checks: Option<&'a Cell<usize>>,
  /// How long the reclaim waits for the Bazel servers it asked to exit
  pub server_timeout: Duration,
  /// The program of `bazel shutdown`, which stops the servers on Windows; `None`: `bazel` of the `PATH`
  pub bazel: Option<&'a OsStr>,
  pub lock_timeout: Duration,
}

/// What `brd lease reclaim` did with each idle lease of the pool
#[derive(Debug, Clone, Default)]
pub struct LeaseReclaim {
  /// Returned, as their work landed (with `--dry-run`: would be)
  pub reclaimed: Vec<ReclaimedLease>,
  /// Left alone, with the work their home branch lacks: the user's to decide
  pub kept: Vec<KeptLease>,
  /// Not checked or not returned now, and why
  pub skipped: Vec<SkippedIdle>,
  pub process_scan: bool,
}

#[derive(Debug, Clone)]
pub struct ReclaimedLease {
  pub path: String,
  /// As it was before the reclaim
  pub lease: Lease,
  pub landed: Vec<LandedCommit>,
  pub stopped_servers: Vec<StoppedServer>,
  pub cleaned: Vec<Cleaned>,
  /// What the trash kept: a path that could not be deleted once the slot was free
  pub trash_left: Vec<String>,
  /// Its quiet local changes, preserved in its notes before the reclaim discarded them (with `--dry-run`: where they
  /// would be, no patch yet)
  pub preserved: Option<Preserved>,
}

impl ReclaimedLease {
  /// The bytes its clean rules freed
  pub fn freed_bytes(&self) -> u64 {
    self.cleaned.iter().map(|rule| rule.freed_bytes).sum()
  }
}

#[derive(Debug, Clone)]
pub struct KeptLease {
  pub path: String,
  pub lease: Lease,
  pub check: IdleCheck,
}

#[derive(Debug, Clone)]
pub struct SkippedIdle {
  pub path: String,
  pub lease: Lease,
  pub reason: IdleSkip,
}

/// Why `brd lease reclaim` left an idle lease for now
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdleSkip {
  /// Its get or the last move of its HEAD is younger than `IDLE_QUIET`
  NotQuiet,
  /// A process of another session works in it, or a Bazel server of it did not exit
  Busy { processes: Vec<BusyProcess> },
  /// It changed while the reclaim checked it: a get took it over, a session committed in it
  Changed,
  /// No holder process is recorded (by a brd that named none on Windows, or where the system named no ancestors):
  /// whether its session still runs can't be told, so it is never checked; the user returns it once the session is done
  HolderUnknown,
  /// The check or the return failed
  Failed { error: String },
}

/// Return every idle lease of the pool whose work landed (`check_idle`: the commits in its home branch, a clean tree,
/// no operation in progress), each under its own CAS on its lease, its holder gone and its slot unchanged, as a return
/// does: its Bazel servers stopped, its seeded paths removed, its branch detached, the clean rules applied into the
/// trash, the slot free with the record of its lease (`Slot::reclaimed`), returned as the coldest free slot of the pool
/// (`stamp_reclaimed`). The trash is deleted once, after the last slot, so no slot waits for the delete of another's
/// build output. One with work is kept, its check recorded for the gets; with `apply` false nothing is written. The
/// config and the process scan are read at the first lease that needs them, so a pool with no idle lease runs no git,
/// and one whose kept leases hold their verdict (`reuse_checks`) runs one `rev-parse` each. With `full_checks` the
/// leases checked longest ago come first
pub fn lease_reclaim_core(git_executor: &GitCommandExecutor, params: &LeaseReclaimParams<'_>) -> Result<LeaseReclaim> {
  let git_common_dir = PathBuf::from(canonical_path(params.git_common_dir));
  let pool = Pool::new(&git_common_dir);
  let mut store = pool.read()?;
  if params.full_checks.is_some() {
    // stable: the leases never checked first, in the order of the state
    store.slots.sort_by_key(|slot| slot.lease.as_ref().map_or(0, Lease::idle_checked_at));
  }
  let mut scan: Option<ProcessScan> = None;
  let mut config: Option<LeaseConfig> = None;
  let trash = Trash::new(&pool.trash_dir());
  let notes_roots = pool.notes_roots(None, None);
  let now = now();
  let mut answer = LeaseReclaim::default();
  for slot in store.slots {
    let Some(lease) = slot
      .lease
      .clone()
      .filter(|lease| lease.is_agent_ready() && params.volume.is_none_or(|volume| volume.holds(Path::new(&slot.path))) && is_worktree_of(Path::new(&slot.path), &git_common_dir))
    else {
      continue;
    };
    let path = slot.path.clone();
    let skip = |reason: IdleSkip| SkippedIdle {
      path: path.clone(),
      lease: lease.clone(),
      reason,
    };
    match lease.holder_alive() {
      Some(false) => {}
      Some(true) => continue,
      None => {
        answer.skipped.push(skip(IdleSkip::HolderUnknown));
        continue;
      }
    }
    if !is_quiet(&slot, &lease, now) {
      answer.skipped.push(skip(IdleSkip::NotQuiet));
      continue;
    }
    // a get tries a failed reclaim again only after `RECLAIM_RETRY`, as past its bound
    if params.full_checks.is_some() && lease.reclaim_waits(now) {
      continue;
    }
    let scan = scan.get_or_insert_with(ProcessScan::scan);
    let (servers, others) = slot_servers(scan, Path::new(&path));
    if !others.is_empty() {
      answer.skipped.push(skip(IdleSkip::Busy {
        processes: busy_processes(&others),
      }));
      continue;
    }
    // without `reuse_checks` a check of its own, whatever a get found before
    let checked = if params.reuse_checks {
      lease.clone()
    } else {
      Lease {
        idle_check: None,
        ..lease.clone()
      }
    };
    // a failure after the check in full is recorded (`Lease::reclaim_failed_at`): else the lease stays the one checked
    // longest ago, and each get spends its full checks on it before any other
    let failed = |restore: bool| {
      if params.apply {
        record_failed_reclaim(&pool, params.lock_timeout, &path, &lease.id, restore);
      }
    };
    let mut full_check = false;
    let verdict = match IdleState::of(git_executor, &path, &checked) {
      Ok(state) => match state.held(&path, &checked, held_config(git_executor, &git_common_dir, &mut config, &checked).as_deref()) {
        Some(verdict) => Ok(verdict),
        // past the bound of the get: a later get checks it, the ones checked longest ago first
        None if params.full_checks.is_some_and(|left| left.get() == 0) => continue,
        None => {
          if let Some(left) = params.full_checks {
            left.set(left.get() - 1);
          }
          full_check = true;
          pool_config(git_executor, &git_common_dir, &mut config)
            .and_then(|config| check_idle_work(git_executor, (&path, &slot.name), &checked, (config.untracked, PreserveConfig::of(config)), state))
        }
      },
      Err(error) => Err(error),
    };
    // with the local changes the preserve checks first (`idle_changes`)
    let (landed, mark, changes) = match verdict {
      Ok(IdleVerdict::Landed { landed, mark }) => (landed, mark, None),
      Ok(IdleVerdict::Work { check, landed, .. }) if preserves(&check, now) => (landed, check.mark.clone(), Some(check)),
      Ok(IdleVerdict::Work { check, fresh, .. }) => {
        if params.apply && fresh {
          record_idle_check(&pool, params.lock_timeout, (&path, &lease.id), &check, false);
        }
        answer.kept.push(KeptLease { path, lease, check: *check });
        continue;
      }
      Err(error) => {
        if full_check {
          failed(false);
        }
        answer.skipped.push(skip(IdleSkip::Failed { error: format!("{error:#}") }));
        continue;
      }
    };
    let mut reclaimed = ReclaimedLease {
      path: path.clone(),
      lease: lease.clone(),
      landed,
      stopped_servers: Vec::new(),
      cleaned: Vec::new(),
      trash_left: Vec::new(),
      preserved: None,
    };
    let preserve = |check: &IdleCheck, config: &mut Option<LeaseConfig>| {
      let config = PreserveConfig::of(pool_config(git_executor, &git_common_dir, config)?);
      preserve_changes(git_executor, &path, (&lease, check), config, &notes_roots, params.apply)
    };
    if !params.apply {
      if let Some(check) = &changes {
        match preserve(check, &mut config) {
          Ok(Preserve::Planned(preserved)) => reclaimed.preserved = Some(preserved),
          Ok(Preserve::Kept(check)) => {
            answer.kept.push(KeptLease { path, lease, check });
            continue;
          }
          Ok(Preserve::Ready { .. }) => unreachable!("a dry run writes no patch"),
          Err(error) => {
            answer.skipped.push(skip(IdleSkip::Failed { error: format!("{error:#}") }));
            continue;
          }
        }
      }
      answer.reclaimed.push(reclaimed);
      continue;
    }
    if !reserve_reclaim(&pool, params.lock_timeout, &path, &lease.id, &mark)? {
      failed(false);
      answer.skipped.push(skip(IdleSkip::Changed));
      continue;
    }
    let servers: Vec<&ProcessInfo> = servers.iter().collect();
    let ServerStop { stopped, running } = stop_servers(&servers, params.bazel, params.server_timeout);
    reclaimed.stopped_servers = stopped;
    if !running.is_empty() {
      failed(true);
      answer.skipped.push(skip(IdleSkip::Busy { processes: running }));
      continue;
    }
    // the record of the notes before the commit point, the commit point (the slot dirty) before the discard
    if let Some(check) = &changes {
      match preserve(check, &mut config) {
        Ok(Preserve::Ready { preserved, dirty }) if commit_preserved_reclaim(&pool, params.lock_timeout, (&path, &lease, &mark), (&preserved, &dirty))? => {
          reclaimed.preserved = Some(preserved);
        }
        Ok(Preserve::Ready { .. }) => {
          withdraw_preserved(&notes_roots, &lease.id);
          failed(true);
          answer.skipped.push(skip(IdleSkip::Changed));
          continue;
        }
        Ok(Preserve::Kept(check)) => {
          record_idle_check(&pool, params.lock_timeout, (&path, &lease.id), &check, true);
          answer.kept.push(KeptLease { path, lease, check });
          continue;
        }
        Ok(Preserve::Planned(_)) => unreachable!("a reclaim that applies plans no preserve"),
        Err(error) => {
          failed(true);
          record_idle_check(&pool, params.lock_timeout, (&path, &lease.id), &check.failed_preserve(&error, now), false);
          answer.skipped.push(skip(IdleSkip::Failed { error: format!("{error:#}") }));
          continue;
        }
      }
    }
    let discard = match (&reclaimed.preserved, &config) {
      (Some(_), Some(config)) => Some(config.untracked),
      _ => None,
    };
    // no wait for a lock of cargo, which only another clean holds: the slot is free all the same
    match finish_return(git_executor, &path, &lease, discard, (Some(params.clean), Duration::ZERO), (&trash, &notes_roots)) {
      Ok(finished) => reclaimed.cleaned = finished.cleaned,
      // after the commit point of a preserve the lease never comes back: a discard cut short may have left part of
      // the changes, which a later preserve would take for all of them. The slot stays free and dirty, as a reclaim
      // killed there leaves it
      Err(error) if reclaimed.preserved.is_some() => {
        release_marker(&pool, params.lock_timeout, &path, &lease.id);
        answer.skipped.push(skip(IdleSkip::Failed { error: format!("{error:#}") }));
        continue;
      }
      Err(error) => {
        failed(true);
        answer.skipped.push(skip(IdleSkip::Failed { error: format!("{error:#}") }));
        continue;
      }
    }
    // the time its lease went quiet, the two times `is_quiet` reads, never past now
    let quiet_at = lease.leased_at.max(mark.log_modified).min(now);
    pool.update(params.lock_timeout, |store| {
      if let Some(index) = leased_index(store, &path, &lease.id) {
        // the coldest free slot of the pool, by its number and by its time, and no other free slot moves
        stamp_reclaimed(store, index, quiet_at);
        let slot = &mut store.slots[index];
        slot.lease = None;
        slot.dirty = false;
        slot.dirty_check = None;
        slot.reclaimed = Some(Reclaimed {
          preserved: reclaimed.preserved.clone(),
          ..Reclaimed::of(&lease, None)
        });
      }
      Ok(())
    })?;
    debug!(slot = %path, "an idle lease is reclaimed: its slot is free");
    answer.reclaimed.push(reclaimed);
  }
  answer.process_scan = scan.as_ref().map_or(ProcessScan::SUPPORTED, ProcessScan::is_available);
  // every reclaimed slot is free: the delete of what the rules moved into the trash keeps no slot waiting
  debug!(slots = answer.reclaimed.len(), "deleting the trash of the reclaimed slots");
  for reclaimed in &mut answer.reclaimed {
    reclaimed.trash_left = trash.empty(&mut reclaimed.cleaned);
  }
  Ok(answer)
}

/// The keys of the preserve that the recorded check of `lease` needs to hold (`IdleCheck::holds`): read only for local
/// changes that wait for no quiet time, so a pool whose kept leases wait for one reads no config; `None` when it needs
/// none or none can be read
fn held_config(git_executor: &GitCommandExecutor, git_common_dir: &Path, read: &mut Option<LeaseConfig>, lease: &Lease) -> Option<String> {
  let needed = lease.idle_check.as_ref().is_some_and(|check| !check.paths.is_empty() && check.quiet_at.is_none());
  // a config that can't be read compares nothing: the check of the lease reads it again and fails there
  needed
    .then(|| pool_config(git_executor, git_common_dir, read).ok().map(|config| PreserveConfig::of(config).fingerprint()))
    .flatten()
}

/// The config of the pool of `git_common_dir` (`lease_config`), read once into `read`
fn pool_config<'a>(git_executor: &GitCommandExecutor, git_common_dir: &Path, read: &'a mut Option<LeaseConfig>) -> Result<&'a LeaseConfig> {
  if read.is_none() {
    *read = Some(lease_config(git_executor, &display_path(git_common_dir))?);
  }
  read.as_ref().ok_or_else(|| anyhow!("The config of the pool was not read"))
}

/// Under the pool lock, with CAS on the idle lease `lease_id` of `path`: still idle and its slot as the check found it
/// (`mark`), the lease becomes `returning`, of this process, so no get takes it over meanwhile
fn reserve_reclaim(pool: &Pool, timeout: Duration, path: &str, lease_id: &str, mark: &SlotMark) -> Result<bool> {
  let pid = std::process::id();
  pool.update(timeout, |store| {
    let Some(lease) = leased_slot(store, path, lease_id).and_then(|slot| slot.lease.as_mut()).filter(|lease| lease.is_idle()) else {
      return Ok(false);
    };
    if SlotMark::of(Path::new(path)) != *mark {
      return Ok(false);
    }
    lease.state = LeaseState::Returning;
    lease.pid = pid;
    lease.process_started = process_started(pid);
    Ok(true)
  })
}

/// The commit point of a reclaim that preserved the local changes of `lease` in `path` (`idle_changes`), under the pool
/// lock: the lease still `returning` by this process and the slot as the check found it (`mark`). The record of the slot
/// names the patches, and the slot is dirty by `dirty` until the discard, so a reclaim killed or failed before it leaves
/// a slot that no get takes and no eviction deletes. False when the reclaim no longer holds
fn commit_preserved_reclaim(pool: &Pool, timeout: Duration, (path, lease, mark): (&str, &Lease, &SlotMark), (preserved, dirty): (&Preserved, &DirtyCheck)) -> Result<bool> {
  let pid = std::process::id();
  pool.update(timeout, |store| {
    let Some(slot) = leased_slot(store, path, &lease.id) else {
      return Ok(false);
    };
    let ours = slot.lease.as_ref().is_some_and(|current| current.state == LeaseState::Returning && current.pid == pid);
    if !ours || SlotMark::of(Path::new(path)) != *mark {
      return Ok(false);
    }
    slot.reclaimed = Some(Reclaimed {
      preserved: Some(preserved.clone()),
      ..Reclaimed::of(lease, None)
    });
    mark_dirty(slot, dirty);
    Ok(true)
  })
}

/// Record a failed reclaim of the lease `lease_id` of `path` (`Lease::reclaim_failed_at`); with `restore` the lease is
/// `ready` again, after a reclaim that reserved it. Best effort
fn record_failed_reclaim(pool: &Pool, timeout: Duration, path: &str, lease_id: &str, restore: bool) {
  update_idle_lease(pool, timeout, (path, lease_id), restore, "a failed reclaim", |lease| lease.reclaim_failed_at = Some(now()));
}

/// Record `check` on the idle lease `lease_id` of `path`, for the gets after it; with `restore` the lease is `ready`
/// again, after a reclaim that reserved it. Best effort
fn record_idle_check(pool: &Pool, timeout: Duration, (path, lease_id): (&str, &str), check: &IdleCheck, restore: bool) {
  update_idle_lease(pool, timeout, (path, lease_id), restore, "the check", |lease| lease.idle_check = Some(check.clone()));
}

/// `change` of the idle lease `lease_id` of `path`, `ready` again with `restore`; a failure is a warning about `what`
fn update_idle_lease(pool: &Pool, timeout: Duration, (path, lease_id): (&str, &str), restore: bool, what: &str, change: impl FnOnce(&mut Lease)) {
  let recorded = pool.update(timeout, |store| {
    if let Some(lease) = leased_slot(store, path, lease_id).and_then(|slot| slot.lease.as_mut()) {
      if restore {
        lease.state = LeaseState::Ready;
      }
      change(lease);
    }
    Ok(())
  });
  if let Err(error) = recorded {
    warn!(slot = %path, error = %format!("{error:#}"), "cannot record {what} of an idle lease");
  }
}

/// A directory `brd lease adopt` added, or refused
#[derive(Debug, Clone)]
pub enum Adopted {
  /// With `moved_from` when `--move` moved it into the pool directory
  Added {
    slot: Box<Slot>,
    moved_from: Option<String>,
  },
  Refused {
    path: String,
    refusal: LeaseRefusal,
  },
}

/// Add existing linked worktrees of this repository to the pool as free slots, where they are. Each is refused when it
/// is not a linked worktree of the same common dir, the pool holds it, a process is in it, or it has local changes (one
/// `status`)
/// What `brd lease adopt` adds
pub struct LeaseAdoptParams<'a> {
  /// A worktree of the repository, where `git worktree move` runs
  pub repository: &'a str,
  pub git_common_dir: &'a Path,
  pub dirs: &'a [PathBuf],
  /// `--move`: the pool directory each worktree moves into, as `<pool>/<n>/<repository name>`
  pub move_into: Option<&'a Path>,
  pub lock_timeout: Duration,
}

/// Add existing linked worktrees of this repository to the pool as free slots, where they are, or with `move_into`
/// moved into the pool directory by `git worktree move` (same volume only). Each is refused when it is not a linked
/// worktree of the same common dir, the pool holds it, a process is in it, or it has local changes (one `status`)
pub fn lease_adopt_core(git_executor: &GitCommandExecutor, params: &LeaseAdoptParams<'_>) -> Result<(Vec<Adopted>, bool)> {
  let git_common_dir = PathBuf::from(canonical_path(params.git_common_dir));
  let pool = Pool::new(&git_common_dir);
  let scan = ProcessScan::scan();
  let untracked = lease_config(git_executor, params.repository)?.untracked;
  let move_into = match params.move_into {
    Some(dir) => {
      std::fs::create_dir_all(dir).with_context(|| format!("Cannot create the pool directory {}", dir.display()))?;
      Some(PathBuf::from(canonical_path(dir)))
    }
    None => None,
  };
  let mut outcomes = Vec::new();
  for dir in params.dirs {
    let path = canonical_path(dir);
    let adopted = match adopt_one(git_executor, &pool, &git_common_dir, &path, &scan, untracked)? {
      Ok(()) => match &move_into {
        Some(pool_dir) => move_in(git_executor, &pool, params, &git_common_dir, pool_dir, &path)?,
        None => add_slot_entry(&pool, params.lock_timeout, &path, None)?,
      },
      Err(refusal) => Err(refusal),
    };
    outcomes.push(match adopted {
      Ok((slot, moved_from)) => Adopted::Added { slot: Box::new(slot), moved_from },
      Err(refusal) => Adopted::Refused { path, refusal },
    });
  }
  Ok((outcomes, scan.is_available()))
}

type AdoptResult = Result<Result<(Slot, Option<String>), LeaseRefusal>>;

/// The free slot of the worktree `name` at `path`, as an adopt records it
const fn free_slot(name: String, path: String) -> Slot {
  Slot {
    name,
    path,
    dirty: false,
    dirty_check: None,
    returned: None,
    returned_at: None,
    lease: None,
    reclaimed: None,
  }
}

/// Record the worktree at `path` (moved from `moved_from`) as a free slot, under the pool lock
fn add_slot_entry(pool: &Pool, lock_timeout: Duration, path: &str, moved_from: Option<String>) -> AdoptResult {
  let name = linked_worktree(Path::new(path)).map_or_default(|worktree| worktree.id);
  pool.update(lock_timeout, |store| {
    if store.slots.iter().any(|slot| slot.path == path) {
      return Ok(Err(LeaseRefusal::InPool(path.to_string())));
    }
    let slot = free_slot(name, path.to_string());
    store.slots.push(slot.clone());
    Ok(Ok((slot, moved_from)))
  })
}

/// `--move`: under the pool lock, the next slot path, held by a `preparing` entry of this process (`Lease::marker`) so
/// no get or other adopt takes it; then without that lock `git worktree move` there (under the lock of the pool's
/// `worktree` commands only: a git under load holds it for seconds, against every get and return of the pool lock);
/// then under the pool lock the free slot in place of the entry, or the entry dropped when the move failed. An adopt
/// that dies meanwhile leaves an abandoned entry, which the next reserve frees. The parent of the old directory goes
/// when it is left empty (`<treehouse pool>/<n>/`); the state file of Treehouse is not touched, and Treehouse sees the
/// old path as missing
fn move_in(git_executor: &GitCommandExecutor, pool: &Pool, params: &LeaseAdoptParams<'_>, git_common_dir: &Path, pool_dir: &Path, path: &str) -> AdoptResult {
  if !same_volume(Path::new(path), pool_dir) {
    return Ok(Err(LeaseRefusal::OtherVolume {
      path: path.to_string(),
      pool: display_path(pool_dir),
    }));
  }
  let entry = Lease::marker("adopt", "brd lease adopt", LeaseState::Preparing);
  // the worktree id, which `worktree move` keeps: an adopt killed during the move leaves a named slot, which no get takes
  // for one whose add never finished (`is_unfinished`)
  let id = linked_worktree(Path::new(path)).map_or_default(|worktree| worktree.id);
  let reserved = pool.update(params.lock_timeout, |store| {
    if store.slots.iter().any(|slot| slot.path == path) {
      return Ok(Err(LeaseRefusal::InPool(path.to_string())));
    }
    let target = next_slot_path(store, pool_dir, git_common_dir);
    store.slots.push(Slot {
      lease: Some(entry.clone()),
      ..free_slot(id.clone(), display_path(&target))
    });
    Ok(Ok(target))
  })?;
  let target = match reserved {
    Ok(target) => target,
    Err(refusal) => return Ok(Err(refusal)),
  };
  let moved = target.parent().map_or(Ok(()), std::fs::create_dir_all).map_err(|error| error.to_string()).and_then(|()| {
    pool
      .worktree_command(params.lock_timeout, || move_slot(git_executor, params.repository, path, &target))
      .map_err(|error| format!("{error:#}"))
  });
  let text = display_path(&target);
  let name = linked_worktree(&target).map_or_default(|worktree| worktree.id);
  let recorded = pool.update(params.lock_timeout, |store| {
    store
      .slots
      .retain(|slot| !(slot.path == text && slot.lease.as_ref().is_some_and(|lease| lease.id == entry.id)));
    if moved.is_err() {
      return Ok(None);
    }
    let slot = free_slot(name, text.clone());
    store.slots.push(slot.clone());
    Ok(Some(slot))
  })?;
  let Some(slot) = recorded else {
    if let Some(parent) = target.parent() {
      let _ = fs_retry::remove_dir(parent);
    }
    return Ok(Err(LeaseRefusal::CannotMove {
      path: path.to_string(),
      reason: moved.err().unwrap_or_default(),
    }));
  };
  if let Some(parent) = Path::new(path).parent() {
    // only an empty one goes
    let _ = fs_retry::remove_dir(parent);
  }
  Ok(Ok((slot, Some(path.to_string()))))
}

/// Whether `a` and `b` are on one volume (`st_dev`, `free_space::Volume`); elsewhere than on Unix the move is left to git
fn same_volume(a: &Path, b: &Path) -> bool {
  Volume::of(a).holds(b)
}

/// The checks of an adopt: a linked worktree of this repository, not in the pool, no process in it, clean
fn adopt_one(git_executor: &GitCommandExecutor, pool: &Pool, git_common_dir: &Path, path: &str, scan: &ProcessScan, untracked: UntrackedFiles) -> Result<Result<(), LeaseRefusal>> {
  let not_a_slot = |reason: &str| {
    Ok(Err(LeaseRefusal::NotASlot {
      path: path.to_string(),
      reason: reason.to_string(),
    }))
  };
  let Some(worktree) = linked_worktree(Path::new(path)) else {
    return not_a_slot("not the top level of a linked worktree");
  };
  if display_path(&worktree.git_common_dir) != display_path(git_common_dir) {
    return not_a_slot("a worktree of another repository");
  }
  if pool.read()?.slots.iter().any(|slot| slot.path == path) {
    return Ok(Err(LeaseRefusal::InPool(path.to_string())));
  }
  let processes = busy_processes(&scan.inside(Path::new(path)));
  if !processes.is_empty() {
    return Ok(Err(LeaseRefusal::SlotBusy {
      path: path.to_string(),
      processes,
    }));
  }
  let changes = slot_changes(git_executor, path, untracked)?;
  if !changes.is_empty() {
    return Ok(Err(LeaseRefusal::DirtyCandidate {
      path: path.to_string(),
      paths: changes,
    }));
  }
  Ok(Ok(()))
}

#[cfg(test)]
#[path = "lease_test.rs"]
mod lease_test;
