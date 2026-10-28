//! The operations of the remote as the page asks them (docs/design/rust-domain-authority.md, "Планирование и
//! исполнение операций"): `prepare` answers the plan of an intent in the session of the repository, which the popover
//! of a start renders and a click starts by (`operations::engine::prepare`); `execute` runs it on the plan the page
//! showed (`operations::engine::execute`: admitted over the operations of the session, the witness, the lock only for
//! the freeze, then the effects) over the services of the context, its steps, the progress of its push and its outcome
//! sent to the channel of the window that asked; `cancel_operation` stops one that waits for the lock or whose next
//! effect has not started.

use crate::context::AppContext;
use crate::session::windows::Windows;
use crate::transport::{Call, Sink, WindowId};
use domain_types::error::ApiError;
use domain_types::operation::{CancelOperationParams, ExecuteParams, Intent, OperationEvent, Outcome, Plan, PrepareParams};
use operations::engine::Hosts;
use operations::events::EventSink;
use operations::land_execute::Accepted;
use repository_model::session::SessionKey;
use std::sync::Arc;
use tracing::instrument;

/// `operations::engine::prepare`: a read, no lock, no effect
pub async fn prepare(ctx: &AppContext, _call: &Call, key: SessionKey, params: PrepareParams) -> Result<Option<Plan>, ApiError> {
  operations::engine::prepare(&ctx.sessions, &key, params.intent).await
}

/// The events of an operation to the channel of the page that asked
struct PageEvents(Sink<OperationEvent>);

impl EventSink for PageEvents {
  fn send(&self, event: OperationEvent) {
    // a page that went away hears nothing: the operation goes on
    let _ = self.0.send(event);
  }
}

/// The events of a review fetch of `window`: its first step names its operation, which the close of the window stops
/// (`AppContext::forget_window`), until its end
struct WindowFetchEvents {
  events: PageEvents,
  windows: Arc<Windows>,
  window: WindowId,
}

impl EventSink for WindowFetchEvents {
  fn send(&self, event: OperationEvent) {
    match &event {
      OperationEvent::Step { op, .. } => self.windows.fetch_started(&self.window, op),
      OperationEvent::Done { outcome } => self.windows.fetch_ended(&self.window, &outcome.op),
      OperationEvent::Progress { .. } => {}
    }
    self.events.send(event);
  }
}

/// `operations::engine::execute` on the plan the page showed, over the services and the rewrites of the context; the
/// review fetch of a window is stopped when the window closes
pub async fn execute(ctx: &AppContext, call: &Call, key: SessionKey, params: ExecuteParams, events: Sink<OperationEvent>) -> Result<Outcome, ApiError> {
  let events: Arc<dyn EventSink> = if matches!(params.intent, Intent::FetchReview { .. }) {
    Arc::new(WindowFetchEvents {
      events: PageEvents(events),
      windows: Arc::clone(&ctx.windows),
      window: call.window.clone(),
    })
  } else {
    Arc::new(PageEvents(events))
  };
  let accepted = Accepted {
    fingerprint: params.fingerprint,
    confirmations: params.accepted,
  };
  let hosts = Hosts {
    services: &*ctx.services,
    rewrites: &*ctx.rewrites,
  };
  // the run of a start is a large future: on the heap
  Box::pin(operations::engine::execute(&ctx.registry, &key, params.intent, accepted, hosts, events)).await
}

/// `Registry::cancel`: false for an operation that ended
#[instrument(skip_all, fields(op = %params.op))]
#[expect(clippy::needless_pass_by_value, reason = "the registry hands a body its arguments by value")]
pub fn cancel_operation(ctx: &AppContext, _call: &Call, params: CancelOperationParams) -> bool {
  ctx.registry.cancel(&params.op)
}
