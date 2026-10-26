//! Where the caller of an operation hears it (`OperationEvent`): its steps, the progress of its push and its outcome.
//! The hosts give each operation a sink of their transport (a Tauri `Channel` of the window that asked, a server-sent
//! event stream of the E2E server); `bd` hears nothing while one runs (`NoEvents`). The view of the session shows the
//! step to every window besides (`RunningOperation::step`); the progress and the outcome go only to the caller.

use domain_types::operation::OperationEvent;

/// What an operation tells its caller while it runs
pub trait EventSink: Send + Sync {
  /// Delivers one event; a caller that went away hears nothing, and the operation goes on
  fn send(&self, event: OperationEvent);
}

/// A caller that listens to nothing but the answer
pub struct NoEvents;

impl EventSink for NoEvents {
  fn send(&self, _event: OperationEvent) {}
}
