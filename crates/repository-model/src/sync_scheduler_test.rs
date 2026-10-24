use crate::sync_scheduler::{RunEnd, Settled, Step, SyncScheduler};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::session::{SyncOutcome, SyncReason};
use pretty_assertions::assert_eq;

const SYNCED: Settled = Ok(SyncOutcome::Synced);
const OK: RunEnd = RunEnd::Succeeded { retry: false };

fn run(run: u64, inputs: Option<&str>, retry: bool) -> Step {
  Step::Run {
    run,
    inputs: inputs.map(str::to_string),
    retry,
  }
}

/// A scheduler whose first sync read the inputs `f1`
fn synced_once() -> SyncScheduler<&'static str> {
  let mut scheduler = SyncScheduler::new();
  assert_eq!(scheduler.request(SyncReason::Open, "open", true).next, run(1, None, false));
  let decision = scheduler.run_finished(1, Some("f1".into()), OK, true);
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

  let decision = scheduler.run_finished(2, Some("f2".into()), OK, true);
  assert_eq!(decision.resolve, [("manual", SYNCED), ("window", SYNCED)]);
  assert_eq!(decision.next, run(3, None, false), "one follow-up for both requests");

  let decision = scheduler.run_finished(3, Some("f3".into()), OK, true);
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

  let decision = scheduler.run_finished(2, Some("f2".into()), OK, true);
  assert_eq!(decision.resolve, [("manual", SYNCED)]);
  assert_eq!(decision.next, run(3, None, false), "the follow-up comes before the check");
  let decision = scheduler.run_finished(3, Some("f3".into()), OK, true);
  assert_eq!(decision.resolve, [("rewrite", SYNCED)]);
  assert_eq!(decision.next, Step::CheckInputs, "one check for both focuses");

  let decision = scheduler.inputs_read(Some("f3".into()), true);
  assert_eq!(decision.resolve, [("focus", Ok(SyncOutcome::Unchanged)), ("focus again", Ok(SyncOutcome::Unchanged))]);
  assert_eq!(decision.next, Step::Nothing);
}

/// A focus syncs only when the inputs changed since the last successful sync, and the run takes the fingerprint the
/// check read; a request made during the check is served by that run
#[test]
fn a_focus_runs_only_when_the_inputs_changed() {
  let mut scheduler = synced_once();
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  let decision = scheduler.inputs_read(Some("f1".into()), true);
  assert_eq!(decision.resolve, [("focus", Ok(SyncOutcome::Unchanged))]);

  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  assert_eq!(scheduler.request(SyncReason::Open, "window", true).next, Step::Nothing);
  assert_eq!(scheduler.request(SyncReason::AfterRewrite, "rewrite", true).next, Step::Nothing);
  let decision = scheduler.inputs_read(Some("f2".into()), true);
  assert_eq!(decision.resolve.len(), 0);
  assert_eq!(decision.next, run(2, Some("f2"), false));
  let decision = scheduler.run_finished(2, Some("f2".into()), OK, true);
  assert_eq!(decision.resolve, [("focus", SYNCED), ("window", SYNCED), ("rewrite", SYNCED)]);

  // a fingerprint git could not read runs
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  assert_eq!(scheduler.inputs_read(None, true).next, run(3, None, false));
}

/// A run whose report asks for a retry runs once more, silently, for the same requests; a second ask is not retried
#[test]
fn a_run_that_asks_for_a_retry_is_retried_once() {
  let mut scheduler = synced_once();
  assert_eq!(scheduler.request(SyncReason::Manual, "manual", true).next, run(2, None, false));
  let decision = scheduler.run_finished(2, Some("f2".into()), RunEnd::Succeeded { retry: true }, true);
  assert_eq!(decision.resolve.len(), 0, "the request waits for the retry");
  assert_eq!(decision.next, run(3, None, true));
  let decision = scheduler.run_finished(3, Some("f3".into()), RunEnd::Succeeded { retry: true }, true);
  assert_eq!(decision.resolve, [("manual", SYNCED)]);
  assert_eq!(decision.next, Step::Nothing);
}

/// A failed or deferred run settles its requests with the error and keeps the inputs of the last successful sync; the
/// end of a run that is not in flight changes nothing
#[test]
fn a_failed_run_settles_its_requests_with_the_error() {
  let mut scheduler = synced_once();
  assert_eq!(scheduler.request(SyncReason::Manual, "manual", true).next, run(2, None, false));
  assert_eq!(scheduler.run_finished(1, Some("old".into()), OK, true).next, Step::Nothing);
  let deferred = ApiError::new(ErrorCode::OperationInProgress, "Not synced: rebase is in progress");
  let decision = scheduler.run_finished(2, Some("f2".into()), RunEnd::Failed(deferred.clone()), true);
  assert_eq!(decision.resolve, [("manual", Err(deferred))]);
  assert_eq!(scheduler.request(SyncReason::Focus, "focus", true).next, Step::CheckInputs);
  assert_eq!(scheduler.inputs_read(Some("f1".into()), true).resolve, [("focus", Ok(SyncOutcome::Unchanged))]);
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
  let decision = scheduler.run_finished(2, Some("f2".into()), OK, false);
  assert_eq!(decision.resolve, [("manual", SYNCED), ("focus", Ok(SyncOutcome::Skipped))]);
  assert_eq!(decision.next, Step::Nothing);
  assert!(scheduler.is_idle());
}
