//! The heavy turn of a host (openspec/specs/cli/host/spec.md, "serve: Heavy runs share the cores of the host by their
//! measure"; docs/design/host-runs.md, "Heavy runs"): a run that `heavy` of `[host]` or `brd-host run --heavy` marks
//! heavy holds the turn from before its lend to its give-back, and the other heavy runs of the host wait in one queue,
//! whatever their repository, by the turns of their groups (below). A run that is not heavy never waits for it.
//!
//! The turn holds cores, not one run: the head of the queue is admitted while the cores that the holders are expected
//! to use, the live cores of the runs that are not heavy and its own expectation fit the cores of the host. A run
//! without a measure of its command (`TurnEntry::expected` is `None`), or one whose command measured the whole host,
//! takes the whole host: it waits for no holder, and no heavy run joins it. A holder counts its expectation only until
//! its use is known ([`TurnEntry::need`]): from [`HOLD_GRACE`] (10 min) after its admission it counts the most of its
//! live use of the last [`USE_WINDOW`] (5 min, `TurnEntry::recent`, which its supervisor notes at each sample) instead,
//! a holder without a measure and one that measured the whole host too; a window that spans less, or whose last sample
//! is older than [`USE_GAP`], or that leaves less than a fifth of the host ([`quiet`]) beside it, counts as before the
//! grace, but never below the most of the samples of the last 5 min that it holds. So a gate whose tail runs on one core for hours lets the next gate start its build beside it, and a burst
//! raises what it holds at its next sample. The risk taken: a holder quiet for longer than the window that bursts then
//! runs over the runs admitted beside it, until it ends or quiets down. Only the head of the queue is admitted, so a
//! run that is admitted later never takes the cores that a run before it waits for; the one exception is an exclusive
//! run at the head whose drain ended (below).
//!
//! The order of the queue takes turns between the groups of its runs ([`join_queue`], [`turn_place`]): a run names its
//! group (`brd-host run --group`, `RunSpec::group`), else its group is its affinity ([`group_key`]), so the runs of one
//! checkout are one group as before and an orchestrator names one group for the checkouts of all its agents. The round
//! of a run of the queue is the count of the runs of its group before it. A new run may not pass the head (index 0), a
//! run of its own group, or a run that [`TURN_PASS_BUDGET`] (8) runs of the turns got ahead of; behind the last of those
//! it joins before the first run whose round is above the count of the waiting runs of its own group, else at the end,
//! and passes every run from there to the end, a short run among them. So a group of many runs no longer keeps a later
//! run of another group waiting for all of them, a short run at the end of the queue included, and with at most one
//! waiting run of each group every round is 0 and the place is the end: without `--group`, where each checkout is a
//! group, the queue is first come first exactly as before. At most 8 runs of the turns get ahead of a run, counting the
//! ones that wait ahead of it and the ones that took the turn ahead of it, whatever the count of the groups and whether
//! the head can start; with the short lane's 4 a run so waits at most 12 runs longer than first come first, which
//! behind long runs can be past its 2 h and end with `heavy_timeout`. The turns keep none of the state of the short
//! lane: `groups.json` beside the table ([`QueueGroups`], a file that no older build reads or writes, of a schema of its
//! own) names the group of each run, the runs it went ahead of, and whether it took the turn, and the count of the
//! passes of a run is derived from it and the queue ([`QueueGroups::passes_in`]), never kept as a number. So a run that
//! leaves without the turn, by a cancel, a start of `admit_now` refused `busy`, the end of its process or a write of the
//! table that failed, counts no pass, and no order of the writes of the two files can count a pass for a run that is not
//! in the queue. A run that `groups.json` does not name is of the group of its entry, so a rewrite of `turn.json` by an
//! older build, which drops `group`, moves a run of `--group` to the group of its affinity at worst. The estimate of a
//! start applies the same [`join_queue`] to a copy of the table and of its files, so it places a run as the join does.
//!
//! Inside those turns the order is first come first, but for short runs ([`place`]): a run whose command has a measure below
//! the whole host, is not exclusive, and whose last passed runs took at most [`SHORT_WALL`] (2 min; the 80th percentile
//! of the walls of its last 5 passed runs, of 3 at least: `host_cores::passed_wall`) joins the queue before the first
//! entry from which every entry to the end can be passed: one that is not short and that short runs passed fewer than
//! [`PASS_BUDGET`] (4) times, not an exclusive head whose drain began. So a lint of a minute waits for the holders, not
//! for an hour of runs of the whole host, and none of those waits more than 4 short runs longer; the short runs keep
//! their order among themselves, and so do the others. The queue changes in the table itself, never by a rule of the
//! admission, so a supervisor of the build before, which admits its head, reads it right. A short run that passed
//! entries takes the earliest join of them for its [`LOAD_BOUND`], so a head that it passed on a busy free turn waits
//! no new bound. The pass counters and the classes are in `passes.json` beside the table, by the id of the run, which
//! builds 1.24 to 1.27 rewrite without the fields they do not know, keeping these; a run that supersedes a waiting one
//! takes its counter. A run without
//! a measure is never short: it takes the whole host. A holder in its grace counts the larger of its
//! expectation and its live use (`live`, which its supervisor notes at each sample), so a command that got heavier
//! holds more at once; the runs that are not heavy note theirs into `light`. The expectation is the level of the heavy
//! phases of the last runs of the command, not their mean (`host_cores::expected`).
//!
//! The samples of the runs miss what no run counts, and the agent samples the busy cores of the whole host every 2 s
//! into `host.json` ([`note_host`]). An admission counts the larger of `light` and the busy cores of the host outside
//! the live use of the holders: the load of the runs that are not heavy and of processes no run counts holds a heavy run
//! back too. A run that takes the whole host waits for a quiet host, busy with at most a fifth of its cores (at least 1)
//! outside the heavy runs; a run of a measure starts on a free turn while it fits beside that load, or the host is quiet.
//! A sample older than 10 s counts as none, and an admission then counts `light` only. The runs that are not heavy wait
//! for no heavy run, so a host of steady light runs is never quiet: a head of the queue that has waited
//! [`LOAD_BOUND`] (15 min) since it joined the queue takes a free turn whatever that load, and the runs after it wait
//! behind it still.
//!
//! An exclusive run (`brd-host run --exclusive`, a measurement such as an A/B) takes the whole host of a quiet host:
//! it is admitted only without a holder, with no run that is not heavy in its command (`light`, which a run joins as its
//! command starts and leaves as it ends), and when every sample of the host of the last 30 s is quiet, as a bursty run
//! in a lull of one sample is no quiet host ([`HostWindow`]). While it holds no heavy run is admitted and each new run
//! that is not heavy waits for its end before its lend ([`exclusive_holder`]). While it drains, at the head of the queue
//! with no holder, the new runs that are not heavy wait too, for at most 5 min from the start of the drain, so a host
//! of steady light runs drains at all ([`bar`], `drain.json`). No heavy run passes it before its drain ends either: the
//! drain is the one window of the host for the measurement. Once its drain has ended without a quiet host, the first heavy
//! run behind it that is not exclusive starts past it while the head cannot start and the ordinary admission admits that
//! run ([`passes`]): the runs that are not heavy keep a host busy for hours, and every heavy run behind a measurement
//! would wait to its timeout. The exclusive run keeps its place at the head and starts once the host is quiet, with no
//! second drain ([`HeadDrain`]). The holder is also in `exclusive.json` beside the table: a
//! supervisor of a build before `exclusive` rewrites `turn.json` at each sample of its run and drops the field it does
//! not know, and nothing of such a build touches that file. A look of the run puts the field back on its entry. Such a
//! rewrite drops `recent` too: the window of a holder without a measure begins again at the next sample of its own
//! supervisor, and until it spans 5 min again the holder holds the whole host, as before the window.
//!
//! The table is `turn.json` with its lock `turn.lock` in a directory of the agent of its own (`~/.cache/brd-host/heavy/`),
//! not in a repository. Each entry names the process of its run (the supervisor), as a lend and a ticket of `host_slots`
//! do, so a gone process leaves the table without a keeper. The lock is a leaf, held only while the table changes,
//! never across git or another lock. A run that waits watches the directory once for its whole wait (`TurnWatch`) and
//! looks again every 5 s, as the end of a process makes no file event. A look of a run behind the head of the queue
//! reads the table without the lock ([`look`]): each change of the table wakes every run that waits, and only the head
//! can be admitted. The table names its schema: a table of another schema refuses. `heavy.json` is the table of the
//! build before (one holder, schema 1): its supervisors go on with it after an upgrade, and its live holder counts here
//! as one that holds the whole host, so no run of this build joins it.
//!
//! A later heavy run of the same checkout (its affinity) and the same command (`host_link::heavy::run_key`) supersedes
//! the runs of the queue that wait for the turn with them: it takes the place of the first in the queue, and each of
//! them moves to `superseded` with the id of the later run, which its own next look answers (`TurnAnswer::Superseded`).
//! A run of the queue has no slot and has started no command: the turn comes before the lend. A holder is never
//! superseded. An entry without an affinity and a key supersedes nothing and nobody

use crate::host_slots::{TableWake, Ticket, WAIT_RECHECK, now_ms};
use crate::state_file::StateFile;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};
use sync_utils::cores::Cores;
use sync_utils::process_scan::is_running;

/// The schema of `turn.json` that this build reads and writes
const SCHEMA: u32 = 2;
const STATE_FILE: &str = "turn.json";
/// The lock of `turn.json`, which the agent creates private before the first change
pub const LOCK_FILE: &str = "turn.lock";
/// The table of the build before, of one holder: read for its holder only
const LEGACY_FILE: &str = "heavy.json";
/// The exclusive run that holds the turn, beside the table that an older build rewrites without it
const EXCLUSIVE_FILE: &str = "exclusive.json";
/// The exclusive run that drains the host: at the head of the queue, with no holder, since when
const DRAIN_FILE: &str = "drain.json";
/// The classes of the runs of the queue and how often short runs passed each, beside the table that an older build
/// rewrites without them
const PASSES_FILE: &str = "passes.json";
/// The longest wall of the last passed runs of a short command ([`short_wall`])
pub const SHORT_WALL: Duration = Duration::from_mins(2);
/// How often short runs pass a run of the queue at most ([`place`]): a run waits at most 4 short runs longer
pub const PASS_BUDGET: u32 = 4;
/// The groups of the runs of the queue and the runs each went ahead of by its turn, beside the table, a file no older
/// build knows
const GROUPS_FILE: &str = "groups.json";
/// The schema of `groups.json` that this build reads and writes
const GROUPS_SCHEMA: u32 = 1;
/// How many runs of the turns of the groups may be ahead of a run of the queue at most ([`turn_place`]), counting the
/// ones that wait ahead of it and the ones that took the turn ahead of it, whatever the count of the groups and whether
/// the head can start. With the short lane's [`PASS_BUDGET`] a run so waits at most 12 runs longer than first come
/// first; behind long runs that can be past its 2 h of a heavy wait, and it ends with `heavy_timeout` then
pub const TURN_PASS_BUDGET: u32 = 8;
/// How long the new runs that are not heavy wait for an exclusive run that drains the host, from the start of the drain
pub const DRAIN_BAR: Duration = Duration::from_mins(5);
/// How long the head of the queue waits in the queue for a quiet host, or for room beside the load of the host outside
/// the heavy runs, before a free turn admits it whatever that load ([`admits`]): the runs that are not heavy wait for no
/// heavy run, so on a host of steady light runs a run of the whole host would wait for the whole of its 2 h
pub const LOAD_BOUND: Duration = Duration::from_mins(15);
/// How long a holder counts its expectation from its admission, the whole host without a measure, whatever its use
/// ([`TurnEntry::need`]): twice [`USE_WINDOW`], so the window that frees its cores begins after its first 5 min, which a
/// build or the start of the servers of a gate fills, and a run of minutes (a test suite, an E2E run) ends before it
/// counts by its use
pub const HOLD_GRACE: Duration = Duration::from_mins(10);
/// The span of the live samples of a holder whose most it counts once its grace ended: longer than the
/// lull between two bursts of a test run (a link, the start of the next binary), short enough that a tail on one core
/// frees the cores within minutes
pub const USE_WINDOW: Duration = Duration::from_mins(5);
/// The step of the window of the live use ([`RecentUse`]): the most of each half minute, so the table keeps at most 11
/// steps of a holder, whatever the interval of its samples
const USE_STEP_MS: u64 = 30_000;
/// The longest time between two samples of a holder in one window, and the oldest last sample of a window that counts:
/// 6 samples of 10 s missed say that the supervisor no longer tells its use
pub const USE_GAP: Duration = Duration::from_mins(1);
/// The busy cores of the whole host, which the agent samples ([`note_host`])
const HOST_FILE: &str = "host.json";
/// The age past which a sample of the host counts as none: its agent is gone or stuck
const HOST_FRESH_MS: u64 = 10_000;
/// The files whose change can admit a waiting run
const WAKE_FILES: [&str; 2] = [STATE_FILE, LEGACY_FILE];
/// The longest command of an entry, in characters
const COMMAND_CHARS: usize = 200;

/// The heavy turn of a host
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeavyTurn {
  /// The schema of the table, `SCHEMA` of this build; a table without one is of no build
  #[serde(default)]
  pub schema: u32,
  /// The cores of the host that the last change counted with
  #[serde(default)]
  pub cores: u32,
  /// The heavy runs that hold cores of the turn, in the order they were admitted
  #[serde(default)]
  pub holders: Vec<TurnEntry>,
  /// The heavy runs that wait for the turn, first come first
  #[serde(default)]
  pub queue: Vec<TurnEntry>,
  /// The runs that a later run of their checkout and command took out of the queue: the next look of each answers the
  /// later run and takes it out
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub superseded: Vec<SupersededEntry>,
  /// The runs that are not heavy whose command runs, with their live cores
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub light: Vec<LightEntry>,
}

/// A run that a later run of its checkout and command superseded while it waited for the turn
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupersededEntry {
  /// The superseded run and its process
  #[serde(flatten)]
  pub ticket: Ticket,
  /// The later run that took its place
  pub by: String,
}

/// A run that is not heavy whose command runs, and the cores it used at its last sample
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LightEntry {
  #[serde(flatten)]
  pub ticket: Ticket,
  pub repo: String,
  /// The command as one line, at most 200 characters
  pub command: String,
  pub live: Cores,
}

/// A heavy run that holds cores of the turn or waits for them
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnEntry {
  /// The run and its process, as a ticket of the host slots names them
  #[serde(flatten)]
  pub ticket: Ticket,
  /// The main repository of the run on the host
  pub repo: String,
  /// The command of the run as one line, at most 200 characters
  pub command: String,
  /// When it was admitted, or joined the queue, in Unix milliseconds
  pub since_ms: u64,
  /// The checkout of the client of the run (`RunSpec::affinity`); `None` from an older build
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub affinity: Option<String>,
  /// The key of the whole command by what it runs (`host_link::heavy::run_key`), not cut as `command`; `None` from an
  /// older build
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub command_key: Option<String>,
  /// The cores that the last runs of its command lead to expect (`host_cores::expected`); `None` without a measure:
  /// the run takes the whole host
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub expected: Option<Cores>,
  /// The cores of its last sample, while it holds cores
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub live: Option<Cores>,
  /// A measurement that needs a quiet host (`brd-host run --exclusive`): it takes the whole host, and while it holds,
  /// the new runs that are not heavy wait too
  #[serde(default, skip_serializing_if = "is_false")]
  pub exclusive: bool,
  /// The exclusive run at the head of the queue that this holder started past, as the drain of that run had ended
  /// ([`passes`]); a supervisor of an older build that rewrites the table drops it, and only the lines lose it
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub passed: Option<String>,
  /// The class of the measures of the run (`brd-host run --measure-as`), whose measures `expected` are; a supervisor
  /// of an older build that rewrites the table drops it, and only the lines lose it
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub measure_as: Option<String>,
  /// The live use of the last 5 min of a holder that is not exclusive, which its supervisor notes at each sample: what
  /// it holds once its grace ended ([`HOLD_GRACE`]); a supervisor of an older build that rewrites the table drops it,
  /// and the holder counts as in its grace until a new window spans 5 min
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub recent: Option<RecentUse>,
  /// The group of the run in the heavy queue (`brd-host run --group`, `RunSpec::group`); `None`: its group is its
  /// affinity. The order reads the group from `groups.json` ([`QueueGroups`]): a supervisor of an older build that
  /// rewrites the table drops this field, and only the lines lose it
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub group: Option<String>,
}

/// The live samples of a holder over the last [`USE_WINDOW`], the most of each half minute
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentUse {
  /// The first sample since which none was more than [`USE_GAP`] after the one before: the window counts once its
  /// samples span 5 min from it
  pub since_ms: u64,
  /// The last sample
  pub at_ms: u64,
  /// The start of each half minute of the window and the most cores of its samples, the oldest first
  pub steps: Vec<(u64, Cores)>,
}

impl RecentUse {
  /// Adds the sample `live` at `at_ms`, and leaves out the steps older than the window; a sample after a gap longer
  /// than [`USE_GAP`], or before the last one, begins the window again
  pub fn push(&mut self, at_ms: u64, live: Cores) {
    if self.steps.is_empty() || at_ms < self.at_ms || at_ms - self.at_ms > millis(USE_GAP) {
      *self = Self {
        since_ms: at_ms,
        at_ms,
        steps: Vec::new(),
      };
    }
    let step = at_ms - at_ms % USE_STEP_MS;
    match self.steps.last_mut() {
      Some((start, most)) if *start == step => *most = (*most).max(live),
      _ => self.steps.push((step, live)),
    }
    self.at_ms = at_ms;
    let from = at_ms.saturating_sub(millis(USE_WINDOW));
    self.steps.retain(|&(start, _)| start + USE_STEP_MS > from);
  }

  /// The most cores of the samples of the last 5 min at `now_ms`, by the half minute, so up to 30 s more; `None` while
  /// the samples span less than 5 min, or when the last is more than [`USE_GAP`] before `now_ms`
  pub fn peak(&self, now_ms: u64) -> Option<Cores> {
    let spans = self.at_ms.saturating_sub(self.since_ms) >= millis(USE_WINDOW);
    let fresh = now_ms.saturating_sub(self.at_ms) <= millis(USE_GAP);
    self.most(now_ms).filter(|_| spans && fresh)
  }

  /// The most cores of the samples of the last 5 min at `now_ms` that the window holds, by the half minute, whatever
  /// their span and the age of the last: the least that a holder past its grace counts ([`TurnEntry::need`]); `None`
  /// without such a sample
  pub fn most(&self, now_ms: u64) -> Option<Cores> {
    let from = now_ms.saturating_sub(millis(USE_WINDOW));
    self.steps.iter().filter(|&&(start, _)| start + USE_STEP_MS > from).map(|&(_, most)| most).max()
  }
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature of `skip_serializing_if`")]
const fn is_false(value: &bool) -> bool {
  !*value
}

/// How much of the host an entry takes, for the admission and the lines
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
  /// These cores, by the measure of its command, or its live use when that is more
  Cores { cores: Cores, live: bool },
  /// The whole host: its command has no measure yet
  Unmeasured,
  /// These cores, the most of its live use of the last 5 min: a holder past its grace ([`HOLD_GRACE`]), with the
  /// measure of its command, if any, which it counted before
  Recent { cores: Cores, measured: Option<Cores> },
  /// The whole host: its command measured these cores, the cores of the host or more
  Measured(Cores),
  /// The whole host of a quiet host, for a measurement: an exclusive run
  Exclusive,
}

impl Need {
  /// The cores of the need on a host of `cores`
  pub const fn cores(self, cores: u32) -> Cores {
    match self {
      Self::Cores { cores, .. } | Self::Recent { cores, .. } => cores,
      Self::Unmeasured | Self::Measured(_) | Self::Exclusive => Cores::whole(cores),
    }
  }

  pub const fn is_whole_host(self) -> bool {
    !matches!(self, Self::Cores { .. } | Self::Recent { .. })
  }
}

impl Default for HeavyTurn {
  fn default() -> Self {
    Self {
      schema: SCHEMA,
      cores: 0,
      holders: Vec::new(),
      queue: Vec::new(),
      superseded: Vec::new(),
      light: Vec::new(),
    }
  }
}

impl HeavyTurn {
  /// The table, or an error when an agent of another build wrote it in another schema
  fn of_this_schema(&mut self, dir: &Path) -> Result<&mut Self> {
    if self.schema != SCHEMA {
      bail!(
        "{} has the schema {} of another build of brd-host, and this one reads {SCHEMA}",
        dir.join(STATE_FILE).display(),
        self.schema
      );
    }
    Ok(self)
  }

  /// Without the entries whose process is gone
  fn prune(&mut self) {
    self.holders.retain(TurnEntry::is_running);
    self.queue.retain(TurnEntry::is_running);
    self.superseded.retain(|entry| entry.ticket.is_running());
    self.light.retain(|entry| entry.ticket.is_running());
  }

  /// The live cores of the runs that are not heavy
  pub fn light_cores(&self) -> Cores {
    self.light.iter().map(|entry| entry.live).sum()
  }
}

impl TurnEntry {
  fn is_running(&self) -> bool {
    self.ticket.is_running()
  }

  /// What the entry takes of a host of `cores` at `now_ms`: a holder past its grace its use of the last 5 min
  /// ([`Self::recent_use`]); else the larger of its expectation and its live use, the whole host without a measure or
  /// with one of the whole host. A holder past its grace counts at least the most of its samples of the last 5 min
  /// ([`RecentUse::most`]), whatever its window: a window that counts as the grace never counts less than its peak
  pub fn need(&self, cores: u32, now_ms: u64) -> Need {
    if self.exclusive {
      return Need::Exclusive;
    }
    if let Some(used) = self.recent_use(cores, now_ms) {
      return Need::Recent {
        cores: used,
        measured: self.expected,
      };
    }
    let before = match self.expected {
      None => Need::Unmeasured,
      Some(expected) if expected >= Cores::whole(cores) => Need::Measured(expected),
      Some(expected) => match self.live {
        Some(live) if live > expected => Need::Cores { cores: live, live: true },
        _ => Need::Cores { cores: expected, live: false },
      },
    };
    match (before, self.past_grace_most(now_ms)) {
      (Need::Cores { cores: counted, .. }, Some(most)) if most > counted => Need::Recent {
        cores: most,
        measured: self.expected,
      },
      (before, _) => before,
    }
  }

  /// The most of the live use of a holder past its grace over the last 5 min, its last sample too, as its window holds
  /// it at `now_ms`; `None` in its grace and without a sample
  fn past_grace_most(&self, now_ms: u64) -> Option<Cores> {
    if now_ms.saturating_sub(self.since_ms) < millis(HOLD_GRACE) {
      return None;
    }
    let most = self.recent.as_ref()?.most(now_ms)?;
    Some(most.max(self.live.unwrap_or_default()))
  }

  /// The cores that a holder counts at `now_ms` past its grace: the most of its live use of the last 5 min
  /// ([`RecentUse::peak`]) and of its last sample, once [`HOLD_GRACE`] passed since its admission (`since_ms`);
  /// `None`, the count of the grace, in its grace, without a window that counts (an entry of the queue has none), or
  /// with less than a fifth of the host ([`quiet`]) left beside it, as that room is the background of a quiet host;
  /// past the grace that count is at least the most of its samples of the last 5 min ([`Self::need`])
  fn recent_use(&self, cores: u32, now_ms: u64) -> Option<Cores> {
    if now_ms.saturating_sub(self.since_ms) < millis(HOLD_GRACE) {
      return None;
    }
    let used = self.recent.as_ref()?.peak(now_ms)?.max(self.live.unwrap_or_default());
    (used.saturating_add(quiet(cores)) < Cores::whole(cores)).then_some(used)
  }

  /// Whether the entry is of the checkout of `request` (`same_command`: and of its command), as both name them; an
  /// entry of an older build names neither
  fn is_of(&self, request: &TurnRequest<'_>, same_command: bool) -> bool {
    !request.affinity.is_empty()
      && self.repo == request.repo
      && self.affinity.as_deref() == Some(request.affinity)
      && (self.command_key.as_deref() == Some(request.command_key)) == same_command
  }
}

impl Ticket {
  fn is_running(&self) -> bool {
    is_running(self.pid, self.process_started)
  }
}

/// The directory of the table of a host and its cores
pub struct TurnParams<'a> {
  /// The directory of `turn.json`, of its lock, of `heavy.json` of the build before and of `cores.json`
  /// (`host_cores`): `~/.cache/brd-host/heavy/`
  pub dir: &'a Path,
  /// The wait for the lock of the table
  pub lock_timeout: Duration,
  /// How often a wait looks again without a file event (`RECHECK`): the end of a holder makes none, and `FSEvents` of
  /// macOS delays and drops events under load, so the look bounds every wait
  pub recheck: Duration,
  /// The cores of the host that the heavy runs share
  pub cores: u32,
  /// How long the head of the queue waits for the load of the host outside the heavy runs on a free turn
  /// ([`LOAD_BOUND`])
  pub load_bound: Duration,
  /// How long an exclusive run at the head of the queue drains the host from the start of its drain ([`DRAIN_BAR`]): no
  /// heavy run passes it before that time is up
  pub drain_bar: Duration,
}

/// How often a wait for the turn looks again without a file event, as a lend that waits does
pub const RECHECK: Duration = WAIT_RECHECK;

/// The watch of the directory of the table for one whole wait: a change of the table wakes the wait
pub struct TurnWatch(TableWake);

/// The watch of the table of `params`, started before the first look of a wait, so no change between a look and the
/// wait after it is lost
pub fn watch(params: &TurnParams<'_>) -> TurnWatch {
  TurnWatch(TableWake::start(params.dir, &WAKE_FILES))
}

impl TurnParams<'_> {
  fn table(&self) -> StateFile {
    table_file(self.dir)
  }
}

fn table_file(dir: &Path) -> StateFile {
  StateFile::in_dir(dir, STATE_FILE, LOCK_FILE, "the heavy turn")
}

/// The table of the build before capacities: only its holder counts here
#[derive(Debug, Default, Deserialize)]
struct LegacyTurn {
  #[serde(default)]
  schema: u32,
  #[serde(default)]
  holder: Option<LegacyHolder>,
}

#[derive(Debug, Deserialize)]
struct LegacyHolder {
  #[serde(flatten)]
  ticket: Ticket,
  #[serde(default)]
  repo: String,
  #[serde(default)]
  command: String,
  #[serde(default)]
  since_ms: u64,
}

/// The live holder of `heavy.json` of the build before, as an entry that takes the whole host; read without its lock,
/// as nothing of this build writes it
fn legacy_holder(dir: &Path) -> Option<TurnEntry> {
  let text = std::fs::read_to_string(dir.join(LEGACY_FILE)).ok()?;
  let table: LegacyTurn = serde_json::from_str(&text).ok()?;
  let holder = table.holder.filter(|holder| table.schema == 1 && holder.ticket.is_running())?;
  Some(TurnEntry {
    ticket: holder.ticket,
    repo: holder.repo,
    command: holder.command,
    since_ms: holder.since_ms,
    affinity: None,
    command_key: None,
    expected: None,
    live: None,
    exclusive: false,
    passed: None,
    measure_as: None,
    recent: None,
    group: None,
  })
}

/// A heavy run that asks for the turn
pub struct TurnRequest<'a> {
  /// The id of the run
  pub run: &'a str,
  /// The process that runs it and waits for the turn, its own for each run (the supervisor): its ticket and its turn
  /// last as long as it
  pub pid: u32,
  pub process_started: Option<u64>,
  /// The main repository of the run on the host
  pub repo: &'a str,
  /// The command of the run
  pub command: &'a [String],
  /// The checkout of the client of the run (`RunSpec::affinity`): an empty one supersedes nothing
  pub affinity: &'a str,
  /// The key of the command by what it runs (`host_link::heavy::run_key`)
  pub command_key: &'a str,
  /// The cores that the last runs of the command lead to expect (`host_cores::expected`), `None` without a measure
  pub expected: Option<Cores>,
  /// A measurement that needs a quiet host (`brd-host run --exclusive`)
  pub exclusive: bool,
  /// The wall that the last passed runs of the command lead to expect (`host_cores::passed_wall`), `None` with fewer
  /// than 3: whether the run is short ([`short_wall`])
  pub passed_wall: Option<Duration>,
  /// The class of the measures of the run (`brd-host run --measure-as`), of which `expected` and `passed_wall` are
  pub measure_as: Option<&'a str>,
  /// The group of the run in the heavy queue (`brd-host run --group`); `None`: its affinity ([`group_key`])
  pub group: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnAnswer {
  /// The run holds its cores of the turn
  Taken,
  /// The run holds its cores of the turn, admitted past `exclusive`, the exclusive run at the head of the queue whose
  /// drain ended without a quiet host ([`passes`])
  Passed { exclusive: TurnEntry },
  /// Not yet: who holds cores, what the runs that are not heavy use, and who waits before the run
  Queued(TurnQueue),
  /// A later run of the same checkout and command, named here, took its place in the queue: the run runs nothing
  Superseded { by: String },
}

/// The answer to a run that asks for the turn, and what its first ask did to the queue
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joined {
  pub answer: TurnAnswer,
  /// The runs of the same checkout and command that waited for the turn: this run took the place of the first, and each
  /// is superseded
  pub replaced: Vec<TurnEntry>,
  /// The other runs of the same checkout that wait for the turn, with another command
  pub beside: Vec<TurnEntry>,
  /// The wall of the last passed runs of a short run ([`short_wall`]); `None` for a run that is not short, or one that
  /// was in the queue already
  pub short: Option<Duration>,
  /// The runs of the queue that this short run went ahead of, with the passes counted so far, its own among them
  pub passed: Vec<(TurnEntry, u32)>,
}

/// What the queue knows of its runs beyond `turn.json`, by the id of the run: `passes.json` beside the table, which a
/// supervisor of an older build rewrites without the fields it does not know. A run that it does not name is not short
/// and was never passed
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueMarks {
  #[serde(default)]
  pub runs: BTreeMap<String, RunMark>,
}

/// The groups of the runs of the queue and the runs each went ahead of by its turn: `groups.json` beside the table, a
/// file that no older build reads or writes. The passes of a run are counted from it, never kept as a number
/// ([`Self::passes_in`]): a run counts a pass of each run it went ahead of while it waits in the queue or once it took
/// the turn ([`GroupMark::took`]), so a run that leaves without the turn, by a cancel, a start of `admit_now` refused
/// `busy`, the end of its process or a write of the table that failed, counts none, and no write order can make a
/// count disagree with the queue. A run that it does not name is of the group of its entry ([`Self::group_of`])
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueGroups {
  /// The schema of the file, [`GROUPS_SCHEMA`]
  #[serde(default)]
  pub schema: u32,
  #[serde(default)]
  pub runs: BTreeMap<String, GroupMark>,
}

impl Default for QueueGroups {
  fn default() -> Self {
    Self {
      schema: GROUPS_SCHEMA,
      runs: BTreeMap::new(),
    }
  }
}

/// The group of a run of the queue, the runs it went ahead of by its turn, and whether it took the turn
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMark {
  pub group: String,
  /// The runs of the queue that it went ahead of by its turn, and those of a run of its group that it superseded
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub passed: Vec<String>,
  /// It took the turn: its passes count after it left the queue
  #[serde(default, skip_serializing_if = "is_false")]
  pub took: bool,
}

impl QueueGroups {
  /// The group of `entry`: by its mark, else by its entry ([`group_key`]): `--group`, else its affinity
  pub fn group_of<'a>(&'a self, entry: &'a TurnEntry) -> &'a str {
    self.runs.get(&entry.ticket.run).map_or_else(
      || group_key(entry.group.as_deref(), entry.affinity.as_deref().unwrap_or_default(), &entry.ticket.run),
      |mark| mark.group.as_str(),
    )
  }

  /// How often runs of the turns went ahead of each run, by the runs that went ahead of it and wait in `queue` or took
  /// the turn
  pub fn passes_in(&self, queue: &[TurnEntry]) -> BTreeMap<String, u32> {
    let mut passes = BTreeMap::new();
    for (run, mark) in &self.runs {
      if mark.took || queue.iter().any(|entry| entry.ticket.run == *run) {
        for passed in &mark.passed {
          *passes.entry(passed.clone()).or_default() += 1;
        }
      }
    }
    passes
  }
}

/// The groups of the runs of the queue in `dir`, read without the lock: a file replaced by a rename. A missing file names
/// no group: each run is of the group of its entry, never passed. A file of another schema refuses, as the table does;
/// a file that does not parse, as a power loss can leave it, says so in the log and names none: the runs of the queue
/// get their whole budget of passes again, and the next join writes the file anew
pub fn read_groups(dir: &Path) -> Result<QueueGroups> {
  let path = dir.join(GROUPS_FILE);
  let Ok(bytes) = std::fs::read(&path) else {
    return Ok(QueueGroups::default());
  };
  match serde_json::from_slice::<QueueGroups>(&bytes) {
    Ok(groups) if groups.schema == GROUPS_SCHEMA => Ok(groups),
    Ok(groups) => bail!(
      "{} has the schema {} of another build of brd-host, and this one reads {GROUPS_SCHEMA}",
      path.display(),
      groups.schema
    ),
    Err(error) => {
      tracing::warn!(path = %path.display(), error = %error, "the groups of the heavy queue do not parse: every run of the queue is of the group of its entry and gets its whole budget of passes again");
      Ok(QueueGroups::default())
    }
  }
}

/// Writes `groups` into `groups.json` of `dir`, under the lock of the table, before the table itself
fn write_groups(dir: &Path, groups: &QueueGroups) -> Result<()> {
  sync_utils::atomic_file::write_atomically(
    &dir.join(GROUPS_FILE),
    &serde_json::to_vec(groups)?,
    sync_utils::atomic_file::AtomicWrite { private: true, durable: false },
  )?;
  Ok(())
}

/// What a join reads beside the table and changes: the marks of the short lane (`passes.json`), the groups of the
/// turns (`groups.json`) and the drain of an exclusive head (`drain.json`). [`join_queue`] changes them in memory; the
/// join writes them, the estimate changes a copy
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueueFiles {
  pub marks: QueueMarks,
  pub groups: QueueGroups,
  pub drain: DrainFile,
}

impl QueueFiles {
  /// The files of the heavy turn in `dir`
  pub fn read(dir: &Path) -> Result<Self> {
    Ok(Self {
      marks: read_marks(dir),
      groups: read_groups(dir)?,
      drain: DrainFile::read(dir),
    })
  }
}

/// The group of a run in the heavy queue: its `group` (`brd-host run --group`), else its affinity, else the run `run`
/// alone, as a run of an older client names no affinity
pub fn group_key<'a>(group: Option<&'a str>, affinity: &'a str, run: &'a str) -> &'a str {
  group
    .filter(|group| !group.is_empty())
    .or_else(|| Some(affinity).filter(|affinity| !affinity.is_empty()))
    .unwrap_or(run)
}

/// The class of a run of the queue and how often short runs passed it
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunMark {
  /// The wall of the last passed runs of a short run ([`short_wall`]), in milliseconds; `None`: not short
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub short_ms: Option<u64>,
  /// How often short runs went ahead of it ([`PASS_BUDGET`] at most)
  #[serde(default)]
  pub passes: u32,
  /// When a short run that passed runs joined the queue, as its `since_ms` is the earliest join of them
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub joined_ms: Option<u64>,
}

impl QueueMarks {
  /// The mark of the run `run`: the default for a run that the file does not name
  pub fn of(&self, run: &str) -> RunMark {
    self.runs.get(run).copied().unwrap_or_default()
  }
}

/// The marks of the runs of the queue in `dir`, read without the lock: a file replaced by a rename. A file that is
/// missing or cannot be read marks no run, so every run is as first come first
pub fn read_marks(dir: &Path) -> QueueMarks {
  std::fs::read(dir.join(PASSES_FILE))
    .ok()
    .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    .unwrap_or_default()
}

/// Writes `marks` into `passes.json` of `dir`, under the lock of the table
fn write_marks(dir: &Path, marks: &QueueMarks) -> Result<()> {
  sync_utils::atomic_file::write_atomically(
    &dir.join(PASSES_FILE),
    &serde_json::to_vec(marks)?,
    sync_utils::atomic_file::AtomicWrite { private: true, durable: false },
  )?;
  Ok(())
}

/// The wall of a short run, `None` for a run that is not short: a run that is not `exclusive`, whose command has a
/// measure (`expected`) below the whole host of `cores`, and whose last passed runs took at most [`SHORT_WALL`]
/// (`passed_wall`, of 3 at least). A run without a measure is never short: it takes the whole host
pub fn short_wall(exclusive: bool, expected: Option<Cores>, passed_wall: Option<Duration>, cores: u32) -> Option<Duration> {
  let measured = expected.is_some_and(|expected| expected < Cores::whole(cores));
  passed_wall.filter(|&wall| !exclusive && measured && wall <= SHORT_WALL)
}

/// The place of a new run in `queue`, whose runs `marks` marks, with `head_drain` the drain of an exclusive head
/// ([`head_drain`]): a `short` run goes before the first entry from which every entry to the end can be passed
/// ([`passable`]); a run of the group `turn` of `groups` takes its turn ([`turn_place`]); a run without a turn, one
/// that takes the place of a run it supersedes, goes to the end. The entries from the place to the end are the runs it
/// passes. A pure function of the queue: an estimate of a start reads the same place
fn place(queue: &[TurnEntry], marks: &QueueMarks, head_drain: Option<HeadDrain>, short: bool, turn: Option<(&QueueGroups, &str)>) -> usize {
  if short {
    let mut position = queue.len();
    while position > 0 && passable(&queue[position - 1], position - 1, marks, head_drain) {
      position -= 1;
    }
    return position;
  }
  turn.map_or(queue.len(), |(groups, group)| turn_place(queue, groups, group))
}

/// The place of a new run of `group` by the turns of the groups. The round of an entry is the count of the entries of its
/// group before it. The run may not pass the head (index 0), an entry of its own group, or an entry that runs of the
/// turns passed [`TURN_PASS_BUDGET`] times ([`QueueGroups::passes_in`]); behind the last of those it goes before the
/// first entry of a round above the count of the waiting runs of its own group, else to the end, and passes every
/// entry from there to the end, a short run among them. So a run of a group with fewer runs waiting goes ahead of the
/// later runs of a group with more, also behind a short run that waits at the end; with at most one waiting run of
/// each group every round is 0 and the place is the end: first come first
fn turn_place(queue: &[TurnEntry], groups: &QueueGroups, group: &str) -> usize {
  let passes = groups.passes_in(queue);
  let own = queue.iter().filter(|entry| groups.group_of(entry) == group).count();
  let mut before: BTreeMap<&str, usize> = BTreeMap::new();
  let mut rounds = Vec::with_capacity(queue.len());
  let mut from = 0;
  for (position, entry) in queue.iter().enumerate() {
    let of = groups.group_of(entry);
    let round = before.entry(of).or_default();
    rounds.push(*round);
    *round += 1;
    let spent = passes.get(&entry.ticket.run).is_some_and(|&passes| passes >= TURN_PASS_BUDGET);
    if position == 0 || of == group || spent {
      from = position + 1;
    }
  }
  rounds.iter().skip(from).position(|&round| round > own).map_or(queue.len(), |at| from + at)
}

/// Whether a short run may go ahead of `entry` at `position` of the queue: an entry that is not short, passed fewer
/// than [`PASS_BUDGET`] times, and not an exclusive head whose drain began, the one window of the host for its
/// measurement
fn passable(entry: &TurnEntry, position: usize, marks: &QueueMarks, head_drain: Option<HeadDrain>) -> bool {
  let mark = marks.of(&entry.ticket.run);
  let drains = position == 0 && entry.exclusive && matches!(head_drain, Some(HeadDrain::Draining { .. } | HeadDrain::Ended { .. }));
  mark.short_ms.is_none() && mark.passes < PASS_BUDGET && !drains
}

/// No turn for the run yet, for the line of the wait
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnQueue {
  /// The cores of the host
  pub cores: u32,
  /// What the run needs of the host
  pub need: Need,
  /// The runs that hold cores, the holder of the build before among them
  pub holders: Vec<TurnEntry>,
  /// The live cores of the runs that are not heavy
  pub light: Cores,
  /// The busy cores of the host outside the live use of the holders, by the last sample of the host; `None` without a
  /// fresh one
  pub outside: Option<Cores>,
  /// The most busy cores of the host over its last 30 s, when its samples span them: what an exclusive run waits on
  pub window: Option<Cores>,
  /// The runs that are not heavy in their command: an exclusive run waits for each to end
  pub light_runs: Vec<LightEntry>,
  /// The runs before this one in the queue
  pub ahead: Vec<TurnEntry>,
  /// The drain of the exclusive run at the head of the queue, this run or the first ahead; `None` when the head is no
  /// exclusive run
  pub head_drain: Option<HeadDrain>,
  /// The group of the run (`brd-host run --group`), as its entry names it; `None`: its affinity
  pub group: Option<String>,
}

/// Where the exclusive run at the head of the queue is in its drain: what it lets pass
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum HeadDrain {
  /// It drains the host once no heavy run holds cores; no run passes it before its drain ends
  Pending,
  /// It drains the host until `until_ms`: the new runs that are not heavy wait for it, and no heavy run passes it
  Draining { until_ms: u64 },
  /// Its drain ended at `ended_ms` without a quiet host: the heavy runs behind it start past it in their order while
  /// they fit, and it starts once the host is quiet
  Ended { ended_ms: u64 },
}

/// A sample of the busy cores of the whole host, by the agent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HostSample {
  busy: Cores,
  at_ms: u64,
  /// The most busy cores of the samples of the last 30 s, and the time from the first of them to the last; none from an
  /// agent of the build before the window
  #[serde(default, skip_serializing_if = "Option::is_none")]
  window_max: Option<Cores>,
  #[serde(default)]
  window_ms: u64,
}

/// The span of the samples of the host that an exclusive run reads: every sample of the last 30 s must be quiet, as a
/// bursty run in a lull of one sample is no quiet host
pub const QUIET_WINDOW_MS: u64 = 30_000;
/// The least span of a window that counts: the samples of an agent that began less than this ago tell no quiet yet
const QUIET_WINDOW_MIN_MS: u64 = 28_000;

/// The samples of the busy cores of the whole host of the last 30 s, which `serve` keeps and notes ([`note_host_window`])
#[derive(Debug, Default)]
pub struct HostWindow {
  samples: std::collections::VecDeque<(u64, Cores)>,
}

impl HostWindow {
  /// Adds the sample `busy` at `at_ms`, and leaves out the samples more than 30 s before it
  pub fn push(&mut self, at_ms: u64, busy: Cores) {
    self.samples.push_back((at_ms, busy));
    while self.samples.front().is_some_and(|&(first, _)| at_ms.saturating_sub(first) > QUIET_WINDOW_MS) {
      self.samples.pop_front();
    }
  }

  fn sample(&self) -> Option<HostSample> {
    let &(at_ms, busy) = self.samples.back()?;
    let first = self.samples.front()?.0;
    Some(HostSample {
      busy,
      at_ms,
      window_max: self.samples.iter().map(|&(_, busy)| busy).max(),
      window_ms: at_ms.saturating_sub(first),
    })
  }
}

/// Notes `busy`, the busy cores of the whole host over the last interval, as a window of that one sample in `host.json`
/// of `dir` for the admissions
pub fn note_host(dir: &Path, busy: Cores) -> Result<()> {
  let mut window = HostWindow::default();
  window.push(now_ms(), busy);
  note_host_window(dir, &window)
}

/// Notes the last sample of `window` and the most of its samples in `host.json` of `dir`, for the admissions
pub fn note_host_window(dir: &Path, window: &HostWindow) -> Result<()> {
  let Some(sample) = window.sample() else { return Ok(()) };
  let json = serde_json::to_vec(&sample)?;
  sync_utils::atomic_file::write_atomically(&dir.join(HOST_FILE), &json, sync_utils::atomic_file::AtomicWrite { private: true, durable: false })?;
  Ok(())
}

/// The sample of the whole host at most 10 s old at `now_ms`; `None` without one
pub(crate) fn host_sample(dir: &Path, now_ms: u64) -> Option<HostSample> {
  let sample: HostSample = serde_json::from_slice(&std::fs::read(dir.join(HOST_FILE)).ok()?).ok()?;
  (now_ms.saturating_sub(sample.at_ms) <= HOST_FRESH_MS).then_some(sample)
}

/// The busy cores of the whole host by its last sample at most 10 s old at `now_ms`; `None` without one
pub(crate) fn host_busy(dir: &Path, now_ms: u64) -> Option<Cores> {
  host_sample(dir, now_ms).map(|sample| sample.busy)
}

/// The most busy cores of the whole host over the last 30 s at `now_ms`, when its samples span them; `None` without
/// such a window
pub(crate) fn host_window(dir: &Path, now_ms: u64) -> Option<Cores> {
  host_sample(dir, now_ms).and_then(window_of)
}

/// The most busy cores of the samples of `sample`, when they span 30 s
fn window_of(sample: HostSample) -> Option<Cores> {
  sample.window_max.filter(|_| sample.window_ms >= QUIET_WINDOW_MIN_MS)
}

/// The busy cores of the last sample of the host and the most of its last 30 s ([`host_window`]) at `now_ms`, from one
/// read of `host.json`, so both are of one sample; `None` without a fresh sample
pub(crate) fn host_now(dir: &Path, now_ms: u64) -> (Option<Cores>, Option<Cores>) {
  let sample = host_sample(dir, now_ms);
  (sample.map(|sample| sample.busy), sample.and_then(window_of))
}

/// The busy cores of the whole host by its last sample of at most 10 s ago, as the admissions read it; `None` without one
pub fn host_busy_now(dir: &Path) -> Option<Cores> {
  host_busy(dir, now_ms())
}

/// The busy cores of a quiet host of `cores` outside the heavy runs: a fifth of them, at least 1, as a host has a
/// standing background (a video editor and the window server held 1.2 of the 20 cores of the Studio for days)
pub fn quiet(cores: u32) -> Cores {
  Cores(cores.saturating_mul(2).max(10))
}

/// The busy cores outside the heavy holders: the larger of the live use of the runs that are not heavy and the busy
/// cores `busy` of the host less the live use of the holders
pub(crate) fn outside(holders: &[TurnEntry], light: Cores, busy: Option<Cores>) -> Option<Cores> {
  let live: Cores = holders.iter().filter_map(|holder| holder.live).sum();
  busy.map(|busy| Cores(busy.0.saturating_sub(live.0)).max(light))
}

/// Whether the head of the queue `entry` is admitted beside `holders` on a host of `cores` that is busy with `outside`
/// cores outside them (`light` without a sample of the host): to a turn without a holder when the host is quiet
/// ([`quiet`]), when the run has a measure that fits beside `outside`, or when it `waited_out` the load bound in the
/// queue ([`LOAD_BOUND`]); a run that takes the whole host to no holder; a run of a measure beside holders that do not
/// take the whole host, while their needs, `outside` and its own fit. An exclusive run, a measurement, waits for its
/// quiet host and its quiet window, whatever its wait
pub(crate) fn admits(entry: &TurnEntry, holders: &[TurnEntry], load: &Load<'_>, cores: u32, waited_out: bool) -> bool {
  let need = entry.need(cores, load.now_ms);
  let whole = Cores::whole(cores);
  let outside = load.outside;
  if need == Need::Exclusive {
    // no other run in its command, of any repository, and a quiet host over its window: a lull of one sample of a
    // bursty run is no quiet host
    return holders.is_empty() && load.light.is_empty() && load.quiet_over_window(cores);
  }
  if holders.is_empty() {
    return waited_out || outside <= quiet(cores) || (!need.is_whole_host() && outside.saturating_add(need.cores(cores)) <= whole);
  }
  if need.is_whole_host() || holders.iter().any(|holder| holder.need(cores, load.now_ms).is_whole_host()) {
    return false;
  }
  let held: Cores = holders.iter().map(|holder| holder.need(cores, load.now_ms).cores(cores)).sum();
  held.saturating_add(outside).saturating_add(need.cores(cores)) <= whole
}

/// The position of the run that may pass the exclusive run at the head of `queue`: the first that is not exclusive. The
/// exclusive runs between them wait for a quiet host as the head does; `None` when the head is no exclusive run
pub(crate) fn passer(queue: &[TurnEntry]) -> Option<usize> {
  queue.first().filter(|head| head.exclusive)?;
  queue.iter().position(|entry| !entry.exclusive)
}

/// Whether the run at `position` of `queue` starts past the exclusive run at its head: it is the first that is not
/// exclusive ([`passer`]), the drain of the head ended (`seen.head_drain`), the head cannot start now, and the run is
/// admitted by the ordinary rule ([`admits`]), its own wait counting for the load bound. Before the end of the drain
/// no run passes: the drain is the one window of the host for the measurement, and a holder beside it keeps the drain
/// from its start
fn passes(queue: &[TurnEntry], position: usize, seen: &Seen, load: &Load<'_>, cores: u32, waited_out: bool) -> bool {
  passer(queue) == Some(position)
    && matches!(seen.head_drain, Some(HeadDrain::Ended { .. }))
    && !admits(&queue[0], &seen.holders, load, cores, false)
    && admits(&queue[position], &seen.holders, load, cores, waited_out)
}

/// What an admission reads of the load of the host beside the holders
pub(crate) struct Load<'a> {
  /// The busy cores outside the holders (`light` without a sample of the host)
  pub(crate) outside: Cores,
  /// The most busy cores of the host over its last 30 s, when its samples span them
  pub(crate) window: Option<Cores>,
  /// Whether the agent noted a sample of the host at all: without one an exclusive run reads `outside`
  pub(crate) sampled: bool,
  /// The runs that are not heavy in their command
  pub(crate) light: &'a [LightEntry],
  /// The time of the admission, which the grace of a holder without a measure counts to ([`TurnEntry::need`])
  pub(crate) now_ms: u64,
}

impl Load<'_> {
  /// Every sample of the last 30 s is quiet; without a sample of the host, `outside` is; with a window shorter than 30 s,
  /// none is
  fn quiet_over_window(&self, cores: u32) -> bool {
    match (self.sampled, self.window) {
      (false, _) => self.outside <= quiet(cores),
      (true, Some(window)) => window <= quiet(cores),
      (true, None) => false,
    }
  }
}

/// Give the turn to the run of `request`, or put it in the queue ([`join`])
pub fn take(params: &TurnParams<'_>, request: &TurnRequest<'_>) -> Result<TurnAnswer> {
  join(params, request).map(|joined| joined.answer)
}

/// Give the run of `request` its cores of the turn, or put it in the queue, and say what its first ask did. Under the
/// lock of the table: the entries of gone processes leave it; a superseded run gets the later run and leaves
/// `superseded`; a holder holds still (a repeat is safe); a run new to the queue supersedes the runs of the queue of
/// its checkout and command and takes the place of the first of them, else joins the end, a short run ahead of the runs
/// it may pass ([`place`]); the head of the queue is
/// admitted while it fits ([`admits`]), beside the holder of `heavy.json` of the build before too, and so is the first
/// run behind an exclusive head whose drain ended, past it ([`passes`])
pub fn join(params: &TurnParams<'_>, request: &TurnRequest<'_>) -> Result<Joined> {
  let legacy = legacy_holder(params.dir);
  params.table().update(params.lock_timeout, |table: &mut HeavyTurn| {
    let table = table.of_this_schema(params.dir)?;
    table.prune();
    table.cores = params.cores;
    let mut joined = Joined {
      answer: TurnAnswer::Taken,
      replaced: Vec::new(),
      beside: Vec::new(),
      short: None,
      passed: Vec::new(),
    };
    if let Some(at) = table.superseded.iter().position(|entry| entry.ticket.run == request.run) {
      joined.answer = TurnAnswer::Superseded {
        by: table.superseded.remove(at).by,
      };
      return Ok(joined);
    }
    if let Some(holder) = table.holders.iter_mut().find(|holder| holder.ticket.run == request.run) {
      // an older build that rewrote the table dropped the field: the run puts it back, and its file too
      if request.exclusive && !holder.exclusive {
        holder.exclusive = true;
        write_exclusive(params.dir, holder)?;
      }
      return Ok(joined);
    }
    let queued = table.queue.iter().position(|entry| entry.ticket.run == request.run);
    let position = match queued {
      Some(position) => position,
      None => enqueue(table, params, request, &mut joined)?,
    };
    // the expectation of a run that waits follows the measures that came while it waited
    table.queue[position].expected = request.expected;
    table.queue[position].exclusive |= request.exclusive;
    let mut seen = Seen::of(params, table, legacy.as_ref());
    let waited_out = seen.now.saturating_sub(table.queue[position].since_ms) >= millis(params.load_bound);
    let load = seen.load(&table.light);
    // `Some(None)`: the head of the queue is admitted; `Some(Some(head))`: the run starts past the exclusive head
    let admitted = if position == 0 {
      admits(&table.queue[0], &seen.holders, &load, params.cores, waited_out).then_some(None)
    } else {
      passes(&table.queue, position, &seen, &load, params.cores, waited_out).then(|| Some(table.queue[0].clone()))
    };
    if let Some(past) = admitted {
      note_took(params.dir, &table.queue[position].ticket.run)?;
      let mut entry = table.queue.remove(position);
      entry.since_ms = seen.now;
      entry.passed = past.as_ref().map(|head| head.ticket.run.clone());
      if entry.exclusive {
        write_exclusive(params.dir, &entry)?;
        remove_named(&params.dir.join(DRAIN_FILE), &entry.ticket.run)?;
      }
      table.holders.push(entry);
      if let Some(exclusive) = past {
        joined.answer = TurnAnswer::Passed { exclusive };
      }
      return Ok(joined);
    }
    // an exclusive run at the head of the queue with no holder drains the host: the new runs that are not heavy wait
    if position == 0 && request.exclusive && seen.holders.is_empty() {
      let since = start_drain(params.dir, &table.queue[0], seen.now)?;
      seen.head_drain = Some(drain_state(since, seen.now, params.drain_bar));
    }
    joined.answer = TurnAnswer::Queued(seen.queue(params, table, position));
    Ok(joined)
  })
}

/// What a look of a run sees of the turn beside the queue: the holders, the holder of `heavy.json` of the build before
/// first, the live cores of the runs that are not heavy, the load of the host by its last sample, and the drain of an
/// exclusive head of the queue, at `now`
struct Seen {
  holders: Vec<TurnEntry>,
  light: Cores,
  /// The busy cores outside the holders by a fresh sample of the host ([`outside`]); `None` without one
  outside: Option<Cores>,
  window: Option<Cores>,
  sampled: bool,
  head_drain: Option<HeadDrain>,
  now: u64,
}

impl Seen {
  fn of(params: &TurnParams<'_>, table: &HeavyTurn, legacy: Option<&TurnEntry>) -> Self {
    let holders: Vec<TurnEntry> = legacy.into_iter().cloned().chain(table.holders.iter().cloned()).collect();
    let light = table.light_cores();
    let now = now_ms();
    let sample = host_sample(params.dir, now);
    Self {
      outside: outside(&holders, light, sample.map(|sample| sample.busy)),
      window: host_window(params.dir, now),
      sampled: sample.is_some(),
      head_drain: head_drain(params.dir, &table.queue, now, params.drain_bar),
      holders,
      light,
      now,
    }
  }

  fn load<'a>(&self, light: &'a [LightEntry]) -> Load<'a> {
    Load {
      outside: self.outside.unwrap_or(self.light),
      window: self.window,
      sampled: self.sampled,
      light,
      now_ms: self.now,
    }
  }

  /// The answer to the run at `position` of the queue of `table` that is not admitted
  fn queue(self, params: &TurnParams<'_>, table: &HeavyTurn, position: usize) -> TurnQueue {
    TurnQueue {
      cores: params.cores,
      need: table.queue[position].need(params.cores, self.now),
      holders: self.holders,
      light: self.light,
      outside: self.outside,
      window: self.window,
      light_runs: table.light.clone(),
      ahead: table.queue[..position].to_vec(),
      head_drain: self.head_drain,
      group: table.queue[position].group.clone(),
    }
  }
}

/// A look of the run of `request` that waits for the turn: without the lock of the table while the run waits behind the
/// head of the queue and its entry is as the request says, as such a look changes nothing; else [`take`]. So only the
/// head of the queue, the run that may pass an exclusive head whose drain ended ([`passes`]) and a run whose entry
/// changes take the lock at each change of the table, not every run that waits
pub fn look(params: &TurnParams<'_>, request: &TurnRequest<'_>) -> Result<TurnAnswer> {
  let mut table: HeavyTurn = params.table().read()?;
  table.of_this_schema(params.dir)?;
  table.prune();
  let behind = table.queue.iter().position(|entry| entry.ticket.run == request.run).filter(|&position| {
    let entry = &table.queue[position];
    position > 0 && entry.expected == request.expected && (entry.exclusive || !request.exclusive)
  });
  // a superseded run, a holder and a run new to the queue are in no place of the queue
  let Some(position) = behind else {
    return take(params, request);
  };
  let legacy = legacy_holder(params.dir);
  let seen = Seen::of(params, &table, legacy.as_ref());
  if passer(&table.queue) == Some(position) && matches!(seen.head_drain, Some(HeadDrain::Ended { .. })) {
    return take(params, request);
  }
  Ok(TurnAnswer::Queued(seen.queue(params, &table, position)))
}

/// Puts the run of `request`, new to the queue, in its place in `table` ([`join_queue`]), with the files beside it
/// read before and written after, under the lock of the table
fn enqueue(table: &mut HeavyTurn, params: &TurnParams<'_>, request: &TurnRequest<'_>, joined: &mut Joined) -> Result<usize> {
  let mut files = QueueFiles::read(params.dir)?;
  let before = files.clone();
  let position = join_queue(table, &mut files, (params.cores, params.drain_bar), now_ms(), request, joined);
  if files.marks != before.marks {
    write_marks(params.dir, &files.marks)?;
  }
  if files.groups != before.groups {
    write_groups(params.dir, &files.groups)?;
  }
  Ok(position)
}

/// Puts the run of `request`, new to the queue of `table`, in its place at `now_ms`, on a host of `cores` with the
/// drain of an exclusive head of `drain_bar`, and changes `files` with it: the one rule of the join, which the join
/// applies to the files of the host and the estimate of a start to a copy, so both place a run alike. The runs of the
/// queue of its checkout and command go to `superseded` with it as the later run (`joined.replaced`), and it takes the
/// place of the first of them when it is of its group, with the most passes of them and the runs they went ahead of,
/// else its turn ([`turn_place`]), and the passes of the replaced runs end with them; a short run goes ahead of the runs
/// before that place that it may pass ([`place`]), counts a pass of each in `passes.json` and takes the earliest join of
/// them (`joined.short`, `joined.passed`); a run of the turns names each run it goes ahead of in `groups.json` and keeps
/// its own join; `joined.beside` gets the runs of its checkout with another command. Answers the place
pub fn join_queue(table: &mut HeavyTurn, files: &mut QueueFiles, (cores, drain_bar): (u32, Duration), now: u64, request: &TurnRequest<'_>, joined: &mut Joined) -> usize {
  let superseded_at = table.queue.iter().position(|entry| entry.is_of(request, true));
  let (replaced, kept): (Vec<TurnEntry>, Vec<TurnEntry>) = std::mem::take(&mut table.queue).into_iter().partition(|entry| entry.is_of(request, true));
  table.queue = kept;
  table.superseded.extend(replaced.iter().map(|entry| SupersededEntry {
    ticket: entry.ticket.clone(),
    by: request.run.to_owned(),
  }));
  joined.beside = table.queue.iter().filter(|entry| entry.is_of(request, false)).cloned().collect();
  let QueueFiles { marks, groups, drain } = files;
  let group = group_key(request.group, request.affinity, request.run).to_owned();
  // a run that supersedes takes the place of the first run it replaces, of the same group, and the runs they went
  // ahead of; of another group it takes its own turn, and their passes end with them, as they left the queue
  let takes_place = superseded_at.is_some() && replaced.first().is_some_and(|first| groups.group_of(first) == group);
  let mut mark = GroupMark { group, ..GroupMark::default() };
  for entry in &replaced {
    if let Some(replaced_mark) = groups.runs.remove(&entry.ticket.run)
      && takes_place
    {
      mark.passed.extend(replaced_mark.passed);
    }
  }
  // the place of the first superseded run: the runs before it are before it still
  let end = if takes_place {
    superseded_at.unwrap_or_default().min(table.queue.len())
  } else {
    table.queue.len()
  };
  let short = short_wall(request.exclusive, request.expected, request.passed_wall, cores);
  let head = drain.head_drain(&table.queue, now, drain_bar);
  let turn = (!takes_place).then_some((&*groups, mark.group.as_str()));
  let position = place(&table.queue[..end], marks, head, short.is_some(), turn);
  let carried = replaced.iter().map(|entry| marks.of(&entry.ticket.run).passes).max().unwrap_or(0);
  // the bookkeeping of the short lane is of a short run only: a run of the turns names the runs it passes in groups.json
  if short.is_some() {
    for entry in &table.queue[position..end] {
      let mark = marks.runs.entry(entry.ticket.run.clone()).or_default();
      mark.passes += 1;
      joined.passed.push((entry.clone(), mark.passes));
    }
  } else if !takes_place {
    mark.passed.extend(table.queue[position..end].iter().map(|entry| entry.ticket.run.clone()));
  }
  let run_mark = RunMark {
    short_ms: short.map(millis),
    passes: carried,
    joined_ms: (!joined.passed.is_empty()).then_some(now),
  };
  if run_mark != RunMark::default() {
    marks.runs.insert(request.run.to_owned(), run_mark);
  }
  let queue = &table.queue;
  let in_queue = |run: &String| run == request.run || queue.iter().any(|entry| entry.ticket.run == *run);
  marks.runs.retain(|run, _| in_queue(run));
  groups.runs.insert(request.run.to_owned(), mark);
  // a mark counts while its run waits, or once it took the turn while a run it passed waits
  groups.runs.retain(|run, mark| in_queue(run) || (mark.took && mark.passed.iter().any(in_queue)));
  joined.replaced = replaced;
  joined.short = short;
  let entry = TurnEntry {
    ticket: Ticket {
      run: request.run.to_owned(),
      pid: request.pid,
      process_started: request.process_started,
    },
    repo: request.repo.to_owned(),
    command: one_line(request.command),
    // a short run: the earliest join of the runs it passed, so a head that it passed on a busy free turn waits no new
    // load bound; a run of the turns, which never passes the head, its own join
    since_ms: joined.passed.iter().map(|(entry, _)| entry.since_ms).fold(now, u64::min),
    affinity: Some(request.affinity.to_owned()),
    command_key: Some(request.command_key.to_owned()),
    expected: request.expected,
    live: None,
    exclusive: request.exclusive,
    passed: None,
    measure_as: request.measure_as.map(str::to_owned),
    recent: None,
    group: request.group.map(str::to_owned),
  };
  table.queue.insert(position, entry);
  position
}

/// Notes in `groups.json` of `dir` that the run `run` of the queue took the turn, under the lock of the table and before
/// the table: the runs it went ahead of count its pass from now on, after it left the queue too. A run without a mark,
/// of the build before, notes nothing
fn note_took(dir: &Path, run: &str) -> Result<()> {
  let mut groups = read_groups(dir)?;
  match groups.runs.get_mut(run) {
    Some(mark) if !mark.took => {
      mark.took = true;
      write_groups(dir, &groups)
    }
    _ => Ok(()),
  }
}

/// Give the turn to the run of `request` (`take`), waiting up to `timeout` while it is queued, woken by `watch`, which
/// the caller started before its first look and keeps for its whole wait; without a file event it looks again every
/// `params.recheck`, since the end of a holder makes none and an event may come late or never (`FSEvents` under load).
/// The answer is the turn, the later run that superseded it, or the place in the queue once the time is up: the ticket
/// stays while the process lives, and `leave` takes it out
pub fn take_waiting(params: &TurnParams<'_>, request: &TurnRequest<'_>, timeout: Duration, watch: &TurnWatch) -> Result<TurnAnswer> {
  let started = Instant::now();
  loop {
    let queued = match look(params, request)? {
      TurnAnswer::Queued(queued) => queued,
      answer => return Ok(answer),
    };
    let waited = started.elapsed();
    if waited >= timeout {
      return Ok(TurnAnswer::Queued(queued));
    }
    watch.0.wait(timeout.saturating_sub(waited).min(params.recheck));
  }
}

/// The live use of a run whose command runs, at a sample of its supervisor
pub struct UseNote<'a> {
  pub run: &'a str,
  pub pid: u32,
  pub process_started: Option<u64>,
  pub repo: &'a str,
  pub command: &'a [String],
  pub live: Cores,
}

/// Notes the live cores of the run of `note`: a holder's `live`, which raises what it holds once it is past its
/// expectation, and its window too (`recent`), which it holds past its grace; a run that
/// is neither a holder nor waits is a run that is not heavy, and its entry of `light` gets them. The admissions that
/// follow count them
pub fn note_use(params: &TurnParams<'_>, note: &UseNote<'_>) -> Result<()> {
  params.table().update(params.lock_timeout, |table: &mut HeavyTurn| {
    let table = table.of_this_schema(params.dir)?;
    table.prune();
    table.cores = params.cores;
    if let Some(holder) = table.holders.iter_mut().find(|holder| holder.ticket.run == note.run) {
      holder.live = Some(note.live);
      if !holder.exclusive {
        holder.recent.get_or_insert_default().push(now_ms(), note.live);
      }
    } else if let Some(light) = table.light.iter_mut().find(|light| light.ticket.run == note.run) {
      light.live = note.live;
    } else if !table.queue.iter().any(|entry| entry.ticket.run == note.run) {
      table.light.push(LightEntry {
        ticket: Ticket {
          run: note.run.to_owned(),
          pid: note.pid,
          process_started: note.process_started,
        },
        repo: note.repo.to_owned(),
        command: one_line(note.command),
        live: note.live,
      });
    }
    Ok(())
  })
}

/// The run `run` leaves the turn: it gives its cores back, or its entry leaves the queue, `superseded` or `light`.
/// Answers whether it held cores
pub fn leave(params: &TurnParams<'_>, run: &str) -> Result<bool> {
  params.table().update(params.lock_timeout, |table: &mut HeavyTurn| {
    let table = table.of_this_schema(params.dir)?;
    // a run that leaves the queue without the turn counts no pass of the runs it went ahead of: its mark names no run
    // of the queue and no turn taken (`QueueGroups::passes_in`), and the next join drops it
    table.queue.retain(|entry| entry.ticket.run != run);
    table.superseded.retain(|entry| entry.ticket.run != run);
    table.light.retain(|entry| entry.ticket.run != run);
    let held = table.holders.iter().any(|holder| holder.ticket.run == run);
    table.holders.retain(|holder| holder.ticket.run != run);
    remove_exclusive(params.dir, run)?;
    remove_named(&params.dir.join(DRAIN_FILE), run)?;
    Ok(held)
  })
}

/// The heavy turn of the host in `dir` without the runs whose process is gone, read without the lock, the live holder
/// of `heavy.json` of the build before first among the holders: what `brd-host runs` shows
pub fn read_live(dir: &Path) -> Result<HeavyTurn> {
  let mut table: HeavyTurn = table_file(dir).read()?;
  table.of_this_schema(dir)?;
  table.prune();
  if let Some(exclusive) = exclusive_holder(dir)
    && let Some(holder) = table.holders.iter_mut().find(|holder| holder.ticket.run == exclusive.ticket.run)
  {
    holder.exclusive = true;
  }
  if let Some(legacy) = legacy_holder(dir) {
    table.holders.insert(0, legacy);
  }
  Ok(table)
}

/// The exclusive run that holds the turn of the host in `dir`, if one does and its process lives: a new run that is not
/// heavy waits for its end before its lend. A table that cannot be read holds none, so it stops no run
pub fn exclusive_holder(dir: &Path) -> Option<TurnEntry> {
  let entry: TurnEntry = serde_json::from_slice(&std::fs::read(dir.join(EXCLUSIVE_FILE)).ok()?).ok()?;
  entry.is_running().then_some(entry)
}

/// An exclusive run that drains the host, and since when
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Drain {
  #[serde(flatten)]
  entry: TurnEntry,
  /// When the drain began; not `since_ms`, which the entry has for its place in the queue
  drain_since_ms: u64,
}

/// Notes that `entry`, an exclusive run at the head of the queue with no holder, drains the host since `now_ms`, or
/// since it began to, under the lock of the table; answers the start of its drain. A drain of a run never starts again
fn start_drain(dir: &Path, entry: &TurnEntry, now_ms: u64) -> Result<u64> {
  if let Some(since) = drain_since(dir, &entry.ticket) {
    return Ok(since);
  }
  let drain = Drain {
    entry: entry.clone(),
    drain_since_ms: now_ms,
  };
  sync_utils::atomic_file::write_atomically(
    &dir.join(DRAIN_FILE),
    &serde_json::to_vec(&drain)?,
    sync_utils::atomic_file::AtomicWrite { private: true, durable: false },
  )?;
  Ok(now_ms)
}

/// The start of the drain of the run of `ticket` by `drain.json` of `dir`; `None` when the file names no drain of it
fn drain_since(dir: &Path, ticket: &Ticket) -> Option<u64> {
  DrainFile::read(dir).since(ticket)
}

/// The exclusive run that `drain.json` names as one that drains the host, and the start of its drain: what the drain
/// of the head of any queue is ([`Self::head_drain`])
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DrainFile {
  pub drained: Option<(Ticket, u64)>,
}

impl DrainFile {
  /// The drain of `drain.json` of `dir`; none when the file is missing or cannot be read
  pub fn read(dir: &Path) -> Self {
    let drain = std::fs::read(dir.join(DRAIN_FILE)).ok().and_then(|bytes| serde_json::from_slice::<Drain>(&bytes).ok());
    Self {
      drained: drain.map(|drain| (drain.entry.ticket, drain.drain_since_ms)),
    }
  }

  fn since(&self, ticket: &Ticket) -> Option<u64> {
    self.drained.as_ref().filter(|(drained, _)| drained == ticket).map(|&(_, since)| since)
  }

  /// The drain of the exclusive run at the head of `queue` at `now_ms`, with a drain of `drain_bar`; `None` when the head
  /// is no exclusive run
  pub fn head_drain(&self, queue: &[TurnEntry], now_ms: u64, drain_bar: Duration) -> Option<HeadDrain> {
    let head = queue.first().filter(|head| head.exclusive)?;
    Some(self.since(&head.ticket).map_or(HeadDrain::Pending, |since| drain_state(since, now_ms, drain_bar)))
  }
}

/// The drain of an exclusive run that began at `since_ms`, at `now_ms`, with a drain of `drain_bar`
fn drain_state(since_ms: u64, now_ms: u64, drain_bar: Duration) -> HeadDrain {
  let until_ms = since_ms.saturating_add(millis(drain_bar));
  if now_ms < until_ms {
    HeadDrain::Draining { until_ms }
  } else {
    HeadDrain::Ended { ended_ms: until_ms }
  }
}

/// The drain of the exclusive run at the head of `queue` at `now_ms`, with a drain of `drain_bar` ([`DRAIN_BAR`]), by
/// `drain.json` of `dir`; `None` when the head is no exclusive run. What `brd-host runs` says of the head
pub fn head_drain(dir: &Path, queue: &[TurnEntry], now_ms: u64, drain_bar: Duration) -> Option<HeadDrain> {
  DrainFile::read(dir).head_drain(queue, now_ms, drain_bar)
}

fn millis(duration: Duration) -> u64 {
  u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// What keeps a new run that is not heavy from its lend: an exclusive run that holds the host, or one that drains it
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bar {
  /// The measurement holds the host, until its end
  Holds(TurnEntry),
  /// The measurement waits for the runs to end, and the bar lifts at `until_ms`
  Drains { entry: TurnEntry, since_ms: u64, until_ms: u64 },
}

/// The bar of the new runs that are not heavy at `now_ms` in the turn of `dir`, with a drain that bars them for at most
/// `drain_bar` from its start ([`DRAIN_BAR`]); `None` when they start
pub fn bar(dir: &Path, now_ms: u64, drain_bar: Duration) -> Option<Bar> {
  if let Some(holder) = exclusive_holder(dir) {
    return Some(Bar::Holds(holder));
  }
  let drain: Drain = serde_json::from_slice(&std::fs::read(dir.join(DRAIN_FILE)).ok()?).ok()?;
  let until_ms = drain.drain_since_ms.saturating_add(millis(drain_bar));
  (drain.entry.is_running() && now_ms < until_ms).then_some(Bar::Drains {
    entry: drain.entry,
    since_ms: drain.drain_since_ms,
    until_ms,
  })
}

/// Writes `entry`, an exclusive run that takes its turn, into `exclusive.json` of `dir`, under the lock of the table
fn write_exclusive(dir: &Path, entry: &TurnEntry) -> Result<()> {
  let json = serde_json::to_vec(entry)?;
  sync_utils::atomic_file::write_atomically(&dir.join(EXCLUSIVE_FILE), &json, sync_utils::atomic_file::AtomicWrite { private: true, durable: false })?;
  Ok(())
}

/// Removes `exclusive.json` of `dir` when it names the run `run`, or a run whose process is gone, under the lock of the
/// table
fn remove_exclusive(dir: &Path, run: &str) -> Result<()> {
  remove_named(&dir.join(EXCLUSIVE_FILE), run)
}

/// Removes the file of an entry at `path` (`exclusive.json`, `drain.json`) when it names the run `run`, a run whose
/// process is gone, or none, under the lock of the table
fn remove_named(path: &Path, run: &str) -> Result<()> {
  let Ok(bytes) = std::fs::read(path) else { return Ok(()) };
  let named = serde_json::from_slice::<TurnEntry>(&bytes).ok();
  if named.is_none_or(|entry| entry.ticket.run == run || !entry.is_running()) {
    match sync_utils::fs_retry::remove_file(path) {
      Ok(()) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(error) => return Err(error.into()),
    }
  }
  Ok(())
}

/// The words of a command as one line, at most `COMMAND_CHARS` characters
pub(crate) fn one_line(command: &[String]) -> String {
  // a word of a script holds its line ends: `zsh -c 'a\nb'`
  let line = command.join(" ").replace(['\n', '\r', '\t'], " ");
  match line.char_indices().nth(COMMAND_CHARS) {
    Some((cut, _)) => format!("{}…", &line[..cut]),
    None => line,
  }
}

#[cfg(test)]
#[path = "host_turn_test.rs"]
pub(crate) mod host_turn_test;
