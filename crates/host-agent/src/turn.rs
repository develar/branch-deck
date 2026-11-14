//! The heavy turn of the host for the heavy runs (`lease_pool::host_turn`; openspec/specs/cli/host/spec.md, "serve: Heavy
//! runs take the heavy turn of the host one at a time"; docs/design/host-runs.md, "Heavy runs"). The table is
//! `~/.cache/brd-host/heavy/heavy.json`, one for every repository of the host, in a directory of its own, so the watch
//! of a wait sees no event of the logs of the runs.
//!
//! - A supervisor of a heavy run takes the turn before its lend ([`take`], with its own pid: the turn lasts as long as
//!   the supervisor). Its wait is the wait for a host slot ([`slots::wait_in_looks`]): looks of 2 s, so a cancel ends
//!   it, one watch of the table for the whole wait, and the line of the wait once a minute
//!   (`lease_pool::host_runs::turn_line`).
//! - On every way out, after the give-back of its slot, the supervisor leaves the turn ([`leave`]): it gives the turn
//!   back, or its ticket leaves the queue.
//! - `Reply::Runs` carries the table without the gone runs ([`live`]), and `brd-host runs` says it ([`lines`])

use crate::home::Home;
use crate::run::epoch_ms;
use crate::slots::{self, Look};
use host_link::refusal::{Code, Refusal};
use lease_pool::host_runs::{run_of_repo, span_since, turn_line};
use lease_pool::host_turn::{HeavyTurn, RECHECK, TurnAnswer, TurnParams, TurnQueue, TurnRequest, read_live, take as take_turn, take_waiting, watch};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};
use sync_utils::process_scan::process_started;

/// The wait for the lock of the table of the heavy turn
const LOCK_TIMEOUT: Duration = Duration::from_secs(60);

const fn params(dir: &Path) -> TurnParams<'_> {
  TurnParams {
    dir,
    lock_timeout: LOCK_TIMEOUT,
    recheck: RECHECK,
  }
}

fn now_ms() -> u64 {
  epoch_ms(SystemTime::now())
}

/// Takes the heavy turn of the host for the heavy run `run` of `repo`, which this process runs, waiting up to `wait`
/// while other heavy runs hold it or wait before it. A queued first look says the line of the wait through `say` at
/// once, and each minute of the wait says it again. Answers `Some(TurnAnswer::Taken)`, the place in the queue once the
/// time is up, or `None` when `stop` ended the wait: the end of the run takes the ticket out of the queue ([`leave`]). A
/// table that cannot be read or written refuses with `agent_error`
pub(crate) fn take(
  home: &Home,
  run: &str,
  repo: &str,
  command: &[String],
  wait: Duration,
  stop: &dyn Fn() -> bool,
  say: &mut dyn FnMut(&str),
) -> Result<Option<TurnAnswer>, Refusal> {
  let pid = std::process::id();
  let request = TurnRequest {
    run,
    pid,
    process_started: process_started(pid),
    repo,
    command,
  };
  let dir = home.heavy_dir();
  let params = params(&dir);
  // the watch starts before the first look, so no change between a look and the wait after it is lost
  let watched = watch(&params);
  let refused = |error: anyhow::Error| Refusal::new(Code::AgentError, format!("cannot take the heavy turn of the host: {error:#}"));
  let started = Instant::now();
  let line = |queue: &TurnQueue, waited: Duration| turn_line(run, queue, waited, wait.saturating_sub(waited), now_ms());
  match take_turn(&params, &request).map_err(refused)? {
    TurnAnswer::Taken => return Ok(Some(TurnAnswer::Taken)),
    TurnAnswer::Queued(queue) => say(&line(&queue, Duration::ZERO)),
  }
  let mut look = |chunk: Duration| match take_waiting(&params, &request, chunk, &watched).map_err(refused)? {
    TurnAnswer::Taken => Ok(Look::Got(())),
    TurnAnswer::Queued(queue) => Ok(Look::Queued(queue)),
  };
  let left = wait.saturating_sub(started.elapsed());
  let waited = slots::wait_in_looks(left, stop, &mut look, &|queue: &TurnQueue, _| line(queue, started.elapsed()), say)?;
  Ok(waited.map(|look| match look {
    Look::Got(()) => TurnAnswer::Taken,
    Look::Queued(queue) => TurnAnswer::Queued(queue),
  }))
}

/// The line of a wait for the turn that ended in the queue after `waited`, for the refusal `heavy_timeout`
pub(crate) fn timeout_line(run: &str, queue: &TurnQueue, waited: Duration) -> String {
  turn_line(run, queue, waited, Duration::ZERO, now_ms())
}

/// The run `run` leaves the heavy turn of the host: it gives the turn back, or its ticket leaves the queue. Answers
/// whether it held the turn
pub(crate) fn leave(home: &Home, run: &str) -> Result<bool, Refusal> {
  lease_pool::host_turn::leave(&params(&home.heavy_dir()), run).map_err(|error| Refusal::new(Code::AgentError, format!("cannot leave the heavy turn of the host: {error:#}")))
}

/// The heavy turn of the host without the runs whose process is gone, as JSON for `Reply::Runs`. A table that cannot be
/// read is none, and the log says why
pub(crate) fn live(home: &Home) -> Result<serde_json::Value, Refusal> {
  let table = read_live(&home.heavy_dir()).map_err(|error| Refusal::new(Code::AgentError, format!("cannot read the heavy turn of the host: {error:#}")))?;
  serde_json::to_value(table).map_err(|error| Refusal::new(Code::AgentError, format!("cannot write the heavy turn of the host: {error}")))
}

/// The lines of the heavy turn of a reply of the agent (`Reply::Runs`) now, for `brd-host runs` ([`lines_at`])
pub fn lines(table: &serde_json::Value) -> Result<Vec<String>, Refusal> {
  lines_at(table, now_ms())
}

/// The lines of the heavy turn of a reply of the agent at `now_ms`: who holds it and for how long, then each run that
/// waits and for how long. A table of another shape refuses with `link`, because the agent sent it
fn lines_at(table: &serde_json::Value, now_ms: u64) -> Result<Vec<String>, Refusal> {
  let table: HeavyTurn = serde_json::from_value(table.clone()).map_err(|error| Refusal::new(Code::Link, format!("the agent sent a heavy turn of another shape: {error}")))?;
  let mut lines = vec![table.holder.as_ref().map_or_else(
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
  Ok(lines)
}

#[cfg(test)]
#[path = "turn_test.rs"]
mod turn_test;
