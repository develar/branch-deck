//! The host slots that a run borrows from the pool of Branch Deck, in process (`lease_pool::host_slots`;
//! spec/cli/lease.spec.md, "Host slots"). A remote slot holds no state of its own: the commit comes from the client, and
//! the stage carries the outputs back. So a slot is lent to one run for that run only, and the host keeps a fixed set of
//! warm slots (`branchdeck.hostSlots`, 4 by default) that runs share.
//!
//! - A supervisor lends a slot for its run ([`lend`], with its own pid: the lend lasts as long as the supervisor). A
//!   wait while the slots are lent to other runs goes in short lends of the pool, so a cancel ends it, and the queue
//!   line goes to the log of the run once a minute (`lease_pool::host_runs::queue_line`).
//! - On every way out the supervisor gives the slot back ([`give_back`]), which also takes a run that got none out of
//!   the queue, deletes the ref of its pack, and starts the deferred git maintenance under `branchdeck.leaseMaintenance`.
//! - [`checkout`] moves the slot of a run to its commit by the diff of the two commits.
//! - The keeper of `serve` reclaims and fills the slots of each repository (`keeper`)

use crate::repo::{Repository, pool_refusal};
use host_link::refusal::{Code, Refusal};
use lease_pool::host_runs::queue_line;
use lease_pool::host_slots::{
  CheckedOut, HostProcess, Lend, LendAnswer, LendRequest, Queued, WAIT_PROGRESS, checkout as checkout_slot, give_back_run, lend as lend_slot, lend_waiting,
};
use std::time::{Duration, Instant};
use sync_utils::process_scan::process_started;

/// The longest lend of the pool within a wait: a cancel ends the wait after it
const STOP_CHECK: Duration = Duration::from_secs(2);

/// Lends a slot of the pool of `repo` to `run`, which this process runs, with the affinity `affinity`. With `wait`, the
/// call waits up to that long while the run is queued, and answers the place in the queue when the wait ends; without
/// it, it answers at once. Each minute of a wait says the queue line through `say`. A git of the host below its floor
/// refuses with `old_git`, another failure with `lease_failed`. Answers `None` when `stop` ended the wait: the give-back
/// at the end of the run takes its ticket out of the queue
pub(crate) fn lend(
  repo: &Repository,
  run: &str,
  affinity: &str,
  wait: Option<Duration>,
  stop: &dyn Fn() -> bool,
  say: &mut dyn FnMut(&str),
) -> Result<Option<LendAnswer>, Refusal> {
  let pid = std::process::id();
  let request = LendRequest {
    run,
    pid,
    process_started: process_started(pid),
    affinity: Some(affinity),
  };
  let params = repo.params();
  let refused = |error: anyhow::Error| pool_refusal(Code::LeaseFailed, &error);
  let Some(wait) = wait else {
    return lend_slot(&params, &request).map(Some).map_err(refused);
  };
  let started = Instant::now();
  let mut said = started;
  loop {
    if stop() {
      return Ok(None);
    }
    let chunk = wait.saturating_sub(started.elapsed()).min(STOP_CHECK);
    let queued = match lend_waiting(&repo.git, &params, &request, chunk, &|_: &Queued, _: Duration| {}).map_err(refused)? {
      LendAnswer::Lent(lent) => return Ok(Some(LendAnswer::Lent(lent))),
      LendAnswer::Queued(queued) => queued,
    };
    let waited = started.elapsed();
    if waited >= wait {
      return Ok(Some(LendAnswer::Queued(queued)));
    }
    if said.elapsed() >= WAIT_PROGRESS {
      say(&queue_line(run, &queued, waited));
      said = Instant::now();
    }
  }
}

/// What a give-back did, for the log of the supervisor
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct GaveBack {
  /// The slot given back; `None` when the run had none, and then it left the queue
  pub(crate) n: Option<u32>,
  /// The processes of the run that the give-back stopped
  pub(crate) stopped: usize,
  /// The processes that keep the slot in quarantine
  pub(crate) quarantined: Vec<String>,
  /// The ref of the pack of the run that the give-back deleted
  pub(crate) deleted_ref: Option<String>,
  /// Whether the give-back started the deferred git maintenance of the repository
  pub(crate) maintenance_started: bool,
}

/// Gives back the slot of `run`, or takes the run out of the queue when it has none. Nothing stops it, so a signal to
/// the supervisor does not leave the slot half given back
pub(crate) fn give_back(repo: &Repository, run: &str) -> Result<GaveBack, Refusal> {
  let gave_back = give_back_run(&repo.git, &repo.params(), run).map_err(|error| Refusal::new(Code::LeaseFailed, format!("{error:#}")))?;
  Ok(GaveBack {
    n: gave_back.slot.as_ref().map(|slot| slot.n),
    stopped: gave_back.slot.as_ref().map_or(0, |slot| slot.stopped.len()),
    quarantined: gave_back.slot.as_ref().map_or_else(Vec::new, |slot| slot.quarantined.iter().map(describe).collect()),
    deleted_ref: gave_back.deleted_ref,
    maintenance_started: gave_back.maintenance_started,
  })
}

/// A process of a slot for a line of the log: `412 (cargo build)`
pub(crate) fn describe(process: &HostProcess) -> String {
  let what = if process.command.is_empty() { &process.name } else { &process.command };
  format!("{} ({what})", process.pid)
}

/// Moves the slot of `run` to `commit`, detached, by the diff of the two commits: the give-back of the last run left the
/// slot clean, so the move writes only the diff and runs no `status`. A failure is `checkout_failed`
pub(crate) fn checkout(repo: &Repository, run: &str, commit: &str) -> Result<CheckedOut, Refusal> {
  checkout_slot(&repo.git, &repo.params(), run, commit).map_err(|error| pool_refusal(Code::CheckoutFailed, &error))
}

/// The slot of a lend for a line of the run: `slot 3 of idea-1: <path> (last used by this checkout 12 min ago)`
pub(crate) fn lent_line(lent: &Lend, repo: &Repository, now_ms: u64) -> String {
  let used = lent.last_used_by_affinity.map_or_else(
    || "new to this checkout".to_owned(),
    |ms| format!("last used by this checkout {} min ago", now_ms.saturating_sub(ms) / 60_000),
  );
  format!("slot {} of {}: {} ({used})", lent.n, repo_name(repo), lent.path)
}

/// `waiting for a host slot of idea-1: 4 of 4 lent (1792-81, …), 1 run(s) ahead`, and the slots in quarantine
pub(crate) fn waiting_line(queued: &Queued, repo: &Repository) -> String {
  let runs: Vec<&str> = queued.lent.iter().map(|(_, run)| run.as_str()).collect();
  let mut line = format!(
    "waiting for a host slot of {}: {} of {} lent ({}), {} run(s) ahead",
    repo_name(repo),
    queued.lent.len(),
    queued.slots,
    runs.join(", "),
    queued.ahead
  );
  for (n, processes) in &queued.quarantined {
    let processes: Vec<String> = processes.iter().map(describe).collect();
    line.push_str("; slot ");
    line.push_str(&n.to_string());
    line.push_str(" in quarantine while these work in it: ");
    line.push_str(&processes.join(", "));
  }
  if queued.slots == 0 {
    line.push_str("; the host prepares its slots");
  }
  line
}

fn repo_name(repo: &Repository) -> String {
  repo
    .path
    .file_name()
    .map_or_else(|| repo.path.display().to_string(), |name| name.to_string_lossy().into_owned())
}
