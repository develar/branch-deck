//! The history of the ended runs of the host (openspec/specs/cli/host/spec.md, "stats"; docs/design/host-runs.md, "The
//! history of the runs"): `~/.cache/brd-host/history/<YYYY-MM>.jsonl`, one line per run
//! (`lease_pool::host_history::HistoryLine`), in the month of its end by UTC, and `bases-<YYYY-MM>.jsonl`, one line per
//! pack of a base, which belongs to no run. Only `serve` writes them, at one point each: a run leaves its line as its
//! directory goes (`run::Runs::remove_expired`, `remove_forgotten_past`), when the bytes of its fetches are final, and a
//! pack of a base as it lands. The hourly cleanup removes the months past 90 days ([`History::expire`]).
//!
//! `Request::Stats` sums the lines of its window and the ended runs that the agent still keeps, which have no line yet,
//! on the host ([`stats`]), so only the summary travels. The supervisor of a heavy run writes its place and its wait in
//! the queue of the heavy turn into `queue.json` of the run ([`QueueWatch`])

use crate::files::{append_private, private_dir};
use crate::home::Home;
use crate::log::log;
use crate::run::{RunDir, Runs, epoch_ms};
use host_link::heavy::measure_key;
use host_link::proto::Reply;
use host_link::refusal::{Code, Refusal};
use lease_pool::host_history::{
  BasePackLine, HistoryLine, KEEP_DAYS, LineBytes, LineCores, LinePhase, NeedKind, QueueClock, QueueRecord, SCHEMA, StatsReport, aggregate, command_line, spot, utc_date,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};
use sync_utils::cores::Cores;
use sync_utils::fs_retry;

const DAY_MS: u64 = 24 * 60 * 60 * 1000;
/// The file name of the lines of the packs of the bases, before the month
const BASES: &str = "bases-";
/// The file name extension of a month
const LINES: &str = ".jsonl";

/// The history of the agent of `home`
#[derive(Clone, Debug)]
pub(crate) struct History {
  home: Home,
}

impl History {
  pub(crate) fn new(home: &Home) -> Self {
    Self { home: home.clone() }
  }

  fn dir(&self) -> PathBuf {
    self.home.history_dir()
  }

  /// The line of the ended run `run`, from its directory; `None` while it goes on
  pub(crate) fn line_of(&self, run: &RunDir) -> io::Result<Option<HistoryLine>> {
    let spec = run.spec()?;
    let info = run.info()?;
    let Some(exit) = info.exit else { return Ok(None) };
    Ok(Some(HistoryLine {
      schema: SCHEMA,
      repo: self.home.expand(&spec.repo).display().to_string(),
      command: command_line(&spec.command),
      command_key: measure_key(&spec.command, &spec.fetch),
      heavy: spec.heavy,
      exclusive: spec.exclusive,
      run: spec.run,
      affinity: spec.affinity,
      commit: spec.commit,
      started_ms: info.started_ms,
      ended_ms: info.ended_ms,
      exit,
      refused: info.refused.map(|refused| refused.code),
      lost: run.is_lost(),
      phases: info.phases.into_iter().map(|phase| LinePhase { name: phase.name, ms: phase.ms }).collect(),
      cores: run.cores().map(|cores| LineCores {
        mean: cores.mean,
        peak: cores.peak,
        cpu_ms: cores.cpu_ms,
        wall_ms: cores.wall_ms,
      }),
      bytes: LineBytes {
        pack: run.pack_bytes().bytes,
        output: run.output_bytes(),
        fetch: run.fetched_bytes().bytes,
      },
      queue: run.queue(),
    }))
  }

  /// Appends `line` to the month of its end; a failure says so in the log
  pub(crate) fn append(&self, line: &HistoryLine) {
    let month = month_of(line.at_ms());
    if let Err(error) = self.append_to(&format!("{month}{LINES}"), line) {
      log(&format!("cannot keep the run {} in the history: {error}", line.run));
    }
  }

  /// Appends the pack of `bytes` of the base `base` that landed at `at_ms`; a failure says so in the log
  pub(crate) fn note_base_pack(&self, base: &str, bytes: u64, at_ms: u64) {
    let line = BasePackLine {
      at_ms,
      base: base.to_owned(),
      bytes,
    };
    if let Err(error) = self.append_to(&format!("{BASES}{}{LINES}", month_of(at_ms)), &line) {
      log(&format!("cannot keep the pack of the base {base} in the history: {error}"));
    }
  }

  /// One line of JSON in one write, so the lines of two threads never mix
  fn append_to(&self, name: &str, line: &impl Serialize) -> io::Result<()> {
    let dir = self.dir();
    private_dir(&dir)?;
    let mut bytes = serde_json::to_vec(line).map_err(io::Error::other)?;
    bytes.push(b'\n');
    append_private(&dir.join(name))?.write_all(&bytes)
  }

  /// Removes the months whose every line is more than 90 days before `now_ms`. Answers a log line for each
  pub(crate) fn expire(&self, now_ms: u64) -> Vec<String> {
    let oldest = month_of(now_ms.saturating_sub(u64::from(KEEP_DAYS) * DAY_MS));
    let Ok(entries) = fs::read_dir(self.dir()) else { return Vec::new() };
    let mut lines = Vec::new();
    for entry in entries.filter_map(Result::ok) {
      let name = entry.file_name().to_string_lossy().into_owned();
      let Some(month) = name.strip_suffix(LINES).map(|month| month.strip_prefix(BASES).unwrap_or(month)) else {
        continue;
      };
      if is_month(month) && month < oldest.as_str() {
        match fs_retry::remove_file(entry.path()) {
          Ok(()) => lines.push(format!("removed the history {name}, past {KEEP_DAYS} days")),
          Err(error) => lines.push(format!("cannot remove the history {name}: {error}")),
        }
      }
    }
    lines
  }

  /// The lines of the files `<prefix><month>.jsonl` of the months of `since_ms..=until_ms` whose time is in it, and the
  /// count of the lines that do not parse
  fn read<T: DeserializeOwned>(&self, prefix: &str, since_ms: u64, until_ms: u64, at_ms: impl Fn(&T) -> u64) -> (Vec<T>, usize) {
    let (mut lines, mut unread) = (Vec::new(), 0);
    for month in months(since_ms, until_ms) {
      let Ok(text) = fs::read_to_string(self.dir().join(format!("{prefix}{month}{LINES}"))) else {
        continue;
      };
      for text in text.lines().filter(|text| !text.is_empty()) {
        match serde_json::from_str::<T>(text) {
          Ok(line) if (since_ms..=until_ms).contains(&at_ms(&line)) => lines.push(line),
          Ok(_) => {}
          Err(_) => unread += 1,
        }
      }
    }
    (lines, unread)
  }
}

/// The month of `ms` by UTC, `2026-11`
fn month_of(ms: u64) -> String {
  let mut date = utc_date(ms / DAY_MS);
  date.truncate(7);
  date
}

/// The months of `since_ms..=until_ms`, the first first
fn months(since_ms: u64, until_ms: u64) -> BTreeSet<String> {
  (since_ms / DAY_MS..=until_ms / DAY_MS).map(|day| month_of(day * DAY_MS)).collect()
}

/// Whether `text` names a month, `2026-11`
fn is_month(text: &str) -> bool {
  let bytes = text.as_bytes();
  bytes.len() == 7 && bytes[4] == b'-' && bytes.iter().enumerate().all(|(at, byte)| at == 4 || byte.is_ascii_digit())
}

/// The summary of the runs of the host that ended in the last `days` days before `now_ms` (at most 90): the lines of
/// the history and the ended runs that the agent keeps, which have none yet, with the packs of the bases
pub(crate) fn stats(home: &Home, days: u32, now_ms: u64) -> Result<Reply, Refusal> {
  if days == 0 || days > KEEP_DAYS {
    return Err(Refusal::new(Code::BadRequest, format!("a summary covers 1 to {KEEP_DAYS} days, not {days}")));
  }
  let since_ms = now_ms.saturating_sub(u64::from(days) * DAY_MS);
  let history = History::new(home);
  let (mut lines, unread) = history.read::<HistoryLine>("", since_ms, now_ms, HistoryLine::at_ms);
  let (bases, unread_bases) = history.read::<BasePackLine>(BASES, since_ms, now_ms, |line| line.at_ms);
  if unread + unread_bases > 0 {
    log(&format!("the summary of the runs skips {} line(s) of the history that do not parse", unread + unread_bases));
  }
  // the kept runs leave their line as their directory goes, so none of them is in the history yet
  for run in Runs::new(home.runs_dir()).list() {
    match history.line_of(&run) {
      Ok(Some(line)) => lines.push(line),
      Ok(None) => {}
      Err(error) => log(&format!("the summary of the runs skips {}: {error}", run.path().display())),
    }
  }
  let report = aggregate(&lines, &bases, since_ms, now_ms, days);
  serde_json::to_value(report)
    .map(|report| Reply::Stats { report })
    .map_err(|error| Refusal::new(Code::AgentError, error.to_string()))
}

/// The lines of the summary of `Reply::Stats` of the host `alias`, for `brd-host stats`
pub fn report_lines(report: &serde_json::Value, alias: &str) -> serde_json::Result<Vec<String>> {
  serde_json::from_value::<StatsReport>(report.clone()).map(|report| report.lines(alias))
}

/// How often the watch of a heavy run in the queue reads the heavy turn at most
const QUEUE_LOOK: Duration = Duration::from_secs(1);

/// The watch of a heavy run in the queue of the heavy turn, for its `queue.json`: its first look before the run joins
/// counts the holders and the runs that wait, and each later look, at most once a second, finds the run in the queue
/// and what it waits for there (`lease_pool::host_history::spot`). It reads the table without its lock, as
/// `brd-host runs` does, so it holds no wait back
pub(crate) struct QueueWatch {
  dir: PathBuf,
  run: String,
  started: Instant,
  looked: Option<Instant>,
  clock: QueueClock,
  /// The runs that waited as the run joined, until a look finds its place
  waiting: u32,
  place: Option<u32>,
  /// The mark of the run in the queue (`passes.json`) at the last look that found one: whether it was short, and how
  /// often short runs passed it
  mark: lease_pool::host_turn::RunMark,
  holders: u32,
  cores: u32,
  expected: Option<Cores>,
  need: NeedKind,
}

impl QueueWatch {
  /// The watch of the heavy run `of`, before it joins the queue
  pub(crate) fn start(home: &Home, of: &crate::turn::TurnRun<'_>) -> Self {
    let dir = home.heavy_dir();
    let expected = lease_pool::host_cores::expected(&dir, of.repo, &measure_key(of.command, of.fetch)).ok().flatten();
    let table = lease_pool::host_turn::read_live(&dir).ok();
    Self {
      run: of.run.to_owned(),
      started: Instant::now(),
      looked: None,
      clock: QueueClock::default(),
      waiting: table.as_ref().map_or(0, |table| count(table.queue.len())),
      place: None,
      mark: lease_pool::host_turn::RunMark::default(),
      holders: table.as_ref().map_or(0, |table| count(table.holders.len())),
      cores: of.cores,
      expected,
      need: NeedKind::of(expected, of.exclusive, of.cores),
      dir,
    }
  }

  /// Looks at the queue, unless it did less than a second ago
  pub(crate) fn look(&mut self) {
    let now = Instant::now();
    if self.looked.is_some_and(|looked| now.duration_since(looked) < QUEUE_LOOK) {
      return;
    }
    self.looked = Some(now);
    let Ok(table) = lease_pool::host_turn::read_live(&self.dir) else { return };
    let found = spot(&table, &self.run);
    if let Some((position, _)) = found {
      // the place that the rule of the queue gave the run as it joined: a short run joins ahead of the long ones
      self.place.get_or_insert_with(|| count(position));
      // the mark goes with the run as it leaves the queue, so the last one of the queue stays
      let mark = lease_pool::host_turn::read_marks(&self.dir).of(&self.run);
      if mark != lease_pool::host_turn::RunMark::default() {
        self.mark = mark;
      }
    }
    self.clock.note(self.ms(now), found.map(|(_, spot)| spot));
  }

  /// The wait ended: writes `queue.json` of `run`, and says in the log of the supervisor when it cannot
  pub(crate) fn finish(mut self, run: &RunDir) {
    self.clock.close(self.ms(Instant::now()));
    let record = QueueRecord {
      place: self.place.unwrap_or(self.waiting),
      holders: self.holders,
      cores: self.cores,
      expected: self.expected,
      need: self.need,
      behind_ms: self.clock.behind_ms,
      load_ms: self.clock.load_ms,
      short_ms: self.mark.short_ms,
      passes: self.mark.passes,
    };
    if let Err(error) = run.write_queue(&record) {
      crate::log::supervisor(&format!("cannot write the queue of the run {}: {error}", self.run));
    }
  }

  fn ms(&self, at: Instant) -> u64 {
    u64::try_from(at.duration_since(self.started).as_millis()).unwrap_or(u64::MAX)
  }
}

fn count(n: usize) -> u32 {
  u32::try_from(n).unwrap_or(u32::MAX)
}

/// Now, in Unix milliseconds
pub(crate) fn now_ms() -> u64 {
  epoch_ms(SystemTime::now())
}

#[cfg(test)]
#[path = "history_test.rs"]
mod history_test;
