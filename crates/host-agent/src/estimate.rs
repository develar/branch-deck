//! `Request::Estimate`: what a run would meet on this host now, the hint of a client of a set of hosts before it
//! chooses a member (openspec/specs/cli/host/spec.md, "serve: An estimate says when a run would start there";
//! docs/design/host-runs.md, "Sets of hosts"). Read without a lock of the turn, and nothing changes.
//!
//! The measures of the command on this host give the cores and the wall that its run is expected to take; a heavy run
//! takes its place in the heavy turn as a new run would (a short one ahead of the runs that it may pass), and the
//! queue plays forward by the walls of the commands of
//! the runs there (`lease_pool::host_estimate`), whose commands the specs of their run directories name. A run that is
//! not heavy starts at once, but while an exclusive run holds the host or drains it

use crate::home::Home;
use crate::log::log;
use crate::repo::Repository;
use crate::run::{Runs, epoch_ms};
use crate::tuning;
use git_ops::revs::is_object_id;
use host_link::heavy::{measure_key, run_key};
use host_link::proto::{Estimate, EstimateAsk, Reply};
use lease_pool::host_estimate::{Asked, Sight, StartEstimate, estimate};
use lease_pool::host_slots::read_host_slots;
use lease_pool::host_turn::{Bar, TurnEntry, bar, group_key};
use std::path::Path;
use std::time::SystemTime;

/// The estimate of the run of `ask` in `repo` of the agent of `home`
pub(crate) fn of_run(home: &Home, repo: &Repository, ask: &EstimateAsk) -> Reply {
  let dir = home.heavy_dir();
  let path = repo.path.display().to_string();
  let cores = crate::supervise::host_cores(repo);
  let key = measure_key(&ask.command, &ask.fetch, ask.measure_as.as_deref());
  // a table of the measures that cannot be read has none: the run takes the whole host, as before any measure
  let expected_cores = lease_pool::host_cores::expected(&dir, &path, &key).unwrap_or_else(|error| {
    log(&format!("cannot read the cores of the commands for an estimate: {error:#}"));
    None
  });
  let runs = Runs::new(home.runs_dir());
  let wall_of_entry = |entry: &TurnEntry| entry_wall(&runs, &dir, entry);
  let now = epoch_ms(SystemTime::now());
  let start = if ask.heavy {
    let group = group_key(ask.group.as_deref(), &ask.affinity, "");
    let asked = Asked {
      repo: &path,
      affinity: &ask.affinity,
      command_key: &run_key(&ask.command),
      expected: expected_cores,
      passed_wall: lease_pool::host_cores::passed_wall(&dir, &path, &key).ok().flatten(),
      group,
    };
    match Sight::of(&dir, cores, now, tuning::drain_bar()) {
      Ok(sight) => {
        for note in &sight.files.notes {
          log(note);
        }
        estimate(&sight, &asked, &wall_of_entry)
      }
      Err(error) => {
        log(&format!("cannot read the heavy turn for an estimate: {error:#}"));
        StartEstimate::default()
      }
    }
  } else {
    light_start(&dir, now, &wall_of_entry)
  };
  Reply::Estimate(Estimate {
    has_commit: is_object_id(&ask.commit) && repo.has_commit(&ask.commit).unwrap_or(false),
    warm: read_host_slots(repo.common_dir()).is_ok_and(|table| table.slots.iter().any(|slot| slot.affinity.as_deref() == Some(ask.affinity.as_str()))),
    cores,
    expected_cores,
    expected_wall_ms: wall(&dir, &path, &key),
    start_in_ms: start.start_in_ms,
    unknown_ahead: start.unknown_ahead,
    os: std::env::consts::OS.to_owned(),
    slots: crate::slots::effective_slots(repo),
  })
}

/// When a run that is not heavy would start at `now_ms`: at once, but after the end of an exclusive run that holds the
/// host (unknown without its wall) or of the drain of one
fn light_start(dir: &Path, now_ms: u64, wall: &dyn Fn(&TurnEntry) -> Option<u64>) -> StartEstimate {
  match bar(dir, now_ms, tuning::drain_bar()) {
    None => StartEstimate::default(),
    Some(Bar::Drains { until_ms, .. }) => StartEstimate {
      start_in_ms: until_ms.saturating_sub(now_ms),
      unknown_ahead: 0,
    },
    Some(Bar::Holds(holder)) => wall(&holder).map_or(StartEstimate { start_in_ms: 0, unknown_ahead: 1 }, |wall| StartEstimate {
      start_in_ms: holder.since_ms.saturating_add(wall).saturating_sub(now_ms),
      unknown_ahead: 0,
    }),
  }
}

/// The key of the measures of the command of `entry`, an entry of the heavy turn, by the spec of its run in `runs`
fn entry_key(runs: &Runs, entry: &TurnEntry) -> Option<String> {
  let spec = runs.get(&entry.ticket.run).ok()?.spec().ok()?;
  Some(measure_key(&spec.command, &spec.fetch, spec.measure_as.as_deref()))
}

/// The wall in milliseconds of the command of `entry`, an entry of the heavy turn in `dir`, by its last runs ([`wall`])
pub(crate) fn entry_wall(runs: &Runs, dir: &Path, entry: &TurnEntry) -> Option<u64> {
  wall(dir, &entry.repo, &entry_key(runs, entry)?)
}

/// The wall in milliseconds of the command of `entry` by its last passed runs, of 3 at least
/// (`host_cores::passed_wall`); `None` without them
pub(crate) fn entry_passed_wall(runs: &Runs, dir: &Path, entry: &TurnEntry) -> Option<u64> {
  let wall = lease_pool::host_cores::passed_wall(dir, &entry.repo, &entry_key(runs, entry)?).ok().flatten()?;
  Some(u64::try_from(wall.as_millis()).unwrap_or(u64::MAX))
}

/// The wall in milliseconds that the last runs of `command_key` in `repo` lead to expect: of the passed runs
/// (`host_cores::passed_wall`), which the queue reads too, else of every measure (`host_cores::measured_wall`), whose
/// failed and cancelled runs count; `None` without either
fn wall(dir: &Path, repo: &str, command_key: &str) -> Option<u64> {
  let wall = lease_pool::host_cores::passed_wall(dir, repo, command_key)
    .ok()
    .flatten()
    .or_else(|| lease_pool::host_cores::measured_wall(dir, repo, command_key).ok().flatten())?;
  Some(u64::try_from(wall.as_millis()).unwrap_or(u64::MAX))
}
