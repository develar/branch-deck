//! The frame every family of intents runs in (docs/design/rust-domain-authority.md, "Планирование и исполнение
//! операций"): `run` admits the operation in its session through the registry, runs the body of its family once it is
//! admitted and ends it with the body's answer, or answers what the operation of the same intent it joined answered. The
//! caller hears the outcome either way (`OperationEvent::Done`). The `prepare` and `execute` of each family (`land`,
//! `push`, `robot`, `pull_request`, `review`, `issue`) build on it as they move here, family by family; `prepare` asks
//! the family of an intent for its plan, and `execute_land` runs a start (`land_execute::execute`), the only family the
//! engine runs so far. A start reserves what its plan lands (`Resource::Lands`), the refs it pushes and the robot it
//! cancels: a second start of the same subject joins it, one that lands a part of it waits for it and is decided again.

use crate::events::EventSink;
use crate::land;
use crate::land_execute::{self, Accepted, StartHost, StartServices, Steps};
use crate::registry::{Admitted, Answer, OperationRequest, Registry, RunningGuard};
use domain_types::branch_names::{to_final_branch_name, to_stack_branch_name};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{Land, PlannedEffect, Subject};
use domain_types::operation::{Intent, OperationEvent, OperationStep, Plan, Resource, RunningMove};
use repository_model::session::{SessionKey, Sessions};
use std::sync::Arc;
use sync_core::transfer_progress::TransferProgress;

/// Runs `request` in the session of `key`: `body` once the registry admitted it (at once, or after the operations it
/// waits for), or the answer of the operation of the same intent it joined; `events` hears its steps and its outcome
pub async fn run(registry: &Registry, key: &SessionKey, request: OperationRequest, events: Arc<dyn EventSink>, body: impl AsyncFnOnce(&RunningGuard) -> Answer) -> Answer {
  match registry.admit(key, request, Arc::clone(&events)).await? {
    Admitted::Run(guard) => {
      let answer = body(&guard).await;
      guard.finish(answer.clone());
      answer
    }
    Admitted::Joined(joined) => {
      let answer = joined.answer().await;
      if let Ok(outcome) = &answer {
        events.send(OperationEvent::Done {
          outcome: Box::new(outcome.clone()),
        });
      }
      answer
    }
  }
}

/// The plan of `intent` in the open session of `key` (the `prepare` of its family): a read, no lock, no effect; `None`
/// for a session that is not open or has no answer of it yet (a start before the Patronus view, a subject its table
/// lacks)
///
/// # Errors
/// `invalid_arguments` for an intent whose family has no plan here yet
pub async fn prepare(sessions: &Sessions, key: &SessionKey, intent: Intent) -> Result<Option<Plan>, ApiError> {
  match intent {
    Intent::Land(start) => Ok(land::prepare(sessions, key, start).await),
    _ => Err(ApiError::new(ErrorCode::InvalidArguments, "Only a Safe Push or a Dry Run has a plan to prepare")),
  }
}

/// Runs the start `land` of the app in the session of `key` on the plan its caller accepted (`accepted`):
/// admitted over the operations of the session with what the plan as the session has it now reserves, then run
/// (`land_execute::execute`) with its steps in the view and its progress and outcome to `events`; stopped by
/// `Registry::cancel`. Answers the outcome
///
/// # Errors
/// What refused the start before its first effect (`land_execute::execute`), a lock another process holds, a session
/// that closed
pub async fn execute_land(
  registry: &Registry,
  key: &SessionKey,
  land: Land,
  accepted: Accepted,
  host: &dyn StartHost,
  services: &StartServices,
  events: Arc<dyn EventSink>,
) -> Answer {
  let planned = land::prepare(registry.sessions(), key, land.clone()).await;
  let request = OperationRequest {
    intent: Intent::Land(land.clone()),
    resources: planned.as_ref().map_or_default(|plan| reserved(&key.prefix, &land.subject, plan)),
    step: OperationStep::Checking,
    moves: vec![RunningMove::Starting {
      subject: land.subject.name().to_string(),
      mode: land.mode,
    }],
  };
  run(registry, key, request, events, async |guard| {
    let stepper = guard.stepper();
    let progress = guard.stepper();
    let steps = Steps::new(
      Arc::new(move |step| stepper.step(step)),
      Arc::new(move |tick: &TransferProgress| progress.progress(u32::from(tick.percent), tick.line.clone())),
      guard.canceled(),
    );
    let started = land_execute::execute(host, services, &steps, guard.id().clone(), &land, &accepted).await?;
    Ok(started.outcome)
  })
  .await
}

/// What a start of `subject` reserves: the branches its plan lands, the refs its push writes, the robot it cancels
fn reserved(prefix: &str, subject: &Subject, plan: &Plan) -> Vec<Resource> {
  let mut resources: Vec<Resource> = plan.lands.iter().map(|branch| Resource::Lands { branch: branch.clone() }).collect();
  for effect in &plan.effects {
    match effect {
      PlannedEffect::Push { branches, .. } => {
        let refs = match subject {
          Subject::WholeStack { root } => to_stack_branch_name(prefix, root).into_iter().collect::<Vec<_>>(),
          Subject::Branch { .. } => branches.iter().filter_map(|branch| to_final_branch_name(prefix, branch).ok()).collect(),
        };
        resources.extend(refs.into_iter().map(|name| Resource::Ref { name }));
      }
      PlannedEffect::CancelRobot { robot_id } => resources.push(Resource::Robot { id: robot_id.clone() }),
      _ => {}
    }
  }
  resources
}
