//! When a heavy run would take the heavy turn of a host (openspec/specs/cli/host/spec.md, "serve: An estimate says when a
//! run would start there"; docs/design/host-runs.md, "Sets of hosts"): the hint that a client of a set of hosts reads of
//! each member before it chooses one, never an admission. The admission of the turn decides (`host_turn::join`); the
//! estimate only plays the queue forward.
//!
//! The run takes its place in the queue as a new run would, by the place function of the queue (`host_turn::place`,
//! which puts a short run ahead of the runs that it may pass); then the holders end, each after the wall that
//! its command took in its last runs, and the queue before the run is admitted in its order by the rule of the turn
//! (`host_turn::admits`), until the run is admitted. The load of the host outside the heavy runs is the most of its last
//! 30 s ([`Sight::of`]): one sample of 2 s misled a choice before. A run of the queue or a holder whose command has no
//! wall counts as one of `unknown_ahead` and takes no time here; an exclusive run before it that cannot start counts so
//! too, as the runs behind its drain pass it. Read without a lock: nothing of the turn changes

use crate::host_slots::Ticket;
use crate::host_turn::{HeadDrain, HeavyTurn, LOAD_BOUND, Load, QueueMarks, TurnEntry, admits, head_drain, host_sample, host_window, outside, read_live, read_marks, short_wall};
use anyhow::Result;
use std::path::Path;
use std::time::Duration;
use sync_utils::cores::Cores;

/// What an estimate reads of the heavy turn of a host
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sight {
  /// The holders and the queue without the gone runs (`host_turn::read_live`)
  pub turn: HeavyTurn,
  /// The cores of the host that the heavy runs share
  pub cores: u32,
  /// The most busy cores of the host over its last 30 s outside the live use of the holders, the live cores of the runs
  /// that are not heavy at least; `None` without a sample of the host, and the runs that are not heavy count then
  pub outside: Option<Cores>,
  /// The most busy cores of the host over its last 30 s, when its samples span them: what an exclusive run waits on
  pub window: Option<Cores>,
  /// The agent sampled the host at all
  pub sampled: bool,
  /// The classes of the runs of the queue and their passes (`passes.json`, `host_turn::read_marks`)
  pub marks: QueueMarks,
  /// The drain of an exclusive head of the queue (`host_turn::head_drain`)
  pub head_drain: Option<HeadDrain>,
  pub now_ms: u64,
}

impl Sight {
  /// The heavy turn of the host in `dir` of `cores` at `now_ms`, with the load of its last 30 s and the drain of an
  /// exclusive head of `drain_bar`
  pub fn of(dir: &Path, cores: u32, now_ms: u64, drain_bar: Duration) -> Result<Self> {
    let turn = read_live(dir)?;
    let window = host_window(dir, now_ms);
    Ok(Self {
      outside: outside(&turn.holders, turn.light_cores(), window),
      window,
      sampled: host_sample(dir, now_ms).is_some(),
      marks: read_marks(dir),
      head_drain: head_drain(dir, &turn.queue, now_ms, drain_bar),
      turn,
      cores,
      now_ms,
    })
  }
}

/// The heavy run that an estimate places
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asked<'a> {
  /// The main repository of the run on the host
  pub repo: &'a str,
  /// The checkout of its client (`RunSpec::affinity`)
  pub affinity: &'a str,
  /// The key of its command by what it runs (`host_link::heavy::run_key`), which a later run supersedes by
  pub command_key: &'a str,
  /// The cores that the last runs of its command used (`host_cores::expected`); `None` takes the whole host
  pub expected: Option<Cores>,
  /// The wall of the last passed runs of its command (`host_cores::passed_wall`): whether it is short
  pub passed_wall: Option<Duration>,
}

/// When the run would take the turn
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StartEstimate {
  /// From now, by the holders and the runs before it whose wall is known
  pub start_in_ms: u64,
  /// The holders and the runs before it whose wall is unknown, and the exclusive runs before it that cannot start
  pub unknown_ahead: u32,
}

/// When `asked` would take the heavy turn of `sight`, with `wall` the wall in milliseconds of the command of an entry of
/// the turn by its last runs (`None` when it has none)
pub fn estimate(sight: &Sight, asked: &Asked<'_>, wall: &dyn Fn(&TurnEntry) -> Option<u64>) -> StartEstimate {
  let cores = sight.cores;
  let now = sight.now_ms;
  let load = Load {
    outside: sight.outside.unwrap_or_else(|| sight.turn.light_cores()),
    window: sight.window,
    sampled: sight.sampled,
    light: &sight.turn.light,
  };
  let mut unknown = 0u32;
  // each holder, and each run admitted on the way, with the time of its end
  let mut running: Vec<(u64, TurnEntry)> = sight
    .turn
    .holders
    .iter()
    .map(|holder| {
      let end = wall(holder).map_or_else(
        || {
          unknown += 1;
          now
        },
        |wall| holder.since_ms.saturating_add(wall).max(now),
      );
      (end, holder.clone())
    })
    .collect();
  let mut at = now;
  let (ahead, since_ms) = place(sight, asked);
  let run = entry_of(asked, since_ms);
  for (index, entry) in ahead.iter().copied().chain(std::iter::once(&run)).enumerate() {
    let last = index == ahead.len();
    loop {
      running.retain(|(end, _)| *end > at);
      let holders: Vec<TurnEntry> = running.iter().map(|(_, holder)| holder.clone()).collect();
      let waited_out = at.saturating_sub(entry.since_ms) >= millis_of_bound();
      if admits(entry, &holders, &load, cores, waited_out) {
        if !last {
          let end = wall(entry).map_or_else(
            || {
              unknown += 1;
              at
            },
            |wall| at.saturating_add(wall),
          );
          running.push((end, entry.clone()));
        }
        break;
      }
      // the next end of a holder, else the bound of the load of the host on a free turn
      let next = running
        .iter()
        .map(|(end, _)| *end)
        .min()
        .or_else(|| (!entry.exclusive && !waited_out).then(|| entry.since_ms.saturating_add(millis_of_bound())));
      match next {
        Some(next) if next > at => at = next,
        // an exclusive run that a busy host keeps out: the runs behind its drain pass it
        _ => {
          unknown += 1;
          break;
        }
      }
    }
  }
  StartEstimate {
    start_in_ms: at.saturating_sub(now),
    unknown_ahead: unknown,
  }
}

/// The runs of the queue of `sight` before a new run of `asked`, as the queue places it (`host_turn::enqueue`), and the
/// join that its bound of the load counts from: every run of its checkout and command that waits leaves the queue,
/// superseded, and the run takes its place within the runs before the first of them, else within the whole queue, by
/// the one place function of the queue (`host_turn::place`), so the estimate and the queue agree. A short run counts
/// the earliest join of the runs that it passes
fn place<'q>(sight: &'q Sight, asked: &Asked<'_>) -> (Vec<&'q TurnEntry>, u64) {
  let same = |entry: &TurnEntry| {
    !asked.affinity.is_empty() && entry.repo == asked.repo && entry.affinity.as_deref() == Some(asked.affinity) && entry.command_key.as_deref() == Some(asked.command_key)
  };
  let queue = &sight.turn.queue;
  let superseded_at = queue.iter().position(same);
  let kept: Vec<&TurnEntry> = queue.iter().filter(|entry| !same(entry)).collect();
  let owned: Vec<TurnEntry> = kept.iter().map(|entry| (*entry).clone()).collect();
  let end = superseded_at.unwrap_or(kept.len()).min(kept.len());
  let short = short_wall(false, asked.expected, asked.passed_wall, sight.cores).is_some();
  let position = crate::host_turn::place(&owned[..end], &sight.marks, sight.head_drain, short);
  let since_ms = kept[position..end].iter().map(|entry| entry.since_ms).fold(sight.now_ms, u64::min);
  (kept[..position].to_vec(), since_ms)
}

/// The entry of `asked` as it would join the queue at `now_ms`
fn entry_of(asked: &Asked<'_>, now_ms: u64) -> TurnEntry {
  TurnEntry {
    ticket: Ticket {
      run: String::new(),
      pid: 0,
      process_started: None,
    },
    repo: asked.repo.to_owned(),
    command: String::new(),
    since_ms: now_ms,
    affinity: Some(asked.affinity.to_owned()),
    command_key: Some(asked.command_key.to_owned()),
    expected: asked.expected,
    live: None,
    exclusive: false,
    passed: None,
    measure_as: None,
  }
}

fn millis_of_bound() -> u64 {
  u64::try_from(LOAD_BOUND.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "host_estimate_test.rs"]
mod host_estimate_test;
