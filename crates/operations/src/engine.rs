//! The frame every family of intents runs in (docs/design/rust-domain-authority.md, "Планирование и исполнение
//! операций"): `run` admits the operation in its session through the registry, runs the body of its family once it is
//! admitted and ends it with the body's answer, or answers what the operation of the same intent it joined answered. The
//! caller hears the outcome either way (`OperationEvent::Done`). The `prepare` and `execute` of each family (`land`,
//! `push`, `robot`, `pull_request`, `review`, `issue`) build on it as they move here, family by family; `prepare` asks
//! the family of an intent for its plan, which only a start has so far (`land::prepare`).

use crate::events::EventSink;
use crate::land;
use crate::registry::{Admitted, Answer, OperationRequest, Registry, RunningGuard};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::operation::{Intent, OperationEvent, Plan};
use repository_model::session::{SessionKey, Sessions};
use std::sync::Arc;

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
        events.send(OperationEvent::Done { outcome: outcome.clone() });
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
