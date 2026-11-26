//! When a heavy run would take the heavy turn of a host (openspec/specs/cli/host/spec.md, "serve: An estimate says when a
//! run would start there"; docs/design/host-runs.md, "Sets of hosts"): the hint that a client of a set of hosts reads of
//! each member before it chooses one, never an admission. The admission of the turn decides (`host_turn::join`); the
//! estimate only plays the queue forward.
//!
//! The run takes its place in the queue as a new run would, by the join itself (`host_turn::join_queue`) applied to a
//! copy of the table and of its files, short runs, supersedes and the turns of the groups alike; then the holders end, each after the wall that
//! its command took in its last runs, and the queue before the run is admitted in its order by the rule of the turn
//! (`host_turn::admits`), until the run is admitted. The load of the host outside the heavy runs is the most of its last
//! 30 s ([`Sight::of`]): one sample of 2 s misled a choice before. A run of the queue or a holder whose command has no
//! wall counts as one of `unknown_ahead` and takes no time here; an exclusive run before it that cannot start counts so
//! too, as the runs behind its drain pass it. Read without a lock: nothing of the turn changes

use crate::host_slots::Ticket;
use crate::host_turn::{
  HeadDrain, HeavyTurn, Joined, LOAD_BOUND, Load, QueueFiles, TurnAnswer, TurnEntry, TurnRequest, admits, head_drain, host_sample, host_window, join_queue, outside, read_live,
};
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
  /// The files beside the table that a join reads: the marks of the short lane, the groups of the turns and the drain
  /// (`host_turn::QueueFiles`)
  pub files: QueueFiles,
  /// The drain of an exclusive head of the queue (`host_turn::head_drain`), and how long a drain bars the host
  pub head_drain: Option<HeadDrain>,
  pub drain_bar: Duration,
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
      files: QueueFiles::read(dir),
      head_drain: head_drain(dir, &turn.queue, now_ms, drain_bar),
      drain_bar,
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
  /// Its group in the heavy queue (`host_turn::group_key`): the turns of the groups place it
  pub group: &'a str,
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
  let mut play = Play::of(sight, wall);
  let (ahead, since_ms) = place(sight, asked);
  let run = entry_of(asked, since_ms);
  for (index, entry) in ahead.iter().chain(std::iter::once(&run)).enumerate() {
    let last = index == ahead.len();
    if play.until_admitted(entry) {
      if !last {
        let end = wall(entry).map_or_else(
          || {
            play.unknown += 1;
            play.at
          },
          |wall| play.at.saturating_add(wall),
        );
        play.running.push((end, entry.clone()));
      }
    } else {
      // an exclusive run that a busy host keeps out: the runs behind its drain pass it
      play.unknown += 1;
    }
  }
  play.estimate()
}

/// When the run at `position` of the queue of `sight` would take the heavy turn, the head of the queue or the run that
/// passes an exclusive head whose drain ended, by the walls of the holders, `wall` as for [`estimate`]: the runs before
/// it in the queue take nothing here. An exclusive run that a busy host keeps out once no holder is left starts at the
/// end of the last holder, as its drain begins there; it counts in `unknown_ahead` of no holder. The start `S` of the
/// backfill of `host_series`, which nothing acts on
pub fn head_start(sight: &Sight, position: usize, wall: &dyn Fn(&TurnEntry) -> Option<u64>) -> StartEstimate {
  let mut play = Play::of(sight, wall);
  if let Some(entry) = sight.turn.queue.get(position)
    && !play.until_admitted(entry)
    && !entry.exclusive
  {
    play.unknown += 1;
  }
  play.estimate()
}

/// The heavy turn of a sight played forward: the holders and the runs admitted on the way with the time of each end
struct Play<'a> {
  load: Load<'a>,
  cores: u32,
  now: u64,
  at: u64,
  running: Vec<(u64, TurnEntry)>,
  /// The holders and the runs whose wall is unknown, and the exclusive runs that cannot start
  unknown: u32,
}

impl<'a> Play<'a> {
  /// The play of `sight` at its time, each holder ending after the wall of its command (`wall`), at once without one
  fn of(sight: &'a Sight, wall: &dyn Fn(&TurnEntry) -> Option<u64>) -> Self {
    let now = sight.now_ms;
    let mut unknown = 0u32;
    let running = sight
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
    Self {
      load: Load {
        outside: sight.outside.unwrap_or_else(|| sight.turn.light_cores()),
        window: sight.window,
        sampled: sight.sampled,
        light: &sight.turn.light,
        now_ms: now,
      },
      cores: sight.cores,
      now,
      at: now,
      running,
      unknown,
    }
  }

  /// Moves the time to the admission of `entry` by the rule of the turn (`host_turn::admits`), holder end by holder end,
  /// or the bound of the load of the host on a free turn. Answers `false` for an exclusive run that a busy host keeps out
  /// once no holder is left: the time is then the end of the last holder
  fn until_admitted(&mut self, entry: &TurnEntry) -> bool {
    loop {
      let at = self.at;
      self.running.retain(|(end, _)| *end > at);
      let holders: Vec<TurnEntry> = self.running.iter().map(|(_, holder)| holder.clone()).collect();
      let waited_out = at.saturating_sub(entry.since_ms) >= millis_of_bound();
      if admits(entry, &holders, &self.load, self.cores, waited_out) {
        return true;
      }
      // the next end of a holder, else the bound of the load of the host on a free turn
      let next = self
        .running
        .iter()
        .map(|(end, _)| *end)
        .min()
        .or_else(|| (!entry.exclusive && !waited_out).then(|| entry.since_ms.saturating_add(millis_of_bound())));
      match next {
        Some(next) if next > at => self.at = next,
        _ => return false,
      }
    }
  }

  const fn estimate(&self) -> StartEstimate {
    StartEstimate {
      start_in_ms: self.at.saturating_sub(self.now),
      unknown_ahead: self.unknown,
    }
  }
}

/// The runs of the queue of `sight` before a new run of `asked`, and the join that its bound of the load counts from:
/// the join itself (`host_turn::join_queue`) on a copy of the table and of its files, so the estimate places a run as
/// the queue does by construction, the runs it supersedes, short runs and the turns of the groups alike
pub(crate) fn place(sight: &Sight, asked: &Asked<'_>) -> (Vec<TurnEntry>, u64) {
  let mut turn = sight.turn.clone();
  let mut files = sight.files.clone();
  let request = TurnRequest {
    run: "",
    pid: 0,
    process_started: None,
    repo: asked.repo,
    command: &[],
    affinity: asked.affinity,
    command_key: asked.command_key,
    expected: asked.expected,
    exclusive: false,
    passed_wall: asked.passed_wall,
    measure_as: None,
    group: Some(asked.group),
  };
  let mut joined = Joined {
    answer: TurnAnswer::Taken,
    replaced: Vec::new(),
    beside: Vec::new(),
    short: None,
    passed: Vec::new(),
    notes: Vec::new(),
  };
  let position = join_queue(&mut turn, &mut files, (sight.cores, sight.drain_bar), sight.now_ms, &request, &mut joined);
  let since_ms = turn.queue[position].since_ms;
  turn.queue.truncate(position);
  (turn.queue, since_ms)
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
    recent: None,
    group: None,
  }
}

fn millis_of_bound() -> u64 {
  u64::try_from(LOAD_BOUND.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "host_estimate_test.rs"]
mod host_estimate_test;
