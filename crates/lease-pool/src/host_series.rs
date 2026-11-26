//! The series of the heavy turn of a host by the minute (openspec/specs/cli/host/spec.md, "stats: serve keeps a series
//! of the heavy turn by the minute"; docs/design/host-runs.md, "The series of the heavy turn"). `serve` writes one
//! [`TurnMinute`] a minute while a heavy run holds cores or waits, into `~/.cache/brd-host/history/turn-<YYYY-MM-DD>.jsonl`
//! of the UTC day, for [`KEEP_DAYS`] days and at most [`DAY_BYTES`] a day. A minute names the holders with what each
//! counts and uses, the head of the queue and whether it fits, the busy cores outside the heavy runs, why no run
//! started ([`IdleCause`]), and what the backfill of a run behind the head would have started ([`DryBackfill`]): a dry
//! run, which no admission reads. The dry runs keep their starts from minute to minute ([`DryState`]), as a backfill
//! would: a started run leaves the candidates, counts as a holder until its wall ends, and counts a pass of its head. The
//! turn decides as before (`host_turn::join`); the series only reads it, without the lock, as `brd-host runs` does, so a
//! supervisor of an older build never meets it. Each enum of a minute reads a value of a later build as its `Other`,
//! so a line of a later agent still counts.
//!
//! [`aggregate`] is the part of the summary of `brd-host stats` that the series gives: the idle cores while heavy runs
//! waited, by cause, and the starts of the dry backfill

use crate::host_estimate::{Sight, head_start};
use crate::host_history::{is_false, is_zero};
use crate::host_turn::{HeadDrain, LOAD_BOUND, Load, PASS_BUDGET, TurnEntry, admits, head_drain, host_now, outside, passer, read_live, read_marks};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;
use sync_utils::cores::Cores;

/// The schema of a minute
pub const SCHEMA: u32 = 1;
/// How long the agent keeps the minutes, by the UTC day of their file
pub const KEEP_DAYS: u32 = 30;
/// The most bytes of the minutes of one day: about 3 times a full day of a busy host, so a fault that writes too much
/// stops there
pub const DAY_BYTES: u64 = 4 * 1024 * 1024;
/// How often `serve` writes a minute
pub const EVERY: Duration = Duration::from_mins(1);
/// The longest wall of the passed runs of a command that the dry backfill starts behind the head (design 4.1): a run of
/// longer walls is a gate, whose wall is no promise to the head
pub const BACKFILL_WALL: Duration = Duration::from_mins(30);
/// The longest command of an entry of a minute, in characters: the series holds a run of each minute
const COMMAND_CHARS: usize = 80;

/// The busy cores of the samples of the host over one minute, as `serve` took them every 2 s
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MinuteBusy {
  /// Their mean
  pub mean: Cores,
  /// Their most
  pub most: Cores,
}

/// One minute of the heavy turn of a host
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnMinute {
  #[serde(default)]
  pub schema: u32,
  pub at_ms: u64,
  /// The cores of the host that the heavy runs share
  pub cores: u32,
  /// The mean busy cores of the samples of the host over the minute; `None` without a sample
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub busy: Option<Cores>,
  /// The most busy cores of those samples
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub busy_most: Option<Cores>,
  /// The live cores of the runs that are not heavy
  #[serde(default)]
  pub light: Cores,
  /// The busy cores outside the heavy runs, as an admission counts them (`host_turn::outside`): the larger of `light`
  /// and the busy cores of the last sample less the live cores of the holders; `None` without a fresh sample
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub outside: Option<Cores>,
  #[serde(default)]
  pub holders: Vec<MinuteHolder>,
  /// The runs that wait for the turn
  #[serde(default)]
  pub queue: u32,
  /// The head of the queue; `None` with no run in the queue
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub head: Option<MinuteHead>,
  /// Why no run of the queue started: `None` with no run in the queue, or when the run that can start fits and starts
  /// at its next look
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cause: Option<IdleCause>,
  /// The runs that the backfill of a run behind the head would have started in this minute ([`DryState`]); nothing
  /// starts them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub backfills: Vec<DryBackfill>,
  /// The runs that the dry backfill started in earlier minutes and that hold still, by their walls
  #[serde(default, skip_serializing_if = "is_zero")]
  pub dry_holders: u32,
}

/// A holder of a minute
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinuteHolder {
  pub run: String,
  /// The file name of its main repository
  pub repo: String,
  /// Its command as one line of at most 80 characters
  pub command: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub measure_as: Option<String>,
  #[serde(default, skip_serializing_if = "is_false")]
  pub exclusive: bool,
  /// What it counts of the host (`TurnEntry::need`)
  pub counted: Cores,
  /// It counts the whole host: no measure, a measure of the whole host, or a measurement
  #[serde(default, skip_serializing_if = "is_false")]
  pub whole: bool,
  /// Its last sample; `None` before its command, or in the first 10 s of it
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub live: Option<Cores>,
  pub since_ms: u64,
}

/// The head of the queue of a minute
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinuteHead {
  pub run: String,
  pub repo: String,
  pub command: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub measure_as: Option<String>,
  #[serde(default, skip_serializing_if = "is_false")]
  pub exclusive: bool,
  /// What it needs of the host
  pub need: Cores,
  #[serde(default, skip_serializing_if = "is_false")]
  pub whole: bool,
  /// It is admitted at its next look (`host_turn::admits`)
  #[serde(default, skip_serializing_if = "is_false")]
  pub fits: bool,
  /// When it joined the queue
  pub since_ms: u64,
  /// The drain of an exclusive head
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub drain: Option<MinuteDrain>,
  /// The run that may pass an exclusive head whose drain ended (`host_turn::passer`)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub passer: Option<String>,
}

/// The drain of an exclusive head of a minute (`host_turn::HeadDrain`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum MinuteDrain {
  Pending,
  Draining {
    until_ms: u64,
  },
  Ended {
    ended_ms: u64,
  },
  /// A state of a later build
  #[serde(other)]
  Other,
}

impl From<HeadDrain> for MinuteDrain {
  fn from(drain: HeadDrain) -> Self {
    match drain {
      HeadDrain::Pending => Self::Pending,
      HeadDrain::Draining { until_ms } => Self::Draining { until_ms },
      HeadDrain::Ended { ended_ms } => Self::Ended { ended_ms },
    }
  }
}

/// Why no run of the queue started at a minute: of the run that can start, the head of the queue or the run that may
/// pass an exclusive head whose drain ended
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdleCause {
  /// An exclusive run holds the host: no heavy run is admitted beside it
  Exclusive,
  /// The head is an exclusive run that drains the host: no run passes it
  Drain,
  /// The head is an exclusive run that waits for the holders to end before its drain: no run passes it
  Pending,
  /// The head is an exclusive run whose drain ended, it waits for a quiet host, and no run behind it may pass it
  Quiet,
  /// The run takes the whole host, by no measure or a measure of the whole host, beside holders or a busy host
  WholeHost,
  /// A holder whose command runs counts the whole host: no measure in its grace, or a measure of the whole host
  WholeHolder,
  /// The run would fit but for the holders whose command has not run yet: they count their cores in their lend of a
  /// slot, their checkout or their prepare
  Slots,
  /// The run of a measure does not fit beside the holders and the busy cores outside them
  Cores,
  /// A cause of a later build
  #[serde(other)]
  Other,
}

impl IdleCause {
  /// The word of the cause in the lines of a summary
  pub const fn word(self) -> &'static str {
    match self {
      Self::Exclusive => "an exclusive run holds",
      Self::Drain => "an exclusive head drains",
      Self::Pending => "an exclusive head waits for the holders",
      Self::Quiet => "an exclusive head waits for a quiet host",
      Self::WholeHost => "the head needs the whole host",
      Self::WholeHolder => "a holder counts the whole host",
      Self::Slots => "holders before their command",
      Self::Cores => "the cores",
      Self::Other => "a cause of a later build",
    }
  }
}

/// The run that the backfill of design 4.1 would have started behind the head: not exclusive, of a measure below the
/// whole host, admitted now with the head left out and no load bound, of a passed wall `W` of at most
/// [`BACKFILL_WALL`], and `1.25 W` within the estimated start `S` of the head, which no unknown wall delays
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DryBackfill {
  pub run: String,
  pub command: String,
  /// What it counts as a holder of the dry run
  pub cores: Cores,
  /// `W`: the 80th percentile of the walls of the last passed runs of its command
  pub wall_ms: u64,
  /// The head that it would start behind: the head of the queue, or the run that may pass an exclusive head
  pub head: String,
  /// `S`: when the head would start, from the minute (`host_estimate::head_start`)
  pub head_start_ms: u64,
}

/// The walls of the runs of the turn that a minute reads, in milliseconds: `wall` of the command of an entry by its last
/// runs, as an estimate reads it, and `passed` of its last passed runs, of 3 at least (`host_cores::passed_wall`)
pub struct Walls<'a> {
  pub wall: &'a dyn Fn(&TurnEntry) -> Option<u64>,
  pub passed: &'a dyn Fn(&TurnEntry) -> Option<u64>,
}

/// The starts of the dry backfill that a minute keeps for the next, in the memory of the writer of the series: a restart
/// of `serve` begins with none
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DryState {
  /// The runs that the dry backfill started, each as a holder since its start, with its wall and its head
  starts: Vec<DryStart>,
  /// The dry starts past each head of the queue, by the id of its run
  passes: BTreeMap<String, u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DryStart {
  entry: TurnEntry,
  wall_ms: u64,
}

impl DryState {
  /// Forgets the starts of the runs that left `queue` and the passes of its heads that left it
  fn forget(&mut self, queue: &[TurnEntry]) {
    let queued = |run: &str| queue.iter().any(|entry| entry.ticket.run == run);
    self.starts.retain(|start| queued(&start.entry.ticket.run));
    self.passes.retain(|head, _| queued(head));
  }

  /// The runs that the dry backfill started and whose walls have not ended at `now_ms`, as holders
  fn holders(&self, now_ms: u64) -> impl Iterator<Item = &TurnEntry> {
    self
      .starts
      .iter()
      .filter(move |start| start.entry.since_ms.saturating_add(start.wall_ms) > now_ms)
      .map(|start| &start.entry)
  }

  fn started(&self, run: &str) -> bool {
    self.starts.iter().any(|start| start.entry.ticket.run == run)
  }

  fn wall_of(&self, entry: &TurnEntry) -> Option<u64> {
    self.starts.iter().find(|start| start.entry.ticket.run == entry.ticket.run).map(|start| start.wall_ms)
  }
}

/// The minute of the heavy turn of the host in `dir` at `now_ms`, with `busy` the samples of the host over it, the
/// cores of the table, else `cores`, a drain of `drain_bar`, and `dry` the starts of the dry backfill of the minutes
/// before, which it updates; `None` while no heavy run holds cores or waits
pub fn minute(dir: &Path, cores: u32, now_ms: u64, drain_bar: Duration, busy: Option<MinuteBusy>, walls: &Walls<'_>, dry: &mut DryState) -> Result<Option<TurnMinute>> {
  let turn = read_live(dir)?;
  dry.forget(&turn.queue);
  if turn.holders.is_empty() && turn.queue.is_empty() {
    return Ok(None);
  }
  let cores = if turn.cores > 0 { turn.cores } else { cores };
  let light = turn.light_cores();
  let (busy_now, window) = host_now(dir, now_ms);
  let outside_now = outside(&turn.holders, light, busy_now);
  let load = Load {
    outside: outside_now.unwrap_or(light),
    window,
    sampled: busy_now.is_some(),
    light: &turn.light,
    now_ms,
  };
  let drain = head_drain(dir, &turn.queue, now_ms, drain_bar);
  let judged = judge(&turn.queue, &turn.holders, drain, &load, cores);
  let dry_holders = count(dry.holders(now_ms).count());
  let backfills = judged.backfill_head.map_or_else(Vec::new, |position| {
    let marks = read_marks(dir);
    let sight = Sight {
      turn: turn.clone(),
      cores,
      outside: outside(&turn.holders, light, window),
      window,
      sampled: load.sampled,
      marks,
      groups: crate::host_turn::read_groups(dir),
      head_drain: drain,
      now_ms,
    };
    dry_backfills(&sight, position, &load, walls, dry)
  });
  let head = turn.queue.first().map(|first| {
    let need = first.need(cores, now_ms);
    MinuteHead {
      run: first.ticket.run.clone(),
      repo: repo_name(&first.repo),
      command: short(&first.command),
      measure_as: first.measure_as.clone(),
      exclusive: first.exclusive,
      need: need.cores(cores),
      whole: need.is_whole_host(),
      fits: judged.head_fits,
      since_ms: first.since_ms,
      drain: drain.map(MinuteDrain::from),
      passer: judged.passer.map(|position| turn.queue[position].ticket.run.clone()),
    }
  });
  let holders = turn
    .holders
    .iter()
    .map(|holder| {
      let need = holder.need(cores, now_ms);
      MinuteHolder {
        run: holder.ticket.run.clone(),
        repo: repo_name(&holder.repo),
        command: short(&holder.command),
        measure_as: holder.measure_as.clone(),
        exclusive: holder.exclusive,
        counted: need.cores(cores),
        whole: need.is_whole_host(),
        live: holder.live,
        since_ms: holder.since_ms,
      }
    })
    .collect();
  Ok(Some(TurnMinute {
    schema: SCHEMA,
    at_ms: now_ms,
    cores,
    busy: busy.map(|busy| busy.mean),
    busy_most: busy.map(|busy| busy.most),
    light,
    outside: outside_now,
    holders,
    queue: u32::try_from(turn.queue.len()).unwrap_or(u32::MAX),
    head,
    cause: judged.cause,
    backfills,
    dry_holders,
  }))
}

/// What a minute judges of the queue
struct Judged {
  head_fits: bool,
  /// The run that may pass an exclusive head whose drain ended
  passer: Option<usize>,
  cause: Option<IdleCause>,
  /// The place of the head of the dry backfill: the head of the queue, or the run that may pass an exclusive head whose
  /// drain ended; `None` when nothing may start behind it: no run waits, the run that can start fits, an exclusive run
  /// holds or drains, or only exclusive runs wait
  backfill_head: Option<usize>,
}

/// Why no run of `queue` starts beside `holders` and `load` on a host of `cores`, with `drain` the drain of an
/// exclusive head: by the rule of the turn (`host_turn::admits`, `host_turn::passes`), the head counting its own wait for
/// the load bound, an exclusive one none
fn judge(queue: &[TurnEntry], holders: &[TurnEntry], drain: Option<HeadDrain>, load: &Load<'_>, cores: u32) -> Judged {
  let Some(first) = queue.first() else {
    return Judged {
      head_fits: false,
      passer: None,
      cause: None,
      backfill_head: None,
    };
  };
  let waited_out = |entry: &TurnEntry| !entry.exclusive && load.now_ms.saturating_sub(entry.since_ms) >= millis(LOAD_BOUND);
  let head_fits = admits(first, holders, load, cores, waited_out(first));
  let passer = passer(queue).filter(|_| matches!(drain, Some(HeadDrain::Ended { .. })));
  // the cause of the run that can start, and the head of a backfill behind it
  let of = |position: usize| -> (Option<IdleCause>, Option<usize>) {
    let entry = &queue[position];
    if admits(entry, holders, load, cores, waited_out(entry)) {
      return (None, None);
    }
    let started: Vec<TurnEntry> = holders.iter().filter(|holder| holder.live.is_some()).cloned().collect();
    let cause = if entry.need(cores, load.now_ms).is_whole_host() {
      IdleCause::WholeHost
    } else if started.iter().any(|holder| holder.need(cores, load.now_ms).is_whole_host()) {
      IdleCause::WholeHolder
    } else if started.len() < holders.len() && admits(entry, &started, load, cores, waited_out(entry)) {
      IdleCause::Slots
    } else {
      IdleCause::Cores
    };
    (Some(cause), Some(position))
  };
  let (cause, backfill_head) = if head_fits {
    (None, None)
  } else if holders.iter().any(|holder| holder.exclusive) {
    (Some(IdleCause::Exclusive), None)
  } else if first.exclusive {
    match drain {
      Some(HeadDrain::Draining { .. }) => (Some(IdleCause::Drain), None),
      Some(HeadDrain::Ended { .. }) => passer.map_or((Some(IdleCause::Quiet), None), of),
      Some(HeadDrain::Pending) | None => (Some(IdleCause::Pending), Some(0)),
    }
  } else {
    of(0)
  };
  Judged {
    head_fits,
    passer,
    cause,
    backfill_head,
  }
}

/// The runs of the queue of `sight` behind its place `head` that the backfill would start now (design 4.1), one after
/// the other: each next one beside the real holders, the dry starts of `dry` whose walls have not ended and the ones of
/// this minute, with `S` of the head played again with them, until none fits or the head has its [`PASS_BUDGET`] of
/// passes, short runs and dry starts together. Every run behind the head is scanned, past a run that cannot backfill
/// and the runs that the dry backfill started before. Each start goes into `dry`
fn dry_backfills(sight: &Sight, head: usize, load: &Load<'_>, walls: &Walls<'_>, dry: &mut DryState) -> Vec<DryBackfill> {
  let queue = &sight.turn.queue;
  let head_run = queue[head].ticket.run.clone();
  let mut passes = sight.marks.of(&head_run).passes + dry.passes.get(&head_run).copied().unwrap_or(0);
  let mut played = sight.clone();
  played.turn.holders.extend(dry.holders(sight.now_ms).cloned());
  let mut started = Vec::new();
  while passes < PASS_BUDGET {
    let start = head_start(&played, head, &|entry| dry.wall_of(entry).or_else(|| (walls.wall)(entry)));
    if start.unknown_ahead > 0 {
      break;
    }
    let found = queue[head + 1..].iter().find_map(|candidate| {
      let need = candidate.need(sight.cores, sight.now_ms);
      if dry.started(&candidate.ticket.run) || candidate.exclusive || need.is_whole_host() || !admits(candidate, &played.turn.holders, load, sight.cores, false) {
        return None;
      }
      let wall_ms = (walls.passed)(candidate).filter(|&wall| wall <= millis(BACKFILL_WALL))?;
      (wall_ms.saturating_mul(5) / 4 <= start.start_in_ms).then(|| (candidate, need.cores(sight.cores), wall_ms))
    });
    let Some((candidate, cores, wall_ms)) = found else { break };
    let entry = TurnEntry {
      since_ms: sight.now_ms,
      ..candidate.clone()
    };
    played.turn.holders.push(entry.clone());
    dry.starts.push(DryStart { entry, wall_ms });
    *dry.passes.entry(head_run.clone()).or_default() += 1;
    passes += 1;
    started.push(DryBackfill {
      run: candidate.ticket.run.clone(),
      command: short(&candidate.command),
      cores,
      wall_ms,
      head: head_run.clone(),
      head_start_ms: start.start_in_ms,
    });
  }
  started
}

/// What the series gives the summary of `brd-host stats`
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeriesTotals {
  /// The minutes of the window
  pub minutes: u32,
  /// The minutes in which heavy runs waited
  pub waiting: u32,
  /// The mean idle cores of the host over the minutes in which heavy runs waited, by the mean busy cores of each minute;
  /// `None` without a sample
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub idle: Option<Cores>,
  /// The minutes in which heavy runs waited, by why none started, the most idle core minutes first
  #[serde(default)]
  pub causes: Vec<CauseTotal>,
  /// The minutes in which the dry backfill started a run, and the runs it started
  #[serde(default)]
  pub backfill_minutes: u32,
  #[serde(default)]
  pub backfill_runs: u32,
}

/// The waiting minutes of one cause
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CauseTotal {
  /// `None`: the run that could start fitted, and started at its next look
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cause: Option<IdleCause>,
  pub minutes: u32,
  /// The mean idle cores of its minutes with a sample
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub idle: Option<Cores>,
}

/// The idle cores of a minute: its cores less its mean busy cores; `None` without a sample
fn idle(minute: &TurnMinute) -> Option<Cores> {
  minute.busy.map(|busy| Cores(Cores::whole(minute.cores).0.saturating_sub(busy.0)))
}

/// The mean of `values`; `None` for none
fn mean(values: &[Cores]) -> Option<Cores> {
  let count = u64::try_from(values.len()).ok().filter(|&count| count > 0)?;
  let sum: u64 = values.iter().map(|cores| u64::from(cores.0)).sum();
  Some(Cores(u32::try_from((sum + count / 2) / count).unwrap_or(u32::MAX)))
}

/// The summary of `minutes`
pub fn aggregate(minutes: &[TurnMinute]) -> SeriesTotals {
  let waiting: Vec<&TurnMinute> = minutes.iter().filter(|minute| minute.queue > 0).collect();
  let mut by: BTreeMap<Option<IdleCause>, (u32, Vec<Cores>)> = BTreeMap::new();
  for minute in &waiting {
    let (count, idles) = by.entry(minute.cause).or_default();
    *count += 1;
    idles.extend(idle(minute));
  }
  let mut causes: Vec<(u64, CauseTotal)> = by
    .into_iter()
    .map(|(cause, (minutes, idles))| {
      let core_minutes: u64 = idles.iter().map(|cores| u64::from(cores.0)).sum();
      (
        core_minutes,
        CauseTotal {
          cause,
          minutes,
          idle: mean(&idles),
        },
      )
    })
    .collect();
  causes.sort_by_key(|(core_minutes, _)| std::cmp::Reverse(*core_minutes));
  let idles: Vec<Cores> = waiting.iter().filter_map(|minute| idle(minute)).collect();
  let runs: BTreeSet<&str> = minutes.iter().flat_map(|minute| minute.backfills.iter().map(|backfill| backfill.run.as_str())).collect();
  SeriesTotals {
    minutes: count(minutes.len()),
    waiting: count(waiting.len()),
    idle: mean(&idles),
    causes: causes.into_iter().map(|(_, total)| total).collect(),
    backfill_minutes: count(minutes.iter().filter(|minute| !minute.backfills.is_empty()).count()),
    backfill_runs: count(runs.len()),
  }
}

impl SeriesTotals {
  /// The line of the summary over the last `days` days, at most [`KEEP_DAYS`] as the series keeps them: the idle cores
  /// while heavy runs waited, by cause, and the dry backfill
  pub fn line(&self, days: u32) -> String {
    let causes: Vec<String> = self
      .causes
      .iter()
      .map(|total| {
        let word = total.cause.map_or("the run that can start fitted", IdleCause::word);
        match total.idle {
          Some(idle) => format!("{word} {} min at {idle}", total.minutes),
          None => format!("{word} {} min", total.minutes),
        }
      })
      .collect();
    let idle = self.idle.map_or_else(String::new, |idle| format!(", {idle} cores idle on average"));
    format!(
      "  heavy turn by the minute in the last {} day(s): {} min recorded, heavy runs waited in {}{idle}: {}; a backfill would have started {} run(s) in {} min (a dry run)",
      days.min(KEEP_DAYS),
      self.minutes,
      self.waiting,
      if causes.is_empty() { "no cause".to_owned() } else { causes.join(", ") },
      self.backfill_runs,
      self.backfill_minutes
    )
  }
}

fn count(n: usize) -> u32 {
  u32::try_from(n).unwrap_or(u32::MAX)
}

fn millis(duration: Duration) -> u64 {
  u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// The file name of the repository at `path`
fn repo_name(path: &str) -> String {
  Path::new(path).file_name().map_or_else(|| path.to_owned(), |name| name.to_string_lossy().into_owned())
}

/// A command of an entry of the turn, cut to 80 characters
fn short(command: &str) -> String {
  match command.char_indices().nth(COMMAND_CHARS) {
    Some((cut, _)) => format!("{}…", &command[..cut]),
    None => command.to_owned(),
  }
}

#[cfg(test)]
#[path = "host_series_test.rs"]
mod host_series_test;
