//! The steps of the host slots that `bd lease host` and the agent of `bd-host` both take, so the two say and do the same
//! (spec/cli/lease.spec.md, "Host slots"; spec/cli/host.spec.md, "serve"): the queue line of a lend that waits, and the
//! reclaim of a tick of the keeper, which takes up the slots and then deletes the refs that no live run uses

use crate::host_refs::reclaim_refs;
use crate::host_slots::{HostSlotsParams, Queued, Reclaimed, live_runs, reclaim};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::time::Duration;

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
