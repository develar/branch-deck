//! The waits of the app's commands for another holder of the repository lock (`LockWait::WhileHeld`), by the id of the
//! channel the page hears each on: a command registers its wait (`LockWaits::register`), tells the page whom it waits
//! for (`LockWaitEvent`, "Waiting for `bd pick` (pid 4242, for 12 s)"), and Stop Waiting on the surface of the
//! operation ends it (`LockWaits::cancel`, the command `cancel_lock_wait`) before anything was read or written. The E2E
//! server keeps the same registry for the page of a test.

use crate::repository_lock::{LockHolder, LockWaiter};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// What a rewrite of the app tells the page while it waits for another holder of the repository lock
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LockWaitEvent {
  /// Whom it waits for, "Waiting for `bd pick` (pid 4242, for 12 s)", told again about once a second; null once the
  /// wait ended
  pub waiting_for: Option<String>,
}

impl LockWaitEvent {
  /// The event of `holder`, or of the end of the wait
  pub fn of(holder: Option<&LockHolder>) -> Self {
    Self {
      waiting_for: holder.map(waiting_for),
    }
  }
}

/// The words of a wait for `holder`: "Waiting for `bd pick` (pid 4242, for 12 s)"
pub fn waiting_for(holder: &LockHolder) -> String {
  format!("Waiting for {holder}")
}

/// The registered waits, by the id of the channel of their command; cloned, it is the same registry
#[derive(Debug, Clone, Default)]
pub struct LockWaits {
  canceled: Arc<Mutex<HashMap<u32, Arc<AtomicBool>>>>,
}

impl LockWaits {
  /// The wait of the command whose channel is `id`, for as long as the ticket lives; `tell` hears whom it waits for
  pub fn register(&self, id: u32, tell: impl Fn(Option<&LockHolder>) + Send + Sync + 'static) -> WaitTicket {
    let canceled = Arc::new(AtomicBool::new(false));
    self.canceled.lock().unwrap_or_else(PoisonError::into_inner).insert(id, Arc::clone(&canceled));
    WaitTicket {
      id,
      canceled,
      ended_by_cancel: AtomicBool::new(false),
      tell: Box::new(tell),
      waits: self.clone(),
    }
  }

  /// Stops the wait of the command whose channel is `id`; false when none is registered (it ended already)
  pub fn cancel(&self, id: u32) -> bool {
    let canceled = self.canceled.lock().unwrap_or_else(PoisonError::into_inner).get(&id).cloned();
    canceled.inspect(|canceled| canceled.store(true, Ordering::Relaxed)).is_some()
  }
}

/// Whom a wait is for, told to the page (`LockWaiter::holder`)
type Tell = Box<dyn Fn(Option<&LockHolder>) + Send + Sync>;

/// A registered wait (`LockWaits::register`), the `LockWaiter` of its command; it leaves the registry when dropped
pub struct WaitTicket {
  id: u32,
  canceled: Arc<AtomicBool>,
  ended_by_cancel: AtomicBool,
  tell: Tell,
  waits: LockWaits,
}

impl WaitTicket {
  /// Whether a wait of the command ended because the user stopped it: nothing was read or written, the command answers
  /// so (`CommandErrorKind::LockWaitCanceled`) instead of an error. A Stop Waiting that came after the lock was taken
  /// ends nothing
  pub fn ended_by_cancel(&self) -> bool {
    self.ended_by_cancel.load(Ordering::Relaxed)
  }
}

impl LockWaiter for WaitTicket {
  fn holder(&self, holder: Option<&LockHolder>) {
    (self.tell)(holder);
  }

  fn canceled(&self) -> bool {
    let canceled = self.canceled.load(Ordering::Relaxed);
    if canceled {
      self.ended_by_cancel.store(true, Ordering::Relaxed);
    }
    canceled
  }
}

impl Drop for WaitTicket {
  fn drop(&mut self) {
    self.waits.canceled.lock().unwrap_or_else(PoisonError::into_inner).remove(&self.id);
  }
}
