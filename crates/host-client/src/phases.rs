//! The steps of a run and their durations (openspec/specs/cli/host/spec.md, "The report of a run"): `pack` and `upload` of the
//! client, the steps of the host from `Reply::Phases` (`git`, `slot`, `pack_wait`, `checkout`, `prepare`, `command`,
//! `stage`), then `fetch` and `total` of the client. A run that ends early has only the steps up to its last one. And one
//! run of `brd-host runs` for a person (`run_line`)

use host_link::proto::{Phase, RunInfo};
use host_link::refusal::on_host;
use jiff::Timestamp;
use jiff::tz::TimeZone;
use std::fmt::Write as _;
use std::time::Duration;

/// The steps of a run that a list shows: each step of 0.1 s or more
const SHORT_FROM_MS: u64 = 100;

/// The line of the steps of a run on the stderr: `brd-host: phases: pack 3.1s, upload 2.0s, … total 10m13s`
pub fn phases_line(phases: &[Phase]) -> String {
  format!("brd-host: phases: {}", join(phases.iter()))
}

/// The steps of a run in a short form for a list: `slot 9m02s, checkout 41.0s, command 25.1s`, without the steps under
/// 0.1 s. Empty when no step is that long
pub fn phases_short(phases: &[Phase]) -> String {
  join(phases.iter().filter(|phase| phase.ms >= SHORT_FROM_MS))
}

/// A duration for a person, rounded: `3.1s` under a minute, `9m02s` under an hour, then `1h05m09s`
pub fn duration_text(ms: u64) -> String {
  let tenths = (ms + 50) / 100;
  if tenths < 600 {
    return format!("{}.{}s", tenths / 10, tenths % 10);
  }
  let seconds = (ms + 500) / 1000;
  match seconds {
    0..3600 => format!("{}m{:02}s", seconds / 60, seconds % 60),
    _ => format!("{}h{:02}m{:02}s", seconds / 3600, seconds / 60 % 60, seconds % 60),
  }
}

/// The lines of the runs of `brd-host runs`, each start in the time zone of the system (`run_line`)
pub fn run_lines(runs: &[RunInfo], alias: &str) -> Vec<String> {
  run_lines_in(runs, alias, &TimeZone::system())
}

/// The lines of `runs` in `zone`: each run ([`run_line`]), and after a run that a refusal ended a line with its code and
/// its message, the alias of the host in it (`superseded: run … was superseded by run …: … follow …`)
pub fn run_lines_in(runs: &[RunInfo], alias: &str, zone: &TimeZone) -> Vec<String> {
  runs
    .iter()
    .flat_map(|run| {
      let refused = run.refused.as_ref().map(|refused| format!("    {}: {}", refused.code, on_host(&refused.message, alias)));
      std::iter::once(run_line(run, zone)).chain(refused)
    })
    .collect()
}

/// One run of `brd-host runs`: its id, its state with its start in `zone` (and for a finished run its exit and how long
/// it took), its affinity, its commit, its command, and the steps of 0.1 s or more. A finished run that its client did
/// not forget still has its files on the host: `brd-host fetch` copies them
///
/// `1-1  running, started 2026-11-16 14:02:11  checkout-…  86551e413c53  cargo test`
/// `1-1  finished, exit 0, started 2026-11-16 14:02:11, took 13m21s, cores 7.5 mean, 14.0 peak  checkout-…  86551e413c53
/// cargo test  (command 13m20s)`
pub fn run_line(run: &RunInfo, zone: &TimeZone) -> String {
  let started = Timestamp::from_millisecond(i64::try_from(run.started_ms).unwrap_or(i64::MAX)).map_or_else(
    |_| "at an unknown time".to_owned(),
    |at| at.to_zoned(zone.clone()).strftime("%Y-%m-%d %H:%M:%S").to_string(),
  );
  let mut state = match (run.exit, &run.refused) {
    (None, _) => format!("running, started {started}"),
    (Some(_), Some(refused)) => format!("finished, refused {}, started {started}", refused.code),
    (Some(code), None) => format!("finished, exit {code}, started {started}"),
  };
  if let Some(ended) = run.ended_ms {
    let _ = write!(state, ", took {}", duration_text(ended.saturating_sub(run.started_ms)));
  }
  if let Some(cores) = run.cores {
    let so_far = if run.exit.is_none() { " so far" } else { "" };
    let _ = write!(state, ", cores {} mean, {} peak{so_far}", cores.mean, cores.peak);
  }
  if run.exit.is_some() && !run.forgotten {
    state.push_str(", its files kept");
  }
  let mut line = format!("{}  {state}  {}  {}  {}", run.run, run.affinity, short_id(&run.commit), run.command.join(" "));
  let phases = phases_short(&run.phases);
  if !phases.is_empty() {
    let _ = write!(line, "  ({phases})");
  }
  line
}

fn short_id(id: &str) -> &str {
  id.get(..12).unwrap_or(id)
}

/// One step and its duration
pub(crate) fn phase(name: &str, took: Duration) -> Phase {
  Phase {
    name: name.to_owned(),
    ms: u64::try_from(took.as_millis()).unwrap_or(u64::MAX),
  }
}

fn join<'a>(phases: impl Iterator<Item = &'a Phase>) -> String {
  phases.map(|phase| format!("{} {}", phase.name, duration_text(phase.ms))).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
#[path = "phases_test.rs"]
mod phases_test;
