//! Forget This Start (`Intent::ForgetStart`, spec/branch/patronus.spec.md, "A start whose answer was lost"; `bd
//! operation forget <op>`, spec/cli/agent-cli.spec.md): the record of a start whose answer was lost leaves the operations
//! registry of its repository (`ProcessRegistry::forget`, under its lock), so what it held is free and its subject can
//! start again. Nothing is asked of Patronus: the robot it sent may still run, which the next list of the robots shows.
//! It holds nothing (the start it forgets holds what it holds), and refuses a record whose process still runs it.

use crate::engine::{check_admitted, outcome_of, run};
use crate::events::EventSink;
use crate::land_execute::Accepted;
use crate::registry::{Answer, OperationRequest, Registry};
use crate::registry_file::{ProcessRegistry, Record};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectRecord, EffectStatus, Intent, OperationId, OperationResult, OperationStep, Plan};
use repository_model::session::SessionKey;
use std::sync::Arc;
use sync_utils::span::in_current_span;
use tracing::{info, instrument};

/// The plan of `intent`, the forget of a start whose answer was lost: its one effect, its plan the intent's
///
/// # Errors
/// `invalid_arguments` for another intent
pub fn forget_plan(intent: &Intent) -> Result<Plan, ApiError> {
  let Intent::ForgetStart { op } = intent else {
    return Err(ApiError::new(ErrorCode::InvalidArguments, "No forget of a start"));
  };
  Ok(crate::pull_request::write_plan(
    intent.clone(),
    vec![PlannedEffect::ForgetStart { op: op.clone() }],
    Vec::new(),
    None,
    [],
    [],
  ))
}

/// What a forget asks the registry for: nothing, the start it forgets holds what it holds
pub(crate) fn forget_request(intent: &Intent) -> OperationRequest {
  OperationRequest {
    intent: intent.clone(),
    resources: Vec::new(),
    step: OperationStep::Writing,
    moves: Vec::new(),
  }
}

/// The record of the start `op` whose answer was lost leaves `registry`, the one core of the app and `bd`: the record
/// it took out
///
/// # Errors
/// As `ProcessRegistry::forget`
pub fn forget_start(registry: &ProcessRegistry, op: &OperationId) -> Result<Record, ApiError> {
  let forgotten = registry.forget(op)?;
  info!(op = %op, subject = ?forgotten.intent.subject(), "a start whose answer was lost was forgotten: Patronus may still run it");
  Ok(forgotten)
}

/// Runs the forget `intent` in the session of `key` on the plan its caller accepted: the record leaves the registry of
/// the repository of the session, and the windows read the registry again
///
/// # Errors
/// Another intent, a changed plan
#[instrument(skip_all)]
pub async fn execute_forget(registry: &Registry, key: &SessionKey, intent: Intent, accepted: Accepted, events: Arc<dyn EventSink>) -> Answer {
  let plan = forget_plan(&intent)?;
  let Intent::ForgetStart { op } = &intent else {
    return Err(ApiError::new(ErrorCode::InvalidArguments, "No forget of a start"));
  };
  let files = registry
    .process_registry(key)
    .await
    .ok_or_else(|| ApiError::new(ErrorCode::InvalidArguments, "The session records no operations: there is no start to forget"))?;
  let request = forget_request(&intent);
  run(registry, key, request, events, async |guard| {
    check_admitted(guard, &accepted, &plan)?;
    let (forgetting, forgotten) = (files.clone(), op.clone());
    let answer = tokio::task::spawn_blocking(in_current_span(move || forget_start(&forgetting, &forgotten)))
      .await
      .map_err(|error| ApiError::internal(format!("The forget of the start failed: {error}")))?;
    let status = match answer {
      Ok(_) => EffectStatus::Done,
      Err(error) => EffectStatus::Failed { error },
    };
    registry.reread_registry(&files);
    let effects = plan.effects.into_iter().map(|effect| EffectRecord::new(effect, status.clone())).collect();
    Ok(outcome_of(guard.id().clone(), effects, None, None, OperationResult::None))
  })
  .await
}

#[cfg(test)]
#[path = "forget_test.rs"]
mod forget_test;
