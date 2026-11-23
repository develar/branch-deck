//! The prologue shared by operations whose plans depend on the session's branches: wait for earlier rewrites, then for
//! the sync of the writes of the session (`Sessions::after_writes`: the one an Update Project, a rewrite or a fetch asked
//! as it ended, which moves the branches the plan pushes), and for any running sync where the caller's plan depends on
//! what a sync reads besides, before deciding again on those facts.

use crate::engine::Hosts;
use crate::registry::RunningGuard;
use domain_types::error::ApiError;
use domain_types::operation::{EffectStatus, Intent, OperationEvent, OperationStep, Outcome, Resource};
use domain_types::session::{SyncReason, SyncState};
use repository_model::session::{SessionKey, Sessions};
use tracing::debug;

/// What an operation found while waiting for facts that can change its plan
#[derive(Debug)]
pub struct Waited {
  /// It waited: the plan must be decided again
  pub waited: bool,
  /// The error of the last sync it waited for, if any
  pub failed: Option<ApiError>,
}

/// Which syncs the operation depends on, besides the sync of the writes of the session, which every operation waits for
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SyncWait {
  /// Any running sync and its quiet follow-up (`SYNCS_TO_WAIT`): the plan reads the remote status a fetch of any sync
  /// brings (a push leases on it)
  Running,
  /// Only the sync of the writes: a sync no write of the session asked (⌘R, a focus, a stamp) is not waited for, and the
  /// operation decides on the table of the last sync, as a start does
  Writes,
}

/// Waits behind operations holding `resources`, then for the syncs `sync` names; the operation's admission may have
/// queued too. A sync asked later is not waited for
pub async fn wait_for_facts(sessions: &Sessions, key: &SessionKey, guard: &RunningGuard, resources: Vec<Resource>, sync: SyncWait) -> Waited {
  let behind = guard.wait_behind(resources, OperationStep::Checking).await;
  let stepper = guard.stepper();
  let running = match sync {
    SyncWait::Running => wait_for_running_sync(sessions, key, &stepper).await,
    SyncWait::Writes => Waited { waited: false, failed: None },
  };
  // a write asked while a focus check ran, or as the follow-up of a run no write asked: the running sync may not be it
  let writes = wait_for_writes(sessions, key, &stepper).await;
  Waited {
    waited: behind || running.waited || writes.waited || guard.queued(),
    failed: writes.failed.or(running.failed),
  }
}

/// Waits for the sync of the writes of the session of `key` that has not ended (`Sessions::after_writes`), with the
/// step `WaitingForSync`: an operation admitted as the Update Project before it ended decides on the branches its sync
/// rebuilt, not on the table before it. Whether it waited, and how that sync failed
pub(crate) async fn wait_for_writes(sessions: &Sessions, key: &SessionKey, stepper: &crate::registry::Stepper) -> Waited {
  let Some(ticket) = sessions.after_writes(key).await else {
    return Waited { waited: false, failed: None };
  };
  stepper.step(OperationStep::WaitingForSync);
  let failed = ticket.wait().await.err();
  if let Some(error) = &failed {
    debug!(error = %error.message, "the sync of the writes failed: the operation checks the table of the last successful sync");
  }
  stepper.step(OperationStep::Checking);
  Waited { waited: true, failed }
}

/// Waits for a sync that shows the branches a plan of the operation of `guard` found moved before its first effect
/// (`land_execute::NotRun::BranchesMoved`), with the step `WaitingForSync`: the sync of the writes of the session, then
/// the sync in flight or one that runs when what a sync reads changed (`SyncReason::Open`: a write of another process
/// moves the branches the virtual branches are built of, which a sync reads), so the plan decided again is on the
/// table that shows them
///
/// # Errors
/// How that sync failed: the table does not show the branches as they are
pub async fn catch_up(sessions: &Sessions, key: &SessionKey, guard: &RunningGuard) -> Result<(), ApiError> {
  let stepper = guard.stepper();
  if let Some(error) = wait_for_writes(sessions, key, &stepper).await.failed {
    return Err(error);
  }
  stepper.step(OperationStep::WaitingForSync);
  let synced = sessions.sync(key, SyncReason::Open).wait().await;
  stepper.step(OperationStep::Checking);
  synced.map(|_| ())
}

/// Runs a follow-up intent under the same registry guard and sends its outcome to the original caller's event channel
pub async fn follow_up(
  registry: &crate::registry::Registry,
  key: &SessionKey,
  guard: &RunningGuard,
  intent: Intent,
  hosts: Hosts<'_>,
  events: &std::sync::Arc<dyn crate::events::EventSink>,
) -> Option<Outcome> {
  let Intent::UpdateProject { fetch, .. } = intent else {
    return None;
  };
  let outcome = crate::update::execute_update_in(registry.sessions(), key, guard, fetch, hosts).await.ok()?;
  events.send(OperationEvent::Done {
    outcome: Box::new(outcome.clone()),
  });
  Some(outcome)
}

/// Refreshes the baseline under the caller's guard before its one retry, without sending an intermediate outcome
pub async fn refresh_baseline(sessions: &Sessions, key: &SessionKey, guard: &RunningGuard, hosts: Hosts<'_>) -> Result<(), ApiError> {
  let outcome = crate::update::execute_update_in(sessions, key, guard, true, hosts).await?;
  if let Some(error) = outcome.effects.iter().find_map(|record| match &record.status {
    EffectStatus::Failed { error } => Some(error),
    _ => None,
  }) {
    return Err(error.clone());
  }
  Ok(())
}

/// How many runs of a sync an operation waits for: the one that runs and the quiet follow-up a rewrite asked during it
const SYNCS_TO_WAIT: usize = 2;

/// Waits for the sync of the session of `key` that runs, if one does (a rewrite's, a focus'), and for the follow-up that
/// starts as it ends (the sync a rewrite asked during it), with the step `WaitingForSync`; they bring the tips into the
/// table the operation is checked on. Whether it waited, and how the last one failed
async fn wait_for_running_sync(sessions: &Sessions, key: &SessionKey, stepper: &crate::registry::Stepper) -> Waited {
  let mut wait = Waited { waited: false, failed: None };
  for _ in 0..SYNCS_TO_WAIT {
    if sessions.apply_with(key, |state| matches!(state.sync(), SyncState::Running { .. })).await != Some(true) {
      break;
    }
    if !wait.waited {
      stepper.step(OperationStep::WaitingForSync);
    }
    wait.waited = true;
    wait.failed = sessions.sync(key, SyncReason::Open).wait().await.err();
    if let Some(error) = &wait.failed {
      debug!(error = %error.message, "the operation checks the table of the last successful sync");
    }
  }
  if wait.waited {
    stepper.step(OperationStep::Checking);
  }
  wait
}

#[cfg(test)]
#[path = "run_frame_test.rs"]
pub(crate) mod run_frame_test;
