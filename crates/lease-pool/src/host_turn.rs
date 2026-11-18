//! The heavy turn of a host (openspec/specs/cli/host/spec.md, "serve: Heavy runs take the heavy turn of the host one at a
//! time"; docs/design/host-runs.md, "Heavy runs"): a run that `heavy` of `[host]` or `brd-host run --heavy` marks heavy
//! holds the turn from before its lend to its give-back, and the other heavy runs of the host wait for it in a
//! first-come queue, whatever their repository. A run that is not heavy never asks for it.
//!
//! The table is the queue of the host slots with one place for the whole host: `heavy.json` with its lock `heavy.lock`
//! in a directory of the agent of its own (`~/.cache/brd-host/heavy/`), not in a repository. The holder and each
//! ticket name the process of their run (the supervisor), as a lend and a ticket of `host_slots` do, so a gone process
//! leaves the queue and a gone holder frees the turn for the next run, without a keeper. The lock is a leaf, held only
//! while the table changes, never across git or another lock. A run that waits watches the directory once for its whole
//! wait (`TurnWatch`) and looks again every 5 s, as the end of a process makes no file event. The table names its
//! schema: a table of another schema, which an agent of another build wrote, refuses, so a supervisor of the build
//! before an upgrade never reads a turn it does not know.
//!
//! A later heavy run of the same checkout (its affinity) and the same command (`host_link::heavy::run_key`) supersedes
//! the runs of the queue that wait for the turn with them: it takes the place of the first in the queue, and each of
//! them moves to `superseded` with the id of the later run, which its own next look answers (`TurnAnswer::Superseded`).
//! A run of the queue has no slot and has started no command: the turn comes before the lend. The holder is never
//! superseded. An entry of an older build has no affinity and no key, and supersedes nothing and nobody

use crate::host_slots::{TableWake, Ticket, WAIT_RECHECK, now_ms};
use crate::state_file::StateFile;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{Duration, Instant};
use sync_utils::process_scan::is_running;

/// The schema of `heavy.json` that this build reads and writes
const SCHEMA: u32 = 1;
const STATE_FILE: &str = "heavy.json";
const LOCK_FILE: &str = "heavy.lock";
/// The files whose change can give a waiting run the turn
const WAKE_FILES: [&str; 1] = [STATE_FILE];
/// The longest command of an entry, in characters
const COMMAND_CHARS: usize = 200;

/// The heavy turn of a host
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeavyTurn {
  /// The schema of the table, `SCHEMA` of this build; a table without one is of no build
  #[serde(default)]
  pub schema: u32,
  /// The run that holds the turn
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub holder: Option<TurnEntry>,
  /// The heavy runs that wait for the turn, first come first
  #[serde(default)]
  pub queue: Vec<TurnEntry>,
  /// The runs that a later run of their checkout and command took out of the queue: the next look of each answers the
  /// later run and takes it out
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub superseded: Vec<SupersededEntry>,
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
  /// The checkout of the client of the run (`RunSpec::affinity`); `None` from an older build
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub affinity: Option<String>,
  /// The key of the whole command by what it runs (`host_link::heavy::run_key`), not cut as `command`; `None` from an
  /// older build
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub command_key: Option<String>,
}

impl Default for HeavyTurn {
  fn default() -> Self {
    Self {
      schema: SCHEMA,
      holder: None,
      queue: Vec::new(),
      superseded: Vec::new(),
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
}

impl TurnEntry {
  fn is_running(&self) -> bool {
    is_running(self.ticket.pid, self.ticket.process_started)
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

/// The directory of the table of a host
pub struct TurnParams<'a> {
  /// The directory of `heavy.json`, of its lock and of nothing else: `~/.cache/brd-host/heavy/`
  pub dir: &'a Path,
  /// The wait for the lock of the table
  pub lock_timeout: Duration,
  /// How often a wait looks again without a file event (`RECHECK`): the end of a holder makes none, and `FSEvents` of
  /// macOS delays and drops events under load, so the look bounds every wait
  pub recheck: Duration,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnAnswer {
  /// The run holds the turn
  Taken,
  /// Not yet: who holds it and who waits before the run
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
}

/// No turn for the run yet, for the line of the wait
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnQueue {
  /// The run that holds the turn; `None` while the run before this one in the queue has yet to take it
  pub holder: Option<TurnEntry>,
  /// The runs before this one in the queue
  pub ahead: Vec<TurnEntry>,
}

/// Give the turn to the run of `request`, or put it in the queue ([`join`])
pub fn take(params: &TurnParams<'_>, request: &TurnRequest<'_>) -> Result<TurnAnswer> {
  join(params, request).map(|joined| joined.answer)
}

/// Give the turn to the run of `request`, or put it in the queue, and say what its first ask did. Under the lock of the
/// table: the tickets of gone processes leave the queue and `superseded`, and a gone holder frees the turn; a superseded
/// run gets the later run and leaves `superseded`; a run that holds the turn holds it again (a repeat is safe); a run
/// new to the queue supersedes the runs of the queue of its checkout and command and takes the place of the first of
/// them, else joins the end; the first run of the queue takes a free turn
pub fn join(params: &TurnParams<'_>, request: &TurnRequest<'_>) -> Result<Joined> {
  params.table().update(params.lock_timeout, |table: &mut HeavyTurn| {
    let table = table.of_this_schema(params.dir)?;
    table.queue.retain(TurnEntry::is_running);
    table.superseded.retain(|entry| is_running(entry.ticket.pid, entry.ticket.process_started));
    if table.holder.as_ref().is_some_and(|holder| !holder.is_running()) {
      table.holder = None;
    }
    let mut joined = Joined {
      answer: TurnAnswer::Taken,
      replaced: Vec::new(),
      beside: Vec::new(),
    };
    if let Some(at) = table.superseded.iter().position(|entry| entry.ticket.run == request.run) {
      joined.answer = TurnAnswer::Superseded {
        by: table.superseded.remove(at).by,
      };
      return Ok(joined);
    }
    if table.holder.as_ref().is_some_and(|holder| holder.ticket.run == request.run) {
      return Ok(joined);
    }
    let queued = table.queue.iter().position(|entry| entry.ticket.run == request.run);
    let position = queued.unwrap_or_else(|| enqueue(table, request, &mut joined));
    if table.holder.is_none() && position == 0 {
      let mut entry = table.queue.remove(0);
      entry.since_ms = now_ms();
      table.holder = Some(entry);
      return Ok(joined);
    }
    joined.answer = TurnAnswer::Queued(TurnQueue {
      holder: table.holder.clone(),
      ahead: table.queue[..position].to_vec(),
    });
    Ok(joined)
  })
}

/// Puts the run of `request`, new to the queue, in its place, and answers it: the runs of the queue of its checkout and
/// command go to `superseded` with it as the later run (`joined.replaced`), and it takes the place of the first of
/// them, else the end; `joined.beside` gets the runs of its checkout with another command
fn enqueue(table: &mut HeavyTurn, request: &TurnRequest<'_>, joined: &mut Joined) -> usize {
  let place = table.queue.iter().position(|entry| entry.is_of(request, true));
  let (replaced, kept): (Vec<TurnEntry>, Vec<TurnEntry>) = std::mem::take(&mut table.queue).into_iter().partition(|entry| entry.is_of(request, true));
  table.queue = kept;
  table.superseded.extend(replaced.iter().map(|entry| SupersededEntry {
    ticket: entry.ticket.clone(),
    by: request.run.to_owned(),
  }));
  joined.beside = table.queue.iter().filter(|entry| entry.is_of(request, false)).cloned().collect();
  joined.replaced = replaced;
  let entry = TurnEntry {
    ticket: Ticket {
      run: request.run.to_owned(),
      pid: request.pid,
      process_started: request.process_started,
    },
    repo: request.repo.to_owned(),
    command: one_line(request.command),
    since_ms: now_ms(),
    affinity: Some(request.affinity.to_owned()),
    command_key: Some(request.command_key.to_owned()),
  };
  // the place of the first superseded run: the runs before it are before it still
  let position = place.unwrap_or(table.queue.len()).min(table.queue.len());
  table.queue.insert(position, entry);
  position
}

/// Give the turn to the run of `request` (`take`), waiting up to `timeout` while it is queued, woken by `watch`, which
/// the caller started before its first look and keeps for its whole wait; without a file event it looks again every
/// `params.recheck`, since the end of a holder makes none and an event may come late or never (`FSEvents` under load). The
/// answer is the turn, the later run that superseded it, or the place in the queue once the time is up: the ticket
/// stays while the process lives, and `leave` takes it out
pub fn take_waiting(params: &TurnParams<'_>, request: &TurnRequest<'_>, timeout: Duration, watch: &TurnWatch) -> Result<TurnAnswer> {
  let started = Instant::now();
  loop {
    let queued = match take(params, request)? {
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

/// The run `run` leaves the turn: it gives the turn back, or its ticket leaves the queue or `superseded`. Answers whether
/// it held the turn
pub fn leave(params: &TurnParams<'_>, run: &str) -> Result<bool> {
  params.table().update(params.lock_timeout, |table: &mut HeavyTurn| {
    let table = table.of_this_schema(params.dir)?;
    table.queue.retain(|entry| entry.ticket.run != run);
    table.superseded.retain(|entry| entry.ticket.run != run);
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
  table.of_this_schema(dir)?;
  table.holder = table.holder.filter(TurnEntry::is_running);
  table.queue.retain(TurnEntry::is_running);
  table.superseded.retain(|entry| is_running(entry.ticket.pid, entry.ticket.process_started));
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
