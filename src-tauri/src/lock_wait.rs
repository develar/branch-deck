//! The app's commands wait for another holder of the repository lock as long as it lives (`LockWait::WhileHeld`): the
//! holder of a rewrite is told on its channel (`LockWaitEvent`), of a push and of an update on their progress, and the
//! page ends a wait with `cancel_lock_wait` of that channel's id (Stop Waiting). A wait so ended changed nothing: the
//! command answers `CommandErrorKind::LockWaitCanceled` (docs/design/git-operations.md, "Repository lock").

use crate::command_error::{CommandError, CommandErrorKind};
use crate::operation::run_blocking;
use history_rewrite::lock_wait::{LockWaitEvent, LockWaits, WaitTicket};
use history_rewrite::repository_lock::{LockHolder, LockWait, LockWaiter};
use std::sync::Arc;
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};

/// The channel a rewrite command of the app tells its wait for the repository lock on (`LockWaitEvent`)
pub type WaitChannel = Channel<LockWaitEvent>;

/// The waits of the app's commands, as a command gets them
pub type Waits<'a> = State<'a, LockWaits>;

/// Registers the wait of the command whose channel is `id`; `tell` hears whom it waits for
pub fn register(lock_waits: &LockWaits, id: u32, tell: impl Fn(Option<&LockHolder>) + Send + Sync + 'static) -> Arc<WaitTicket> {
  Arc::new(lock_waits.register(id, tell))
}

/// Whether the user stopped the wait of `ticket`, for a core that takes the end of its wait as a function (the push)
pub fn canceled_by(ticket: &Arc<WaitTicket>) -> impl Fn() -> bool + Send + 'static {
  let ticket = Arc::clone(ticket);
  move || ticket.canceled()
}

/// `result` of a command whose wait `ticket` is: an error after the user stopped the wait is `LockWaitCanceled`
pub fn canceled_kind<T>(result: Result<T, CommandError>, ticket: &WaitTicket) -> Result<T, CommandError> {
  result.map_err(|mut error| {
    if ticket.ended_by_cancel() {
      error.kind = Some(CommandErrorKind::LockWaitCanceled);
    }
    error
  })
}

/// Runs `step`, a rewrite of the app, on the blocking pool as `run_blocking` does, its lock waited for as long as the
/// holder lives: `wait` hears whom it waits for, and `cancel_lock_wait` of its id ends the wait. Alone it is the
/// operation of its command, as `run_blocking` is (`command_scan_test.rs`)
pub async fn run_waiting<T, E, F>(lock_waits: &LockWaits, wait: WaitChannel, step: F) -> Result<T, CommandError>
where
  F: FnOnce(LockWait<'_>) -> Result<T, E> + Send + 'static,
  T: Send + 'static,
  E: Into<CommandError> + Send + 'static,
{
  let id = wait.id();
  let ticket = register(lock_waits, id, move |holder| {
    if let Err(e) = wait.send(LockWaitEvent::of(holder)) {
      warn!(error = %e, "failed to report the wait for the repository lock");
    }
  });
  let step_ticket = Arc::clone(&ticket);
  let result = run_blocking(move || step(LockWait::WhileHeld(&*step_ticket))).await;
  canceled_kind(result, &ticket)
}

/// Stops the wait of the command whose channel has the id `channel` for another holder of the repository lock: the
/// command answers `LockWaitCanceled`, nothing changed. A wait that ended already (the lock taken) goes on
#[tauri::command]
#[instrument(skip(lock_waits))]
#[specta::specta]
#[expect(clippy::needless_pass_by_value, reason = "tauri passes the state of a command by value")]
pub fn cancel_lock_wait(lock_waits: Waits<'_>, channel: u32) -> bool {
  lock_waits.cancel(channel)
}
