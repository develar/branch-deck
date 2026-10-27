//! Update Project waits for another holder of the repository lock as long as it lives (`LockWait::WhileHeld`), the
//! holder told on its progress, and the page ends the wait with `cancel_lock_wait` of that channel's id (Stop Waiting);
//! a wait so ended changed nothing: the command answers the code `canceled` (docs/design/git-operations.md,
//! "Repository lock"). The rewrites and the pushes are operations of the engine: their wait is the step
//! `WaitingForLock` of their operation, which `cancel_operation` stops.

use domain_types::error::{ApiError, ErrorCode};
use history_rewrite::lock_wait::{LockWaits, WaitTicket};
use history_rewrite::repository_lock::LockHolder;
use std::sync::Arc;
use tauri::State;
use tracing::instrument;

/// The waits of the app's commands, as a command gets them
pub type Waits<'a> = State<'a, LockWaits>;

/// Registers the wait of the command whose channel is `id`; `tell` hears whom it waits for
pub fn register(lock_waits: &LockWaits, id: u32, tell: impl Fn(Option<&LockHolder>) + Send + Sync + 'static) -> Arc<WaitTicket> {
  Arc::new(lock_waits.register(id, tell))
}

/// `result` of a command whose wait `ticket` is: an error after the user stopped the wait is `canceled`, whatever the
/// step it ended answered
pub fn canceled_code<T>(result: Result<T, ApiError>, ticket: &WaitTicket) -> Result<T, ApiError> {
  result.map_err(|error| {
    if ticket.ended_by_cancel() && error.code != ErrorCode::Canceled {
      ApiError {
        operation_id: error.operation_id,
        ..ApiError::new(ErrorCode::Canceled, error.message)
      }
    } else {
      error
    }
  })
}

/// Stops the wait of the command whose channel has the id `channel` for another holder of the repository lock: the
/// command answers `canceled`, nothing changed. A wait that ended already (the lock taken) goes on
#[tauri::command]
#[instrument(skip(lock_waits))]
#[specta::specta]
#[expect(clippy::needless_pass_by_value, reason = "tauri passes the state of a command by value")]
pub fn cancel_lock_wait(lock_waits: Waits<'_>, channel: u32) -> bool {
  lock_waits.cancel(channel)
}
