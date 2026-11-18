//! The steps of the host slots that the agent of `brd-host` takes (openspec/specs/cli/lease/spec.md, "Host slots";
//! openspec/specs/cli/host/spec.md, "serve"): the queue line of a lend that waits, the line of a run that waits for the
//! heavy turn of the host (`host_turn`), and the reclaim of a tick of the keeper, which takes up the slots and then
//! deletes the refs that no live run uses

use crate::host_refs::reclaim_refs;
use crate::host_slots::{HostSlotsParams, Queued, Reclaimed, live_runs, reclaim};
use crate::host_turn::{Need, TurnEntry, TurnQueue};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::path::Path;
use std::time::Duration;
use sync_utils::cores::Cores;

/// The queue line of the run `run` that waited `waited` for a host slot: "run 1792-81 waits 60 s: 2 runs ahead, 4 slots,
/// lent to run-a (1), run-b (2), 1 in quarantine"
pub fn queue_line(run: &str, queued: &Queued, waited: Duration) -> String {
  let lent: Vec<String> = queued.lent.iter().map(|(n, run)| format!("{run} ({n})")).collect();
  let quarantined = match queued.quarantined.len() {
    0 => String::new(),
    count => format!(", {count} in quarantine"),
  };
  let upgrading = queued
    .git_upgrading
    .as_ref()
    .map_or_else(String::new, |command| format!(", git is upgrading (`{command}`)"));
  format!(
    "run {run} waits {} s: {} runs ahead, {} slots, lent to {}{quarantined}{upgrading}",
    waited.as_secs(),
    queued.ahead,
    queued.slots,
    if lent.is_empty() { "none".to_string() } else { lent.join(", ") }
  )
}

/// The line of the heavy run `run` that waited `waited` for its cores of the heavy turn of the host, with `left` until
/// it gives up, at `now_ms`: "run 1792-81 waits 3 min for the heavy turn of the host, 7.5 of its 20 cores, 117 min left:
/// run 1788-12 of
/// branch-deck holds 12.0 for 4 min (pnpm e2e, measured), runs that are not heavy use 2.0, 1 run ahead: 1790-03 of idea-1
/// (./community/tools/bt.cmd --lane fast)"
pub fn turn_line(run: &str, queue: &TurnQueue, waited: Duration, left: Duration, now_ms: u64) -> String {
  let holders = if queue.holders.is_empty() {
    "no heavy run holds cores".to_owned()
  } else {
    let holders: Vec<String> = queue
      .holders
      .iter()
      .map(|holder| {
        let (holds, why) = holds_text(holder.need(queue.cores));
        format!(
          "run {} {holds} for {} ({}, {why})",
          run_of_repo(holder),
          span_since(holder.since_ms, now_ms),
          holder.command
        )
      })
      .collect();
    holders.join(", ")
  };
  let light = if queue.light == Cores(0) {
    String::new()
  } else {
    format!(", runs that are not heavy use {}", queue.light)
  };
  let ahead: Vec<String> = queue.ahead.iter().map(|entry| format!("{} ({})", run_of_repo(entry), entry.command)).collect();
  let ahead = match ahead.len() {
    0 => "no run ahead".to_owned(),
    1 => format!("1 run ahead: {}", ahead[0]),
    count => format!("{count} runs ahead: {}", ahead.join(", ")),
  };
  format!(
    "run {run} waits {} for the heavy turn of the host, {}, {} left: {holders}{light}, {ahead}",
    span(waited),
    need_text(queue.need, queue.cores),
    span(left)
  )
}

/// What a run waits for: `7.5 of its 20 cores`, `all its 20 cores, as the command has no measure yet`
pub fn need_text(need: Need, cores: u32) -> String {
  match need {
    Need::Cores { cores: need, .. } => format!("{need} of its {cores} cores"),
    Need::Unmeasured => format!("all its {cores} cores, as the command has no measure yet"),
    Need::Measured(measured) => format!("all its {cores} cores, as the command measured {measured}"),
  }
}

/// What a holder holds and why: (`holds 12.0`, `measured`), (`holds 14.0`, `live, above its measure`), (`holds the
/// whole host`, `no measure`)
pub fn holds_text(need: Need) -> (String, String) {
  match need {
    Need::Cores { cores, live: false } => (format!("holds {cores}"), "measured".to_owned()),
    Need::Cores { cores, live: true } => (format!("holds {cores}"), "live, above its measure".to_owned()),
    Need::Unmeasured => ("holds the whole host".to_owned(), "no measure".to_owned()),
    Need::Measured(measured) => ("holds the whole host".to_owned(), format!("measured {measured}")),
  }
}

/// `1788-12 of branch-deck`: the run and the name of its repository
pub fn run_of_repo(entry: &TurnEntry) -> String {
  let repo = Path::new(&entry.repo)
    .file_name()
    .map_or_else(|| entry.repo.clone(), |name| name.to_string_lossy().into_owned());
  format!("{} of {repo}", entry.ticket.run)
}

/// The time since `since_ms` at `now_ms`, as `span` says it
pub fn span_since(since_ms: u64, now_ms: u64) -> String {
  span(Duration::from_millis(now_ms.saturating_sub(since_ms)))
}

/// A wait for a line: `45 s` up to two minutes, then `4 min`
fn span(duration: Duration) -> String {
  match duration.as_secs() {
    seconds @ 0..120 => format!("{seconds} s"),
    seconds => format!("{} min", seconds / 60),
  }
}

/// What one reclaim of a repository took up
#[derive(Debug, Clone)]
pub struct ReclaimedAll {
  /// The slots it took up (`host_slots::reclaim`)
  pub reclaimed: Vec<Reclaimed>,
  /// The refs it deleted, by their full names (`host_refs::reclaim_refs`)
  pub deleted_refs: Vec<String>,
}

/// Take up what no process acts on in the host slots of the repository (`reclaim`), then delete the refs that no live
/// run uses (`reclaim_refs`, against the runs that hold a lend or a ticket after the reclaim)
pub fn reclaim_all(git_executor: &GitCommandExecutor, params: &HostSlotsParams<'_>) -> Result<ReclaimedAll> {
  let reclaimed = reclaim(git_executor, params)?;
  let live = live_runs(params)?;
  let deleted_refs = reclaim_refs(git_executor, params.repository, &live, params.lock_timeout)?;
  Ok(ReclaimedAll { reclaimed, deleted_refs })
}

#[cfg(test)]
#[path = "host_runs_test.rs"]
mod host_runs_test;
