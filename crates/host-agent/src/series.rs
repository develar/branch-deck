//! The series of the heavy turn of the host by the minute (`lease_pool::host_series`; openspec/specs/cli/host/spec.md,
//! "stats: serve keeps a series of the heavy turn by the minute"; docs/design/host-runs.md, "The series of the heavy
//! turn"): `~/.cache/brd-host/history/turn-<YYYY-MM-DD>.jsonl`, one line a minute while a heavy run holds cores or
//! waits, in the file of its UTC day. A thread of its own writes it ([`start`]): the sampler of the host hands it each
//! sample through a bounded queue without a wait ([`SeriesFeed`]), so a slow volume, a failure or a panic of the series
//! never delays `host.json`, which the admissions read. A day stops at `host_series::DAY_BYTES`, and the hourly cleanup
//! removes the days past `host_series::KEEP_DAYS` ([`expire`]). `Request::Stats` reads the days of its window ([`read`])

use crate::files::{append_private, private_dir};
use crate::home::Home;
use crate::log::log;
use crate::run::Runs;
use lease_pool::host_history::utc_date;
use lease_pool::host_series::{DAY_BYTES, DryState, KEEP_DAYS, MinuteBusy, TurnMinute, Walls};
use lease_pool::host_turn::TurnEntry;
use std::fs;
use std::io::{self, Write as _};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::{Duration, Instant};
use sync_utils::cores::Cores;
use sync_utils::fs_retry;
use sync_utils::span::in_current_span;

const DAY_MS: u64 = 24 * 60 * 60 * 1000;
/// The file name of a day of the series, before the date
const PREFIX: &str = "turn-";
/// The file name extension of a day
const LINES: &str = ".jsonl";
/// The samples that wait for the writer of the series at most, about 2 min of them: a writer that lags past them loses
/// the newest samples, and the sampler never waits for it
const QUEUED_SAMPLES: usize = 64;

/// The end of the sampler of the host that hands its samples to the writer of the series
#[derive(Debug)]
pub(crate) struct SeriesFeed(SyncSender<Cores>);

impl SeriesFeed {
  /// Hands `busy` to the writer without a wait: a full queue or a writer that is gone drops it
  pub(crate) fn push(&self, busy: Cores) {
    let _ = self.0.try_send(busy);
  }
}

/// A feed and the queue of its writer, of `capacity` samples
pub(crate) fn channel(capacity: usize) -> (SeriesFeed, Receiver<Cores>) {
  let (feed, samples) = sync_channel(capacity);
  (SeriesFeed(feed), samples)
}

/// Starts the writer of the series of `home` on a thread of its own, with `cores` the cores of the host when the table
/// counts none, and answers its feed. A writer that cannot start says so in the log, and its feed drops every sample
pub(crate) fn start(home: &Home, cores: u32) -> SeriesFeed {
  let (feed, samples) = channel(QUEUED_SAMPLES);
  let home = home.clone();
  let started = std::thread::Builder::new().name("brd-host-series".to_owned()).spawn(in_current_span(move || {
    let mut dry = DryState::default();
    write_minutes(
      &samples,
      crate::tuning::series_every(),
      &mut |busy| note(&home, cores, crate::run::epoch_ms(std::time::SystemTime::now()), busy, &mut dry),
      &mut |line| log(line),
    );
  }));
  if let Err(error) = started {
    log(&format!("cannot start the writer of the series of the heavy turn: {error}"));
  }
  feed
}

/// Why a minute of the series was not written
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Failure {
  /// The heavy turn could not be read
  Read(String),
  /// The file of the day could not be written
  Write(String),
  /// The file of the day holds `DAY_BYTES`
  Full(String),
  /// The writer panicked in the minute
  Panic,
}

impl Failure {
  /// The state of the failure: a new state is said in the log, the same one again is not
  fn state(&self) -> String {
    match self {
      Self::Read(_) => "read".to_owned(),
      Self::Write(_) => "write".to_owned(),
      Self::Full(day) => format!("full {day}"),
      Self::Panic => "panic".to_owned(),
    }
  }

  fn line(&self) -> String {
    match self {
      Self::Read(error) => format!("cannot read the heavy turn for its series: {error}; said once until a minute is written"),
      Self::Write(error) => format!("cannot keep a minute of the heavy turn in its series: {error}; said once until a minute is written"),
      Self::Full(day) => format!("the series of the heavy turn of {day} holds {DAY_BYTES} bytes: no more minutes of the day"),
      Self::Panic => "the writer of the series of the heavy turn panicked in a minute; it goes on with the next".to_owned(),
    }
  }
}

/// Takes the samples of `samples` until the sampler is gone, and once `every` passed since the last minute hands their
/// mean and most to `minute`, a panic of it caught. A failure is said through `say` once per state, again only after a
/// minute was written or the state changed
pub(crate) fn write_minutes(samples: &Receiver<Cores>, every: Duration, minute: &mut dyn FnMut(Option<MinuteBusy>) -> Result<(), Failure>, say: &mut dyn FnMut(&str)) {
  let mut sampled = Sampled::new();
  let mut failing: Option<String> = None;
  while let Ok(busy) = samples.recv() {
    sampled.push(busy);
    if sampled.since.elapsed() < every {
      continue;
    }
    let busy = sampled.take();
    let outcome = catch_unwind(AssertUnwindSafe(|| minute(busy))).unwrap_or(Err(Failure::Panic));
    match outcome {
      Ok(()) => failing = None,
      Err(failure) => {
        let state = failure.state();
        if failing.as_deref() != Some(state.as_str()) {
          say(&failure.line());
          failing = Some(state);
        }
      }
    }
  }
}

/// The samples of the busy cores of the host since the last minute of the series
#[derive(Debug)]
pub(crate) struct Sampled {
  since: Instant,
  sum: u64,
  count: u64,
  most: Cores,
}

impl Sampled {
  pub(crate) fn new() -> Self {
    Self {
      since: Instant::now(),
      sum: 0,
      count: 0,
      most: Cores(0),
    }
  }

  /// Adds a sample of the busy cores of the host
  pub(crate) fn push(&mut self, busy: Cores) {
    self.sum += u64::from(busy.0);
    self.count += 1;
    self.most = self.most.max(busy);
  }

  /// The mean and the most of the samples since the last minute, and a new minute begins; `None` without a sample
  fn take(&mut self) -> Option<MinuteBusy> {
    let busy = (self.count > 0).then(|| MinuteBusy {
      mean: Cores(u32::try_from((self.sum + self.count / 2) / self.count).unwrap_or(u32::MAX)),
      most: self.most,
    });
    self.since = Instant::now();
    self.sum = 0;
    self.count = 0;
    self.most = Cores(0);
    busy
  }
}

/// Writes the minute of the heavy turn at `now_ms` with `busy` the samples of the host over it, `cores` the cores of
/// the host when the table counts none, and `dry` the starts of the dry backfill. A turn without a heavy run writes
/// nothing; a full day reads no turn
fn note(home: &Home, cores: u32, now_ms: u64, busy: Option<MinuteBusy>, dry: &mut DryState) -> Result<(), Failure> {
  let history = home.history_dir();
  if is_full(&history, now_ms) {
    return Err(Failure::Full(day_of(now_ms)));
  }
  let dir = home.heavy_dir();
  let runs = Runs::new(home.runs_dir());
  let wall = |entry: &TurnEntry| crate::estimate::entry_wall(&runs, &dir, entry);
  let passed = |entry: &TurnEntry| crate::estimate::entry_passed_wall(&runs, &dir, entry);
  let walls = Walls { wall: &wall, passed: &passed };
  let minute = lease_pool::host_series::minute(&dir, cores, now_ms, crate::tuning::drain_bar(), busy, &walls, dry).map_err(|error| Failure::Read(format!("{error:#}")))?;
  let Some(minute) = minute else { return Ok(()) };
  match append(&history, &minute) {
    Ok(true) => Ok(()),
    Ok(false) => Err(Failure::Full(day_of(minute.at_ms))),
    Err(error) => Err(Failure::Write(error.to_string())),
  }
}

/// The file of the day of `ms` in `dir`
fn day_file(dir: &Path, ms: u64) -> PathBuf {
  dir.join(format!("{PREFIX}{}{LINES}", day_of(ms)))
}

/// Whether the file of the day of `ms` in `dir` holds `DAY_BYTES`
fn is_full(dir: &Path, ms: u64) -> bool {
  fs::metadata(day_file(dir, ms)).is_ok_and(|meta| meta.len() >= DAY_BYTES)
}

/// Appends `minute` to the file of its day in `dir` in one write; `false` when the day holds `DAY_BYTES` already
pub(crate) fn append(dir: &Path, minute: &TurnMinute) -> io::Result<bool> {
  private_dir(dir)?;
  if is_full(dir, minute.at_ms) {
    return Ok(false);
  }
  let path = day_file(dir, minute.at_ms);
  let mut bytes = serde_json::to_vec(minute).map_err(io::Error::other)?;
  bytes.push(b'\n');
  append_private(&path)?.write_all(&bytes)?;
  Ok(true)
}

/// Removes the days of the series in `dir` more than `KEEP_DAYS` days before `now_ms`. Answers a log line for each
pub(crate) fn expire(dir: &Path, now_ms: u64) -> Vec<String> {
  let oldest = day_of(now_ms.saturating_sub(u64::from(KEEP_DAYS) * DAY_MS));
  let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
  let mut lines = Vec::new();
  for entry in entries.filter_map(Result::ok) {
    let name = entry.file_name().to_string_lossy().into_owned();
    let Some(day) = name.strip_prefix(PREFIX).and_then(|name| name.strip_suffix(LINES)) else {
      continue;
    };
    if is_day(day) && day < oldest.as_str() {
      match fs_retry::remove_file(entry.path()) {
        Ok(()) => lines.push(format!("removed the series of the heavy turn {name}, past {KEEP_DAYS} days")),
        Err(error) => lines.push(format!("cannot remove the series of the heavy turn {name}: {error}")),
      }
    }
  }
  lines
}

/// The minutes of the days of `since_ms..=until_ms` in `dir` whose time is in it, and the count of the lines that do
/// not parse
pub(crate) fn read(dir: &Path, since_ms: u64, until_ms: u64) -> (Vec<TurnMinute>, usize) {
  let (mut minutes, mut unread) = (Vec::new(), 0);
  for day in since_ms / DAY_MS..=until_ms / DAY_MS {
    let Ok(text) = fs::read_to_string(dir.join(format!("{PREFIX}{}{LINES}", utc_date(day)))) else {
      continue;
    };
    for text in text.lines().filter(|text| !text.is_empty()) {
      match serde_json::from_str::<TurnMinute>(text) {
        Ok(minute) if (since_ms..=until_ms).contains(&minute.at_ms) => minutes.push(minute),
        Ok(_) => {}
        Err(_) => unread += 1,
      }
    }
  }
  (minutes, unread)
}

/// The UTC date of `ms`, `2026-11-25`
fn day_of(ms: u64) -> String {
  utc_date(ms / DAY_MS)
}

/// Whether `text` names a day, `2026-11-25`
fn is_day(text: &str) -> bool {
  let bytes = text.as_bytes();
  bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-' && bytes.iter().enumerate().all(|(at, byte)| at == 4 || at == 7 || byte.is_ascii_digit())
}

#[cfg(test)]
#[path = "series_test.rs"]
mod series_test;
