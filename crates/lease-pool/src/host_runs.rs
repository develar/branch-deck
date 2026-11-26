//! The steps of the host slots that the agent of `brd-host` takes (openspec/specs/cli/lease/spec.md, "Host slots";
//! openspec/specs/cli/host/spec.md, "serve"): the queue line of a lend that waits, the line of a run that waits for the
//! heavy turn of the host (`host_turn`), and the reclaim of a tick of the keeper, which takes up the slots and then
//! deletes the refs that no live run uses

use crate::host_refs::reclaim_refs;
use crate::host_slots::{HostSlotsParams, Queued, Reclaimed, live_runs, reclaim};
use crate::host_turn::{HOLD_GRACE, HeadDrain, Need, TurnEntry, TurnQueue, USE_WINDOW};
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
/// branch-deck holds 12.0 for 4 min (pnpm e2e, measured), the host is busy with 18.3 of 20 cores outside the heavy runs,
/// 1 run ahead: 1790-03 of idea-1 (./community/tools/bt.cmd --lane fast)"; without a sample of the host, the use of the runs
/// that are not heavy instead: "runs that are not heavy use 2.0". A holder that started past an exclusive run says so,
/// an exclusive run ahead is marked, and the end names the drain of an exclusive head of the queue ([`head_drain_text`]).
/// `measure_as` is the class of the measures of the run (`brd-host run --measure-as`), which its need names; the run, a
/// holder and a run ahead of `--group` name their group: "run 1792-81 in the group typing waits …"
pub fn turn_line(run: &str, queue: &TurnQueue, measure_as: Option<&str>, waited: Duration, left: Duration, now_ms: u64) -> String {
  let holders = if queue.holders.is_empty() {
    "no heavy run holds cores".to_owned()
  } else {
    let holders: Vec<String> = queue
      .holders
      .iter()
      .map(|holder| {
        let (holds, why) = holds_text(holder.need(queue.cores, now_ms));
        format!(
          "run {} {holds} for {} ({}, {why}{})",
          run_of_repo(holder),
          span_since(holder.since_ms, now_ms),
          holder.command,
          passed_text(holder)
        )
      })
      .collect();
    holders.join(", ")
  };
  let mut light = match queue.outside {
    Some(outside) => format!(", the host is busy with {outside} of {} cores outside the heavy runs", queue.cores),
    None if queue.light == Cores(0) => String::new(),
    None => format!(", runs that are not heavy use {}", queue.light),
  };
  if queue.need == Need::Exclusive {
    light.push_str(&exclusive_wait_text(queue));
  }
  let ahead: Vec<String> = queue
    .ahead
    .iter()
    .map(|entry| format!("{} ({}{})", run_of_repo(entry), entry.command, if entry.exclusive { ", exclusive" } else { "" }))
    .collect();
  let ahead = match ahead.len() {
    0 => "no run ahead".to_owned(),
    1 => format!("1 run ahead: {}", ahead[0]),
    count => format!("{count} runs ahead: {}", ahead.join(", ")),
  };
  format!(
    "run {run}{} waits {} for the heavy turn of the host, {}, {} left: {holders}{light}, {ahead}{}",
    group_text(queue.group.as_deref()),
    span(waited),
    need_text(queue.need, queue.cores, measure_as),
    span(left),
    head_drain_text(queue, now_ms)
  )
}

/// How often short runs passed a run of the queue: `passed 1 time`, `passed 3 times`
pub fn passes_text(passes: u32) -> String {
  match passes {
    1 => "passed 1 time".to_owned(),
    passes => format!("passed {passes} times"),
  }
}

/// `, past the exclusive run 1788-12` for a holder that started past an exclusive run, else nothing
pub fn passed_text(holder: &TurnEntry) -> String {
  holder.passed.as_ref().map_or_else(String::new, |exclusive| format!(", past the exclusive run {exclusive}"))
}

/// What the drain of the exclusive run at the head of the queue lets pass, for the line of the head itself or of a run
/// behind it: "; the drain of run 1788-12 ended 3 min ago without a quiet host: the heavy runs behind it start past it
/// in their order while they fit"; nothing when the head is no exclusive run
fn head_drain_text(queue: &TurnQueue, now_ms: u64) -> String {
  let Some(drain) = queue.head_drain else { return String::new() };
  let Some(head) = queue.ahead.first() else {
    return match drain {
      HeadDrain::Pending => "; it drains the host once no heavy run holds cores, and no heavy run starts past it before its drain ends".to_owned(),
      HeadDrain::Draining { until_ms } => format!(
        "; it drains the host for {} more: the new runs that are not heavy wait for it, and no heavy run starts past it",
        span(Duration::from_millis(until_ms.saturating_sub(now_ms)))
      ),
      HeadDrain::Ended { ended_ms } => format!(
        "; its drain ended {} ago without a quiet host: the heavy runs behind it that fit start past it, and it starts once the host is quiet",
        span_since(ended_ms, now_ms)
      ),
    };
  };
  let head = &head.ticket.run;
  match drain {
    HeadDrain::Pending => format!("; run {head} drains the host once no heavy run holds cores, and no run starts past it before its drain ends"),
    HeadDrain::Draining { until_ms } => format!(
      "; run {head} drains the host for {} more, and no run starts past it before then",
      span(Duration::from_millis(until_ms.saturating_sub(now_ms)))
    ),
    HeadDrain::Ended { ended_ms } => format!(
      "; the drain of run {head} ended {} ago without a quiet host: the heavy runs behind it start past it in their order while they fit",
      span_since(ended_ms, now_ms)
    ),
  }
}

/// The line of the run `run` that is not heavy and waited `waited` at `now_ms` while the exclusive run `drainer` drains
/// the host, until `until_ms` at most: "run 1792-81 waits 2 min for a measurement: run 1788-12 of idea-1 waits for the
/// host to drain (zsh ./measure-ab.sh); this run starts when the measurement ends, or in at most 3 min"
pub fn drain_line(run: &str, drainer: &TurnEntry, waited: Duration, until_ms: u64, now_ms: u64) -> String {
  format!(
    "run {run} waits {} for a measurement: run {} waits for the host to drain ({}); this run starts when the measurement ends, or in at most {}",
    span(waited),
    run_of_repo(drainer),
    drainer.command,
    span(Duration::from_millis(until_ms.saturating_sub(now_ms)))
  )
}

/// What an exclusive run waits for beyond the holders: each run that is not heavy in its command, and the busy cores of
/// the host over its last 30 s: ", waits for run 1790-03 of accio to end: scripts/bench ab-run, the host was busy with
/// up to 11.2 of 20 cores in the last 30 s"
fn exclusive_wait_text(queue: &TurnQueue) -> String {
  let runs: Vec<String> = queue.light_runs.iter().map(|run| format!("run {} of {}", run.ticket.run, repo_name(&run.repo))).collect();
  let ends = match runs.as_slice() {
    [] => String::new(),
    [one] => format!(", waits for {one} to end: {}", queue.light_runs[0].command),
    _ => {
      let each: Vec<String> = runs.iter().zip(&queue.light_runs).map(|(run, light)| format!("{run} ({})", light.command)).collect();
      format!(", waits for {} runs to end: {}", runs.len(), each.join(", "))
    }
  };
  let window = queue.window.map_or_else(String::new, |window| {
    format!(", the host was busy with up to {window} of {} cores in the last 30 s", queue.cores)
  });
  format!("{ends}{window}")
}

/// What a command that never gets a measure can do, after `no measure`: a command whose text changes every run has a
/// new key of its measures each time
pub const NEVER_MEASURED: &str = "a command whose text changes every run never gets one: `--measure-as <class>` gives it one";

/// What the host does with a run without a measure, after `no measure yet`: it reserves the cores it assumes of one
pub const ASSUMED: &str = "the host reserves the cores it assumes of a run without one until a run of it, or of its program, measures it";

/// What a run of the class `measure_as` (`brd-host run --measure-as`), or of none, waits for: `7.5 of its 20 cores`,
/// `all its 20 cores, as the command has no measure yet (a command whose text changes every run never gets one: …)`,
/// `all its 20 cores, as the class accio-gate has no measure yet`
pub fn need_text(need: Need, cores: u32, measure_as: Option<&str>) -> String {
  match need {
    Need::Cores { cores: need, .. } | Need::Recent { cores: need, .. } => format!("{need} of its {cores} cores"),
    Need::Assumed { cores: need } => match measure_as {
      Some(class) => format!("{need} of its {cores} cores, as the class {class} has no measure yet ({ASSUMED})"),
      None => format!("{need} of its {cores} cores, as the command has no measure yet ({ASSUMED})"),
    },
    Need::Unmeasured => match measure_as {
      Some(class) => format!("all its {cores} cores, as the class {class} has no measure yet"),
      None => format!("all its {cores} cores, as the command has no measure yet ({NEVER_MEASURED})"),
    },
    Need::Measured(measured) => format!("all its {cores} cores, as the command measured {measured}"),
    Need::Exclusive => format!("all its {cores} cores of a quiet host, as an exclusive run"),
  }
}

/// What a holder holds and why: (`holds 12.0`, `measured`), (`holds 14.0`, `live, above its measure`), (`holds the
/// whole host`, `no measure`), (`holds 1.2`, `no measure: it held the whole host, or the cores of a run without one, for its first 10 min, now its live use
/// of the last 5 min`), (`holds 1.2`, `measured 16.0: it held that for its first 10 min, now its live use of the last 5
/// min`), (`holds the whole host for a measurement`, `exclusive`)
pub fn holds_text(need: Need) -> (String, String) {
  match need {
    Need::Cores { cores, live: false } => (format!("holds {cores}"), "measured".to_owned()),
    Need::Cores { cores, live: true } => (format!("holds {cores}"), "live, above its measure".to_owned()),
    Need::Assumed { cores } => (format!("holds {cores}"), format!("no measure: {ASSUMED}, or the most it used since its start")),
    Need::Unmeasured => ("holds the whole host".to_owned(), "no measure".to_owned()),
    Need::Recent { cores, measured: None } => (
      format!("holds {cores}"),
      format!(
        "no measure: it held the whole host, or the cores of a run without one, for its first {}, now its live use of the last {}",
        span(HOLD_GRACE),
        span(USE_WINDOW)
      ),
    ),
    Need::Recent { cores, measured: Some(measured) } => (
      format!("holds {cores}"),
      format!(
        "measured {measured}: it held that for its first {}, now its live use of the last {}",
        span(HOLD_GRACE),
        span(USE_WINDOW)
      ),
    ),
    Need::Measured(measured) => ("holds the whole host".to_owned(), format!("measured {measured}")),
    Need::Exclusive => ("holds the whole host for a measurement".to_owned(), "exclusive".to_owned()),
  }
}

/// The line of the run `run` that is not heavy and waited `waited` at `now_ms` for the end of the exclusive run
/// `holder`: "run 1792-81 waits 3 min for a measurement: run 1788-12 of idea-1 holds the host for 12 min (zsh
/// ./measure-ab.sh); a measurement holds the host; this run starts when it ends"
pub fn measurement_line(run: &str, holder: &TurnEntry, waited: Duration, now_ms: u64) -> String {
  format!(
    "run {run} waits {} for a measurement: run {} holds the host for {} ({}); a measurement holds the host; this run starts when it ends",
    span(waited),
    run_of_repo(holder),
    span_since(holder.since_ms, now_ms),
    holder.command
  )
}

/// `1788-12 of branch-deck`: the run and the name of its repository, and `1788-12 of branch-deck in the group typing` of
/// a run of `--group`
pub fn run_of_repo(entry: &TurnEntry) -> String {
  format!("{} of {}{}", entry.ticket.run, repo_name(&entry.repo), group_text(entry.group.as_deref()))
}

/// ` in the group <group>` for a run of `brd-host run --group`, else nothing: a run without one is of the group of its
/// checkout
pub fn group_text(group: Option<&str>) -> String {
  group.map_or_else(String::new, |group| format!(" in the group {group}"))
}

/// The name of the repository `repo`: its last component
fn repo_name(repo: &str) -> String {
  Path::new(repo).file_name().map_or_else(|| repo.to_owned(), |name| name.to_string_lossy().into_owned())
}

/// The time since `since_ms` at `now_ms`, as `span` says it
pub fn span_since(since_ms: u64, now_ms: u64) -> String {
  span(Duration::from_millis(now_ms.saturating_sub(since_ms)))
}

/// A wait for a line: `45 s` up to two minutes, then `4 min`
pub fn span(duration: Duration) -> String {
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
