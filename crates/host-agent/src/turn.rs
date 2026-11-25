//! The heavy turn of the host for the heavy runs (`lease_pool::host_turn`; openspec/specs/cli/host/spec.md, "serve: Heavy
//! runs share the cores of the host by their measure"; docs/design/host-runs.md, "Heavy runs"). The table is
//! `~/.cache/brd-host/heavy/turn.json`, one for every repository of the host, in a directory of its own, so the watch
//! of a wait sees no event of the logs of the runs; `cores.json` beside it holds the measures of the commands
//! (`lease_pool::host_cores`).
//!
//! - A supervisor of a heavy run takes its cores of the turn before its lend ([`take`], with its own pid: they last as
//!   long as the supervisor), expecting what the last runs of its command used, read again at each look (the key
//!   `host_link::heavy::measure_key`, without the paths of the directories that the run fetches). Its wait is the wait
//!   for a host slot ([`slots::wait_in_looks`]): looks of 2 s, so a cancel ends it, one watch of the table for the whole
//!   wait, and the line of the wait once a minute (`lease_pool::host_runs::turn_line`). A look that finds the lock of
//!   the table held for 2 s by another process costs the look, not the run: the run keeps its place and says so.
//! - A run new to the queue supersedes the runs that wait there with its checkout and its command
//!   (`lease_pool::host_turn::join`, the key `host_link::heavy::run_key`) and says so, and names the other runs of its
//!   checkout that wait ([`joined_lines`]); a superseded run learns the later one at its next look, and its supervisor
//!   ends it with `superseded` ([`superseded_message`]). A short run, by the walls of the last passed runs of its
//!   command (`lease_pool::host_cores::passed_wall`, read once before its first ask), joins ahead of the runs it may
//!   pass (`lease_pool::host_turn::place`) and names them.
//! - Each sample of the command of a run, heavy or not, notes its live cores in the table ([`note_use`]), and the end
//!   of its command adds its measure to the measures of the command, and its wall to the walls of the passed runs when
//!   its exit was 0 ([`record`]).
//! - On every way out, after the give-back of its slot, the supervisor of every run leaves the turn ([`leave`]): it
//!   gives its cores back, or its entry leaves the queue or the runs that are not heavy.
//! - `Reply::Runs` carries the table without the gone runs ([`live`]), and `brd-host runs` says it ([`lines`])

use crate::files::{open_private, private_dir};
use crate::home::Home;
use crate::run::epoch_ms;
use crate::slots::{self, Look};
use crate::tuning;
use history_rewrite::repository_lock::RepositoryLocked;
use host_link::heavy::{measure_key, run_key};
use host_link::proto::RunSpec;
use host_link::refusal::{Code, Refusal};
use lease_pool::host_cores::Measure;
use lease_pool::host_runs::{NEVER_MEASURED, holds_text, passed_text, passes_text, run_of_repo, span, span_since, turn_line};
use lease_pool::host_turn::{
  Bar, HeadDrain, Joined, LOAD_BOUND, LightEntry, Need, PASS_BUDGET, RECHECK, RunMark, TurnAnswer, TurnEntry, TurnParams, TurnQueue, TurnRequest, UseNote, join, read_live,
  take_waiting, watch,
};
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};
use sync_utils::cores::Cores;
use sync_utils::process_scan::process_started;

/// The wait for the lock of the table of the heavy turn
const LOCK_TIMEOUT: Duration = Duration::from_secs(60);

/// The wait for the lock of the table at one look of a wait for the turn: a lock that another process holds longer
/// costs the look, not the run, which keeps its place and looks again ([`take`])
const LOOK_LOCK_TIMEOUT: Duration = Duration::from_secs(2);

fn params(dir: &Path, cores: u32) -> TurnParams<'_> {
  TurnParams {
    dir,
    lock_timeout: LOCK_TIMEOUT,
    recheck: RECHECK,
    cores,
    load_bound: LOAD_BOUND,
    drain_bar: tuning::drain_bar(),
  }
}

fn now_ms() -> u64 {
  epoch_ms(SystemTime::now())
}

/// The directory of the heavy turn and of the measures, private as every file of the agent, with the locks of both
/// tables made private before the pool opens them. A directory that cannot be made says so in the log, and the change
/// of a table then names its error
fn heavy_dir(home: &Home) -> std::path::PathBuf {
  let dir = home.heavy_dir();
  let made = private_dir(&dir).and_then(|()| {
    for lock in [lease_pool::host_turn::LOCK_FILE, lease_pool::host_cores::LOCK_FILE, lease_pool::host_cores::WALLS_LOCK_FILE] {
      open_private(&dir.join(lock))?;
    }
    Ok(())
  });
  if let Err(error) = made {
    crate::log::log(&format!("cannot make {} private: {error}", dir.display()));
  }
  dir
}

/// What a run is to the heavy turn: its id, its main repository on the host, the checkout of its client
/// (`RunSpec::affinity`), its command, and the cores of the host that the heavy runs share
#[cfg_attr(test, derive(Default))]
pub(crate) struct TurnRun<'a> {
  pub(crate) run: &'a str,
  pub(crate) repo: &'a str,
  pub(crate) affinity: &'a str,
  pub(crate) command: &'a [String],
  pub(crate) cores: u32,
  /// A measurement that needs a quiet host (`RunSpec::exclusive`)
  pub(crate) exclusive: bool,
  /// The directories that the run fetches (`RunSpec::fetch`): their paths in the command are no part of the key of its
  /// measures (`host_link::heavy::measure_key`)
  pub(crate) fetch: &'a [String],
  /// The class of its measures (`RunSpec::measure_as`): with one, the key of its measures is of the class, not of the
  /// command
  pub(crate) measure_as: Option<&'a str>,
}

impl TurnRun<'_> {
  /// The key of the measures of the run (`host_link::heavy::measure_key`)
  pub(crate) fn measure_key(&self) -> String {
    measure_key(self.command, self.fetch, self.measure_as)
  }
}

/// What an agent tells a run that waits, after the line of its wait: the wait goes on by itself, and a cancel or a run
/// on the client only loses the place
const NOT_A_HANG: &str = "a wait, not a hang: do not cancel it and do not run it on the client; `brd-host runs <host>` shows the heavy turn";

/// Takes the heavy turn of the host for the heavy run `of`, which this process runs, waiting up to `wait` while other
/// heavy runs hold it or wait before it. The first look says what it did to the queue ([`joined_lines`]) through `say`,
/// and a queued first look the line of the wait at once, which each minute of the wait says again. Each look reads the
/// measures of the command again, so a measure that came while the run waited counts. Answers
/// `Some(TurnAnswer::Taken)`, `Some(TurnAnswer::Passed)` for a turn taken past an exclusive run at the head of the queue
/// whose drain ended, `Some(TurnAnswer::Superseded)` once a later run of the checkout and command took its
/// place, the place in the queue once the time is up, or `None` when `stop` ended the wait: the end of the run takes
/// the ticket out of the queue ([`leave`]). A lock of the table that another process holds for more than 2 s costs the
/// look, not the run: the run keeps its place, says so at once and once a minute while it lasts, and looks again
/// ([`LockLine`]). A table that cannot be read or written otherwise refuses with `agent_error`, and so does a lock that
/// kept the run out of the queue for its whole wait
pub(crate) fn take(home: &Home, of: &TurnRun<'_>, wait: Duration, stop: &dyn Fn() -> bool, say: &mut dyn FnMut(&str)) -> Result<Option<TurnAnswer>, Refusal> {
  take_with(home, of, wait, LOOK_LOCK_TIMEOUT, stop, say)
}

/// [`take`] with the wait `lock_timeout` for the lock of the table at each look
fn take_with(home: &Home, of: &TurnRun<'_>, wait: Duration, lock_timeout: Duration, stop: &dyn Fn() -> bool, say: &mut dyn FnMut(&str)) -> Result<Option<TurnAnswer>, Refusal> {
  let pid = std::process::id();
  let command_key = run_key(of.command);
  let cores_key = of.measure_key();
  let dir = heavy_dir(home);
  let unread = Cell::new(false);
  // a measure that cannot be read is none: the run takes the whole host, as before any measure
  let expected = || {
    lease_pool::host_cores::expected(&dir, of.repo, &cores_key).unwrap_or_else(|error| {
      if !unread.replace(true) {
        crate::log::supervisor(&format!("cannot read the cores of the commands: {error:#}"));
      }
      None
    })
  };
  // whether the run is short is decided once, as it joins the queue: a wall that cannot be read is none, and the run
  // goes first come first
  let passed_wall = lease_pool::host_cores::passed_wall(&dir, of.repo, &cores_key).unwrap_or_else(|error| {
    crate::log::supervisor(&format!("cannot read the walls of the passed runs of the commands: {error:#}"));
    None
  });
  let process_started = process_started(pid);
  let request = |expected| TurnRequest {
    run: of.run,
    pid,
    process_started,
    repo: of.repo,
    command: of.command,
    affinity: of.affinity,
    command_key: &command_key,
    expected,
    exclusive: of.exclusive,
    passed_wall,
    measure_as: of.measure_as,
  };
  let params = TurnParams {
    lock_timeout,
    ..params(&dir, of.cores)
  };
  // the watch starts before the first look, so no change between a look and the wait after it is lost
  let watched = watch(&params);
  let refused = |error: anyhow::Error| Refusal::new(Code::AgentError, format!("cannot take the heavy turn of the host: {error:#}"));
  let started = Instant::now();
  let run = of.run;
  let line = |queue: &TurnQueue, waited: Duration| format!("{}; {NOT_A_HANG}", turn_line(run, queue, of.measure_as, waited, wait.saturating_sub(waited), now_ms()));
  let mut lock_line = LockLine::default();
  // the first ask, until the run has its place in the queue
  let joined = loop {
    match join(&params, &request(expected())) {
      Ok(joined) => break joined,
      Err(error) => {
        let error = lock_line.held(error, false, say).map_err(refused)?;
        if stop() {
          return Ok(None);
        }
        if started.elapsed() >= wait {
          return Err(refused(error));
        }
      }
    }
  };
  lock_line.ended();
  for said in joined_lines(&joined, now_ms()) {
    say(&said);
  }
  let mut last = match joined.answer {
    TurnAnswer::Queued(queue) => {
      say(&line(&queue, Duration::ZERO));
      queue
    }
    answer => return Ok(Some(answer)),
  };
  // the look and the line of the wait both say, one at a time
  let say = RefCell::new(say);
  let mut look = |chunk: Duration| match take_waiting(&params, &request(expected()), chunk, &watched) {
    Ok(TurnAnswer::Queued(queue)) => {
      lock_line.ended();
      last.clone_from(&queue);
      Ok(Look::Queued(queue))
    }
    Ok(answer) => Ok(Look::Got(answer)),
    Err(error) => {
      lock_line.held(error, true, &mut |text| (say.borrow_mut())(text)).map_err(refused)?;
      Ok(Look::Queued(last.clone()))
    }
  };
  let left = wait.saturating_sub(started.elapsed());
  let waited = slots::wait_in_looks(left, stop, &mut look, &|queue: &TurnQueue, _| line(queue, started.elapsed()), &mut |text| {
    (say.borrow_mut())(text);
  })?;
  Ok(waited.map(|look| match look {
    Look::Got(answer) => answer,
    Look::Queued(queue) => TurnAnswer::Queued(queue),
  }))
}

/// The line of a wait for the turn whose look found the lock of the table held by another process: at once, then once
/// a minute while the lock stays held (`tuning::wait_progress`)
#[derive(Default)]
struct LockLine {
  /// When the looks began to find the lock held, and when the line was said last
  held: Option<(Instant, Option<Instant>)>,
}

impl LockLine {
  /// Says the line for `error` when it is a lock that another process held past the wait of the look, and answers the
  /// error back to retry; answers any other error as the refusal of the run. `placed`: the run has its place in the
  /// queue, else it asks for one again
  fn held(&mut self, error: anyhow::Error, placed: bool, say: &mut dyn FnMut(&str)) -> Result<anyhow::Error, anyhow::Error> {
    let Some(locked) = error.downcast_ref::<RepositoryLocked>() else {
      return Err(error);
    };
    let (since, said) = self.held.get_or_insert_with(|| (Instant::now(), None));
    if said.is_none_or(|at| at.elapsed() >= tuning::wait_progress()) {
      let holder = locked.holder.as_ref().map_or_else(|| "another process of the host".to_owned(), ToString::to_string);
      let next = if placed {
        "this run keeps its place in the queue and looks again"
      } else {
        "this run asks again for its place in the queue"
      };
      say(&format!(
        "the heavy turn of the host is locked by {holder} for {} s ({}); {next}",
        since.elapsed().as_secs(),
        locked.path.display()
      ));
      *said = Some(Instant::now());
    }
    Ok(error)
  }

  /// A look took the lock again
  const fn ended(&mut self) {
    self.held = None;
  }
}

/// Waits up to `wait` while an exclusive run holds the turn of the host, or drains it for at most
/// `tuning::drain_bar` (5 min), for the run `run` that is not heavy, before its lend: its line at once, at once again
/// when the drain becomes a hold, and once a minute (`lease_pool::host_runs::measurement_line`, `drain_line`). Answers
/// `Some(true)` once nothing bars it, `Some(false)` when the time is up, and `None` when `stop` ended the wait
pub(crate) fn wait_for_measurement(home: &Home, run: &str, wait: Duration, stop: &dyn Fn() -> bool, say: &mut dyn FnMut(&str)) -> Option<bool> {
  let dir = heavy_dir(home);
  let started = Instant::now();
  let mut said: Option<(Instant, bool)> = None;
  loop {
    let now = now_ms();
    let Some(bar) = lease_pool::host_turn::bar(&dir, now, tuning::drain_bar()) else {
      return Some(true);
    };
    if stop() {
      return None;
    }
    let waited = started.elapsed();
    if waited >= wait {
      return Some(false);
    }
    let holds = matches!(bar, Bar::Holds(_));
    if said.is_none_or(|(at, held)| held != holds || at.elapsed() >= tuning::wait_progress()) {
      say(&match &bar {
        Bar::Holds(holder) => lease_pool::host_runs::measurement_line(run, holder, waited, now),
        Bar::Drains { entry, until_ms, .. } => lease_pool::host_runs::drain_line(run, entry, waited, *until_ms, now),
      });
      said = Some((Instant::now(), holds));
    }
    std::thread::sleep(MEASUREMENT_LOOK.min(wait.saturating_sub(waited)));
  }
}

/// How often a run that waits for a measurement looks at the turn: the end of the holder makes no event it would see
const MEASUREMENT_LOOK: Duration = Duration::from_millis(500);

/// The lines of what the first ask of a run did to the queue at `now_ms`: each run of the same checkout and command that
/// it superseded, each run of the checkout that waits with another command, and the runs that a short run went ahead of
pub(crate) fn joined_lines(joined: &Joined, now_ms: u64) -> Vec<String> {
  let replaced = joined.replaced.iter().map(|entry| {
    format!(
      "replaces run {}, which waited {} for the heavy turn with the same checkout and command: it ends with `superseded` and runs nothing, and this run takes its place in the queue",
      entry.ticket.run,
      span_since(entry.since_ms, now_ms)
    )
  });
  let beside = joined.beside.iter().map(|entry| {
    format!(
      "run {} of this checkout also waits for the heavy turn, {}: {}. Both run, one after the other; if this run replaces it, cancel it: `brd-host cancel <host> {}`",
      entry.ticket.run,
      span_since(entry.since_ms, now_ms),
      entry.command,
      entry.ticket.run
    )
  });
  let ahead = joined.short.filter(|_| !joined.passed.is_empty()).map(|wall| {
    let passed: Vec<String> = joined
      .passed
      .iter()
      .map(|(entry, passes)| format!("run {} ({})", run_of_repo(entry), passes_text(*passes)))
      .collect();
    format!(
      "a short run, ~{} by its last passed runs: goes ahead of {} run(s) that wait for the heavy turn, each passed at most {PASS_BUDGET} times: {}",
      span(wall),
      passed.len(),
      passed.join(", ")
    )
  });
  replaced.chain(beside).chain(ahead).collect()
}

/// The message of the refusal `superseded` of the run `run`, which the later run `by` superseded: what happened, that
/// nothing ran, and the follow of the later run. The client puts its alias for `<host>`
pub(crate) fn superseded_message(run: &str, by: &str) -> String {
  format!(
    "run {run} was superseded by run {by} of the same checkout and command before it started: nothing ran; do not run it again, here or on the client; follow {by}: `brd-host follow <host> {by}`"
  )
}

/// Notes `live`, the cores of the last sample of the command of the run `of`, in the heavy turn: what a holder holds
/// once it is past its expectation, or the use of a run that is not heavy, which the admissions count
pub(crate) fn note_use(home: &Home, of: &TurnRun<'_>, live: Cores) -> Result<(), Refusal> {
  let pid = std::process::id();
  let note = UseNote {
    run: of.run,
    pid,
    process_started: process_started(pid),
    repo: of.repo,
    command: of.command,
    live,
  };
  lease_pool::host_turn::note_use(&params(&heavy_dir(home), of.cores), &note)
    .map_err(|error| Refusal::new(Code::AgentError, format!("cannot note the cores of the run in the heavy turn: {error:#}")))
}

/// Adds the measure of `file`, the mean, the peak and the level of work of the command of the run `of` and its wall,
/// to the measures of its command, and the wall to the walls of its passed runs when it `passed`
/// (`lease_pool::host_cores::record`): a command shorter than `tuning::min_wall` adds none
pub(crate) fn record(home: &Home, of: &TurnRun<'_>, file: &crate::cores::CoresFile, passed: bool) -> Result<(), Refusal> {
  let measure = Measure {
    run: of.run.to_owned(),
    mean: file.mean,
    peak: file.peak,
    level: file.level,
    wall_ms: file.wall_ms,
    at_ms: now_ms(),
  };
  lease_pool::host_cores::record(&heavy_dir(home), LOCK_TIMEOUT, tuning::min_wall(), of.repo, &of.measure_key(), measure, passed)
    .map_err(|error| Refusal::new(Code::AgentError, format!("cannot keep the cores of the command: {error:#}")))
}

/// Samples the busy cores of the whole host every `every` into `host.json` of the heavy turn, for the admissions
/// (`lease_pool::host_turn::note_host`): the runs miss what no run counts. Each sample goes to the writer of the series
/// of the heavy turn too, on a thread of its own (`crate::series::start`), without a wait. A host that tells no CPU ticks
/// notes nothing, and a note that fails says so in the log once
pub(crate) fn sample_host(home: &Home, every: Duration) {
  let cores = std::thread::available_parallelism().map_or(1, |cores| u32::try_from(cores.get()).unwrap_or(u32::MAX));
  let Some(mut before) = sync_utils::host_cpu::host_ticks() else { return };
  let mut said = false;
  // the samples of the last 30 s: an exclusive run reads the most of them
  let mut window = lease_pool::host_turn::HostWindow::default();
  let feed = crate::series::start(home, cores);
  loop {
    std::thread::sleep(every);
    let Some(after) = sync_utils::host_cpu::host_ticks() else { continue };
    let Some(busy) = sync_utils::host_cpu::busy_cores(before, after, cores) else {
      before = after;
      continue;
    };
    if let Err(error) = note_sample(&heavy_dir(home), &mut window, now_ms(), busy, &feed)
      && !said
    {
      crate::log::log(&format!("cannot note the busy cores of the host for the heavy turn: {error:#}"));
      said = true;
    }
    before = after;
  }
}

/// Notes the sample `busy` at `at_ms` into `window` and `host.json` of `dir`, then hands it to the writer of the series
/// through `feed`, which never waits
pub(crate) fn note_sample(dir: &Path, window: &mut lease_pool::host_turn::HostWindow, at_ms: u64, busy: Cores, feed: &crate::series::SeriesFeed) -> anyhow::Result<()> {
  window.push(at_ms, busy);
  let noted = lease_pool::host_turn::note_host_window(dir, window);
  feed.push(busy);
  noted
}

/// The line of a run that took the heavy turn after `waited`, with `busy` of the `cores` of the host by the sample that
/// its admission read, `past` the exclusive run that it started past: "took the heavy turn of the host after 1200 s,
/// past the exclusive run 1788-12 of idea-1 at the head of the queue, whose drain ended without a quiet host, the host
/// busy with 18.0 of its 20 cores"; `None` for a turn taken at once, as nothing waited
pub(crate) fn took_line(waited: Duration, busy: Option<Cores>, cores: u32, past: Option<&TurnEntry>) -> Option<String> {
  if waited < Duration::from_secs(1) && past.is_none() {
    return None;
  }
  let past = past.map_or_else(String::new, |exclusive| {
    format!(
      ", past the exclusive run {} at the head of the queue, whose drain ended without a quiet host",
      run_of_repo(exclusive)
    )
  });
  let busy = busy.map_or_else(String::new, |busy| format!(", the host busy with {busy} of its {cores} cores"));
  Some(format!("took the heavy turn of the host after {} s{past}{busy}", waited.as_secs()))
}

/// The line of a wait for the turn of the run `spec` that ended in the queue after `waited`, for the refusal
/// `heavy_timeout`
pub(crate) fn timeout_line(spec: &RunSpec, queue: &TurnQueue, waited: Duration) -> String {
  turn_line(&spec.run, queue, spec.measure_as.as_deref(), waited, Duration::ZERO, now_ms())
}

/// The run `run` leaves the heavy turn of the host: it gives the turn back, or its ticket leaves the queue. Answers
/// whether it held the turn
pub(crate) fn leave(home: &Home, run: &str) -> Result<bool, Refusal> {
  // a leave counts no cores
  lease_pool::host_turn::leave(&params(&heavy_dir(home), 0), run).map_err(|error| Refusal::new(Code::AgentError, format!("cannot leave the heavy turn of the host: {error:#}")))
}

/// The heavy turn of the host without the runs whose process is gone, as JSON for `Reply::Runs`, with its first holder
/// as `holder` too, which a client of the build before reads, the drain of an exclusive head of the queue as
/// `head_drain` (`lease_pool::host_turn::head_drain`), and the marks of the runs of the queue as `marks`: the short runs
/// and how often short runs passed each (`lease_pool::host_turn::read_marks`). A table that cannot be read is none, and
/// the log says why
pub(crate) fn live(home: &Home) -> Result<serde_json::Value, Refusal> {
  let dir = heavy_dir(home);
  let table = read_live(&dir).map_err(|error| Refusal::new(Code::AgentError, format!("cannot read the heavy turn of the host: {error:#}")))?;
  let holder = table.holders.first().cloned();
  let head_drain = lease_pool::host_turn::head_drain(&dir, &table.queue, now_ms(), tuning::drain_bar());
  let mut marks = lease_pool::host_turn::read_marks(&dir).runs;
  marks.retain(|run, _| table.queue.iter().any(|entry| entry.ticket.run == *run));
  let mut value = serde_json::to_value(table).map_err(|error| Refusal::new(Code::AgentError, format!("cannot write the heavy turn of the host: {error}")))?;
  if let Some(object) = value.as_object_mut() {
    if let Some(holder) = holder {
      object.insert("holder".to_owned(), serde_json::to_value(holder).unwrap_or_default());
    }
    if let Some(head_drain) = head_drain {
      object.insert("head_drain".to_owned(), serde_json::to_value(head_drain).unwrap_or_default());
    }
    if !marks.is_empty() {
      object.insert("marks".to_owned(), serde_json::to_value(marks).unwrap_or_default());
    }
  }
  Ok(value)
}

/// The heavy turn of a reply of an agent of this build, or of the build before (schema 1), which names one `holder`
#[derive(Deserialize)]
struct TurnView {
  #[serde(default)]
  schema: u32,
  #[serde(default)]
  cores: u32,
  #[serde(default)]
  holders: Vec<TurnEntry>,
  #[serde(default)]
  holder: Option<TurnEntry>,
  #[serde(default)]
  queue: Vec<TurnEntry>,
  #[serde(default)]
  light: Vec<LightEntry>,
  /// The drain of an exclusive head of the queue; none from an agent of a build before the pass
  #[serde(default)]
  head_drain: Option<HeadDrain>,
  /// The short runs of the queue and how often short runs passed each, by the id of the run; none from an agent of a
  /// build before the short runs
  #[serde(default)]
  marks: BTreeMap<String, RunMark>,
}

/// The lines of the heavy turn of a reply of the agent (`Reply::Runs`) now, for `brd-host runs` ([`lines_at`])
pub fn lines(table: &serde_json::Value) -> Result<Vec<String>, Refusal> {
  lines_at(table, now_ms())
}

/// The lines of the heavy turn of a reply of the agent at `now_ms`: the cores held of the cores of the host, each holder
/// with what it holds, why, for how long and the exclusive run it started past, each run that waits with what it waits
/// for, a short run with its wall, a run that short runs passed with how often, an exclusive head with its drain, and
/// each run that is not heavy with its live cores. A reply of the build
/// before (schema 1) has one holder and no cores, and its lines say who holds the turn and who waits. A table of another
/// shape refuses with `link`, because the agent sent it
fn lines_at(table: &serde_json::Value, now_ms: u64) -> Result<Vec<String>, Refusal> {
  let table: TurnView = serde_json::from_value(table.clone()).map_err(|error| Refusal::new(Code::Link, format!("the agent sent a heavy turn of another shape: {error}")))?;
  if table.schema < 2 {
    return Ok(lines_of_one_holder(&table, now_ms));
  }
  let cores = table.cores;
  let held: Cores = table.holders.iter().map(|holder| holder.need(cores, now_ms).cores(cores)).sum();
  let mut lines = vec![if table.holders.is_empty() && cores == 0 {
    "heavy turn: free".to_owned()
  } else if cores == 0 {
    // only the holder of the build before, ahead of the first run of this build, which counts the cores
    format!("heavy turn: {} run(s) hold the whole host; no run of this build counted its cores yet", table.holders.len())
  } else if table.holders.is_empty() {
    format!("heavy turn: free, {cores} cores")
  } else {
    format!("heavy turn: {held} of {cores} cores held")
  }];
  for holder in &table.holders {
    let (holds, why) = holds_text(holder.need(cores, now_ms));
    lines.push(format!(
      "  run {} {holds} for {} ({why}{}): {}",
      run_of_repo(holder),
      span_since(holder.since_ms, now_ms),
      passed_text(holder),
      holder.command
    ));
  }
  for (position, waiting) in table.queue.iter().enumerate() {
    let mark = table.marks.get(&waiting.ticket.run).copied().unwrap_or_default();
    let need = match waiting.need(cores, now_ms) {
      // an entry of the queue has no window of its use, and a measure decides
      Need::Cores { cores, .. } | Need::Recent { cores, .. } => match mark.short_ms {
        Some(wall) => format!("{cores} (measured, short: ~{})", span(Duration::from_millis(wall))),
        None => format!("{cores} (measured)"),
      },
      Need::Unmeasured => match &waiting.measure_as {
        Some(class) => format!("the whole host (no measure of the class {class} yet)"),
        None => format!("the whole host (no measure; {NEVER_MEASURED})"),
      },
      Need::Measured(measured) => format!("the whole host (measured {measured})"),
      Need::Exclusive => match table.head_drain.filter(|_| position == 0) {
        None => "a quiet host (exclusive)".to_owned(),
        Some(HeadDrain::Pending) => "a quiet host (exclusive; it drains the host once no heavy run holds cores, and no run starts past it before then)".to_owned(),
        Some(HeadDrain::Draining { until_ms }) => format!(
          "a quiet host (exclusive; it drains the host for {} more, and no run starts past it before then)",
          span_since(now_ms, until_ms)
        ),
        Some(HeadDrain::Ended { ended_ms }) => format!(
          "a quiet host (exclusive; its drain ended {} ago without one, and the heavy runs behind it start past it while they fit)",
          span_since(ended_ms, now_ms)
        ),
      },
    };
    let passed = match mark.passes {
      0 => String::new(),
      1 => format!(", passed by 1 short run (at most {PASS_BUDGET})"),
      passes => format!(", passed by {passes} short runs (at most {PASS_BUDGET})"),
    };
    lines.push(format!(
      "  run {} waits {} for {need}{passed}: {}",
      run_of_repo(waiting),
      span_since(mark.joined_ms.unwrap_or(waiting.since_ms), now_ms),
      waiting.command
    ));
  }
  for light in &table.light {
    let repo = Path::new(&light.repo)
      .file_name()
      .map_or_else(|| light.repo.clone(), |name| name.to_string_lossy().into_owned());
    lines.push(format!("  run {} of {repo}, not heavy, uses {}: {}", light.ticket.run, light.live, light.command));
  }
  Ok(lines)
}

/// The lines of a heavy turn of the build before: who holds it and for how long, then each run that waits
fn lines_of_one_holder(table: &TurnView, now_ms: u64) -> Vec<String> {
  let holder = table.holder.as_ref().or_else(|| table.holders.first());
  let mut lines = vec![holder.map_or_else(
    || "heavy turn: free".to_owned(),
    |holder| {
      format!(
        "heavy turn: run {} holds it {}: {}",
        run_of_repo(holder),
        span_since(holder.since_ms, now_ms),
        holder.command
      )
    },
  )];
  lines.extend(
    table
      .queue
      .iter()
      .map(|waiting| format!("  run {} waits {}: {}", run_of_repo(waiting), span_since(waiting.since_ms, now_ms), waiting.command)),
  );
  lines
}

#[cfg(test)]
#[path = "turn_test.rs"]
mod turn_test;
