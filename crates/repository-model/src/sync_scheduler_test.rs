use crate::sync_scheduler::{RunEnd, Settled, Step, SyncScheduler};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::session::{SyncOutcome, SyncReason};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use sync_core::sync_inputs::SyncInputs;

const SYNCED: Settled = Ok(SyncOutcome::Synced);
const OK: RunEnd = RunEnd::Succeeded { retry: false };

fn run(run: u64, inputs: Option<SyncInputs>, retry: bool) -> Step {
  Step::Run { run, inputs, retry }
}

/// The inputs of the tests, `rest` apart: no remote branch of the prefix
fn inputs(rest: u64) -> SyncInputs {
  SyncInputs {
    rest,
    remote_refs: BTreeMap::new(),
  }
}

/// A scheduler whose first sync read the inputs `f1`
fn synced_once() -> SyncScheduler<&'static str> {
  let mut scheduler = SyncScheduler::new();
  assert_eq!(scheduler.request(SyncReason::Open, "open", true).next, run(1, None, false));
  let decision = scheduler.run_finished(1, Some(inputs(1)), OK, true);
  assert_eq!(decision.resolve, [("open", SYNCED)]);
  assert_eq!(decision.next, Step::Nothing);
  scheduler
}

/// A request while a run is in flight asks for one quiet follow-up, which serves every such request; the ticket of the
/// first resolves when its own run ends, not after the follow-up
#[test]
fn a_ticket_resolves_when_the_run_that_serves_it_ends() {
  let mut scheduler = synced_once();
  assert_eq!(scheduler.request(SyncReason::Manual, "manual", true).next, run(2, None, false));
  assert_eq!(scheduler.request(SyncReason::AfterRewrite, "rewrite", true).next, Step::Nothing);
  assert_eq!(scheduler.request(SyncReason::Manual, "again", true).next, Step::Nothing);
  // a window that opens meanwhile is served by the run in flight
  assert_eq!(scheduler.request(SyncReason::Open, "window", true).next, Step::Nothing);

  let decision = scheduler.run_finished(2, Some(inputs(2)), OK, true);
  assert_eq!(decision.resolve, [("manual", SYNCED), ("window", SYNCED)]);
  assert_eq!(decision.next, run(3, None, false), "one follow-up for both requests");

  let decision = scheduler.run_finished(3, Some(inputs(3)), OK, true);
  assert_eq!(decision.resolve, [("rewrite", SYNCED), ("again", SYNCED)]);
  assert_eq!(decision.next, Step::Nothing);
  assert!(scheduler.is_idle());
}

/// Focuses while a run is in flight cost one check after the chain, and no run when the chain saw the change
#[test]
fn a_focus_during_a_run_checks_the_inputs_once_after_the_chain() {
  let mut scheduler = synced_once();
  assert_eq!(scheduler.request(SyncReason::Manual, "manual", true).next, run(2, None, false));
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::Nothing);
  assert_eq!(scheduler.request(SyncReason::Focus, "focus again", true).next, Step::Nothing);
  assert_eq!(scheduler.request(SyncReason::AfterRewrite, "rewrite", true).next, Step::Nothing);

  let decision = scheduler.run_finished(2, Some(inputs(2)), OK, true);
  assert_eq!(decision.resolve, [("manual", SYNCED)]);
  assert_eq!(decision.next, run(3, None, false), "the follow-up comes before the check");
  let decision = scheduler.run_finished(3, Some(inputs(3)), OK, true);
  assert_eq!(decision.resolve, [("rewrite", SYNCED)]);
  assert_eq!(decision.next, Step::CheckInputs, "one check for both focuses");

  let decision = scheduler.inputs_read(Some(inputs(3)), true);
  assert_eq!(decision.resolve, [("focus", Ok(SyncOutcome::Unchanged)), ("focus again", Ok(SyncOutcome::Unchanged))]);
  assert_eq!(decision.next, Step::Nothing);
}

/// The first run of a session that opens (its own `Open`, before any window) and a focus while it runs are one run: the
/// focus checks the inputs once after it, the open of a window meanwhile is served by it
#[test]
fn a_focus_during_the_first_run_of_an_opened_session_checks_once_after_it() {
  let mut scheduler = SyncScheduler::new();
  assert_eq!(scheduler.request(SyncReason::Open, "opened", false).next, run(1, None, false));
  assert_eq!(scheduler.request(SyncReason::Open, "window", true).next, Step::Nothing);
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::Nothing);

  let decision = scheduler.run_finished(1, Some(inputs(1)), OK, true);
  assert_eq!(decision.resolve, [("opened", SYNCED), ("window", SYNCED)]);
  assert_eq!(decision.next, Step::CheckInputs);
  let decision = scheduler.inputs_read(Some(inputs(1)), true);
  assert_eq!(decision.resolve, [("focus", Ok(SyncOutcome::Unchanged))]);
  assert_eq!(decision.next, Step::Nothing, "no second run");
}

/// A focus syncs only when the inputs changed since the last successful sync, and the run takes the inputs the check
/// read; a request made during the check is served by that run
#[test]
fn a_focus_runs_only_when_the_inputs_changed() {
  let mut scheduler = synced_once();
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  let decision = scheduler.inputs_read(Some(inputs(1)), true);
  assert_eq!(decision.resolve, [("focus", Ok(SyncOutcome::Unchanged))]);

  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  assert_eq!(scheduler.request(SyncReason::Open, "window", true).next, Step::Nothing);
  assert_eq!(scheduler.request(SyncReason::AfterRewrite, "rewrite", true).next, Step::Nothing);
  let decision = scheduler.inputs_read(Some(inputs(2)), true);
  assert_eq!(decision.resolve.len(), 0);
  assert_eq!(decision.next, run(2, Some(inputs(2)), false));
  let decision = scheduler.run_finished(2, Some(inputs(2)), OK, true);
  assert_eq!(decision.resolve, [("focus", SYNCED), ("window", SYNCED), ("rewrite", SYNCED)]);

  // inputs git could not read run
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  assert_eq!(scheduler.inputs_read(None, true).next, run(3, None, false));
}

/// A stamp of `bd` is a focus no window gained: it runs only when the inputs changed, a stamp while a run is in flight
/// checks once after the chain, and a session no window shows runs nothing
#[test]
fn a_stamp_of_bd_runs_only_when_the_inputs_changed_as_a_focus() {
  let mut scheduler = synced_once();
  assert_eq!(scheduler.request(SyncReason::Stamp, "stamp", true).next, Step::CheckInputs);
  assert_eq!(scheduler.inputs_read(Some(inputs(1)), true).resolve, [("stamp", Ok(SyncOutcome::Unchanged))]);

  assert_eq!(scheduler.request(SyncReason::Stamp, "stamp", true).next, Step::CheckInputs);
  assert_eq!(scheduler.inputs_read(Some(inputs(2)), true).next, run(2, Some(inputs(2)), false));
  assert_eq!(scheduler.request(SyncReason::Stamp, "stamp during the run", true).next, Step::Nothing);
  let decision = scheduler.run_finished(2, Some(inputs(2)), OK, true);
  assert_eq!(decision.resolve, [("stamp", SYNCED)]);
  assert_eq!(decision.next, Step::CheckInputs, "one check after the chain");
  assert_eq!(scheduler.inputs_read(Some(inputs(2)), true).resolve, [("stamp during the run", Ok(SyncOutcome::Unchanged))]);

  let decision = scheduler.request(SyncReason::Stamp, "unwatched", false);
  assert_eq!(decision.resolve, [("unwatched", Ok(SyncOutcome::Skipped))]);
  assert!(scheduler.is_idle());
}

/// A run whose report asks for a retry runs once more, silently, for the same requests; a second ask is not retried
#[test]
fn a_run_that_asks_for_a_retry_is_retried_once() {
  let mut scheduler = synced_once();
  assert_eq!(scheduler.request(SyncReason::Manual, "manual", true).next, run(2, None, false));
  let decision = scheduler.run_finished(2, Some(inputs(2)), RunEnd::Succeeded { retry: true }, true);
  assert_eq!(decision.resolve.len(), 0, "the request waits for the retry");
  assert_eq!(decision.next, run(3, None, true));
  let decision = scheduler.run_finished(3, Some(inputs(3)), RunEnd::Succeeded { retry: true }, true);
  assert_eq!(decision.resolve, [("manual", SYNCED)]);
  assert_eq!(decision.next, Step::Nothing);
}

/// A failed or deferred run settles its requests with the error, and is what a later check compares with: inputs other
/// than it read run, also the inputs of the last successful sync; the end of a run that is not in flight changes nothing
#[test]
fn a_failed_run_settles_its_requests_with_the_error() {
  let mut scheduler = synced_once();
  assert_eq!(scheduler.request(SyncReason::Manual, "manual", true).next, run(2, None, false));
  assert_eq!(scheduler.run_finished(1, Some(inputs(0)), OK, true).next, Step::Nothing);
  let deferred = ApiError::new(ErrorCode::OperationInProgress, "Not synced: rebase is in progress");
  let decision = scheduler.run_finished(2, Some(inputs(2)), RunEnd::Failed(deferred.clone()), true);
  assert_eq!(decision.resolve, [("manual", Err(deferred))]);
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  assert_eq!(scheduler.inputs_read(Some(inputs(1)), true).next, run(3, Some(inputs(1)), false));
}

/// A scheduler whose first run read `inputs(1)` and failed with `error`
fn failed_once(error: &ApiError) -> SyncScheduler<&'static str> {
  let mut scheduler = SyncScheduler::new();
  assert_eq!(scheduler.request(SyncReason::Open, "open", true).next, run(1, None, false));
  let decision = scheduler.run_finished(1, Some(inputs(1)), RunEnd::Failed(error.clone()), true);
  assert_eq!(decision.resolve, [("open", Err(error.clone()))]);
  scheduler
}

/// A failed sync of a large repository is not repeated on every focus: a focus whose inputs are the ones the failed run
/// read answers its error after one cheap check, also the focuses made while a run was in flight
#[test]
fn a_focus_after_a_failed_run_with_the_same_inputs_answers_its_error_without_a_run() {
  let error = ApiError::internal("broken");
  let mut scheduler = failed_once(&error);
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  let decision = scheduler.inputs_read(Some(inputs(1)), true);
  assert_eq!(decision.resolve, [("focus", Err(error.clone()))]);
  assert_eq!(decision.next, Step::Nothing);
  assert!(scheduler.is_idle());

  // ⌘R runs; a focus during it checks once after it, against what that run read
  assert_eq!(scheduler.request(SyncReason::Manual, "manual", true).next, run(2, None, false));
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::Nothing);
  let decision = scheduler.run_finished(2, Some(inputs(1)), RunEnd::Failed(error.clone()), true);
  assert_eq!(decision.resolve, [("manual", Err(error.clone()))]);
  assert_eq!(decision.next, Step::CheckInputs);
  assert_eq!(scheduler.inputs_read(Some(inputs(1)), true).resolve, [("focus", Err(error))]);
}

/// A change of what a sync reads (a commit, the end of a rebase) runs after a failed run; so do inputs git could not read
#[test]
fn a_focus_after_a_failed_run_runs_when_the_inputs_changed() {
  let deferred = ApiError::new(ErrorCode::OperationInProgress, "Not synced: a rebase is in progress");
  let mut scheduler = failed_once(&deferred);
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  assert_eq!(scheduler.inputs_read(Some(inputs(2)), true).next, run(2, Some(inputs(2)), false));
  let decision = scheduler.run_finished(2, Some(inputs(2)), OK, true);
  assert_eq!(decision.resolve, [("focus", SYNCED)]);

  let mut scheduler = failed_once(&deferred);
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  assert_eq!(scheduler.inputs_read(None, true).next, run(2, None, false));
}

/// An open of the repository syncs it again after a failed run: at once, and also when it joins the check of a focus
#[test]
fn an_open_after_a_failed_run_runs_again_also_joining_a_focus_check() {
  let error = ApiError::internal("broken");
  let mut scheduler = failed_once(&error);
  assert_eq!(scheduler.request(SyncReason::Open, "window", true).next, run(2, None, false));
  scheduler.run_finished(2, Some(inputs(1)), RunEnd::Failed(error.clone()), true);

  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  assert_eq!(scheduler.request(SyncReason::Open, "window", true).next, Step::Nothing);
  let decision = scheduler.inputs_read(Some(inputs(1)), true);
  assert_eq!(decision.resolve.len(), 0);
  assert_eq!(decision.next, run(3, Some(inputs(1)), false));
  let decision = scheduler.run_finished(3, Some(inputs(1)), OK, true);
  assert_eq!(decision.resolve, [("focus", SYNCED), ("window", SYNCED)]);

  // after a success an open checks as a focus does
  assert_eq!(scheduler.request(SyncReason::Open, "window", true).next, Step::CheckInputs);
  assert_eq!(scheduler.inputs_read(Some(inputs(1)), true).resolve, [("window", Ok(SyncOutcome::Unchanged))]);
}

/// A run under a prefix the git config left (`prefix_changed`) is no outcome of the session: the last run stays what a
/// check compares with
#[test]
fn a_prefix_change_is_not_remembered() {
  let mut scheduler = synced_once();
  assert_eq!(scheduler.request(SyncReason::Manual, "manual", true).next, run(2, None, false));
  let changed = ApiError::new(ErrorCode::PrefixChanged, "The branch prefix of the repository is \"b\" now");
  scheduler.run_finished(2, Some(inputs(2)), RunEnd::Failed(changed), true);
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  assert_eq!(scheduler.inputs_read(Some(inputs(1)), true).resolve, [("focus", Ok(SyncOutcome::Unchanged))]);
}

/// A focus of a session no window shows runs nothing, also the check of a focus made before the last window left
#[test]
fn a_focus_without_subscribers_runs_nothing() {
  let mut scheduler = synced_once();
  let decision = scheduler.request(SyncReason::Focus, "focus", false);
  assert_eq!(decision.resolve, [("focus", Ok(SyncOutcome::Skipped))]);
  assert_eq!(decision.next, Step::Nothing);

  assert_eq!(scheduler.request(SyncReason::Manual, "manual", true).next, run(2, None, false));
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::Nothing);
  let decision = scheduler.run_finished(2, Some(inputs(2)), OK, false);
  assert_eq!(decision.resolve, [("manual", SYNCED), ("focus", Ok(SyncOutcome::Skipped))]);
  assert_eq!(decision.next, Step::Nothing);
  assert!(scheduler.is_idle());
}

/// The remote-tracking ref of `auth` at `tip`, beside the rest `rest`
fn pushed_inputs(rest: u64, tip: &str) -> SyncInputs {
  let mut pushed = inputs(rest);
  pushed.set_remote_ref(AUTH.to_string(), Some(tip.to_string()));
  pushed
}

const AUTH: &str = "refs/remotes/origin/test/virtual/auth";

/// A push of the session moves the remote-tracking refs of what it sent, which the table has as the push's outcome: the
/// scheduler replaces them in what the last sync read, so a focus after the push runs nothing (C4). A push that ended
/// while a check or a run read the inputs, maybe before it moved them, goes into what it read; anything else changed
/// still runs
#[test]
fn a_focus_after_the_sessions_own_push_runs_nothing() {
  let mut scheduler = synced_once();
  scheduler.pushed(vec![(AUTH.to_string(), Some("pushed".to_string()))]);
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  let decision = scheduler.inputs_read(Some(pushed_inputs(1, "pushed")), true);
  assert_eq!(decision.resolve, [("focus", Ok(SyncOutcome::Unchanged))]);

  assert_eq!(scheduler.request(SyncReason::Manual, "manual", true).next, run(2, None, false));
  scheduler.pushed(vec![(AUTH.to_string(), Some("pushed again".to_string()))]);
  let decision = scheduler.run_finished(2, Some(pushed_inputs(1, "pushed")), OK, true);
  assert_eq!(decision.resolve, [("manual", SYNCED)]);
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  scheduler.pushed(vec![(AUTH.to_string(), None)]);
  let decision = scheduler.inputs_read(Some(pushed_inputs(1, "pushed again")), true);
  assert_eq!(
    decision.resolve,
    [("focus", Ok(SyncOutcome::Unchanged))],
    "the check read before the push removed the branch"
  );

  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  assert_eq!(scheduler.inputs_read(Some(inputs(2)), true).next, run(3, Some(inputs(2)), false));
}
