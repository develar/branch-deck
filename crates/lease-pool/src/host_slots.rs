//! The host slots (spec/cli/lease.spec.md, "Host slots"; "Host slots" in docs/design/git-operations.md): the slots of
//! the pool that the remote-run agent of a host (yonderd, through `bd lease host`) keeps for the runs of other machines,
//! each a plain `ready` lease of the holder `bd-host/<n>`
//! in `leases.json`, and the lend table in `$GIT_COMMON_DIR/branch-deck/host-slots.json`, which lends a slot to one run
//! for the duration of that run. A remote slot holds no state of its own: the commit comes from the client, and the
//! outputs go back to it, so a slot is free once its run ended and its processes are gone.
//!
//! The lock of the table (`host-slots.lock`) is a leaf: it is held only while the table changes, never across git, a
//! clone, a process scan or another lock of the pool. So the order of the locks, host slots before the clones lock, the
//! pool lock and the lock of the `worktree` commands, holds by itself. Every state but `idle` and `quarantined` names
//! the process that acts on the slot; once that process is gone, the state is abandoned and `reclaim` finishes the work
//! (a give-back, a clean, a shrink). Nothing here starts a git of its own: the git of a slot is
//! `git_ops::lease_slot`, the gets and the returns are those of `lease`.

use crate::free_space::available_bytes;
use crate::lease::{
  BusyProcess, CleanTarget, GetStep, LeaseGetParams, LeaseRefusal, LeaseReturnParams, LeaseReturned, LeaseState, Pool, canonical_path, get_host_slot, return_host_slot,
};
use crate::lease_clean::{CleanMode, CleanOverrides, Trash, apply_clean_rules, read_clean_rules};
use crate::state_file::StateFile;
use anyhow::{Result, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::{LeaseConfig, checkout_slot, discard_slot_changes, lease_config, linked_worktree, slot_changes};
use git_ops::revs::RepositoryDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sync_utils::paths::display_path;
use sync_utils::process_scan::{ProcessInfo, ProcessScan, is_running, kill, process_started, terminate};
use sync_utils::size::parse_size;
use tracing::{debug, warn};

/// The prefix of the holders of the host slots: `bd-host/1`, `bd-host/2`, …
pub const HOST_HOLDER_PREFIX: &str = "bd-host/";
const STATE_FILE: &str = "host-slots.json";
const LOCK_FILE: &str = "host-slots.lock";
/// The host slots of a repository without `branchdeck.hostSlots`
pub const DEFAULT_HOST_SLOTS: u32 = 4;
/// Without `branchdeck.hostSlotMaxSize`, an idle slot is cleaned after this many runs …
const CLEAN_AFTER_RUNS: u32 = 50;
/// … or this long after its last clean
const CLEAN_AFTER: Duration = Duration::from_hours(7 * 24);
/// How long a give-back waits for the processes of the run to exit on SIGTERM, before SIGKILL
const STOP_WAIT: Duration = Duration::from_secs(10);
/// How long it waits for them after SIGKILL
const KILL_WAIT: Duration = Duration::from_secs(2);
/// How exactly the system tells the start of a process (`ProcessInfo::started_at`): to the microsecond on macOS, to the
/// second on Linux. A process counts as the run's only when it started that much after the millisecond of the lend
/// (`since_ms`, which a process that started later in it may still be older than), so a process older than the lend is
/// never killed; a process of the run that started within it quarantines the slot until it is gone
#[cfg(target_os = "linux")]
const START_PRECISION: Duration = Duration::from_secs(1);
#[cfg(not(target_os = "linux"))]
const START_PRECISION: Duration = Duration::ZERO;

/// The lend table of one repository
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSlots {
  #[serde(default)]
  pub slots: Vec<HostSlot>,
  /// The runs that wait for a slot, first come first
  #[serde(default)]
  pub queue: Vec<Ticket>,
  /// The count the last shrink kept: a fill adds no slot past it, so a shrink beside a running agent of the host stays
  /// (`set_limit` lifts it)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub limit: Option<u32>,
}

/// A host slot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSlot {
  /// 1, 2, …: its lease is of the holder `bd-host/<n>`
  pub n: u32,
  /// The id of its lease in `leases.json`; empty while its fill runs
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub lease_id: String,
  /// The top level, canonical, as the pool names the slot; empty while its fill runs
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub path: String,
  /// When it was last lent, in Unix milliseconds: the idle slot used longest ago is lent first
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub used: Option<u64>,
  /// The runs it served since its last clean
  #[serde(default)]
  pub runs_since_clean: u32,
  /// When it was last cleaned (or filled), in Unix seconds
  #[serde(default)]
  pub cleaned_at: u64,
  /// One more each time something removes files of the slot: a give-back that discards changes, a clean. What an agent
  /// made in the slot (the outputs of its prepare steps) holds only within one generation, and the table moves it before
  /// the removal, so no next run and no retry after a crash sees a stale one
  #[serde(default)]
  pub generation: u64,
  /// The affinity of the run it served last (`checkout-<12 hex>` of the client's checkout): a run of the same checkout
  /// gets it first, with the outputs of its last run
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub affinity: Option<String>,
  pub state: HostState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostState {
  /// Ready for a run
  Idle,
  /// A run has it
  Lent(Lent),
  /// Its get runs (`fill`)
  Filling(Owner),
  /// Its give-back runs: the processes of the run are stopped, then the changes discarded; `since_ms` is of the lend
  GivingBack { owner: Owner, since_ms: u64 },
  /// The clean rules run in it (`clean_slot`)
  Cleaning(Owner),
  /// It goes back to the pool (`shrink`)
  Shrinking(Owner),
  /// A process older than its last lend, or one that outlived SIGKILL, works in it: no run gets it until the process is
  /// gone (`reclaim`)
  Quarantined { processes: Vec<HostProcess> },
}

/// The process that acts on a slot; pid 0 is none, a state no process acts on, which `reclaim` takes up
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owner {
  pub pid: u32,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub process_started: Option<u64>,
}

impl Owner {
  fn this() -> Self {
    let pid = std::process::id();
    Self {
      pid,
      process_started: process_started(pid),
    }
  }

  const fn nobody() -> Self {
    Self { pid: 0, process_started: None }
  }

  fn is_alive(&self) -> bool {
    self.pid != 0 && is_running(self.pid, self.process_started)
  }
}

/// The lend of a slot to a run
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lent {
  /// The id of the run
  pub run: String,
  /// The process that runs it (the supervisor of the run) and its start: the lend of a gone process is abandoned
  pub pid: u32,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub process_started: Option<u64>,
  /// When it was lent, in Unix milliseconds: a process in the slot that started later is the run's
  pub since_ms: u64,
}

/// A run that waits for a slot; the ticket of a gone process leaves the queue
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ticket {
  pub run: String,
  pub pid: u32,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub process_started: Option<u64>,
}

/// A process that keeps a slot in quarantine
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostProcess {
  pub pid: u32,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub started: Option<u64>,
  /// The command name, as `ps -o comm` shows it
  pub name: String,
  /// The arguments as one line, at most 200 characters
  pub command: String,
}

impl HostProcess {
  fn of(process: &ProcessInfo) -> Self {
    let busy = BusyProcess::of(process);
    Self {
      pid: process.pid,
      started: process.started,
      name: busy.name,
      command: busy.command,
    }
  }

  fn is_running(&self) -> bool {
    is_running(self.pid, self.started)
  }
}

/// What the table refuses
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HostSlotRefusal {
  #[error("There is no host slot {0}")]
  UnknownSlot(u32),
  #[error("Host slot {n} is not lent to the run {run}")]
  NotLent { n: u32, run: String },
  #[error("Host slot {n} is {state}")]
  Busy { n: u32, state: String },
  #[error("{key} {value:?}: {reason}")]
  InvalidConfig { key: String, value: String, reason: String },
}

/// The repository of the host slots
pub struct HostSlotsParams<'a> {
  /// The checkout of the host whose HEAD a fill leases slots at; its common dir holds the table and the pool
  pub repository: &'a RepositoryDirs,
  /// The home directory of the default pool directory
  pub home: Option<&'a Path>,
  /// The wait for the lock of the table and of the pool
  pub lock_timeout: Duration,
  /// How long a get or a return waits for the Bazel servers it asked to exit
  pub server_timeout: Duration,
}

impl HostSlotsParams<'_> {
  fn table(&self) -> StateFile {
    table_file(Path::new(&canonical_path(Path::new(&self.repository.git_common_dir))))
  }

  fn pool(&self) -> Pool {
    Pool::new(Path::new(&canonical_path(Path::new(&self.repository.git_common_dir))))
  }

  fn update<R>(&self, change: impl FnOnce(&mut HostSlots) -> Result<R>) -> Result<R> {
    self.table().update(self.lock_timeout, change)
  }
}

fn table_file(git_common_dir: &Path) -> StateFile {
  StateFile::new(git_common_dir, STATE_FILE, LOCK_FILE, "the host slots")
}

/// The lend table of the repository whose `$GIT_COMMON_DIR` is `git_common_dir`, read without the lock
pub fn read_host_slots(git_common_dir: &Path) -> Result<HostSlots> {
  table_file(Path::new(&canonical_path(git_common_dir))).read()
}

/// The host slot that `lease` is, by its holder `bd-host/<n>` or by the table, which names the lease by its id (not by
/// the path: the record of a shrink that died after the return would name a path that another session leased since).
/// The table is read without its lock, which is a leaf and never taken under the pool lock
pub(crate) fn host_slot_number(git_common_dir: &Path, lease: &crate::lease::Lease) -> Result<Option<u32>> {
  if let Some(n) = lease.holder.strip_prefix(HOST_HOLDER_PREFIX) {
    return Ok(Some(n.parse().unwrap_or(0)));
  }
  let table = read_host_slots(git_common_dir)?;
  Ok(table.slots.iter().find(|slot| !slot.lease_id.is_empty() && slot.lease_id == lease.id).map(|slot| slot.n))
}

fn now_ms() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
}

fn now_secs() -> u64 {
  SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

/// `branchdeck.hostSlots`, else `DEFAULT_HOST_SLOTS`
fn slot_count(config: &LeaseConfig) -> Result<u32> {
  config.host_slots.as_ref().map_or(Ok(DEFAULT_HOST_SLOTS), |value| {
    value.parse().map_err(|error: std::num::ParseIntError| {
      HostSlotRefusal::InvalidConfig {
        key: "branchdeck.hostSlots".to_string(),
        value: value.clone(),
        reason: error.to_string(),
      }
      .into()
    })
  })
}

/// A size of the config, `None` when unset
fn config_size(key: &str, value: Option<&String>) -> Result<Option<u64>> {
  value
    .map(|value| {
      parse_size(value).map_err(|reason| {
        HostSlotRefusal::InvalidConfig {
          key: key.to_string(),
          value: value.clone(),
          reason,
        }
        .into()
      })
    })
    .transpose()
}

/// A run that asks for a slot
pub struct LendRequest<'a> {
  /// The id of the run
  pub run: &'a str,
  /// The process that runs it and waits for the slot, its own for each run (the supervisor): its ticket and its lend
  /// last as long as it
  pub pid: u32,
  pub process_started: Option<u64>,
  /// `checkout-<12 hex>` of the client's checkout
  pub affinity: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LendAnswer {
  Lent(Lend),
  Queued(Queued),
}

/// The slot lent to a run
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lend {
  pub n: u32,
  pub path: String,
  pub lease_id: String,
  /// When it was last cleaned or filled, in Unix seconds
  pub cleaned_at: u64,
  /// The generation of the files of the slot (`HostSlot::generation`): the agent keys what its prepare steps made in
  /// the slot by it, since a give-back or a clean can delete that
  pub generation: u64,
  /// When the same affinity used it last, in Unix milliseconds: the wait line says "last used by this checkout 12 min
  /// ago"
  pub last_used_by_affinity: Option<u64>,
}

/// No slot for the run yet: its place, and what the slots are doing, for the line of the wait
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queued {
  /// The runs before it in the queue
  pub ahead: usize,
  /// Every slot of the table
  pub slots: usize,
  /// The runs that have a slot: the number of the slot and the run
  pub lent: Vec<(u32, String)>,
  /// The slots in quarantine with the processes that keep them
  pub quarantined: Vec<(u32, Vec<HostProcess>)>,
}

/// Lend a slot to the run of `request`, or put it in the queue. Under the lock of the table, without git: the tickets of
/// gone processes leave the queue; a run that has a slot gets it again (a repeat is safe); the run's ticket joins the
/// queue; a run gets an idle slot whose lease `leases.json` still holds as `ready` when fewer runs wait before it than
/// there are such slots: the slot its affinity used last first, else the one used longest ago. An abandoned lend is not
/// lent again: `reclaim` gives it back first, since its run may have left processes and changes in it
pub fn lend(params: &HostSlotsParams<'_>, request: &LendRequest<'_>) -> Result<LendAnswer> {
  let store = params.pool().read()?;
  let ready: HashSet<String> = store
    .slots
    .iter()
    .filter_map(|slot| slot.lease.as_ref())
    .filter(|lease| lease.state == LeaseState::Ready)
    .map(|lease| lease.id.clone())
    .collect();
  params.update(|table| {
    table.queue.retain(|ticket| is_running(ticket.pid, ticket.process_started));
    if let Some(slot) = table.slots.iter().find(|slot| matches!(&slot.state, HostState::Lent(lent) if lent.run == request.run)) {
      return Ok(LendAnswer::Lent(Lend {
        n: slot.n,
        path: slot.path.clone(),
        lease_id: slot.lease_id.clone(),
        cleaned_at: slot.cleaned_at,
        generation: slot.generation,
        last_used_by_affinity: None,
      }));
    }
    let position = table.queue.iter().position(|ticket| ticket.run == request.run).unwrap_or_else(|| {
      table.queue.push(Ticket {
        run: request.run.to_string(),
        pid: request.pid,
        process_started: request.process_started,
      });
      table.queue.len() - 1
    });
    let idle: Vec<usize> = (0..table.slots.len())
      .filter(|&index| table.slots[index].state == HostState::Idle && ready.contains(&table.slots[index].lease_id))
      .collect();
    if idle.len() > position {
      let affine = |index: &&usize| request.affinity.is_some() && table.slots[**index].affinity.as_deref() == request.affinity;
      let index = idle
        .iter()
        .find(affine)
        .or_else(|| idle.iter().min_by_key(|&&index| table.slots[index].used))
        .copied()
        .unwrap_or(idle[0]);
      let slot = &mut table.slots[index];
      let last_used_by_affinity = slot.used.filter(|_| request.affinity.is_some() && slot.affinity.as_deref() == request.affinity);
      let since_ms = now_ms();
      slot.state = HostState::Lent(Lent {
        run: request.run.to_string(),
        pid: request.pid,
        process_started: request.process_started,
        since_ms,
      });
      slot.used = Some(since_ms);
      slot.affinity = request.affinity.map(str::to_string);
      let lend = Lend {
        n: slot.n,
        path: slot.path.clone(),
        lease_id: slot.lease_id.clone(),
        cleaned_at: slot.cleaned_at,
        generation: slot.generation,
        last_used_by_affinity,
      };
      table.queue.remove(position);
      return Ok(LendAnswer::Lent(lend));
    }
    Ok(LendAnswer::Queued(Queued {
      ahead: position,
      slots: table.slots.len(),
      lent: table
        .slots
        .iter()
        .filter_map(|slot| match &slot.state {
          HostState::Lent(lent) => Some((slot.n, lent.run.clone())),
          _ => None,
        })
        .collect(),
      quarantined: table
        .slots
        .iter()
        .filter_map(|slot| match &slot.state {
          HostState::Quarantined { processes } => Some((slot.n, processes.clone())),
          _ => None,
        })
        .collect(),
    }))
  })
}

/// Take the run's ticket out of the queue: it stopped waiting
pub fn leave_queue(params: &HostSlotsParams<'_>, run: &str) -> Result<()> {
  params.update(|table| {
    table.queue.retain(|ticket| ticket.run != run);
    Ok(())
  })
}

/// What a give-back did
#[derive(Debug, Clone)]
pub struct GaveBack {
  pub n: u32,
  /// The processes of the run it stopped
  pub stopped: Vec<HostProcess>,
  /// The processes that keep the slot in quarantine; empty when the slot is idle again
  pub quarantined: Vec<HostProcess>,
  /// The local changes it discarded
  pub discarded: Vec<String>,
  /// The clean that followed, when one was due (`clean_due`)
  pub cleaned: Option<CleanTarget>,
  /// Whether the processes were checked (`ProcessScan::Unavailable` on Windows)
  pub processes_checked: bool,
}

/// Which slot a give-back or a clean takes, by its state
#[derive(Clone, Copy)]
enum Take<'a> {
  /// The lend of this run
  Run(&'a str),
  /// An idle slot
  Idle,
  /// A state whose process is gone
  Abandoned,
  /// A slot in quarantine
  Quarantined,
}

/// Give back the slot `n` that the run `run` had: off the path of the client, once the run's command group was killed.
/// The processes in the slot that started after the lend are the run's: SIGTERM, up to 10 s, then SIGKILL (each only
/// while its pid still has the start time of the scan). The Bazel servers of the slot stay, as its warm build state; a
/// process older than the lend is never signalled. Any process left after that quarantines the slot, which keeps its
/// changes. Else the changes are discarded (`reset --hard`, `clean -fd`; the ignored files stay as the warm cache,
/// and a clean slot runs neither) and the slot is idle again; then the clean rules run, when they are due
pub fn give_back(git_executor: &GitCommandExecutor, params: &HostSlotsParams<'_>, n: u32, run: &str) -> Result<GaveBack> {
  give_back_core(git_executor, params, n, Take::Run(run))
}

/// Give back the slot that the run `run` has (`give_back`), found by the run; `None` when it has none, and then its
/// ticket leaves the queue: the one call of an agent on every way out of a run, also when the run never got a slot, or
/// when a kill cut its lend after the table recorded it
pub fn give_back_run(git_executor: &GitCommandExecutor, params: &HostSlotsParams<'_>, run: &str) -> Result<Option<GaveBack>> {
  let table: HostSlots = params.table().read()?;
  let lent = table
    .slots
    .iter()
    .find(|slot| matches!(&slot.state, HostState::Lent(lent) if lent.run == run))
    .map(|slot| slot.n);
  match lent {
    Some(n) => give_back(git_executor, params, n, run).map(Some),
    None => leave_queue(params, run).map(|()| None),
  }
}

fn give_back_core(git_executor: &GitCommandExecutor, params: &HostSlotsParams<'_>, n: u32, take: Take<'_>) -> Result<GaveBack> {
  let me = Owner::this();
  let (path, since_ms) = params.update(|table| {
    let slot = slot_mut(table, n)?;
    let since_ms = match (&slot.state, take) {
      (HostState::Lent(lent), Take::Run(run)) if lent.run == run => lent.since_ms,
      (HostState::Lent(lent), Take::Abandoned) if !is_running(lent.pid, lent.process_started) => lent.since_ms,
      (HostState::GivingBack { owner, since_ms }, Take::Abandoned) if !owner.is_alive() => *since_ms,
      // the run that put it there is gone: every process in it now is older than any lend
      (HostState::Quarantined { .. }, Take::Quarantined) => u64::MAX,
      (_, Take::Run(run)) => bail!(HostSlotRefusal::NotLent { n, run: run.to_string() }),
      (state, _) => bail!(busy(n, state)),
    };
    slot.state = HostState::GivingBack { owner: me.clone(), since_ms };
    Ok((slot.path.clone(), since_ms))
  })?;
  let mine = |state: &HostState| matches!(state, HostState::GivingBack { owner, .. } if *owner == me);
  let stopped = stop_processes(&path, since_ms);
  if !stopped.left.is_empty() {
    debug!(slot = %path, processes = stopped.left.len(), "a process stays in the host slot: quarantined");
    let left = stopped.left.clone();
    set_state(params, n, mine, |slot| slot.state = HostState::Quarantined { processes: left })?;
    return Ok(GaveBack {
      n,
      stopped: stopped.stopped,
      quarantined: stopped.left,
      discarded: Vec::new(),
      cleaned: None,
      processes_checked: stopped.checked,
    });
  }
  let reset = (|| {
    let config = lease_config(git_executor, &path)?;
    let changes = slot_changes(git_executor, &path, config.untracked)?;
    if !changes.is_empty() {
      // the new generation is written before the discard: a give-back that dies after the discard leaves a clean slot,
      // whose retry by `reclaim` discards nothing and would move no generation
      set_state(params, n, mine, |slot| slot.generation = slot.generation.saturating_add(1))?;
      discard_slot_changes(git_executor, &path)?;
    }
    Ok::<_, anyhow::Error>((config, changes))
  })();
  let (config, discarded) = match reset {
    Ok(reset) => reset,
    Err(error) => {
      // no process acts on it now: `reclaim` tries again
      set_state(params, n, mine, |slot| slot.state = HostState::GivingBack { owner: Owner::nobody(), since_ms })?;
      return Err(error);
    }
  };
  let slot = set_state(params, n, mine, |slot| {
    slot.state = HostState::Idle;
    slot.runs_since_clean = slot.runs_since_clean.saturating_add(1);
  })?;
  let cleaned = match clean_due(git_executor, &slot, &config) {
    Ok(true) => clean_core(git_executor, params, n, CleanOverrides::default(), Take::Idle)
      .inspect_err(|error| warn!(slot = %path, error = %format!("{error:#}"), "cannot clean the host slot after its run"))
      .ok(),
    Ok(false) => None,
    Err(error) => {
      warn!(slot = %path, error = %format!("{error:#}"), "cannot tell whether the host slot is due a clean");
      None
    }
  };
  Ok(GaveBack {
    n,
    stopped: stopped.stopped,
    quarantined: Vec::new(),
    discarded,
    cleaned,
    processes_checked: stopped.checked,
  })
}

/// The refusal of a slot in `state`
fn busy(n: u32, state: &HostState) -> HostSlotRefusal {
  HostSlotRefusal::Busy { n, state: describe_state(state) }
}

/// "lent to the run 1792-81", "being cleaned by the process 4242", …
pub fn describe_state(state: &HostState) -> String {
  match state {
    HostState::Idle => "idle".to_string(),
    HostState::Lent(lent) => format!("lent to the run {} (pid {})", lent.run, lent.pid),
    HostState::Filling(owner) => format!("being prepared by the process {}", owner.pid),
    HostState::GivingBack { owner, .. } => format!("being given back by the process {}", owner.pid),
    HostState::Cleaning(owner) => format!("being cleaned by the process {}", owner.pid),
    HostState::Shrinking(owner) => format!("being returned to the pool by the process {}", owner.pid),
    HostState::Quarantined { processes } => {
      let names: Vec<String> = processes.iter().map(|process| format!("{} ({}: {})", process.name, process.pid, process.command)).collect();
      format!("in quarantine while processes work in it: {}", names.join(", "))
    }
  }
}

fn slot_mut(table: &mut HostSlots, n: u32) -> Result<&mut HostSlot> {
  table.slots.iter_mut().find(|slot| slot.n == n).ok_or_else(|| HostSlotRefusal::UnknownSlot(n).into())
}

/// Under the lock, with CAS on the state: `change` the slot `n` while `expected` holds of its state; the slot as it left
/// it
fn set_state(params: &HostSlotsParams<'_>, n: u32, expected: impl Fn(&HostState) -> bool, change: impl FnOnce(&mut HostSlot)) -> Result<HostSlot> {
  params.update(|table| {
    let slot = slot_mut(table, n)?;
    if !expected(&slot.state) {
      bail!(busy(n, &slot.state));
    }
    change(slot);
    Ok(slot.clone())
  })
}

/// What `stop_processes` did
struct ProcessStop {
  stopped: Vec<HostProcess>,
  /// What works in the slot after it: older processes, survivors of SIGKILL, ones started meanwhile
  left: Vec<HostProcess>,
  checked: bool,
}

/// Stop the processes in `path` that started after `since_ms`, leaving its Bazel servers alone, then scan again
fn stop_processes(path: &str, since_ms: u64) -> ProcessStop {
  let scan = ProcessScan::scan();
  if !scan.is_available() {
    return ProcessStop {
      stopped: Vec::new(),
      left: Vec::new(),
      checked: false,
    };
  }
  let after_ms = u64::try_from(START_PRECISION.as_millis()).unwrap_or(0).saturating_add(1);
  let after = UNIX_EPOCH + Duration::from_millis(since_ms.saturating_add(after_ms));
  let runs: Vec<&ProcessInfo> = workers_inside(&scan, path)
    .into_iter()
    .filter(|process| process.started.is_some() && process.started_at().is_some_and(|started| started > after))
    .collect();
  let running = |process: &ProcessInfo| process.started.is_some_and(|started| is_running(process.pid, Some(started)));
  for process in &runs {
    if running(process) {
      debug!(pid = process.pid, argv = ?process.argv, "stopping a process of the run in its host slot");
      terminate(process.pid);
    }
  }
  wait_while(STOP_WAIT, || runs.iter().any(|process| running(process)));
  for process in runs.iter().filter(|process| running(process)) {
    if let Some(started) = process.started {
      debug!(pid = process.pid, "killing a process of the run that outlived SIGTERM");
      kill(process.pid, started);
    }
  }
  wait_while(KILL_WAIT, || runs.iter().any(|process| running(process)));
  let stopped = runs.iter().filter(|process| !running(process)).map(|process| HostProcess::of(process)).collect();
  let rescan = ProcessScan::scan();
  let left = workers_inside(&rescan, path).into_iter().map(HostProcess::of).collect();
  ProcessStop { stopped, left, checked: true }
}

/// The processes in `path` but its Bazel servers, which serve only the slot and may stay
fn workers_inside<'a>(scan: &'a ProcessScan, path: &str) -> Vec<&'a ProcessInfo> {
  scan.inside(Path::new(path)).into_iter().filter(|process| process.bazel_workspace().is_none()).collect()
}

fn wait_while(timeout: Duration, condition: impl Fn() -> bool) {
  let deadline = Instant::now() + timeout;
  while condition() && Instant::now() < deadline {
    std::thread::sleep(Duration::from_millis(50));
  }
}

/// Whether the idle `slot` is due a clean. With `branchdeck.hostSlotMaxSize`: when what the clean rules would free
/// (`CleanMode::Measure`: the stale cargo units and the `delete` paths, not what a clean keeps) passes it, or when the
/// volume has less free space than `branchdeck.leaseMinFree` plus it. Without it: after 50 runs or 7 days, or when the
/// volume has less than `branchdeck.leaseMinFree` and the rules would free something
fn clean_due(git_executor: &GitCommandExecutor, slot: &HostSlot, config: &LeaseConfig) -> Result<bool> {
  let max_size = config_size("branchdeck.hostSlotMaxSize", config.host_slot_max_size.as_ref())?;
  let min_free = config_size("branchdeck.leaseMinFree", config.min_free.as_ref())?.unwrap_or(0);
  let available = available_bytes(Path::new(&slot.path));
  let freeable = || -> Result<u64> {
    let rules = read_clean_rules(Path::new(&slot.path))?;
    Ok(
      apply_clean_rules(git_executor, &slot.path, &rules, CleanMode::Measure)
        .iter()
        .map(|cleaned| cleaned.freed_bytes)
        .sum(),
    )
  };
  Ok(match max_size {
    Some(max_size) => {
      let freeable = freeable()?;
      freeable > max_size || (freeable > 0 && available.is_some_and(|available| available < min_free.saturating_add(max_size)))
    }
    None => {
      slot.runs_since_clean >= CLEAN_AFTER_RUNS
        || now_secs().saturating_sub(slot.cleaned_at) >= CLEAN_AFTER.as_secs()
        || (available.is_some_and(|available| available < min_free) && freeable()? > 0)
    }
  })
}

/// Apply the clean rules of the slot's own `.branchdeck.toml`, with `overrides`, to the idle slot `n`: `cleaning`
/// meanwhile, so no run gets it, then idle with its counts reset
pub fn clean_slot(git_executor: &GitCommandExecutor, params: &HostSlotsParams<'_>, n: u32, overrides: CleanOverrides) -> Result<CleanTarget> {
  clean_core(git_executor, params, n, overrides, Take::Idle)
}

fn clean_core(git_executor: &GitCommandExecutor, params: &HostSlotsParams<'_>, n: u32, overrides: CleanOverrides, take: Take<'_>) -> Result<CleanTarget> {
  let me = Owner::this();
  let slot = params.update(|table| {
    let slot = slot_mut(table, n)?;
    let takes = match (&slot.state, take) {
      (HostState::Idle, Take::Idle) => true,
      (HostState::Cleaning(owner), Take::Abandoned) => !owner.is_alive(),
      _ => false,
    };
    if !takes {
      bail!(busy(n, &slot.state));
    }
    slot.state = HostState::Cleaning(me.clone());
    // before the delete, as a give-back moves it before its discard
    slot.generation = slot.generation.saturating_add(1);
    Ok(slot.clone())
  })?;
  let cleaned = read_clean_rules(Path::new(&slot.path)).map(|rules| {
    let trash = Trash::new(&params.pool().trash_dir());
    let mut cleaned = apply_clean_rules(git_executor, &slot.path, &overrides.apply(rules), CleanMode::Trash(&trash));
    let trash_left = trash.empty(&mut cleaned);
    CleanTarget {
      path: slot.path.clone(),
      skipped: None,
      cleaned,
      trash_left,
    }
  });
  let mine = |state: &HostState| matches!(state, HostState::Cleaning(owner) if *owner == me);
  let succeeded = cleaned.is_ok();
  set_state(params, n, mine, |slot| {
    slot.state = HostState::Idle;
    if succeeded {
      slot.runs_since_clean = 0;
      slot.cleaned_at = now_secs();
    }
  })?;
  cleaned
}

/// `bd lease clean --host`: the clean rules of each idle host slot's own `.branchdeck.toml` with `overrides`, or with
/// `apply` false only measured. A slot that is not idle, or that has a process in it, is skipped with the reason
pub fn clean_host_slots(git_executor: &GitCommandExecutor, params: &HostSlotsParams<'_>, apply: bool, overrides: CleanOverrides) -> Result<(Vec<CleanTarget>, bool)> {
  let table = params.table().read::<HostSlots>()?;
  let scan = ProcessScan::scan();
  let skipped = |path: &str, reason: String| CleanTarget {
    path: path.to_string(),
    skipped: Some(reason),
    cleaned: Vec::new(),
    trash_left: Vec::new(),
  };
  let targets = table
    .slots
    .iter()
    .filter(|slot| !slot.path.is_empty())
    .map(|slot| {
      if slot.state != HostState::Idle {
        return skipped(&slot.path, format!("host slot {} is {}", slot.n, describe_state(&slot.state)));
      }
      let processes: Vec<String> = workers_inside(&scan, &slot.path)
        .iter()
        .map(|process| format!("{} ({})", process.name, process.pid))
        .collect();
      if !processes.is_empty() {
        return skipped(&slot.path, format!("processes work in it: {}", processes.join(", ")));
      }
      let cleaned = if apply {
        clean_slot(git_executor, params, slot.n, overrides)
      } else {
        read_clean_rules(Path::new(&slot.path)).map(|rules| CleanTarget {
          path: slot.path.clone(),
          skipped: None,
          cleaned: apply_clean_rules(git_executor, &slot.path, &overrides.apply(rules), CleanMode::Measure),
          trash_left: Vec::new(),
        })
      };
      cleaned.unwrap_or_else(|error| skipped(&slot.path, format!("{error:#}")))
    })
    .collect();
  Ok((targets, scan.is_available()))
}

/// A slot a fill added
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filled {
  pub n: u32,
  pub path: String,
  /// The get moved a free slot of the pool rather than adding a new one
  pub reused: bool,
}

/// Lease the missing host slots, up to `branchdeck.hostSlots` (4 by default) and the limit of the last shrink: each
/// number reserved as `filling` under the lock of the table, then, without it, the get of the pool with the holder
/// `bd-host/<n>` at the HEAD of the host checkout (a free slot reused, else one clone at a time), detached; then idle.
/// A get that fails frees the numbers it had not got and is the error; the slots got so far stay. `progress` hears each
/// step of each get with the number of its slot
pub fn fill(git_executor: &GitCommandExecutor, params: &HostSlotsParams<'_>, progress: Option<&dyn Fn(u32, GetStep)>) -> Result<Vec<Filled>> {
  let count = slot_count(&lease_config(git_executor, &params.repository.top_level)?)?;
  let me = Owner::this();
  let missing: Vec<u32> = params.update(|table| {
    let count = table.limit.map_or(count, |limit| limit.min(count));
    let have: HashSet<u32> = table.slots.iter().map(|slot| slot.n).collect();
    let missing: Vec<u32> = (1..=count).filter(|n| !have.contains(n)).collect();
    for &n in &missing {
      table.slots.push(HostSlot {
        n,
        lease_id: String::new(),
        path: String::new(),
        used: None,
        runs_since_clean: 0,
        cleaned_at: 0,
        generation: 0,
        affinity: None,
        state: HostState::Filling(me.clone()),
      });
    }
    table.slots.sort_by_key(|slot| slot.n);
    Ok(missing)
  })?;
  let mine = |state: &HostState| matches!(state, HostState::Filling(owner) if *owner == me);
  let mut filled = Vec::new();
  for (index, &n) in missing.iter().enumerate() {
    let report = |step: GetStep| {
      if let Some(progress) = progress {
        progress(n, step);
      }
    };
    let got = get_host_slot(
      git_executor,
      &LeaseGetParams {
        source: params.repository,
        home: params.home,
        holder: Some(format!("{HOST_HOLDER_PREFIX}{n}")),
        branch: None,
        min_free: None,
        server_timeout: params.server_timeout,
        lock_timeout: params.lock_timeout,
        progress: Some(&report),
      },
    );
    let got = match got {
      Ok(got) => got,
      Err(error) => {
        let unfilled: HashSet<u32> = missing[index..].iter().copied().collect();
        params.update(|table| {
          table.slots.retain(|slot| !(unfilled.contains(&slot.n) && mine(&slot.state)));
          Ok(())
        })?;
        return Err(error);
      }
    };
    let lease_id = got.slot.lease.as_ref().map_or_default(|lease| lease.id.clone());
    let path = got.slot.path.clone();
    set_state(params, n, mine, |slot| {
      slot.lease_id = lease_id;
      slot.path.clone_from(&path);
      slot.cleaned_at = now_secs();
      slot.state = HostState::Idle;
    })?;
    filled.push(Filled {
      n,
      path,
      reused: got.created_by.is_none(),
    });
  }
  Ok(filled)
}

/// What a shrink did to a slot
#[derive(Debug, Clone)]
pub struct Shrunk {
  pub n: u32,
  pub path: String,
  pub outcome: ShrinkOutcome,
}

#[derive(Debug, Clone)]
pub enum ShrinkOutcome {
  /// Its lease went back to the pool
  Returned(Box<LeaseReturned>),
  /// It had no lease (a fill that died before its get, a lease that is gone): only its record went
  Dropped,
  /// It stays, and why: a live run has it, a process works in it, the return refused
  Refused(String),
}

/// Give the host slots past `keep` back to the pool, and keep the table at most at `keep` (`limit`), so a running fill
/// adds none of them again. A slot is taken when it is idle, in quarantine, or abandoned by its process, also without a
/// running agent; a slot that a live run has, or that a live process prepares, cleans or gives back, is
/// refused. Each taken slot: a process in it refuses it (none is signalled: it is older than any lend), its changes are
/// discarded, it is checked out at the commit it was leased at, which `unlanded_commits` counts as landed, so the
/// snapshot commits of the runs never refuse it, and its lease goes back through the return of the pool, with its clean
/// rules. A slot the return refuses stays idle
pub fn shrink(git_executor: &GitCommandExecutor, params: &HostSlotsParams<'_>, keep: u32) -> Result<Vec<Shrunk>> {
  let me = Owner::this();
  let taken: Vec<Result<HostSlot, Shrunk>> = params.update(|table| {
    table.limit = Some(keep);
    Ok(
      table
        .slots
        .iter_mut()
        .filter(|slot| slot.n > keep)
        .map(|slot| {
          let free = matches!(slot.state, HostState::Idle | HostState::Quarantined { .. }) || is_abandoned(&slot.state);
          if !free {
            return Err(Shrunk {
              n: slot.n,
              path: slot.path.clone(),
              outcome: ShrinkOutcome::Refused(busy(slot.n, &slot.state).to_string()),
            });
          }
          let before = slot.clone();
          slot.state = HostState::Shrinking(me.clone());
          Ok(before)
        })
        .collect(),
    )
  })?;
  let mine = |state: &HostState| matches!(state, HostState::Shrinking(owner) if *owner == me);
  let mut shrunk = Vec::new();
  for slot in taken {
    let slot = match slot {
      Ok(slot) => slot,
      Err(refused) => {
        shrunk.push(refused);
        continue;
      }
    };
    let outcome = match return_slot(git_executor, params, &slot.path, &slot.lease_id) {
      Ok(Some(returned)) => ShrinkOutcome::Returned(Box::new(returned)),
      Ok(None) => ShrinkOutcome::Dropped,
      Err(error) => ShrinkOutcome::Refused(format!("{error:#}")),
    };
    params.update(|table| {
      match outcome {
        ShrinkOutcome::Refused(_) => {
          if let Some(record) = table.slots.iter_mut().find(|record| record.n == slot.n && mine(&record.state)) {
            record.state = HostState::Idle;
          }
        }
        _ => table.slots.retain(|record| !(record.n == slot.n && mine(&record.state))),
      }
      Ok(())
    })?;
    shrunk.push(Shrunk {
      n: slot.n,
      path: slot.path,
      outcome,
    });
  }
  Ok(shrunk)
}

/// Lift the limit of the last shrink (`None`), or set one
pub fn set_limit(params: &HostSlotsParams<'_>, limit: Option<u32>) -> Result<()> {
  params.update(|table| {
    table.limit = limit;
    Ok(())
  })
}

/// Whether no process acts on a slot in `state` any more: a lend, a fill, a give-back, a clean or a shrink of a gone
/// process
pub fn is_abandoned(state: &HostState) -> bool {
  match state {
    HostState::Idle | HostState::Quarantined { .. } => false,
    HostState::Lent(lent) => !is_running(lent.pid, lent.process_started),
    HostState::Filling(owner) | HostState::Cleaning(owner) | HostState::Shrinking(owner) | HostState::GivingBack { owner, .. } => !owner.is_alive(),
  }
}

/// The return of the host slot at `path` with the lease `lease_id` to the pool: `None` when the pool has no such lease
/// (only the record is left)
fn return_slot(git_executor: &GitCommandExecutor, params: &HostSlotsParams<'_>, path: &str, lease_id: &str) -> Result<Option<LeaseReturned>> {
  if lease_id.is_empty() {
    return Ok(None);
  }
  let store = params.pool().read()?;
  let Some(lease) = store
    .slots
    .iter()
    .filter(|slot| slot.path == path)
    .find_map(|slot| slot.lease.clone().filter(|lease| lease.id == lease_id))
  else {
    return Ok(None);
  };
  let scan = ProcessScan::scan();
  let workers = workers_inside(&scan, path);
  if !workers.is_empty() {
    bail!(LeaseRefusal::SlotBusy {
      path: path.to_string(),
      processes: workers.iter().map(|process| BusyProcess::of(process)).collect(),
    });
  }
  let config = lease_config(git_executor, path)?;
  if !slot_changes(git_executor, path, config.untracked)?.is_empty() {
    discard_slot_changes(git_executor, path)?;
  }
  checkout_slot(git_executor, path, &lease.source_head, None, config.workers)?;
  let workspace = RepositoryDirs {
    top_level: path.to_string(),
    git_dir: linked_worktree(Path::new(path)).map_or_default(|worktree| display_path(&worktree.git_dir)),
    git_common_dir: params.repository.git_common_dir.clone(),
  };
  let returned = return_host_slot(
    git_executor,
    &LeaseReturnParams {
      workspace: &workspace,
      lease_id: Some(lease_id),
      find_by_lease_id: false,
      confirm_preserved: false,
      server_timeout: params.server_timeout,
      clean: Some(CleanOverrides::default()),
      lock_timeout: params.lock_timeout,
    },
  )?;
  Ok(Some(returned))
}

/// What `reclaim` did to a slot
#[derive(Debug, Clone)]
pub struct Reclaimed {
  pub n: u32,
  pub action: ReclaimAction,
  /// Why the action failed; the slot is taken up again by the next `reclaim`
  pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReclaimAction {
  /// The record of a fill that died before its get went
  DroppedFill,
  /// An abandoned lend or give-back was given back
  GaveBack,
  /// The quarantine ended: its processes were gone, and the slot was given back (or stays in quarantine with the
  /// processes it has now)
  Requarantined,
  /// An abandoned clean ran again
  Cleaned,
  /// An abandoned shrink, or a lease `bd-host/<n>` the table does not name (a fill that died after its get), went back
  /// to the pool
  Returned,
}

/// Take up what no process acts on, a tick of the keeper of the host agent (every 60 s), and in its absence of any
/// command of the host slots: the tickets of gone processes leave the queue; the record of a fill that died before its
/// get goes; an abandoned lend or give-back is given back; a slot in quarantine whose processes are gone is given back;
/// an abandoned clean runs again; an abandoned shrink returns its slot; a `ready` lease of the holder `bd-host/<n>`
/// that the table does not name, and whose number no live fill holds, goes back to the pool
pub fn reclaim(git_executor: &GitCommandExecutor, params: &HostSlotsParams<'_>) -> Result<Vec<Reclaimed>> {
  let table = params.update(|table| {
    table.queue.retain(|ticket| is_running(ticket.pid, ticket.process_started));
    let dead_fills: HashSet<u32> = table
      .slots
      .iter()
      .filter(|slot| matches!(&slot.state, HostState::Filling(owner) if !owner.is_alive()))
      .map(|slot| slot.n)
      .collect();
    table.slots.retain(|slot| !dead_fills.contains(&slot.n));
    Ok((table.clone(), dead_fills))
  });
  let (table, dead_fills) = table?;
  let mut reclaimed: Vec<Reclaimed> = dead_fills
    .into_iter()
    .map(|n| Reclaimed {
      n,
      action: ReclaimAction::DroppedFill,
      error: None,
    })
    .collect();
  let mut record = |n: u32, action: ReclaimAction, result: Result<()>| {
    reclaimed.push(Reclaimed {
      n,
      action,
      error: result.err().map(|error| format!("{error:#}")),
    });
  };
  for slot in &table.slots {
    match &slot.state {
      HostState::Lent(_) | HostState::GivingBack { .. } if is_abandoned(&slot.state) => {
        record(slot.n, ReclaimAction::GaveBack, give_back_core(git_executor, params, slot.n, Take::Abandoned).map(drop));
      }
      HostState::Quarantined { processes } if !processes.iter().any(HostProcess::is_running) => {
        record(
          slot.n,
          ReclaimAction::Requarantined,
          give_back_core(git_executor, params, slot.n, Take::Quarantined).map(drop),
        );
      }
      HostState::Cleaning(_) if is_abandoned(&slot.state) => {
        record(
          slot.n,
          ReclaimAction::Cleaned,
          clean_core(git_executor, params, slot.n, CleanOverrides::default(), Take::Abandoned).map(drop),
        );
      }
      HostState::Shrinking(_) if is_abandoned(&slot.state) => {
        let result = shrink_one(git_executor, params, slot.n);
        record(slot.n, ReclaimAction::Returned, result);
      }
      _ => {}
    }
  }
  let named: HashSet<&str> = table.slots.iter().map(|slot| slot.lease_id.as_str()).collect();
  let filling: HashSet<u32> = table.slots.iter().filter(|slot| matches!(slot.state, HostState::Filling(_))).map(|slot| slot.n).collect();
  let store = params.pool().read()?;
  for slot in &store.slots {
    let Some(lease) = slot.lease.as_ref().filter(|lease| lease.state == LeaseState::Ready && !named.contains(lease.id.as_str())) else {
      continue;
    };
    let Some(n) = lease.holder.strip_prefix(HOST_HOLDER_PREFIX).and_then(|n| n.parse::<u32>().ok()) else {
      continue;
    };
    if filling.contains(&n) {
      continue;
    }
    warn!(slot = %slot.path, holder = %lease.holder, "returning a host slot lease that the table does not name");
    record(n, ReclaimAction::Returned, return_slot(git_executor, params, &slot.path, &lease.id).map(drop));
  }
  Ok(reclaimed)
}

/// The shrink of one slot that a dead shrink left
fn shrink_one(git_executor: &GitCommandExecutor, params: &HostSlotsParams<'_>, n: u32) -> Result<()> {
  let me = Owner::this();
  let slot = params.update(|table| {
    let slot = slot_mut(table, n)?;
    if !matches!(&slot.state, HostState::Shrinking(owner) if !owner.is_alive()) {
      bail!(busy(n, &slot.state));
    }
    slot.state = HostState::Shrinking(me.clone());
    Ok(slot.clone())
  })?;
  let returned = return_slot(git_executor, params, &slot.path, &slot.lease_id);
  let mine = |state: &HostState| matches!(state, HostState::Shrinking(owner) if *owner == me);
  params.update(|table| {
    if returned.is_ok() {
      table.slots.retain(|record| !(record.n == n && mine(&record.state)));
    } else if let Some(record) = table.slots.iter_mut().find(|record| record.n == n && mine(&record.state)) {
      record.state = HostState::Idle;
    }
    Ok(())
  })?;
  returned.map(drop)
}
