//! The queue of the gets that wait for a slot (openspec/specs/cli/lease/spec.md, "Get: The gets that wait take their
//! slots in the order they asked"; "Lease" in docs/design/git-operations.md). A get that finds no free slot enters the
//! queue with a ticket in the state of the pool (`LeaseStore::queue`), in the write of that reserve; each later reserve
//! writes its phase in its own write, so a reserve and the place in the queue never part. The ticket leaves the queue
//! in the write of a new slot or of the `ready` lease of the get. A free slot goes to the first waiting ticket that has
//! not tried it, an idle lease to the first waiting ticket whose get takes idle leases over and has not tried it, and a
//! get without a ticket comes after every ticket (`Gate`). Only the first waiting ticket waits for the clones lock.
//!
//! The queue passes over a busy ticket (its get checks a slot, or frees space without the clones lock) and a ticket
//! whose heartbeat is older than `QueueTiming::stale`; both keep their place by their time of entry. A ticket also
//! leaves the queue when its get ends or its process is gone. A brd that knows no queue drops the queue when it writes
//! the state, and takes a slot past the waiting gets; each waiting get then writes its ticket again, with its time of
//! entry

use crate::host_slots::now_ms;
use crate::lease::{Lease, LeaseStore, Pool};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::time::{Duration, Instant};
use sync_utils::process_scan::is_running;
use tracing::warn;

/// A get that waits for a slot, in the state of the pool (`LeaseStore::queue`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueTicket {
  /// The id of the lease of the get
  pub id: String,
  /// The holder of that lease
  pub holder: String,
  /// The process of the get, and when it started (`process_started`): the ticket of a gone process leaves the queue
  pub pid: u32,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub process_started: Option<u64>,
  /// When the get entered the queue, Unix milliseconds: its place
  pub entered_ms: u64,
  /// When the get last wrote the ticket as waiting, Unix milliseconds
  pub heartbeat_ms: u64,
  pub phase: TicketPhase,
  /// The slots the get looked at: those go to the tickets behind it
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub tried: Vec<String>,
  /// Whether the get takes an idle lease over: it has checks left (`IDLE_CHECKS_PER_GET`), and it is no get of a host
  /// slot
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub takes_over: bool,
}

/// What the get of a ticket does
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketPhase {
  /// It waits for a slot or for the clones lock, and writes its heartbeat
  Waiting,
  /// It checks a slot it reserved (its Bazel servers, its `status`, the work of an idle lease), or frees space without
  /// the clones lock. It writes no heartbeat, and the queue passes it
  Busy,
  /// A phase that a later brd wrote: the queue passes it, as a busy one
  #[serde(other)]
  Unknown,
}

/// How often a waiting get writes its heartbeat, and how old a heartbeat makes its ticket stale
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueTiming {
  pub heartbeat: Duration,
  pub stale: Duration,
}

impl Default for QueueTiming {
  /// A heartbeat every 10 s, stale after 60 s
  fn default() -> Self {
    Self {
      heartbeat: Duration::from_secs(10),
      stale: Duration::from_mins(1),
    }
  }
}

impl QueueTicket {
  fn is_running(&self) -> bool {
    is_running(self.pid, self.process_started)
  }

  /// The place of the ticket in the queue: its time of entry, then its id
  fn place(&self) -> (u64, &str) {
    (self.entered_ms, &self.id)
  }

  /// Whether the ticket waits at `now_ms`: waiting, its heartbeat not older than `stale`, its process alive
  fn waits(&self, now_ms: u64, stale: Duration) -> bool {
    self.phase == TicketPhase::Waiting && now_ms.saturating_sub(self.heartbeat_ms) <= millis(stale) && self.is_running()
  }

  fn has_tried(&self, path: &str) -> bool {
    self.tried.iter().any(|tried| tried == path)
  }
}

fn millis(duration: Duration) -> u64 {
  u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// `queue` without the tickets whose process is gone: under the pool lock, by every write of a get
pub(crate) fn settle_queue(queue: &mut Vec<QueueTicket>) {
  queue.retain(QueueTicket::is_running);
}

/// `ticket` in `queue`, in the place of the entry of the same id, or added
fn put(queue: &mut Vec<QueueTicket>, ticket: QueueTicket) {
  match queue.iter_mut().find(|entry| entry.id == ticket.id) {
    Some(entry) => *entry = ticket,
    None => queue.push(ticket),
  }
}

/// The waiting tickets ahead of a get, in their order: what the queue lets the get reserve
#[derive(Debug, Default)]
pub struct Gate {
  ahead: Vec<QueueTicket>,
}

impl Gate {
  /// The tickets of `queue` that wait at `now_ms` (`stale`: the age of a stale heartbeat) and are ahead of `own`, the
  /// ticket of the get; every waiting ticket for a get without one
  pub fn of(queue: &[QueueTicket], own: Option<&QueueTicket>, now_ms: u64, stale: Duration) -> Self {
    let mut ahead: Vec<QueueTicket> = queue
      .iter()
      .filter(|ticket| own.is_none_or(|own| ticket.id != own.id && ticket.place() < own.place()) && ticket.waits(now_ms, stale))
      .cloned()
      .collect();
    ahead.sort_by(|a, b| a.place().cmp(&b.place()));
    Self { ahead }
  }

  /// Whether the get may reserve the free slot at `path`: every waiting ticket ahead tried it
  pub fn lets_free(&self, path: &str) -> bool {
    self.ahead.iter().all(|ticket| ticket.has_tried(path))
  }

  /// Whether the get may take over the idle lease of the slot at `path`: every waiting ticket ahead that takes idle
  /// leases over tried it
  pub fn lets_idle(&self, path: &str) -> bool {
    self.ahead.iter().all(|ticket| !ticket.takes_over || ticket.has_tried(path))
  }

  /// Whether no ticket waits ahead of the get: it is the one that waits for the clones lock
  pub const fn is_first(&self) -> bool {
    self.ahead.is_empty()
  }

  /// The ticket right ahead of the get, and how many wait ahead of it
  pub fn behind(&self) -> Option<Behind> {
    let next = self.ahead.last()?;
    Some(Behind {
      holder: next.holder.clone(),
      pid: next.pid,
      entered_ms: next.entered_ms,
      ahead: self.ahead.len(),
    })
  }
}

/// Whom a get waits behind in the queue (`GetStep::WaitBehind`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Behind {
  /// The holder of the get right ahead of it
  pub holder: String,
  /// The process of that get
  pub pid: u32,
  /// When that get entered the queue, Unix milliseconds
  pub entered_ms: u64,
  /// How many gets wait ahead of it in all
  pub ahead: usize,
}

impl fmt::Display for Behind {
  /// "waiting behind the get of lane-3 (pid 4242, in the queue for 35 s), 2 get(s) ahead: …"
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let waited = now_ms().saturating_sub(self.entered_ms) / 1000;
    let waited = if waited >= 60 {
      format!("{} min {} s", waited / 60, waited % 60)
    } else {
      format!("{waited} s")
    };
    write!(
      f,
      "waiting behind the get of {} (pid {}, in the queue for {waited}), {} get(s) ahead: the gets that wait take their slots in the order they asked",
      self.holder, self.pid, self.ahead
    )
  }
}

/// What a reserve makes of the ticket of its get (`QueuePlace::reserved`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Turn {
  /// No free slot, or a return or a clean to wait for under the clones lock: the get waits, and enters the queue at its
  /// first such reserve
  Wait,
  /// A slot to check
  Busy,
  /// A new slot: the get leaves the queue
  Final,
}

/// The place of one get in the queue: its ticket as it wrote it last, which its drop takes out of the state
pub(crate) struct QueuePlace<'a> {
  pool: &'a Pool,
  lock_timeout: Duration,
  timing: QueueTiming,
  /// The ticket the get enters with: its lease and its process
  entry: QueueTicket,
  /// The ticket as the get wrote it last; `None` before its first wait and once it left the queue
  ticket: Option<QueueTicket>,
  /// When the get last wrote its heartbeat
  beat: Instant,
}

impl<'a> QueuePlace<'a> {
  /// The place of the get of `lease` in the queue of `pool`, not entered yet
  pub(crate) fn new(pool: &'a Pool, lock_timeout: Duration, timing: QueueTiming, lease: &Lease) -> Self {
    Self {
      pool,
      lock_timeout,
      timing,
      entry: QueueTicket {
        id: lease.id.clone(),
        holder: lease.holder.clone(),
        pid: lease.pid,
        process_started: lease.process_started,
        entered_ms: 0,
        heartbeat_ms: 0,
        phase: TicketPhase::Waiting,
        tried: Vec::new(),
        takes_over: false,
      },
      ticket: None,
      beat: Instant::now(),
    }
  }

  /// The tickets ahead of the get in `store`
  pub(crate) fn gate(&self, store: &LeaseStore) -> Gate {
    Gate::of(&store.queue, self.ticket.as_ref(), now_ms(), self.timing.stale)
  }

  /// In the write of a reserve that `turn` tells: the get enters the queue at its first wait, writes its phase, the
  /// slots it tried (`tried`) and whether it takes an idle lease over (`takes_over`), or leaves the queue (`left` once
  /// the write is done)
  pub(crate) fn reserved(&mut self, store: &mut LeaseStore, turn: Turn, tried: &HashSet<String>, takes_over: bool) {
    let now = now_ms();
    let ticket = match turn {
      Turn::Final => {
        self.leave_in(store);
        return;
      }
      Turn::Busy => match self.ticket.as_mut() {
        Some(ticket) => {
          ticket.phase = TicketPhase::Busy;
          ticket
        }
        None => return,
      },
      Turn::Wait => {
        let ticket = self.ticket.get_or_insert_with(|| QueueTicket {
          entered_ms: now,
          ..self.entry.clone()
        });
        ticket.phase = TicketPhase::Waiting;
        ticket.heartbeat_ms = now;
        self.beat = Instant::now();
        ticket
      }
    };
    let mut paths: Vec<String> = tried.iter().cloned().collect();
    paths.sort();
    ticket.tried = paths;
    ticket.takes_over = takes_over;
    put(&mut store.queue, ticket.clone());
  }

  /// While the get waits, at each look of `store` (read without the lock): its heartbeat once `QueueTiming::heartbeat`
  /// passed, and its ticket again, with its time of entry, when a brd that knows no queue dropped it. A write that
  /// fails is tried again at the next look
  pub(crate) fn keep(&mut self, store: &LeaseStore) {
    let Some(ticket) = &self.ticket else { return };
    let listed = store.queue.iter().any(|entry| entry.id == ticket.id);
    if listed && self.beat.elapsed() < self.timing.heartbeat {
      return;
    }
    let ticket = QueueTicket {
      heartbeat_ms: now_ms(),
      phase: TicketPhase::Waiting,
      ..ticket.clone()
    };
    let written = self.pool.update(self.lock_timeout, |store| {
      settle_queue(&mut store.queue);
      put(&mut store.queue, ticket.clone());
      Ok(())
    });
    match written {
      Ok(()) => {
        self.ticket = Some(ticket);
        self.beat = Instant::now();
      }
      Err(error) => warn!(error = %format!("{error:#}"), "cannot write the heartbeat of the get in the queue"),
    }
  }

  /// The get frees space without the clones lock, in a write of its own: the queue passes it meanwhile
  pub(crate) fn busy(&mut self) {
    let Some(ticket) = self.ticket.as_mut() else { return };
    ticket.phase = TicketPhase::Busy;
    let ticket = ticket.clone();
    let written = self.pool.update(self.lock_timeout, |store| {
      put(&mut store.queue, ticket);
      Ok(())
    });
    if let Err(error) = written {
      warn!(error = %format!("{error:#}"), "cannot mark the get in the queue busy");
    }
  }

  /// In a write of the get that makes its reserve final: its ticket leaves the queue. The get forgets the ticket only
  /// once that write is done (`left`), so a write that fails leaves it to the drop
  pub(crate) fn leave_in(&self, store: &mut LeaseStore) {
    if let Some(ticket) = &self.ticket {
      store.queue.retain(|entry| entry.id != ticket.id);
    }
  }

  /// The write of `leave_in` is done: the drop has nothing to take out
  pub(crate) fn left(&mut self) {
    self.ticket = None;
  }
}

impl Drop for QueuePlace<'_> {
  /// A get that ends in the queue (an error, a refusal) takes its ticket out of the state; one killed leaves it to the
  /// next write of a get (`settle_queue`)
  fn drop(&mut self) {
    let Some(ticket) = self.ticket.take() else { return };
    let left = self.pool.update(self.lock_timeout, |store| {
      store.queue.retain(|entry| entry.id != ticket.id);
      Ok(())
    });
    if let Err(error) = left {
      warn!(error = %format!("{error:#}"), "cannot take the ticket of the get out of the queue");
    }
  }
}
