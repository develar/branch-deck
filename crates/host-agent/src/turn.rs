//! The heavy turn of the host for the heavy runs (`lease_pool::host_turn`; openspec/specs/cli/host/spec.md, "serve: Heavy
//! runs share the cores of the host by their measure"; docs/design/host-runs.md, "Heavy runs"). The table is
//! `~/.cache/brd-host/heavy/turn.json`, one for every repository of the host, in a directory of its own, so the watch
//! of a wait sees no event of the logs of the runs; `cores.json` beside it holds the measures of the commands
//! (`lease_pool::host_cores`).
//!
//! - A supervisor of a heavy run takes its cores of the turn before its lend ([`take`], with its own pid: they last as
//!   long as the supervisor), expecting what the last runs of its command used. Its wait is the wait for a host slot ([`slots::wait_in_looks`]): looks of 2 s, so a cancel ends
//!   it, one watch of the table for the whole wait, and the line of the wait once a minute
//!   (`lease_pool::host_runs::turn_line`).
//! - A run new to the queue supersedes the runs that wait there with its checkout and its command
//!   (`lease_pool::host_turn::join`, the key `host_link::heavy::run_key`) and says so, and names the other runs of its
//!   checkout that wait ([`joined_lines`]); a superseded run learns the later one at its next look, and its supervisor
//!   ends it with `superseded` ([`superseded_message`]).
//! - Each sample of the command of a run, heavy or not, notes its live cores in the table ([`note_use`]), and the end
//!   of its command adds its measure to the measures of the command ([`record`]).
//! - On every way out, after the give-back of its slot, the supervisor of every run leaves the turn ([`leave`]): it
//!   gives its cores back, or its entry leaves the queue or the runs that are not heavy.
//! - `Reply::Runs` carries the table without the gone runs ([`live`]), and `brd-host runs` says it ([`lines`])

use crate::files::{open_private, private_dir};
use crate::home::Home;
use crate::run::epoch_ms;
use crate::slots::{self, Look};
use crate::tuning;
use host_link::heavy::run_key;
use host_link::refusal::{Code, Refusal};
use lease_pool::host_cores::Measure;
use lease_pool::host_runs::{holds_text, run_of_repo, span_since, turn_line};
use lease_pool::host_turn::{Joined, LightEntry, Need, RECHECK, TurnAnswer, TurnEntry, TurnParams, TurnQueue, TurnRequest, UseNote, join, read_live, take_waiting, watch};
use serde::Deserialize;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};
use sync_utils::cores::Cores;
use sync_utils::process_scan::process_started;

/// The wait for the lock of the table of the heavy turn
const LOCK_TIMEOUT: Duration = Duration::from_secs(60);

const fn params(dir: &Path, cores: u32) -> TurnParams<'_> {
  TurnParams {
    dir,
    lock_timeout: LOCK_TIMEOUT,
    recheck: RECHECK,
    cores,
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
    for lock in [lease_pool::host_turn::LOCK_FILE, lease_pool::host_cores::LOCK_FILE] {
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
pub(crate) struct TurnRun<'a> {
  pub(crate) run: &'a str,
  pub(crate) repo: &'a str,
  pub(crate) affinity: &'a str,
  pub(crate) command: &'a [String],
  pub(crate) cores: u32,
}

/// What an agent tells a run that waits, after the line of its wait: the wait goes on by itself, and a cancel or a run
/// on the client only loses the place
const NOT_A_HANG: &str = "a wait, not a hang: do not cancel it and do not run it on the client; `brd-host runs <host>` shows the heavy turn";

/// Takes the heavy turn of the host for the heavy run `of`, which this process runs, waiting up to `wait` while other
/// heavy runs hold it or wait before it. The first look says what it did to the queue ([`joined_lines`]) through `say`,
/// and a queued first look the line of the wait at once, which each minute of the wait says again. Answers
/// `Some(TurnAnswer::Taken)`, `Some(TurnAnswer::Superseded)` once a later run of the checkout and command took its
/// place, the place in the queue once the time is up, or `None` when `stop` ended the wait: the end of the run takes
/// the ticket out of the queue ([`leave`]). A table that cannot be read or written refuses with `agent_error`
pub(crate) fn take(home: &Home, of: &TurnRun<'_>, wait: Duration, stop: &dyn Fn() -> bool, say: &mut dyn FnMut(&str)) -> Result<Option<TurnAnswer>, Refusal> {
  let pid = std::process::id();
  let command_key = run_key(of.command);
  let dir = heavy_dir(home);
  // a measure that cannot be read is none: the run takes the whole host, as before any measure
  let expected = lease_pool::host_cores::expected(&dir, of.repo, &command_key).unwrap_or_else(|error| {
    crate::log::supervisor(&format!("cannot read the cores of the commands: {error:#}"));
    None
  });
  let request = TurnRequest {
    run: of.run,
    pid,
    process_started: process_started(pid),
    repo: of.repo,
    command: of.command,
    affinity: of.affinity,
    command_key: &command_key,
    expected,
  };
  let params = params(&dir, of.cores);
  // the watch starts before the first look, so no change between a look and the wait after it is lost
  let watched = watch(&params);
  let refused = |error: anyhow::Error| Refusal::new(Code::AgentError, format!("cannot take the heavy turn of the host: {error:#}"));
  let started = Instant::now();
  let run = of.run;
  let line = |queue: &TurnQueue, waited: Duration| format!("{}; {NOT_A_HANG}", turn_line(run, queue, waited, wait.saturating_sub(waited), now_ms()));
  let joined = join(&params, &request).map_err(refused)?;
  for said in joined_lines(&joined, now_ms()) {
    say(&said);
  }
  match joined.answer {
    TurnAnswer::Queued(queue) => say(&line(&queue, Duration::ZERO)),
    answer => return Ok(Some(answer)),
  }
  let mut look = |chunk: Duration| match take_waiting(&params, &request, chunk, &watched).map_err(refused)? {
    TurnAnswer::Queued(queue) => Ok(Look::Queued(queue)),
    answer => Ok(Look::Got(answer)),
  };
  let left = wait.saturating_sub(started.elapsed());
  let waited = slots::wait_in_looks(left, stop, &mut look, &|queue: &TurnQueue, _| line(queue, started.elapsed()), say)?;
  Ok(waited.map(|look| match look {
    Look::Got(answer) => answer,
    Look::Queued(queue) => TurnAnswer::Queued(queue),
  }))
}

/// The lines of what the first ask of a run did to the queue at `now_ms`: each run of the same checkout and command that
/// it superseded, and each run of the checkout that waits with another command
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
  replaced.chain(beside).collect()
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

/// Adds the measure `mean` and `peak` of the command of the run `of`, which ran `wall`, to the measures of its command
/// (`lease_pool::host_cores::record`): a command shorter than `tuning::min_wall` adds none
pub(crate) fn record(home: &Home, of: &TurnRun<'_>, mean: Cores, peak: Cores, wall: Duration) -> Result<(), Refusal> {
  let measure = Measure {
    run: of.run.to_owned(),
    mean,
    peak,
    wall_ms: u64::try_from(wall.as_millis()).unwrap_or(u64::MAX),
    at_ms: now_ms(),
  };
  lease_pool::host_cores::record(&heavy_dir(home), LOCK_TIMEOUT, tuning::min_wall(), of.repo, &run_key(of.command), measure)
    .map_err(|error| Refusal::new(Code::AgentError, format!("cannot keep the cores of the command: {error:#}")))
}

/// The line of a wait for the turn that ended in the queue after `waited`, for the refusal `heavy_timeout`
pub(crate) fn timeout_line(run: &str, queue: &TurnQueue, waited: Duration) -> String {
  turn_line(run, queue, waited, Duration::ZERO, now_ms())
}

/// The run `run` leaves the heavy turn of the host: it gives the turn back, or its ticket leaves the queue. Answers
/// whether it held the turn
pub(crate) fn leave(home: &Home, run: &str) -> Result<bool, Refusal> {
  // a leave counts no cores
  lease_pool::host_turn::leave(&params(&heavy_dir(home), 0), run).map_err(|error| Refusal::new(Code::AgentError, format!("cannot leave the heavy turn of the host: {error:#}")))
}

/// The heavy turn of the host without the runs whose process is gone, as JSON for `Reply::Runs`, with its first holder
/// as `holder` too, which a client of the build before reads. A table that cannot be read is none, and the log says why
pub(crate) fn live(home: &Home) -> Result<serde_json::Value, Refusal> {
  let table = read_live(&heavy_dir(home)).map_err(|error| Refusal::new(Code::AgentError, format!("cannot read the heavy turn of the host: {error:#}")))?;
  let holder = table.holders.first().cloned();
  let mut value = serde_json::to_value(table).map_err(|error| Refusal::new(Code::AgentError, format!("cannot write the heavy turn of the host: {error}")))?;
  if let (Some(object), Some(holder)) = (value.as_object_mut(), holder) {
    object.insert("holder".to_owned(), serde_json::to_value(holder).unwrap_or_default());
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
}

/// The lines of the heavy turn of a reply of the agent (`Reply::Runs`) now, for `brd-host runs` ([`lines_at`])
pub fn lines(table: &serde_json::Value) -> Result<Vec<String>, Refusal> {
  lines_at(table, now_ms())
}

/// The lines of the heavy turn of a reply of the agent at `now_ms`: the cores held of the cores of the host, each holder
/// with what it holds, why and for how long, each run that waits with what it waits for, and each run that is not
/// heavy with its live cores. A reply of the build before (schema 1) has one holder and no cores, and its lines say who
/// holds the turn and who waits. A table of another shape refuses with `link`, because the agent sent it
fn lines_at(table: &serde_json::Value, now_ms: u64) -> Result<Vec<String>, Refusal> {
  let table: TurnView = serde_json::from_value(table.clone()).map_err(|error| Refusal::new(Code::Link, format!("the agent sent a heavy turn of another shape: {error}")))?;
  if table.schema < 2 {
    return Ok(lines_of_one_holder(&table, now_ms));
  }
  let cores = table.cores;
  let held: Cores = table.holders.iter().map(|holder| holder.need(cores).cores(cores)).sum();
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
    let (holds, why) = holds_text(holder.need(cores));
    lines.push(format!(
      "  run {} {holds} for {} ({why}): {}",
      run_of_repo(holder),
      span_since(holder.since_ms, now_ms),
      holder.command
    ));
  }
  for waiting in &table.queue {
    let need = match waiting.need(cores) {
      Need::Cores { cores, .. } => format!("{cores} (measured)"),
      Need::Unmeasured => "the whole host (no measure)".to_owned(),
      Need::Measured(measured) => format!("the whole host (measured {measured})"),
    };
    lines.push(format!(
      "  run {} waits {} for {need}: {}",
      run_of_repo(waiting),
      span_since(waiting.since_ms, now_ms),
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
