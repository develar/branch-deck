//! The frame every family of intents runs in (docs/design/rust-domain-authority.md, "Планирование и исполнение
//! операций"): `run` admits the operation in its session through the registry, runs the body of its family once it is
//! admitted and ends it with the body's answer, or answers what the operation of the same intent it joined answered. The
//! caller hears the outcome either way (`OperationEvent::Done`). `prepare` asks the family of an intent for its plan,
//! and `execute` runs it on the plan its caller accepted: a start (`execute_land`, `land_execute::execute`), a restart
//! and a cancel of a robot (`robot`), a retarget of a pull request (`pull_request`); the other families (`push`,
//! `review`, `issue`) build on it as they move here. A start reserves what its plan lands (`Resource::Lands`), the refs
//! it pushes and the robot it cancels: a second start of the same subject joins it, one that lands a part of it waits for
//! it and is decided again.

use crate::events::EventSink;
use crate::land_execute::{self, Accepted, StartHost, StartServices, Steps};
use crate::land_session::session_start;
use crate::registry::{Admitted, Answer, OperationRequest, Registry, RunningGuard};
use crate::{land, pull_request, robot};
use domain_types::branch_names::{to_final_branch_name, to_stack_branch_name};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{Land, PlannedEffect, Subject};
use domain_types::operation::{Intent, OperationEvent, OperationStep, Plan, Resource, RunningMove};
use remote_facts::service_sources::ServiceHost;
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
/// lacks, a robot it offers no restart of, a pull request it lists not)
///
/// # Errors
/// `invalid_arguments` for an intent whose family has no plan here yet
pub async fn prepare(sessions: &Sessions, key: &SessionKey, intent: Intent) -> Result<Option<Plan>, ApiError> {
  match intent {
    Intent::Land(start) => Ok(land::prepare(sessions, key, start).await),
    Intent::RestartRobot { robot_id } => Ok(robot::prepare_restart(sessions, key, &robot_id).await.map(|planned| planned.plan)),
    Intent::CancelRobot { robot_id } => Ok(robot::prepare_cancel(sessions, key, &robot_id).await.map(|(plan, _)| plan)),
    Intent::Retarget { number } => Ok(pull_request::prepare_retarget(sessions, key, number).await),
    _ => Err(not_yet()),
  }
}

/// An intent whose family runs through its own commands until it moves here
fn not_yet() -> ApiError {
  ApiError::new(
    ErrorCode::InvalidArguments,
    "Only a start, a restart and a cancel of a robot and a retarget run through prepare and execute so far",
  )
}

/// Runs `intent` in the session of `key` on the plan its caller accepted (`accepted`: the fingerprint of the plan it
/// showed, the confirmations the user accepted of it), over the services `services` resolves for the session (the
/// app's and the E2E server's `ServiceHost`): admitted over the operations of the session, planned again, then its
/// effects; its steps in the view, its progress and its outcome to `events`
///
/// # Errors
/// What refused it before its first effect, a session that is not open, an intent whose family runs elsewhere
pub async fn execute(registry: &Registry, key: &SessionKey, intent: Intent, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  match intent {
    Intent::Land(land) => {
      let (host, run) = session_start(services, registry.sessions(), key).await?;
      // the run of a start is a large future: on the heap
      Box::pin(execute_land(registry, key, land, accepted, &host, &run, events)).await
    }
    Intent::RestartRobot { robot_id } => Box::pin(robot::execute_restart(registry, key, &robot_id, accepted, services, events)).await,
    Intent::CancelRobot { robot_id } => robot::execute_cancel(registry, key, &robot_id, accepted, services, events).await,
    Intent::Retarget { number } => pull_request::execute_retarget(registry, key, number, accepted, services, events).await,
    _ => Err(not_yet()),
  }
}

/// Where the steps of the operation of `guard` go: the view and its caller, the progress of its push to the caller, and
/// whether the user stopped it
pub(crate) fn steps_of(guard: &RunningGuard) -> Steps {
  let stepper = guard.stepper();
  let progress = guard.stepper();
  Steps::new(
    Arc::new(move |step| stepper.step(step)),
    Arc::new(move |tick: &TransferProgress| progress.progress(u32::from(tick.percent), tick.line.clone())),
    guard.canceled(),
  )
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
    let steps = steps_of(guard);
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
