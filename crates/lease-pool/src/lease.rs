//! The lease pool of a repository (`bd lease`, spec/cli/lease.spec.md, "Lease" in docs/design/git-operations.md): linked
//! worktrees that agent sessions lease, one session per slot. The state in `$GIT_COMMON_DIR/branch-deck/leases.json` is
//! the authority: a free slot is pool property, and a get replaces its content, so no ancestry proves anything. The
//! pool has a lock of its own (`leases.lock`), held only while the state is read and written, never across a git that
//! writes a slot: a get that reuses a slot never waits for another's checkout, and no get waits for an update. A get
//! that adds a slot holds the clones lock of the pool (`clones.lock`, `Pool::lock_clones`) for the whole clone, so the
//! pool adds one slot at a time. The `worktree` commands of the pool, which git can't run two at once, take a lock of
//! their own (`worktrees.lock`, `Pool::worktree_command`). The order is `clones.lock`, `leases.lock`, `worktrees.lock`:
//! a get takes the other two under the clones lock, the move of an adopt the lock of the `worktree` commands under the
//! pool lock; nothing takes the clones lock under another lock of the pool, and a return or a clean never takes it. The
//! lock of the lend table of the host slots (`host-slots.lock`, `host_slots`) comes before all three and is a leaf: it is
//! never held while another is taken.

use crate::bazel_servers::{servers_by_output_base, shutdown};
use crate::clone_dir::{CloneSkips, clone_directory};
use crate::free_space::available_bytes;
use crate::host_slots::{HOST_HOLDER_PREFIX, host_slot_number};
use crate::landed_work::{LandedCheck, LandedCommit, Purpose, landed_work};
use crate::lease_clean::{CleanMode, CleanOverrides, Cleaned, Trash, apply_clean_rules, clone_skips, describe_size, disk_size, holds_gone_runs, read_clean_rules, remove_together};
use crate::state_file::StateFile;
use anyhow::{Context, Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::{
  CheckoutWorkers, LeaseConfig, SlotMove, UnlandedCommit, UntrackedFiles, attach_new_branch, checkout_slot, clean_cloned_slot, detach_slot_head, discard_slot_changes,
  ignored_paths, is_valid_branch_name, lease_config, linked_worktree, move_clean_slot, move_slot, refresh_cloned_index, register_slot, slot_changes, start_deferred_maintenance,
  unlock_slot,
};
use git_ops::revs::{RepositoryDirs, read_head, ref_exists, resolve_commit, short_id};
use git_ops::update_resolve::session_ref;
use history_rewrite::repository_lock::{LockHolder, LockWaitCanceled, RepositoryLock, lock_file_while_held};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fmt;
use std::fmt::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sync_utils::fs_retry;
use sync_utils::paths::display_path;
use sync_utils::process_scan::{ProcessInfo, ProcessScan, is_running, process_started, terminate};
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
/// The file at the root of the source that names the ignored files a get copies into the slot
pub const WORKTREE_INCLUDE: &str = ".worktreeinclude";

/// The state of the pool: every slot, free or leased
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseStore {
  #[serde(default)]
  pub slots: Vec<Slot>,
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
  /// The number of its last return in the pool, higher for a later one: a get takes the free slot returned last first
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub returned: Option<u64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub lease: Option<Lease>,
  /// The lease a takeover or `bd lease reclaim` took from its dead holder: a command of another session in the slot is
  /// refused (`reclaimed_refusal`) until the next get of the free slot
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reclaimed: Option<Reclaimed>,
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
  /// The lease of the get that took it over; none for `bd lease reclaim`, which left the slot free
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by_lease_id: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by_holder: Option<String>,
}

/// The last check of an idle lease that found work (`check_idle`): a later get checks it again only when what the
/// verdict rests on moved
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
  /// The local changes
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub paths: Vec<String>,
  /// A git operation in progress (`rebase`, `merge`, `cherry-pick`, `revert`, `bisect`) or an open resolve session
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub operation: Option<String>,
}

impl IdleCheck {
  /// The work, in words: "2 commit(s) its home branch lacks", "3 local change(s)", "a rebase in progress"
  pub fn describe(&self) -> String {
    let mut work = Vec::new();
    if !self.commits.is_empty() {
      work.push(format!("{} commit(s) its home branch lacks", self.commits.len()));
    }
    if !self.paths.is_empty() {
      work.push(format!("{} local change(s)", self.paths.len()));
    }
    if let Some(operation) = &self.operation {
      work.push(format!("a {operation} in progress"));
    }
    work.join(", ")
  }

  /// Whether the verdict holds for the slot as `mark` and the home tip `home_tip` tell it now: the tree work while the
  /// slot did not change (its holder is gone), the commits while the home branch did not move either
  fn holds(&self, mark: &SlotMark, home_tip: Option<&str>) -> bool {
    self.mark == *mark && (!self.paths.is_empty() || self.operation.is_some() || self.home_tip.as_deref() == home_tip)
  }
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
  /// terminal), and when it started. `pid` is of the get itself, which exits at once
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
  /// A free slot that `bd lease clean` cleans: no get takes it until the clean ends
  Cleaning,
}

impl Lease {
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
  /// `bd-host slots --local --shrink` gives one back)
  pub fn is_idle(&self) -> bool {
    self.state == LeaseState::Ready && self.holder_alive() == Some(false) && !self.holder.starts_with(HOST_HOLDER_PREFIX)
  }
}

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
fn settle_abandoned(slot: &mut Slot) {
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
}

impl BusyProcess {
  pub fn of(process: &ProcessInfo) -> Self {
    Self {
      pid: process.pid,
      name: process.name.clone(),
      command: process.command_line(COMMAND_LIMIT),
      bazel_workspace: process.bazel_workspace().map(|workspace| display_path(&workspace)),
      stop_error: None,
    }
  }
}

fn busy_processes(processes: &[&ProcessInfo]) -> Vec<BusyProcess> {
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
      // a host slot is idle between runs by design: `bd-host slots --local --shrink` gives it back, never a hand return
      lease.is_idle().then(|| IdleLease {
        path: slot.path.clone(),
        holder: lease.holder.clone(),
        lease_id: lease.id.clone(),
        check: lease.idle_check.clone(),
      })
    })
    .collect()
}

/// What the pool refuses; `bd` gives each its error code
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
  #[error("Processes work in {path}: {}; wait until they are done{}", describe_processes(.processes), servers_note(.processes))]
  SlotBusy { path: String, processes: Vec<BusyProcess> },
  #[error("Bazel servers of {path} did not exit on {SERVER_STOP}: {}; wait, or stop them with `bazel shutdown` in their workspace", describe_processes(.processes))]
  ServersRunning { path: String, processes: Vec<BusyProcess> },
  #[error(
    "The volume of the pool {pool} has {} free, less than the {} a lease needs (`branchdeck.leaseMinFree`, `--min-free`); {free_slots} free slot(s){}{}. Tell the user (`bd lease clean --free`, `bd lease reclaim --idle` and the Bazel output bases of slots no longer used free space); never pass `--min-free 0` on your own",
    describe_size(*.available),
    describe_size(*.required),
    describe_idle(.idle),
    describe_trash(*.trash)
  )]
  LowDiskSpace {
    pool: String,
    available: u64,
    required: u64,
    free_slots: usize,
    idle: Vec<IdleLease>,
    /// The trash a process still running keeps
    trash: u64,
  },
  #[error("{key} {value:?}: {reason}")]
  InvalidSize { key: String, value: String, reason: String },
  #[error(
    "HEAD of {path} has {} commit(s) that {}; land them first (`bd -C {path} lease pick`); --confirm-preserved gives them up, only when the user says they are kept elsewhere",
    .commits.len(),
    .check.describe()
  )]
  UnlandedCommits { path: String, commits: Vec<UnlandedCommit>, check: LandedCheck },
  #[error("{path} has local changes; commit or land them; --confirm-preserved discards them, only when the user says they are kept elsewhere")]
  DirtyWorktree { path: String, paths: Vec<String> },
  #[error(
    "{path} has a resolve session open ({session}); finish it with `bd -C {path} update --continue` and land the commits with `bd -C {path} lease pick`, or close it with `bd -C {path} update --abort`"
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
    "{path} is host slot {n}, which the agent of `bd-host` on this host lends to the runs of other machines; `bd-host slots --local --shrink <count>` on this host gives the host slots past the count back to the pool"
  )]
  HostSlot { path: String, n: u32 },
  #[error("The holder {0:?} is reserved: the host slots of `bd-host` are `{HOST_HOLDER_PREFIX}<n>`")]
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
    ..
  } = reclaimed;
  let home = source_branch.as_deref().unwrap_or("its home branch");
  let by = match (by_lease_id, by_holder) {
    (Some(id), Some(by)) => format!("the lease {id} of {by}"),
    _ => "`bd lease reclaim`, which left it free".to_string(),
  };
  let branch = branch.as_ref().map_or_else(String::new, |branch| format!("; its branch {branch} stays"));
  if in_progress {
    format!(
      "The lease {lease_id} of {holder} in {path} is being taken over by {by}, which checks whether its work landed on {home}: this worktree is not yours now{branch}. Wait for `bd lease status`, then get a lease of your own"
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
      None => format!("{} ({}: {})", process.name, process.pid, process.command),
    })
    .collect::<Vec<_>>()
    .join(", ")
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
}

impl Pool {
  /// The pool of the repository whose `$GIT_COMMON_DIR` is `git_common_dir`
  pub fn new(git_common_dir: &Path) -> Self {
    Self {
      state: StateFile::new(git_common_dir, POOL_FILE, LOCK_FILE, "the lease state"),
    }
  }

  /// The state, without the lock: the file is replaced by a rename, so a read sees one whole state. A missing file is an
  /// empty pool
  pub fn read(&self) -> Result<LeaseStore> {
    self.state.read()
  }

  /// The trash of the returns and the cleans (`lease_clean::Trash`)
  pub fn trash_dir(&self) -> PathBuf {
    self.state.dir().join(TRASH_DIR)
  }

  /// The bytes on disk of what the trash holds: what a run killed before its delete left behind
  pub fn trash_bytes(&self) -> u64 {
    std::fs::read_dir(self.trash_dir()).map_or(0, |entries| entries.flatten().map(|entry| disk_size(&entry.path())).sum())
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
  /// every get and return; the move of an adopt takes this lock inside the pool lock, and nothing takes the pool lock
  /// inside this one
  pub fn worktree_command<T>(&self, timeout: Duration, command: impl FnOnce() -> Result<T>) -> Result<T> {
    let _lock = self.state.lock_at(WORKTREES_LOCK_FILE, timeout, "waiting for the lock of the pool's worktree commands")?;
    command()
  }

  /// The lock of the gets that add a slot (`clones.lock`), which a get holds from the reserve of its new slot until the
  /// slot is ready or given back: on 2026-10-24 fourteen gets cloned the idea-1 checkout at once, several ran past the
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
  Dirty { paths: Vec<String> },
  /// Processes have their working directory in it, or a Bazel server of it did not exit
  Busy { processes: Vec<BusyProcess> },
  /// An idle lease, its holder gone, with work its home branch lacks: the get leaves it to the user
  IdleWithWork { holder: String, lease_id: String, check: IdleCheck },
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
  /// Under the clones lock, no free slot, and the return or the clean of the slot `slot` by the process `pid` runs: the
  /// get waits for it to end rather than add a slot (`clean`: a clean, else a return)
  WaitForSlot { slot: String, pid: u32, clean: bool },
  /// Below the minimum free space: the trash of the runs that are gone is deleted, without the clones lock
  DeleteTrash,
  /// The Bazel servers of the reserved free slot are asked to exit
  StopServers { slot: String, servers: usize },
  /// One `status` of the slot: of a reserved free slot, or of a clone after its checkout
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
  /// The copy of the paths that `.worktreeinclude` names
  Seed { slot: String, paths: usize },
  /// The check of the work of an idle lease, whose holder is gone: landed in its home branch, a clean tree
  TakeOver { slot: String, holder: String },
  /// The return of the idle lease the get took over: its seeded paths, its branch detached, the clean rules
  CleanTakenOver { slot: String },
}

impl fmt::Display for GetStep {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Reserve { adding: false } => f.write_str("reserving a free slot"),
      Self::Reserve { adding: true } => f.write_str("reserving a slot freed meanwhile, else a new one"),
      Self::WaitForClone(holder) => write!(f, "no free slot; {holder}"),
      Self::WaitForSlot { slot, pid, clean } => write!(
        f,
        "no free slot; waiting for the {} of {slot} (pid {pid}): the slot it frees is leased rather than a new one",
        if *clean { "clean" } else { "return" }
      ),
      Self::DeleteTrash => f.write_str("deleting the trash of runs that are gone: the free space is below the minimum"),
      Self::StopServers { slot, servers } => write!(f, "stopping {servers} Bazel server(s) of {slot}"),
      Self::Status { slot } => write!(f, "looking for local changes in {slot}"),
      Self::Clone { template, slot } => write!(f, "cloning {template} into {slot}"),
      Self::Refresh { slot } => write!(f, "refreshing the index of {slot}"),
      Self::Checkout { slot, commit } => write!(f, "checking out {} in {slot}", short_id(commit)),
      Self::CheckoutEveryFile { slot, reason } => write!(f, "checking out every file of {slot}, because git refused the diff: {reason}"),
      Self::Seed { slot, paths } => write!(f, "copying {paths} path(s) that {WORKTREE_INCLUDE} names into {slot}"),
      Self::TakeOver { slot, holder } => write!(f, "checking whether the work of the idle lease of {holder} in {slot} landed"),
      Self::CleanTakenOver { slot } => write!(f, "returning the idle lease of {slot}: its seeded paths, its branch and the clean rules"),
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
  /// "waiting for `bd lease get --holder lane-3` (pid 4242, for 35 s), which adds /pool/8/idea-1 for lane-3: …"
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

/// Lease a slot at the HEAD of the source. First the free space of the volume of the pool, when a minimum is set
/// (`LowDiskSpace`). Under the pool lock: a free slot of this repository (a linked worktree of the same common dir, no
/// process in it but its Bazel servers, clean slots before dirty ones), its lease recorded as `preparing`. Then,
/// without the lock: its Bazel servers stopped (one that does not exit skips the slot as busy), one `status` (a dirty
/// slot is marked, skipped, and the next one tried), `checkout --force` to the source HEAD (only the diff is written),
/// the seed. No free slot: the clones lock (`Pool::lock_clones`), waited for while another get adds a slot; a slot freed
/// during the wait ends it, and the get reserves that one without the lock. After a wait the minimum and the processes
/// as they are now; the free space again (a trash to delete releases the lock for the delete), and the reserve again,
/// where a slot freed meanwhile wins, and a slot whose return or clean runs is waited for. Else, under the lock until
/// the slot is ready, a clone of a template without what the `delete` rules delete (`add_cloned_slot`), else a
/// registration and the only full checkout. A failure after the lease was recorded gives the slot back
///
/// The holders `bd-host/<n>` are the host slots' (`host_slots`), which only `host_slots::fill` gets: another get with
/// such a holder is refused (`ReservedHolder`)
#[instrument(skip_all, fields(source = %params.source.top_level))]
pub fn lease_get_core(git_executor: &GitCommandExecutor, params: &LeaseGetParams<'_>) -> Result<LeaseGot> {
  if let Some(holder) = params.holder.as_ref().filter(|holder| holder.starts_with(HOST_HOLDER_PREFIX)) {
    bail!(LeaseRefusal::ReservedHolder(holder.clone()));
  }
  get_lease(git_executor, params, TakeOver::Idle)
}

/// The get of a host slot (`host_slots::fill`), with its holder `bd-host/<n>`: it takes over no idle lease, as the
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
  // whether the get deleted the trash since its last wait for the clones lock
  let mut swept = false;
  let mut free_bytes = loop {
    match free_space(&pool, &pool_dir, min_free, swept)? {
      FreeSpace::Enough(available) => break available,
      FreeSpace::Trash => {
        delete_trash(&pool, &report);
        swept = true;
      }
    }
  };
  let seeds = seed_paths(git_executor, source)?;
  // the rules of the source leave out of a cloned slot what a return would delete; a file that can't be read clones all
  let skips = CloneSkips::new(&clone_skips(&read_clean_rules(Path::new(source)).unwrap_or_else(|error| {
    warn!(source = %source, error = %format!("{error:#}"), "cannot read the clean rules; a clone copies everything");
    Vec::new()
  })));
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
    holder_pid: holder.map(|process| process.pid),
    holder_started: holder.and_then(|process| process.started),
    taking_over: None,
    idle_check: None,
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
  // held from the reserve of a new slot until the slot is ready or given back (dropped on return)
  let mut clones_lock: Option<RepositoryLock> = None;
  let mut waited_for_clone: Option<Duration> = None;
  loop {
    let adding = clones_lock.is_some();
    report(GetStep::Reserve { adding });
    let takes_over = take_over == TakeOver::Idle && idle_checks < IDLE_CHECKS_PER_GET;
    let reserved = pool.update(params.lock_timeout, |store| {
      let reserved = reserve(store, &git_common_dir, &pool_dir, &scan, &mut tried, &mut awaited, &mut skipped, &lease, adding, takes_over);
      if matches!(reserved, Reserved::New { .. }) {
        idle = idle_leases(store);
      }
      Ok(reserved)
    })?;
    let (path, reused) = match reserved {
      Reserved::NoFreeSlot => match lock_clones(&pool, params.lock_timeout, &tried, &report)? {
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
            swept = false;
          }
          // a queue of gets never adds slots past the minimum: each measures what the ones before it left
          match free_space(&pool, &pool_dir, min_free, swept)? {
            FreeSpace::Enough(available) => {
              free_bytes = available;
              clones_lock = Some(lock);
            }
            FreeSpace::Trash => {
              // the delete of a trash takes minutes for a Bazel output base of tens of GB: the gets behind this one go on
              // meanwhile, and it takes its turn again
              drop(lock);
              delete_trash(&pool, &report);
              swept = true;
            }
          }
          continue;
        }
      },
      Reserved::Freeing { path, lease_id, pid, clean } => {
        report(GetStep::WaitForSlot { slot: path.clone(), pid, clean });
        wait_while_freeing(&pool, &path, &lease_id)?;
        continue;
      }
      Reserved::Free { path, name, servers } => {
        // a slot freed while the get waited for the clones lock: the next get that adds a slot goes on
        clones_lock = None;
        if let Some(busy) = stop_reserved_servers(&pool, params, (&name, &path), &lease.id, &servers, &mut stopped_servers) {
          skipped.push(busy);
          continue;
        }
        report(GetStep::Status { slot: path.clone() });
        let changes = slot_changes(git_executor, &path, config.untracked).map_err(|error| give_back(&pool, params.lock_timeout, &path, &lease.id, false, error))?;
        if !changes.is_empty() {
          debug!(slot = %path, changes = changes.len(), "skipping a dirty slot");
          pool.update(params.lock_timeout, |store| {
            if let Some(slot) = leased_slot(store, &path, &lease.id) {
              slot.lease = None;
              slot.dirty = true;
            }
            Ok(())
          })?;
          skipped.push(SkippedSlot {
            name,
            path,
            reason: SkipReason::Dirty { paths: changes },
          });
          continue;
        }
        move_leased_slot(git_executor, &pool, params, &path, &lease.id, &source_head, config.workers)?;
        (path, true)
      }
      Reserved::TakeOver { path, name, servers, old } => {
        clones_lock = None;
        report(GetStep::TakeOver {
          slot: path.clone(),
          holder: old.holder.clone(),
        });
        let verdict = check_idle(git_executor, &path, &name, &old, config.untracked);
        let landed = match verdict {
          Ok(IdleVerdict::Landed { landed, mark }) if commit_takeover(&pool, params.lock_timeout, &path, &lease, &mark)? => landed,
          Ok(IdleVerdict::Landed { .. }) => {
            debug!(slot = %path, "the idle slot moved while the get checked it");
            keep_idle(&pool, params.lock_timeout, &path, &lease.id, *old);
            idle_checks += 1;
            continue;
          }
          Ok(IdleVerdict::Work { check, fresh }) => {
            idle_checks += usize::from(fresh);
            let mut kept = *old;
            kept.idle_check = Some(check.clone());
            skipped.push(SkippedSlot {
              name,
              path: path.clone(),
              reason: SkipReason::IdleWithWork {
                holder: kept.holder.clone(),
                lease_id: kept.id.clone(),
                check,
              },
            });
            keep_idle(&pool, params.lock_timeout, &path, &lease.id, kept);
            continue;
          }
          Err(error) => {
            warn!(slot = %path, error = %format!("{error:#}"), "cannot check the work of an idle lease");
            keep_idle(&pool, params.lock_timeout, &path, &lease.id, *old);
            idle_checks += 1;
            continue;
          }
        };
        // the lease is the get's now: what fails from here frees the slot, whose work landed
        if let Some(busy) = stop_reserved_servers(&pool, params, (&name, &path), &lease.id, &servers, &mut stopped_servers) {
          skipped.push(busy);
          continue;
        }
        report(GetStep::CleanTakenOver { slot: path.clone() });
        // the trash stays: the next return or clean deletes it as the trash of a gone run, and the get goes on
        let trash = Trash::new(&pool.trash_dir());
        finish_return(git_executor, &path, &old, &[], Some(CleanOverrides::default()), &trash)
          .map_err(|error| give_back(&pool, params.lock_timeout, &path, &lease.id, false, error))?;
        move_leased_slot(git_executor, &pool, params, &path, &lease.id, &source_head, config.workers)?;
        took_over = Some(TookOver { lease: *old, landed });
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
    if !seeds.is_empty() {
      report(GetStep::Seed {
        slot: path.clone(),
        paths: seeds.len(),
      });
    }
    let seeded = seed(Path::new(source), Path::new(&path), &seeds).map_err(|error| give_back(&pool, params.lock_timeout, &path, &lease.id, false, error))?;
    let name = linked_worktree(Path::new(&path)).map_or_default(|worktree| worktree.id);
    let slot = pool.update(params.lock_timeout, |store| {
      let slot = leased_slot(store, &path, &lease.id).ok_or_else(|| anyhow!("The lease of {path} was taken back while it was being prepared"))?;
      slot.name = name;
      slot.dirty = false;
      if let Some(lease) = slot.lease.as_mut() {
        lease.state = LeaseState::Ready;
        lease.seeded = seeded;
        lease.reused = reused;
      }
      Ok(slot.clone())
    })?;
    return Ok(LeaseGot {
      slot,
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
    });
  }
}

/// Stop the Bazel servers of the slot `(name, path)` that the get of `lease_id` reserved, added to `stopped`: with one
/// that did not exit, the slot is given back free and skipped as busy
fn stop_reserved_servers(
  pool: &Pool,
  params: &LeaseGetParams<'_>,
  (name, path): (&str, &str),
  lease_id: &str,
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
  release_marker(pool, params.lock_timeout, path, lease_id);
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
}

/// The free space of the volume of the pool, at least `min_free`, measured only: one `statfs`, and below the minimum
/// the names in the trash. Below it with no trash of a gone run, or one the get deleted since its last wait (`swept`):
/// `LowDiskSpace`, naming what the trash keeps
fn free_space(pool: &Pool, pool_dir: &Path, min_free: u64, swept: bool) -> Result<FreeSpace> {
  let available = available_bytes(pool_dir);
  let Some(available) = available.filter(|&available| available < min_free) else {
    return Ok(FreeSpace::Enough(available));
  };
  if !swept && holds_gone_runs(&pool.trash_dir()) {
    return Ok(FreeSpace::Trash);
  }
  let store = pool.read()?;
  bail!(LeaseRefusal::LowDiskSpace {
    pool: display_path(pool_dir),
    available,
    required: min_free,
    free_slots: store.slots.iter().filter(|slot| slot.lease.is_none()).count(),
    idle: idle_leases(&store),
    trash: pool.trash_bytes(),
  });
}

/// Delete the trash of the runs that are gone (`Trash::sweep`), told as a step
fn delete_trash(pool: &Pool, report: &dyn Fn(GetStep)) {
  report(GetStep::DeleteTrash);
  Trash::new(&pool.trash_dir()).sweep();
}

/// The turn of a get at the clones lock (`lock_clones`); `waited` is how long the get waited for it, when the wait was
/// told (`GetStep::WaitForClone`)
enum CloneTurn {
  Locked {
    lock: RepositoryLock,
    waited: Option<Duration>,
  },
  /// A slot was freed while the get waited (`reservable`): the get reserves it without the lock
  SlotFreed {
    waited: Option<Duration>,
  },
}

/// The clones lock of `pool` (`Pool::lock_clones`), or the end of the wait for it once a slot the get has not tried
/// (`tried`) is free: the state it reads for the holder anyway (`clone_holder`, about once a second) tells it, and an
/// error of that read is no slot. Not the modification time of the state: two writes within one tick of the clock look
/// like none. The wait is told (`GetStep::WaitForClone`) when it starts, when the holder or the slot it adds changes (a
/// holder that took the lock a moment ago reserves its slot next), and once a minute
fn lock_clones(pool: &Pool, unknown_timeout: Duration, tried: &HashSet<String>, report: &dyn Fn(GetStep)) -> Result<CloneTurn> {
  let started = Instant::now();
  // the holder told last, and when
  let mut told: Option<(CloneHolder, Instant)> = None;
  let freed = Cell::new(false);
  let lock = pool.lock_clones(
    unknown_timeout,
    &mut |process| {
      let store = pool.read().unwrap_or_else(|error| {
        debug!(error = %format!("{error:#}"), "cannot read the state while waiting for the clones lock");
        LeaseStore::default()
      });
      if store.slots.iter().any(|slot| reservable(slot, tried)) {
        freed.set(true);
        return;
      }
      let holder = clone_holder(&store, process);
      if told.as_ref().is_none_or(|(last, at)| *last != holder || at.elapsed() >= CLONE_WAIT_TICK) {
        report(GetStep::WaitForClone(holder.clone()));
        told = Some((holder, Instant::now()));
      }
    },
    &|| freed.get(),
  );
  let waited = told.map(|_| started.elapsed());
  match lock {
    Ok(lock) => Ok(CloneTurn::Locked { lock, waited }),
    Err(error) if freed.get() && error.is::<LockWaitCanceled>() => Ok(CloneTurn::SlotFreed { waited }),
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
  scan: &ProcessScan,
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
    let mut order: Vec<usize> = (0..store.slots.len())
      .filter(|&index| reservable(&store.slots[index], tried) && store.slots[index].dirty == dirty)
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
  if take_over && let Some(reserved) = reserve_idle(store, git_common_dir, scan, tried, skipped, lease) {
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
  let template = CLONE_AVAILABLE.then(|| mark_template(store, git_common_dir, scan, lease)).flatten();
  let path = next_slot_path(store, pool_dir, git_common_dir);
  store.slots.push(Slot {
    name: String::new(),
    path: display_path(&path),
    dirty: false,
    returned: None,
    lease: Some(lease.clone()),
    reclaimed: None,
  });
  Reserved::New { path, template }
}

/// Whether a get that looked at the slots of `tried` may reserve `slot` as free: not tried yet, and no lease, or one
/// that `settle_abandoned` frees (its process gone, no idle lease to give back). The candidates of `reserve`, and the
/// slot freed meanwhile that ends the wait for the clones lock (`lock_clones`)
fn reservable(slot: &Slot, tried: &HashSet<String>) -> bool {
  !tried.contains(&slot.path) && slot.lease.as_ref().is_none_or(|lease| lease.taking_over.is_none() && lease.is_abandoned())
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

/// Under the pool lock: the idle lease (`Lease::is_idle`) that was quiet longest (`is_quiet`), of a slot of this
/// repository with no process in it but its Bazel servers, taken by the `preparing` lease of the get, which keeps the
/// idle one as `taking_over`; a busy slot is skipped
fn reserve_idle(store: &mut LeaseStore, git_common_dir: &Path, scan: &ProcessScan, tried: &mut HashSet<String>, skipped: &mut Vec<SkippedSlot>, lease: &Lease) -> Option<Reserved> {
  let now = now();
  let mut order: Vec<usize> = (0..store.slots.len())
    .filter(|&index| {
      let slot = &store.slots[index];
      slot.lease.as_ref().is_some_and(|idle| idle.is_idle() && is_quiet(slot, idle, now)) && !tried.contains(&slot.path) && is_worktree_of(Path::new(&slot.path), git_common_dir)
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
  /// Work the get leaves alone; `fresh` when this check found it, not the one before (its verdict still holds)
  Work { check: IdleCheck, fresh: bool },
}

/// Whether the work of the idle lease `old` of the slot at `path` (the worktree `name`) landed, cheap checks first:
/// the verdict of the check before while it holds (`IdleCheck::holds`, one `rev-parse` of the home tip), the commits on
/// git objects only (`landed_work`), a git operation or a resolve session in progress (no git but one `show-ref`), the
/// local changes (one `status`)
fn check_idle(git_executor: &GitCommandExecutor, path: &str, name: &str, old: &Lease, untracked: UntrackedFiles) -> Result<IdleVerdict> {
  let mark = SlotMark::of(Path::new(path));
  let home_tip = match &old.source_branch {
    Some(branch) => resolve_commit(git_executor, path, &format!("refs/heads/{branch}"))?,
    None => None,
  };
  if let Some(check) = old.idle_check.as_ref().filter(|check| check.holds(&mark, home_tip.as_deref())) {
    return Ok(IdleVerdict::Work {
      check: check.clone(),
      fresh: false,
    });
  }
  let work = |commits: Vec<WorkCommit>, paths: Vec<String>, operation: Option<String>| IdleVerdict::Work {
    check: IdleCheck {
      at: now(),
      mark: mark.clone(),
      home_tip: home_tip.clone(),
      commits,
      paths,
      operation,
    },
    fresh: true,
  };
  let landed = landed_work(git_executor, path, &old.source_head, old.source_branch.as_deref(), Purpose::TakeOver)?;
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
    return Ok(work(Vec::new(), paths, None));
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
    slot.reclaimed = Some(Reclaimed::of(&old, Some(lease)));
    Ok(true)
  })
}

/// Give the idle lease `old` back to the slot at `path` that the get of `lease_id` reserved, from the copy of the get: a
/// bd before `taking_over` that wrote the state meanwhile dropped the one in the state. Best effort: a get gone before
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
  /// The record of `old`, taken over by `by` (none: `bd lease reclaim`)
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
}

/// The path of the next new slot: `<pool>/<n>/<repository name>`, the smallest `n` whose directory does not exist and
/// holds no slot of the state
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
/// process in it first; `None` without one, and the source checkout is the template. The free slots left here are the
/// ones the get passed over (dirty or busy): the checkout and the clean of the clone make them right
fn mark_template(store: &mut LeaseStore, git_common_dir: &Path, scan: &ProcessScan, lease: &Lease) -> Option<String> {
  let candidates: Vec<usize> = (0..store.slots.len())
    .filter(|&index| store.slots[index].lease.is_none() && is_worktree_of(Path::new(&store.slots[index].path), git_common_dir))
    .collect();
  let index = candidates
    .iter()
    .copied()
    .find(|&index| scan.inside(Path::new(&store.slots[index].path)).is_empty())
    .or_else(|| candidates.first().copied())?;
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
fn release_marker(pool: &Pool, timeout: Duration, path: &str, marker_id: &str) {
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
  untracked: UntrackedFiles,
  /// What the clone leaves out: the `delete` rules of the source
  skips: &'a CloneSkips,
  /// Hears each step as it starts
  progress: &'a dyn Fn(GetStep),
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
  match move_clean_slot(git_executor, slot, commit, workers)? {
    SlotMove::Kept | SlotMove::Switched => branch.map_or(Ok(()), |branch| attach_new_branch(git_executor, slot, branch, commit)),
    SlotMove::Refused(reason) => {
      report(GetStep::CheckoutEveryFile {
        slot: slot.to_string(),
        reason: reason.lines().next().unwrap_or_default().to_string(),
      });
      checkout_slot(git_executor, slot, commit, branch, workers)
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
  checkout_slot(git_executor, &slot, request.commit, request.branch, request.workers).map_err(Added::Registered)?;
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
/// 3. the template's index copied in and refreshed (`refresh_cloned_index`);
/// 4. `checkout --force` to the commit, `clean -fd`, one `status`: the changes and the untracked files of the
///    template never reach the lease, and its ignored files stay as the warm cache. A tracked file under a skipped path
///    comes back with the checkout
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
  // without the index of the template the checkout writes every file: slow, still right
  match std::fs::copy(template_git_dir.join("index"), worktree.git_dir.join("index")) {
    Ok(_) => {
      (request.progress)(GetStep::Refresh { slot: slot.clone() });
      refresh_cloned_index(git_executor, &slot).map_err(registered)?;
    }
    Err(error) => warn!(template = %request.template, error = %error, "no index of the template to copy"),
  }
  debug!(elapsed_ms = started.elapsed().as_millis(), "refreshed the index of the clone");
  (request.progress)(GetStep::Checkout {
    slot: slot.clone(),
    commit: request.commit.to_string(),
  });
  checkout_slot(git_executor, &slot, request.commit, request.branch, request.workers).map_err(registered)?;
  clean_cloned_slot(git_executor, &slot).map_err(registered)?;
  (request.progress)(GetStep::Status { slot: slot.clone() });
  let changes = slot_changes(git_executor, &slot, request.untracked).map_err(registered)?;
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
  store
    .slots
    .iter_mut()
    .find(|slot| slot.path == path && slot.lease.as_ref().is_some_and(|lease| lease.id == lease_id))
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
    }
    Ok(())
  });
  match released {
    Ok(()) => error.context(format!("The lease was rolled back: {path} is free again")),
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
  /// The caller named no worktree (`bd lease return --lease-id` without `--workspace`): when `workspace` holds no lease
  /// or another one, the slot of `lease_id` is returned, so a coordinator returns the lease of another session from any
  /// checkout of the repository
  pub find_by_lease_id: bool,
  /// Give up the unlanded commits and discard the local changes
  pub confirm_preserved: bool,
  /// How long the return waits for the Bazel servers it asked to exit
  pub server_timeout: Duration,
  /// The program of `bazel shutdown`, which stops the servers on Windows; `None`: `bazel` of the `PATH`
  pub bazel: Option<&'a OsStr>,
  /// Apply the clean rules of `.branchdeck.toml`, with these overrides; `None` (`--no-clean`) applies none
  pub clean: Option<CleanOverrides>,
  pub lock_timeout: Duration,
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
  pub process_scan: bool,
}

/// Give the lease of a slot back. Refused while a process other than `bd` and its ancestors is in the slot, while HEAD
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
/// A host slot (`host_slots`: its holder is `bd-host/<n>`, or the lend table names its lease) is refused (`HostSlot`):
/// the agent of the host lends it to its runs, and only a shrink (`bd-host slots --local --shrink`) gives it back
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
  /// `bd lease return`: a host slot is refused
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
  loop {
    let mut checked = None;
    let step = match return_once(git_executor, params, returning_as, &pool, &path, waited_for.as_ref(), &mut checked) {
      Ok(step) => step,
      // a check refused the slot after another return of the lease gave it back (a get may have leased it again):
      // the refusal is of the next lease, the lease of this return was returned
      Err(error) => match checked.filter(|slot| !holds_lease(&pool, slot).unwrap_or(true)) {
        Some(slot) => return Ok(joined_return(slot, None, false)),
        None => return Err(error),
      },
    };
    match step {
      ReturnStep::Done(returned) => return Ok(*returned),
      ReturnStep::WaitFor(slot) => {
        if let Some(lease) = &slot.lease {
          wait_while_freeing(&pool, &path, &lease.id)?;
        }
        waited_for = Some(*slot);
      }
    }
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
    maintenance: Vec::new(),
    maintenance_started: false,
    trash_left: Vec::new(),
    removed: Vec::new(),
    detached_branch: None,
    cleaned: Vec::new(),
    clean_error: None,
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
/// process died. No lock of the state is held: it is only read
fn wait_while_freeing(pool: &Pool, path: &str, lease_id: &str) -> Result<()> {
  loop {
    let store = pool.read()?;
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

/// One pass of a return; `checked` is the slot with the lease it checks, once no other return of it runs
fn return_once(
  git_executor: &GitCommandExecutor,
  params: &LeaseReturnParams<'_>,
  returning_as: ReturnAs<'_>,
  pool: &Pool,
  path: &str,
  waited_for: Option<&Slot>,
  checked: &mut Option<Slot>,
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
  *checked = Some(slot.clone());
  let scan = ProcessScan::scan();
  let inside = scan.inside(Path::new(&path));
  let maintenance = scan.maintenance_inside(Path::new(&path));
  // the Bazel servers of the slot, which serve only it; every other process keeps the slot busy
  let (servers, others) = slot_servers(&scan, Path::new(&path));
  if !others.is_empty() {
    bail!(LeaseRefusal::SlotBusy {
      path,
      processes: busy_processes(&inside),
    });
  }
  // a resolve session of the slot (`bd lease pick` opens one onto its home): the next lease of the slot would find it
  let session = session_ref(&slot.name);
  if ref_exists(git_executor, &path, &session) {
    bail!(LeaseRefusal::ResolveInProgress { path, session });
  }
  let work = landed_work(git_executor, &path, &lease.source_head, lease.source_branch.as_deref(), Purpose::Return)?;
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
  let returned = finish_return(git_executor, &path, &lease, &changes, params.clean, trash);
  let FinishedReturn {
    removed,
    detached_branch,
    mut cleaned,
    clean_error,
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
    let last_return = store.slots.iter().filter_map(|slot| slot.returned).max().unwrap_or(0);
    let slot = leased_slot(store, &path, &lease.id).ok_or_else(|| anyhow!("The lease of {path} changed while it was returned"))?;
    slot.lease = None;
    slot.dirty = false;
    slot.returned = Some(last_return + 1);
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
    maintenance,
    maintenance_started,
    trash_left,
    removed,
    detached_branch,
    cleaned,
    clean_error,
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
}

/// What a reserved return does to the slot: discard the `changes` it may give up, remove the seeded paths, detach HEAD
/// and apply the clean rules, moving what a `delete` removes into `trash`
fn finish_return(git_executor: &GitCommandExecutor, path: &str, lease: &Lease, changes: &[String], clean: Option<CleanOverrides>, trash: &Trash) -> Result<FinishedReturn> {
  if !changes.is_empty() {
    discard_slot_changes(git_executor, path)?;
  }
  let removed = remove_seeded(Path::new(path), &lease.seeded, trash)?;
  let detached_branch = read_head(git_executor, path)?.and_then(|(_, branch)| branch);
  if detached_branch.is_some() {
    detach_waiting_for_head_lock(git_executor, path)?;
  }
  // the rules of the source, as the seed is read; the slot's own when the source is gone. They never fail the return
  let rules_from = Some(Path::new(&lease.source))
    .filter(|source| !lease.source.is_empty() && source.is_dir())
    .unwrap_or_else(|| Path::new(path));
  let (cleaned, clean_error) = match clean.map(|overrides| read_clean_rules(rules_from).map(|rules| overrides.apply(rules))) {
    None => (Vec::new(), None),
    Some(Ok(rules)) => (apply_clean_rules(git_executor, path, &rules, CleanMode::Trash(trash)), None),
    Some(Err(error)) => (Vec::new(), Some(format!("{error:#}"))),
  };
  Ok(FinishedReturn {
    removed,
    detached_branch,
    cleaned,
    clean_error,
  })
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

/// How `bd lease status` sees a slot, from the state alone
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
  Free,
  Preparing,
  Ready,
  /// A free slot that a get clones into a new one
  Templating,
  /// Its return runs
  Returning,
  /// A free slot that `bd lease clean` cleans
  Cleaning,
  /// A lease that is not `ready` and whose process is gone: the next get takes the slot as free
  Abandoned,
}

/// A slot of `bd lease status`
#[derive(Debug, Clone)]
pub struct SlotStatus {
  pub slot: Slot,
  pub state: SlotState,
  /// The directory is gone
  pub missing: bool,
  /// With `processes`: the processes in it, except the session of `bd`, git's maintenance and `caffeinate`
  pub processes: Option<Vec<BusyProcess>>,
}

/// Where the commits of a lease go (`bd lease pick`): the source checkout of its get and the branch it had checked out
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
  let (slots, _) = lease_status_core(git_common_dir, false)?;
  Ok(
    slots
      .into_iter()
      .find(|status| status.state == SlotState::Ready && status.slot.path == top_level && status.slot.lease.is_some())
      .map(|status| status.slot),
  )
}

/// Why a command of `bd` in the worktree `top_level` must not run: the slot's lease was taken over from a dead holder
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
      if scan.is_available() && !scan.has_ancestor(holder, lease.holder_started) {
        Ok(refusal(reclaimed, false))
      } else {
        Ok(None)
      }
    }
    Some(_) => Ok(None),
  }
}

/// Every slot of the pool from the state, no git: with `processes`, the process scan of each slot too
pub fn lease_status_core(git_common_dir: &Path, processes: bool) -> Result<(Vec<SlotStatus>, bool)> {
  let mut store = Pool::new(Path::new(&canonical_path(git_common_dir))).read()?;
  // a get gone while it took an idle lease over gave it back: the state as the next writer settles it
  store.slots.iter_mut().for_each(settle_abandoned_takeover);
  let scan = if processes { ProcessScan::scan() } else { ProcessScan::Unavailable };
  let slots = store
    .slots
    .into_iter()
    .map(|slot| {
      let state = match &slot.lease {
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
      SlotStatus { slot, state, missing, processes }
    })
    .collect();
  Ok((slots, scan.is_available()))
}

/// What `bd lease clean` cleans
pub struct LeaseCleanParams<'a> {
  /// The checkout bd runs in: its `.branchdeck.toml` holds the rules, and without `free` and `dirs` it is the target
  pub repository: &'a RepositoryDirs,
  /// The free slots of the pool
  pub free: bool,
  /// Worktrees to clean
  pub dirs: &'a [PathBuf],
  /// Delete, the default; false (`--dry-run`) only measures
  pub apply: bool,
  /// The parameters of this call that change the rules of the file
  pub overrides: CleanOverrides,
  /// The wait for the pool lock, for the `cleaning` marker of a slot
  pub lock_timeout: Duration,
}

/// A checkout `bd lease clean` cleaned, or skipped
#[derive(Debug, Clone)]
pub struct CleanTarget {
  pub path: String,
  /// Why it was not cleaned: a lease, a process in it
  pub skipped: Option<String>,
  pub cleaned: Vec<Cleaned>,
  /// What the trash kept: a path that could not be deleted
  pub trash_left: Vec<String>,
}

/// Apply the rules of `.branchdeck.toml` of the repository, read once, to the free slots, to `dirs`, or to the
/// repository itself. A slot of the pool with a lease or with a process in it is skipped, as a get would skip it; a
/// checkout that is no slot is cleaned as asked. The rules that can't be read are the error, and nothing is cleaned
pub fn lease_clean_core(git_executor: &GitCommandExecutor, params: &LeaseCleanParams<'_>) -> Result<(Vec<CleanTarget>, bool)> {
  let rules = params.overrides.apply(read_clean_rules(Path::new(&params.repository.top_level))?);
  let pool = Pool::new(Path::new(&canonical_path(Path::new(&params.repository.git_common_dir))));
  let store = pool.read()?;
  let mut paths: Vec<String> = Vec::new();
  if params.free {
    paths.extend(
      store
        .slots
        .iter()
        .filter(|slot| slot.lease.is_none() && Path::new(&slot.path).is_dir())
        .map(|slot| slot.path.clone()),
    );
  }
  paths.extend(params.dirs.iter().map(|dir| canonical_path(dir)));
  if !params.free && params.dirs.is_empty() {
    paths.push(canonical_path(Path::new(&params.repository.top_level)));
  }
  let mut seen = HashSet::new();
  paths.retain(|path| seen.insert(path.clone()));
  let scan = ProcessScan::scan();
  let marker = Lease {
    id: format!("clean-{}", uuid::Uuid::new_v4()),
    holder: "bd lease clean".to_string(),
    leased_at: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs()),
    state: LeaseState::Cleaning,
    pid: std::process::id(),
    process_started: process_started(std::process::id()),
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
  };
  let trash = Trash::new(&pool.trash_dir());
  let mode = if params.apply { CleanMode::Trash(&trash) } else { CleanMode::Measure };
  let mut targets: Vec<CleanTarget> = paths
    .into_iter()
    .map(|path| {
      let is_slot = store.slots.iter().any(|slot| slot.path == path);
      let processes = busy_processes(&scan.inside(Path::new(&path)));
      // a slot is cleaned only under the `cleaning` marker, set while it is free, so no get leases it meanwhile; a dry
      // run deletes nothing and only reads the state again
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
      CleanTarget {
        path,
        skipped,
        cleaned,
        trash_left: Vec::new(),
      }
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

/// What `bd lease reclaim --idle` reclaims
pub struct LeaseReclaimParams<'a> {
  /// A checkout of the repository: its config and its clean rules
  pub repository: &'a RepositoryDirs,
  /// Reclaim, the default; false (`--dry-run`) only checks and writes nothing
  pub apply: bool,
  /// The parameters of the call that change the clean rules of the file
  pub clean: CleanOverrides,
  /// How long the reclaim waits for the Bazel servers it asked to exit
  pub server_timeout: Duration,
  /// The program of `bazel shutdown`, which stops the servers on Windows; `None`: `bazel` of the `PATH`
  pub bazel: Option<&'a OsStr>,
  pub lock_timeout: Duration,
}

/// What `bd lease reclaim` did with each idle lease of the pool
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

/// Why `bd lease reclaim` left an idle lease for now
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdleSkip {
  /// Its get or the last move of its HEAD is younger than `IDLE_QUIET`
  NotQuiet,
  /// A process of another session works in it, or a Bazel server of it did not exit
  Busy { processes: Vec<BusyProcess> },
  /// It changed while the reclaim checked it: a get took it over, a session committed in it
  Changed,
  /// The check or the return failed
  Failed { error: String },
}

/// Return every idle lease of the pool whose work landed (`check_idle`: the commits in its home branch, a clean tree,
/// no operation in progress), each under its own CAS on its lease, its holder gone and its slot unchanged, as a return
/// does: its Bazel servers stopped, its seeded paths removed, its branch detached, the clean rules applied into the
/// trash, the slot free with the record of its lease (`Slot::reclaimed`). The trash is deleted once, after the last
/// slot, so no slot waits for the delete of another's build output. One with work is kept, its check recorded for the
/// gets; with `apply` false nothing is written
pub fn lease_reclaim_core(git_executor: &GitCommandExecutor, params: &LeaseReclaimParams<'_>) -> Result<LeaseReclaim> {
  let git_common_dir = PathBuf::from(canonical_path(Path::new(&params.repository.git_common_dir)));
  let pool = Pool::new(&git_common_dir);
  let store = pool.read()?;
  let scan = ProcessScan::scan();
  let untracked = lease_config(git_executor, &params.repository.top_level)?.untracked;
  let trash = Trash::new(&pool.trash_dir());
  let now = now();
  let mut answer = LeaseReclaim {
    process_scan: scan.is_available(),
    ..LeaseReclaim::default()
  };
  for slot in store.slots {
    let Some(lease) = slot.lease.clone().filter(|lease| lease.is_idle() && is_worktree_of(Path::new(&slot.path), &git_common_dir)) else {
      continue;
    };
    let path = slot.path.clone();
    let skip = |reason: IdleSkip| SkippedIdle {
      path: path.clone(),
      lease: lease.clone(),
      reason,
    };
    if !is_quiet(&slot, &lease, now) {
      answer.skipped.push(skip(IdleSkip::NotQuiet));
      continue;
    }
    let (servers, others) = slot_servers(&scan, Path::new(&path));
    if !others.is_empty() {
      answer.skipped.push(skip(IdleSkip::Busy {
        processes: busy_processes(&others),
      }));
      continue;
    }
    // a check of its own, whatever a get found before
    let unchecked = Lease {
      idle_check: None,
      ..lease.clone()
    };
    let (landed, mark) = match check_idle(git_executor, &path, &slot.name, &unchecked, untracked) {
      Ok(IdleVerdict::Landed { landed, mark }) => (landed, mark),
      Ok(IdleVerdict::Work { check, .. }) => {
        if params.apply {
          record_idle_check(&pool, params.lock_timeout, &path, &lease.id, &check);
        }
        answer.kept.push(KeptLease { path, lease, check });
        continue;
      }
      Err(error) => {
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
    };
    if !params.apply {
      answer.reclaimed.push(reclaimed);
      continue;
    }
    if !reserve_reclaim(&pool, params.lock_timeout, &path, &lease.id, &mark)? {
      answer.skipped.push(skip(IdleSkip::Changed));
      continue;
    }
    let servers: Vec<&ProcessInfo> = servers.iter().collect();
    let ServerStop { stopped, running } = stop_servers(&servers, params.bazel, params.server_timeout);
    reclaimed.stopped_servers = stopped;
    if !running.is_empty() {
      restore_ready(&pool, params.lock_timeout, &path, &lease.id);
      answer.skipped.push(skip(IdleSkip::Busy { processes: running }));
      continue;
    }
    match finish_return(git_executor, &path, &lease, &[], Some(params.clean), &trash) {
      Ok(finished) => reclaimed.cleaned = finished.cleaned,
      Err(error) => {
        restore_ready(&pool, params.lock_timeout, &path, &lease.id);
        answer.skipped.push(skip(IdleSkip::Failed { error: format!("{error:#}") }));
        continue;
      }
    }
    pool.update(params.lock_timeout, |store| {
      let last_return = store.slots.iter().filter_map(|slot| slot.returned).max().unwrap_or(0);
      if let Some(slot) = leased_slot(store, &path, &lease.id) {
        slot.lease = None;
        slot.dirty = false;
        slot.returned = Some(last_return + 1);
        slot.reclaimed = Some(Reclaimed::of(&lease, None));
      }
      Ok(())
    })?;
    debug!(slot = %path, "an idle lease is reclaimed: its slot is free");
    answer.reclaimed.push(reclaimed);
  }
  // every reclaimed slot is free: the delete of what the rules moved into the trash keeps no slot waiting
  debug!(slots = answer.reclaimed.len(), "deleting the trash of the reclaimed slots");
  for reclaimed in &mut answer.reclaimed {
    reclaimed.trash_left = trash.empty(&mut reclaimed.cleaned);
  }
  Ok(answer)
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

/// The lease `lease_id` of `path` `ready` again, after a reclaim that could not finish; best effort
fn restore_ready(pool: &Pool, timeout: Duration, path: &str, lease_id: &str) {
  let restored = pool.update(timeout, |store| {
    if let Some(lease) = leased_slot(store, path, lease_id).and_then(|slot| slot.lease.as_mut()) {
      lease.state = LeaseState::Ready;
    }
    Ok(())
  });
  if let Err(error) = restored {
    warn!(slot = %path, error = %format!("{error:#}"), "cannot give the lease back after a failed reclaim");
  }
}

/// Record `check` on the idle lease `lease_id` of `path`, for the gets after it; best effort
fn record_idle_check(pool: &Pool, timeout: Duration, path: &str, lease_id: &str, check: &IdleCheck) {
  let recorded = pool.update(timeout, |store| {
    if let Some(lease) = leased_slot(store, path, lease_id).and_then(|slot| slot.lease.as_mut()) {
      lease.idle_check = Some(check.clone());
    }
    Ok(())
  });
  if let Err(error) = recorded {
    warn!(slot = %path, error = %format!("{error:#}"), "cannot record the check of an idle lease");
  }
}

/// A directory `bd lease adopt` added, or refused
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
/// What `bd lease adopt` adds
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

/// Record the worktree at `path` (moved from `moved_from`) as a free slot, under the pool lock
fn add_slot_entry(pool: &Pool, lock_timeout: Duration, path: &str, moved_from: Option<String>) -> AdoptResult {
  let name = linked_worktree(Path::new(path)).map_or_default(|worktree| worktree.id);
  pool.update(lock_timeout, |store| {
    if store.slots.iter().any(|slot| slot.path == path) {
      return Ok(Err(LeaseRefusal::InPool(path.to_string())));
    }
    let slot = Slot {
      name,
      path: path.to_string(),
      dirty: false,
      returned: None,
      lease: None,
      reclaimed: None,
    };
    store.slots.push(slot.clone());
    Ok(Ok((slot, moved_from)))
  })
}

/// `--move`: under the pool lock, the next slot path, then `git worktree move` there (a rename on one volume, so the
/// lock is held for a moment; under the lock of the pool's `worktree` commands too) and the free slot recorded. The
/// parent of the old directory goes when it is left empty (`<treehouse pool>/<n>/`); the state file of Treehouse is not
/// touched, and Treehouse sees the old path as missing
fn move_in(git_executor: &GitCommandExecutor, pool: &Pool, params: &LeaseAdoptParams<'_>, git_common_dir: &Path, pool_dir: &Path, path: &str) -> AdoptResult {
  if !same_volume(Path::new(path), pool_dir) {
    return Ok(Err(LeaseRefusal::OtherVolume {
      path: path.to_string(),
      pool: display_path(pool_dir),
    }));
  }
  let moved = pool.update(params.lock_timeout, |store| {
    if store.slots.iter().any(|slot| slot.path == path) {
      return Ok(Err(LeaseRefusal::InPool(path.to_string())));
    }
    let target = next_slot_path(store, pool_dir, git_common_dir);
    let cannot_move = |reason: String| LeaseRefusal::CannotMove { path: path.to_string(), reason };
    if let Some(parent) = target.parent()
      && let Err(error) = std::fs::create_dir_all(parent)
    {
      return Ok(Err(cannot_move(error.to_string())));
    }
    if let Err(error) = pool.worktree_command(params.lock_timeout, || move_slot(git_executor, params.repository, path, &target)) {
      if let Some(parent) = target.parent() {
        let _ = fs_retry::remove_dir(parent);
      }
      return Ok(Err(cannot_move(format!("{error:#}"))));
    }
    let target = display_path(&target);
    let slot = Slot {
      name: linked_worktree(Path::new(&target)).map_or_default(|worktree| worktree.id),
      path: target,
      dirty: false,
      returned: None,
      lease: None,
      reclaimed: None,
    };
    store.slots.push(slot.clone());
    Ok(Ok(slot))
  })?;
  let slot = match moved {
    Ok(slot) => slot,
    Err(refusal) => return Ok(Err(refusal)),
  };
  if let Some(parent) = Path::new(path).parent() {
    // only an empty one goes
    let _ = fs_retry::remove_dir(parent);
  }
  Ok(Ok((slot, Some(path.to_string()))))
}

/// Whether `a` and `b` are on one volume (`st_dev`)
#[cfg(unix)]
fn same_volume(a: &Path, b: &Path) -> bool {
  use std::os::unix::fs::MetadataExt;
  matches!((std::fs::metadata(a), std::fs::metadata(b)), (Ok(a), Ok(b)) if a.dev() == b.dev())
}

/// Elsewhere than on Unix the move is left to git
#[cfg(not(unix))]
const fn same_volume(_a: &Path, _b: &Path) -> bool {
  true
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
