//! The prologue shared by operations whose plans depend on the session's branches: wait for earlier rewrites and the
//! session sync that runs, then decide again on the facts they left.

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
  /// The error of the last sync it waited for, none when that one committed
  pub failed: Option<ApiError>,
}

/// Waits behind operations holding `resources`, then for a sync of the session that runs, including its quiet follow-up
/// (`SYNCS_TO_WAIT`); the operation's admission may have queued too. A sync asked later is not waited for
pub async fn wait_for_facts(sessions: &Sessions, key: &SessionKey, guard: &RunningGuard, resources: Vec<Resource>) -> Waited {
  let behind = guard.wait_behind(resources, OperationStep::Checking).await;
  let synced = wait_for_running_sync(sessions, key, &guard.stepper()).await;
  Waited {
    waited: behind || synced.waited || guard.queued(),
    failed: synced.failed,
  }
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
  let Intent::UpdateProject { fetch } = intent else {
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
mod run_frame_test;
