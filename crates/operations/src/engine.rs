//! The frame every family of intents runs in (docs/design/rust-domain-authority.md, "Планирование и исполнение
//! операций"): `run` admits the operation in its session through the registry, runs the body of its family once it is
//! admitted and ends it with the body's answer, or answers what the operation of the same intent it joined answered. The
//! caller hears the outcome either way (`OperationEvent::Done`). The `prepare` and `execute` of each family (`land`,
//! `push`, `robot`, `pull_request`, `review`, `issue`) build on it as they move here, family by family.

use crate::events::EventSink;
use crate::registry::{Admitted, Answer, OperationRequest, Registry, RunningGuard};
use domain_types::operation::OperationEvent;
use repository_model::session::SessionKey;
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
