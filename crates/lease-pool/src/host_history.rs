//! The history of the ended runs of a host and its summary (openspec/specs/cli/host/spec.md, "stats";
//! docs/design/host-runs.md, "The history of the runs"). The agent of `brd-host` writes one [`HistoryLine`] per run into
//! `~/.cache/brd-host/history/<YYYY-MM>.jsonl` as the run directory goes, and one [`BasePackLine`] per pack of a base
//! into `bases-<YYYY-MM>.jsonl`, for 90 days ([`KEEP_DAYS`]). The supervisor of a heavy run writes its place and its
//! wait in the queue of the heavy turn ([`QueueRecord`], `queue.json` of the run, beside the tables that a supervisor of
//! an older build rewrites without the fields it does not know).
//!
//! [`aggregate`] is the summary that `brd-host stats` prints, a pure function of the lines, so the host sums them and
//! only the summary travels: the waits against the work by phase, the outcomes by exit and by refusal, the waits of the
//! runs that never ran their command, the cores, the 30 commands that waited longest ([`TOP_COMMANDS`]) and the rest in
//! one entry, and the bytes by day

use crate::host_turn::HeavyTurn;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use sync_utils::cores::Cores;

/// The schema of a line and of the summary
pub const SCHEMA: u32 = 1;
/// How long the agent keeps the lines, and the most days of a summary
pub const KEEP_DAYS: u32 = 90;
/// The commands of a summary by their wait; the rest is one entry
pub const TOP_COMMANDS: usize = 30;
/// The phases that wait for the host: for the heavy turn, for a measurement, for the git of the host, for a slot and
/// for the pack. Every other phase works
pub const WAIT_PHASES: [&str; 5] = ["heavy", "exclusive", "git", "slot", "pack_wait"];
/// The exit of a run that a cancel interrupted (128 plus SIGINT): no failure of its command
const INTERRUPTED: i32 = 130;
const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// One ended run of the host, as the agent keeps it after the run directory went
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryLine {
  #[serde(default)]
  pub schema: u32,
  pub run: String,
  /// The main repository on the host, `~/` expanded
  pub repo: String,
  pub affinity: String,
  pub commit: String,
  /// The command as one line of at most 200 characters, as the heavy turn names it ([`command_line`]): never the whole
  /// text, which can hold a token
  pub command: String,
  /// The key of the measures of the command (`host_link::heavy::measure_key`), with `repo` the key of its measures
  pub command_key: String,
  /// The class of its measures (`brd-host run --measure-as`), whose key `command_key` is; `None` without one and from
  /// an older build
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub measure_as: Option<String>,
  #[serde(default, skip_serializing_if = "is_false")]
  pub heavy: bool,
  #[serde(default, skip_serializing_if = "is_false")]
  pub exclusive: bool,
  pub started_ms: u64,
  /// `None` for a run that lost its supervisor: the agent saw the loss later
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub ended_ms: Option<u64>,
  /// The exit code of the command, or 255 for a refusal
  pub exit: i32,
  /// The code of the refusal that ended the run, by name
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub refused: Option<String>,
  #[serde(default, skip_serializing_if = "is_false")]
  pub lost: bool,
  #[serde(default)]
  pub phases: Vec<LinePhase>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cores: Option<LineCores>,
  #[serde(default)]
  pub bytes: LineBytes,
  /// The place and the wait of a heavy run in the queue of the heavy turn; `None` for a run that is not heavy and from
  /// a supervisor of a build before it
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub queue: Option<QueueRecord>,
}

impl HistoryLine {
  /// When the run counts in a window: its end, else its start
  pub fn at_ms(&self) -> u64 {
    self.ended_ms.unwrap_or(self.started_ms)
  }

  /// The milliseconds of its phases of `name`
  fn phase_ms(&self, name: &str) -> u64 {
    self.phases.iter().filter(|phase| phase.name == name).map(|phase| phase.ms).sum()
  }

  fn wait_ms(&self) -> u64 {
    self.phases.iter().filter(|phase| is_wait(&phase.name)).map(|phase| phase.ms).sum()
  }

  fn work_ms(&self) -> u64 {
    self.phases.iter().filter(|phase| !is_wait(&phase.name)).map(|phase| phase.ms).sum()
  }

  fn ran_command(&self) -> bool {
    self.phases.iter().any(|phase| phase.name == "command")
  }

  /// Its command ran and exited with a code other than 0 and that of a cancel
  fn failed(&self) -> bool {
    self.ran_command() && self.refused.is_none() && self.exit != 0 && self.exit != INTERRUPTED
  }
}

fn is_wait(phase: &str) -> bool {
  WAIT_PHASES.contains(&phase)
}

/// One step of a run and its duration
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinePhase {
  pub name: String,
  pub ms: u64,
}

/// What the command of a run used: its mean, its peak and its level of work, its CPU time and its wall time
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineCores {
  pub mean: Cores,
  pub peak: Cores,
  /// The level of work (`host_cores::work_level`); `None` in a line of a build before it
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub level: Option<Cores>,
  pub cpu_ms: u64,
  pub wall_ms: u64,
}

/// The bytes of a run: its pack as the agent took it, its output (the log of the run with what a forget cut), and its
/// files as a fetch sent them, every fetch of it
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineBytes {
  #[serde(default)]
  pub pack: u64,
  #[serde(default)]
  pub output: u64,
  #[serde(default)]
  pub fetch: u64,
}

/// One pack of a base, which belongs to no run
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasePackLine {
  pub at_ms: u64,
  /// The name of the base: the affinity of its client
  pub base: String,
  pub bytes: u64,
}

/// What a heavy run needs of the host, as the heavy turn counts it (`host_turn::Need`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedKind {
  /// The cores of the measure of its command
  Cores,
  /// The whole host: no measure of its command yet
  Unmeasured,
  /// The whole host: its command measured the cores of the host or more
  Measured,
  /// A quiet host, for a measurement
  Exclusive,
}

impl NeedKind {
  /// The need of a run that `expected` cores of its measure lead to expect, `exclusive` or not, on a host of `cores`
  pub fn of(expected: Option<Cores>, exclusive: bool, cores: u32) -> Self {
    match expected {
      _ if exclusive => Self::Exclusive,
      None => Self::Unmeasured,
      Some(expected) if expected >= Cores::whole(cores) => Self::Measured,
      Some(_) => Self::Cores,
    }
  }
}

/// `queue.json` of a heavy run: its place in the queue of the heavy turn as it joined, what it needed, and its wait
/// split by what it waited for
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueRecord {
  /// The runs before it in the queue as it joined: 0 at the head, or admitted at once
  pub place: u32,
  /// The heavy runs that held cores of the turn as it joined
  pub holders: u32,
  /// The cores of the host that the heavy runs share
  pub cores: u32,
  /// The cores that the measures of its command led to expect
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub expected: Option<Cores>,
  pub need: NeedKind,
  /// The wait behind the holders of the turn or behind the runs before it in the queue
  pub behind_ms: u64,
  /// The wait at the head of the queue on a free turn, for a quiet host or for room beside the load of the host outside
  /// the heavy runs
  pub load_ms: u64,
  /// A short run of the queue (`host_turn::short_wall`): the wall of the last passed runs of its command that made it
  /// short, by the marks of the queue (`passes.json`); `None` for a run that was not short, and from a build before it
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub short_ms: Option<u64>,
  /// How often short runs went ahead of it in the queue, by the marks of the queue
  #[serde(default, skip_serializing_if = "is_zero")]
  pub passes: u32,
}

/// What a heavy run in the queue waits for at one look
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueSpot {
  /// Holders of the turn, or runs before it in the queue
  Behind,
  /// The load of the host: it is the head of the queue, and no heavy run holds cores
  Load,
}

/// The place of the run `run` in the queue of `table` and what it waits for there; `None` when it is not in the queue
pub fn spot(table: &HeavyTurn, run: &str) -> Option<(usize, QueueSpot)> {
  let position = table.queue.iter().position(|entry| entry.ticket.run == run)?;
  let spot = if position == 0 && table.holders.is_empty() {
    QueueSpot::Load
  } else {
    QueueSpot::Behind
  };
  Some((position, spot))
}

/// The wait of a run in the queue split by its spot: each span between two looks goes to the spot of the first of them,
/// and a span after a look that found the run outside the queue to none
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueueClock {
  pub behind_ms: u64,
  pub load_ms: u64,
  last: Option<(u64, Option<QueueSpot>)>,
}

impl QueueClock {
  /// A look at `at_ms` found the run at `spot`
  pub fn note(&mut self, at_ms: u64, spot: Option<QueueSpot>) {
    self.close(at_ms);
    self.last = Some((at_ms, spot));
  }

  /// The wait ended at `at_ms`: the last span goes to the spot of the last look
  pub fn close(&mut self, at_ms: u64) {
    if let Some((since, Some(spot))) = self.last {
      let span = at_ms.saturating_sub(since);
      match spot {
        QueueSpot::Behind => self.behind_ms += span,
        QueueSpot::Load => self.load_ms += span,
      }
    }
    self.last = self.last.map(|(_, spot)| (at_ms, spot));
  }
}

/// The words of a command as one line of at most 200 characters, as the heavy turn names it
pub fn command_line(command: &[String]) -> String {
  crate::host_turn::one_line(command)
}

/// The median and the 90th percentile of a set of milliseconds, by the nearest rank
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spread {
  pub p50: u64,
  pub p90: u64,
}

impl Spread {
  fn of(mut values: Vec<u64>) -> Option<Self> {
    values.sort_unstable();
    Some(Self {
      p50: nearest_rank(&values, 50)?,
      p90: nearest_rank(&values, 90)?,
    })
  }
}

/// The `percent` percentile of the sorted `values` by the nearest rank
fn nearest_rank<T: Copy>(values: &[T], percent: usize) -> Option<T> {
  let rank = (values.len() * percent).div_ceil(100).max(1);
  values.get(rank - 1).copied()
}

/// The summary of the runs of a host over a window, as `brd-host stats` prints it
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsReport {
  pub schema: u32,
  pub since_ms: u64,
  pub until_ms: u64,
  pub days: u32,
  pub runs: u32,
  pub heavy: u32,
  pub exclusive: u32,
  /// The time of the wait phases, and of the other phases, of every run
  pub wait_ms: u64,
  pub work_ms: u64,
  /// Each phase with its time and its runs: the wait phases first, in the order of [`WAIT_PHASES`], then the others by
  /// their first run
  pub phases: Vec<PhaseTotal>,
  pub queue: QueueTotals,
  /// The runs by their exit and their refusal, the most first
  pub outcomes: Vec<Outcome>,
  /// The runs that ended before their command, and what they waited
  pub without_command: WithoutCommand,
  pub cores: CoresTotals,
  /// The [`TOP_COMMANDS`] commands that waited longest, by repository and key
  pub commands: Vec<CommandStats>,
  /// The commands past them, in one entry
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub other: Option<OtherCommands>,
  pub bytes: BytesTotals,
  /// The bytes of each UTC day with a run or a pack of a base, the oldest first
  pub by_day: Vec<DayBytes>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseTotal {
  pub name: String,
  pub wait: bool,
  pub ms: u64,
  pub runs: u32,
}

/// The waits of the heavy runs that wrote their place in the queue
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueTotals {
  pub runs: u32,
  pub behind_ms: u64,
  pub load_ms: u64,
  /// The places as they joined
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub place: Option<Spread>,
  /// The runs that needed the whole host: without a measure, measured past the host, or exclusive
  pub whole_host: u32,
  /// The short runs, which joined the queue ahead of the long ones
  #[serde(default)]
  pub short: u32,
  /// How often short runs went ahead of a run, over every run
  #[serde(default)]
  pub passes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
  pub exit: i32,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub refused: Option<String>,
  pub runs: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithoutCommand {
  pub runs: u32,
  pub wait_ms: u64,
}

/// The CPU time and the wall time of the commands, and the cores that they kept busy on average over the window
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoresTotals {
  pub cpu_ms: u64,
  pub command_ms: u64,
  pub mean_over_window: Cores,
  pub peak: Cores,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandStats {
  pub repo: String,
  pub command_key: String,
  /// The command line of its last run
  pub command: String,
  /// The class of the measures of its last run (`brd-host run --measure-as`), which the line of the text names instead
  /// of the command, as the runs of a class have commands of their own
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub measure_as: Option<String>,
  pub runs: u32,
  /// Its runs whose command exited with a code other than 0 and that of a cancel
  pub failed: u32,
  pub wait_ms: u64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub wait: Option<Spread>,
  /// The phase `command` of the runs that ran it
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub command_ms: Option<Spread>,
  /// The 80th percentile of the levels of work of its runs, each at least its mean, as the heavy turn expects them
  /// (`host_cores::expected`); the mean of a run without a level
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cores: Option<Cores>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub peak: Option<Cores>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtherCommands {
  pub commands: u32,
  pub runs: u32,
  pub failed: u32,
  pub wait_ms: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BytesTotals {
  pub pack: u64,
  pub output: u64,
  pub fetch: u64,
  /// The packs of the bases
  pub base: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayBytes {
  /// The UTC date, `2026-11-24`
  pub day: String,
  pub runs: u32,
  #[serde(flatten)]
  pub bytes: BytesTotals,
}

/// The summary of the `lines` and the packs of the bases `bases` whose time is in `since_ms..=until_ms`, a window of
/// `days` days
pub fn aggregate(lines: &[HistoryLine], bases: &[BasePackLine], since_ms: u64, until_ms: u64, days: u32) -> StatsReport {
  let window = |at: u64| (since_ms..=until_ms).contains(&at);
  let mut lines: Vec<&HistoryLine> = lines.iter().filter(|line| window(line.at_ms())).collect();
  lines.sort_by_key(|line| (line.started_ms, line.run.clone()));
  let bases: Vec<&BasePackLine> = bases.iter().filter(|base| window(base.at_ms)).collect();
  let mut report = StatsReport {
    schema: SCHEMA,
    since_ms,
    until_ms,
    days,
    runs: count(lines.len()),
    heavy: count(lines.iter().filter(|line| line.heavy).count()),
    exclusive: count(lines.iter().filter(|line| line.exclusive).count()),
    wait_ms: lines.iter().map(|line| line.wait_ms()).sum(),
    work_ms: lines.iter().map(|line| line.work_ms()).sum(),
    phases: phase_totals(&lines),
    queue: queue_totals(&lines),
    outcomes: outcomes(&lines),
    without_command: WithoutCommand {
      runs: count(lines.iter().filter(|line| !line.ran_command()).count()),
      wait_ms: lines.iter().filter(|line| !line.ran_command()).map(|line| line.wait_ms()).sum(),
    },
    cores: cores_totals(&lines, until_ms.saturating_sub(since_ms)),
    ..StatsReport::default()
  };
  (report.commands, report.other) = commands(&lines);
  (report.bytes, report.by_day) = bytes(&lines, &bases);
  report
}

fn count(n: usize) -> u32 {
  u32::try_from(n).unwrap_or(u32::MAX)
}

fn phase_totals(lines: &[&HistoryLine]) -> Vec<PhaseTotal> {
  let mut totals: Vec<PhaseTotal> = WAIT_PHASES
    .iter()
    .map(|name| PhaseTotal {
      name: (*name).to_owned(),
      wait: true,
      ms: 0,
      runs: 0,
    })
    .collect();
  for line in lines {
    for phase in &line.phases {
      let at = totals.iter().position(|total| total.name == phase.name).unwrap_or_else(|| {
        totals.push(PhaseTotal {
          name: phase.name.clone(),
          wait: false,
          ms: 0,
          runs: 0,
        });
        totals.len() - 1
      });
      totals[at].ms += phase.ms;
      totals[at].runs += 1;
    }
  }
  totals.retain(|total| total.runs > 0);
  totals
}

fn queue_totals(lines: &[&HistoryLine]) -> QueueTotals {
  let queues: Vec<&QueueRecord> = lines.iter().filter_map(|line| line.queue.as_ref()).collect();
  QueueTotals {
    runs: count(queues.len()),
    behind_ms: queues.iter().map(|queue| queue.behind_ms).sum(),
    load_ms: queues.iter().map(|queue| queue.load_ms).sum(),
    place: Spread::of(queues.iter().map(|queue| u64::from(queue.place)).collect()),
    whole_host: count(queues.iter().filter(|queue| queue.need != NeedKind::Cores).count()),
    short: count(queues.iter().filter(|queue| queue.short_ms.is_some()).count()),
    passes: queues.iter().map(|queue| queue.passes).sum(),
  }
}

fn outcomes(lines: &[&HistoryLine]) -> Vec<Outcome> {
  let mut by: BTreeMap<(i32, Option<String>), u32> = BTreeMap::new();
  for line in lines {
    *by.entry((line.exit, line.refused.clone())).or_default() += 1;
  }
  let mut outcomes: Vec<Outcome> = by.into_iter().map(|((exit, refused), runs)| Outcome { exit, refused, runs }).collect();
  outcomes.sort_by_key(|outcome| std::cmp::Reverse(outcome.runs));
  outcomes
}

fn cores_totals(lines: &[&HistoryLine], window_ms: u64) -> CoresTotals {
  let cores: Vec<&LineCores> = lines.iter().filter_map(|line| line.cores.as_ref()).collect();
  let cpu_ms: u64 = cores.iter().map(|cores| cores.cpu_ms).sum();
  CoresTotals {
    cpu_ms,
    command_ms: cores.iter().map(|cores| cores.wall_ms).sum(),
    mean_over_window: Cores::of(std::time::Duration::from_millis(cpu_ms), std::time::Duration::from_millis(window_ms)),
    peak: cores.iter().map(|cores| cores.peak).max().unwrap_or_default(),
  }
}

fn commands(lines: &[&HistoryLine]) -> (Vec<CommandStats>, Option<OtherCommands>) {
  let mut by: BTreeMap<(&str, &str), Vec<&HistoryLine>> = BTreeMap::new();
  for line in lines {
    by.entry((line.repo.as_str(), line.command_key.as_str())).or_default().push(line);
  }
  let mut commands: Vec<CommandStats> = by
    .into_iter()
    .map(|((repo, command_key), runs)| {
      let mut levels: Vec<Cores> = runs
        .iter()
        .filter_map(|line| line.cores.map(|cores| cores.level.map_or(cores.mean, |level| level.max(cores.mean))))
        .collect();
      levels.sort_unstable();
      CommandStats {
        repo: repo.to_owned(),
        command_key: command_key.to_owned(),
        command: runs.last().map_or_default(|line| line.command.clone()),
        measure_as: runs.last().and_then(|line| line.measure_as.clone()),
        runs: count(runs.len()),
        failed: count(runs.iter().filter(|line| line.failed()).count()),
        wait_ms: runs.iter().map(|line| line.wait_ms()).sum(),
        wait: Spread::of(runs.iter().map(|line| line.wait_ms()).collect()),
        command_ms: Spread::of(runs.iter().filter(|line| line.ran_command()).map(|line| line.phase_ms("command")).collect()),
        cores: nearest_rank(&levels, 80),
        peak: runs.iter().filter_map(|line| line.cores.map(|cores| cores.peak)).max(),
      }
    })
    .collect();
  // the longest wait first, then by repository and key
  commands.sort_by_key(|command| std::cmp::Reverse(command.wait_ms));
  if commands.len() <= TOP_COMMANDS {
    return (commands, None);
  }
  let past = commands.split_off(TOP_COMMANDS);
  let other = OtherCommands {
    commands: count(past.len()),
    runs: past.iter().map(|command| command.runs).sum(),
    failed: past.iter().map(|command| command.failed).sum(),
    wait_ms: past.iter().map(|command| command.wait_ms).sum(),
  };
  (commands, Some(other))
}

fn bytes(lines: &[&HistoryLine], bases: &[&BasePackLine]) -> (BytesTotals, Vec<DayBytes>) {
  let mut days: BTreeMap<u64, (u32, BytesTotals)> = BTreeMap::new();
  for line in lines {
    let (runs, day) = days.entry(line.at_ms() / DAY_MS).or_default();
    *runs += 1;
    day.pack += line.bytes.pack;
    day.output += line.bytes.output;
    day.fetch += line.bytes.fetch;
  }
  for base in bases {
    days.entry(base.at_ms / DAY_MS).or_default().1.base += base.bytes;
  }
  let mut total = BytesTotals::default();
  let by_day = days
    .into_iter()
    .map(|(day, (runs, bytes))| {
      total.pack += bytes.pack;
      total.output += bytes.output;
      total.fetch += bytes.fetch;
      total.base += bytes.base;
      DayBytes { day: utc_date(day), runs, bytes }
    })
    .collect();
  (total, by_day)
}

/// The UTC date of the day `days` since the Unix epoch, `2026-11-24` (the civil calendar of Howard Hinnant's
/// `civil_from_days`)
pub fn utc_date(days: u64) -> String {
  let z = days + 719_468;
  let era = z / 146_097;
  let day_of_era = z % 146_097;
  let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
  let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
  let month_index = (5 * day_of_year + 2) / 153;
  let day = day_of_year - (153 * month_index + 2) / 5 + 1;
  let month = if month_index < 10 { month_index + 3 } else { month_index - 9 };
  let year = year_of_era + era * 400 + u64::from(month <= 2);
  format!("{year:04}-{month:02}-{day:02}")
}

impl StatsReport {
  /// The lines of the summary of the host `alias` for a person
  pub fn lines(&self, alias: &str) -> Vec<String> {
    let mut lines = vec![format!(
      "{alias}: {} run(s) in the last {} day(s), {} heavy, {} exclusive: waited {}, worked {}",
      self.runs,
      self.days,
      self.heavy,
      self.exclusive,
      hours(self.wait_ms),
      hours(self.work_ms)
    )];
    if self.runs == 0 {
      return lines;
    }
    for (wait, label) in [(true, "waits"), (false, "work")] {
      let phases: Vec<String> = self
        .phases
        .iter()
        .filter(|phase| phase.wait == wait)
        .map(|phase| format!("{} {} in {}", phase.name, hours(phase.ms), phase.runs))
        .collect();
      if !phases.is_empty() {
        lines.push(format!("  {label}: {}", phases.join(", ")));
      }
    }
    let queue = &self.queue;
    if queue.runs > 0 {
      let place = queue.place.map_or_else(String::new, |place| format!(", joined at place {} (p90 {})", place.p50, place.p90));
      lines.push(format!(
        "  heavy queue of {} run(s): {} behind holders or runs before them, {} at the head of a free turn for the load of the host{place}, {} for the whole host, {} short, passed {} time(s)",
        queue.runs,
        hours(queue.behind_ms),
        hours(queue.load_ms),
        queue.whole_host,
        queue.short,
        queue.passes
      ));
    }
    let outcomes: Vec<String> = self
      .outcomes
      .iter()
      .map(|outcome| match &outcome.refused {
        Some(code) => format!("{code} {}", outcome.runs),
        None => format!("exit {} {}", outcome.exit, outcome.runs),
      })
      .collect();
    lines.push(format!("  outcomes: {}", outcomes.join(", ")));
    if self.without_command.runs > 0 {
      lines.push(format!(
        "  {} run(s) ended before their command, after a wait of {}",
        self.without_command.runs,
        hours(self.without_command.wait_ms)
      ));
    }
    let cores = &self.cores;
    lines.push(format!(
      "  cores: {} of CPU in {} of commands, {} on average over the window, peak {}",
      hours(cores.cpu_ms),
      hours(cores.command_ms),
      cores.mean_over_window,
      cores.peak
    ));
    lines.push("  commands by their wait:".to_owned());
    for command in &self.commands {
      let mut line = format!("    {} run(s)", command.runs);
      if command.failed > 0 {
        let _ = write!(line, ", {} failed", command.failed);
      }
      if let Some(wait) = command.wait {
        let _ = write!(line, ", wait p50 {} p90 {}", hours(wait.p50), hours(wait.p90));
      }
      if let Some(ran) = command.command_ms {
        let _ = write!(line, ", command p50 {} p90 {}", hours(ran.p50), hours(ran.p90));
      }
      if let Some(cores) = command.cores {
        let _ = write!(line, ", {cores} cores");
      }
      match &command.measure_as {
        Some(class) => {
          let _ = write!(line, ": --measure-as {class}");
        }
        None => {
          let _ = write!(line, ": {}", command.command);
        }
      }
      lines.push(line);
    }
    if let Some(other) = &self.other {
      lines.push(format!(
        "    {} other command(s): {} run(s), {} failed, waited {}",
        other.commands,
        other.runs,
        other.failed,
        hours(other.wait_ms)
      ));
    }
    let bytes = &self.bytes;
    lines.push(format!(
      "  bytes: packs {}, output {}, fetched {}, packs of bases {}",
      size(bytes.pack),
      size(bytes.output),
      size(bytes.fetch),
      size(bytes.base)
    ));
    lines
  }
}

/// A duration for a summary: `45 s` up to two minutes, `17 min` up to two hours, then `53 h 09 min`
fn hours(ms: u64) -> String {
  match ms / 1000 {
    seconds @ 0..120 => format!("{seconds} s"),
    seconds @ 120..7200 => format!("{} min", seconds / 60),
    seconds => format!("{} h {:02} min", seconds / 3600, seconds % 3600 / 60),
  }
}

/// Bytes for a summary: `512 B`, `3.4 MiB`
fn size(bytes: u64) -> String {
  const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
  if bytes < 1024 {
    return format!("{bytes} B");
  }
  let mut tenths = bytes * 10 / 1024;
  let mut unit = 0;
  while tenths >= 10 * 1024 && unit + 1 < UNITS.len() {
    tenths /= 1024;
    unit += 1;
  }
  format!("{}.{} {}", tenths / 10, tenths % 10, UNITS[unit])
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature of `skip_serializing_if`")]
const fn is_false(value: &bool) -> bool {
  !*value
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature of `skip_serializing_if`")]
const fn is_zero(value: &u32) -> bool {
  *value == 0
}

#[cfg(test)]
#[path = "host_history_test.rs"]
mod host_history_test;
