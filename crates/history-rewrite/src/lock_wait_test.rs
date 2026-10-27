use crate::lock_wait::{LockWaits, waiting_for};
use crate::repository_lock::{LockHolder, LockWaiter};
use pretty_assertions::assert_eq;
use std::sync::{Arc, Mutex};

fn holder() -> LockHolder {
  LockHolder {
    command: "bd pick --follow".to_string(),
    pid: 4242,
    process_started: None,
    since: 0,
    subject: None,
  }
}

/// A registered wait tells whom it waits for, ends when Stop Waiting names its channel, and says so afterwards; a Stop
/// Waiting of a channel without a wait (ended, never registered) stops nothing
#[test]
fn a_wait_is_stopped_by_its_channel_and_leaves_the_registry_with_its_ticket() {
  let waits = LockWaits::default();
  let told = Arc::new(Mutex::new(Vec::new()));
  let heard = Arc::clone(&told);
  let ticket = waits.register(7, move |holder| heard.lock().unwrap().push(holder.map(waiting_for)));
  ticket.holder(Some(&holder()));
  ticket.holder(None);
  assert_eq!(*told.lock().unwrap(), [Some(waiting_for(&holder())), None]);
  assert!(waiting_for(&holder()).starts_with("Waiting for `bd pick --follow` (pid 4242, for "));

  assert!(!ticket.canceled());
  assert!(!waits.cancel(8), "no wait on another channel");
  assert!(waits.cancel(7));
  // a Stop Waiting that came after the lock was taken ends nothing: only a wait that saw it ended by it
  assert!(!ticket.ended_by_cancel());
  assert!(ticket.canceled());
  assert!(ticket.ended_by_cancel());

  drop(ticket);
  assert!(!waits.cancel(7), "the ticket left the registry");
}
