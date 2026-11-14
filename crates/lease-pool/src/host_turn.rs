//! The heavy turn of a host (openspec/specs/cli/host/spec.md, "serve: Heavy runs take the heavy turn of the host one at a
//! time"; docs/design/host-runs.md, "Heavy runs"): a run that `heavy` of `[host]` or `brd-host run --heavy` marks heavy
//! holds the turn from before its lend to its give-back, and the other heavy runs of the host wait for it in a
//! first-come queue, whatever their repository. A run that is not heavy never asks for it.
//!
//! The table is the queue of the host slots with one place for the whole host: `heavy.json` with its lock `heavy.lock`
//! in a directory of the agent (`~/.cache/brd-host/`), not in a repository. The holder and each ticket name the process
//! of their run (the supervisor), as a lend and a ticket of `host_slots` do, so a gone process leaves the queue and a
//! gone holder frees the turn for the next run, without a keeper. The lock is a leaf, held only while the table
//! changes, never across git or another lock. A run that waits watches the directory (`TableWake`) and looks again every
//! 5 s, as the end of a process makes no file event

use crate::host_slots::{TableWake, Ticket, WAIT_RECHECK, now_ms};
use crate::state_file::StateFile;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{Duration, Instant};
use sync_utils::process_scan::is_running;

const STATE_FILE: &str = "heavy.json";
const LOCK_FILE: &str = "heavy.lock";
/// The files whose change can give a waiting run the turn
const WAKE_FILES: [&str; 1] = [STATE_FILE];
/// The longest command of an entry, in characters
const COMMAND_CHARS: usize = 200;

/// The heavy turn of a host
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeavyTurn {
  /// The run that holds the turn
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub holder: Option<TurnEntry>,
  /// The heavy runs that wait for the turn, first come first
  #[serde(default)]
  pub queue: Vec<TurnEntry>,
}

/// A heavy run that holds the turn or waits for it
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnEntry {
  /// The run and its process, as a ticket of the host slots names them
  #[serde(flatten)]
  pub ticket: Ticket,
  /// The main repository of the run on the host
  pub repo: String,
  /// The command of the run as one line, at most 200 characters
  pub command: String,
  /// When it took the turn, or joined the queue, in Unix milliseconds
  pub since_ms: u64,
}

impl TurnEntry {
  fn is_running(&self) -> bool {
    is_running(self.ticket.pid, self.ticket.process_started)
  }
}

/// The directory of the table of a host
pub struct TurnParams<'a> {
  /// The directory of `heavy.json`: the cache directory of the agent
  pub dir: &'a Path,
  /// The wait for the lock of the table
  pub lock_timeout: Duration,
}

impl TurnParams<'_> {
  fn table(&self) -> StateFile {
    table_file(self.dir)
  }
}

fn table_file(dir: &Path) -> StateFile {
  StateFile::in_dir(dir, STATE_FILE, LOCK_FILE, "the heavy turn")
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnAnswer {
  /// The run holds the turn
  Taken,
  /// Not yet: who holds it and who waits before the run
  Queued(TurnQueue),
}

/// No turn for the run yet, for the line of the wait
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnQueue {
  /// The run that holds the turn; `None` while the run before this one in the queue has yet to take it
  pub holder: Option<TurnEntry>,
  /// The runs before this one in the queue
  pub ahead: Vec<TurnEntry>,
}

/// Give the turn to the run of `request`, or put it in the queue. Under the lock of the table: the tickets of gone
/// processes leave the queue, and a gone holder frees the turn; a run that holds the turn holds it again (a repeat is
/// safe); the run's ticket joins the queue; the first run of the queue takes a free turn
pub fn take(params: &TurnParams<'_>, request: &TurnRequest<'_>) -> Result<TurnAnswer> {
  params.table().update(params.lock_timeout, |table: &mut HeavyTurn| {
    table.queue.retain(TurnEntry::is_running);
    if table.holder.as_ref().is_some_and(|holder| !holder.is_running()) {
      table.holder = None;
    }
    if table.holder.as_ref().is_some_and(|holder| holder.ticket.run == request.run) {
      return Ok(TurnAnswer::Taken);
    }
    let position = table.queue.iter().position(|entry| entry.ticket.run == request.run).unwrap_or_else(|| {
      table.queue.push(TurnEntry {
        ticket: Ticket {
          run: request.run.to_owned(),
          pid: request.pid,
          process_started: request.process_started,
        },
        repo: request.repo.to_owned(),
        command: one_line(request.command),
        since_ms: now_ms(),
      });
      table.queue.len() - 1
    });
    if table.holder.is_none() && position == 0 {
      let mut entry = table.queue.remove(0);
      entry.since_ms = now_ms();
      table.holder = Some(entry);
      return Ok(TurnAnswer::Taken);
    }
    Ok(TurnAnswer::Queued(TurnQueue {
      holder: table.holder.clone(),
      ahead: table.queue[..position].to_vec(),
    }))
  })
}

/// Give the turn to the run of `request` (`take`), waiting up to `timeout` while it is queued. The directory of the
/// table is watched before the first look, so no change between the look and the wait is lost; without a file event it
/// looks again every 5 s, since the end of a holder makes none. The answer is the turn, or the place in the queue once
/// the time is up: the ticket stays while the process lives, and `leave` takes it out
pub fn take_waiting(params: &TurnParams<'_>, request: &TurnRequest<'_>, timeout: Duration) -> Result<TurnAnswer> {
  let started = Instant::now();
  let wake = TableWake::start(params.dir, &WAKE_FILES);
  loop {
    let queued = match take(params, request)? {
      TurnAnswer::Taken => return Ok(TurnAnswer::Taken),
      TurnAnswer::Queued(queued) => queued,
    };
    let waited = started.elapsed();
    if waited >= timeout {
      return Ok(TurnAnswer::Queued(queued));
    }
    wake.wait(timeout.saturating_sub(waited).min(WAIT_RECHECK));
  }
}

/// The run `run` leaves the turn: it gives the turn back, or its ticket leaves the queue. Answers whether it held the
/// turn
pub fn leave(params: &TurnParams<'_>, run: &str) -> Result<bool> {
  params.table().update(params.lock_timeout, |table: &mut HeavyTurn| {
    table.queue.retain(|entry| entry.ticket.run != run);
    let held = table.holder.as_ref().is_some_and(|holder| holder.ticket.run == run);
    if held {
      table.holder = None;
    }
    Ok(held)
  })
}

/// The heavy turn of the host in `dir` without the runs whose process is gone, read without the lock: the holder and
/// the queue that `brd-host runs` shows
pub fn read_live(dir: &Path) -> Result<HeavyTurn> {
  let mut table: HeavyTurn = table_file(dir).read()?;
  table.holder = table.holder.filter(TurnEntry::is_running);
  table.queue.retain(TurnEntry::is_running);
  Ok(table)
}

/// The words of a command as one line, at most `COMMAND_CHARS` characters
fn one_line(command: &[String]) -> String {
  let line = command.join(" ");
  match line.char_indices().nth(COMMAND_CHARS) {
    Some((cut, _)) => format!("{}…", &line[..cut]),
    None => line,
  }
}

#[cfg(test)]
#[path = "host_turn_test.rs"]
mod host_turn_test;
