use crate::outbox::{Next, Outbox, Pushed, STALE_CAP};
use crate::wire::Frame;
use pretty_assertions::assert_eq;

fn text(byte: char, len: usize) -> Frame {
  Frame::Text(std::iter::repeat_n(byte, len).collect())
}

/// A connection that reads nothing while 9 MiB of patches of a window wait: the actions of that window are dropped
/// past 8 MiB, the frames of the other window and the answers stay in their order, and the window is resynced once the
/// queue drained, never with a backlog past the cap
#[tokio::test]
async fn a_window_past_the_cap_is_resynced_once_the_queue_drains() {
  let outbox = Outbox::default();
  let (main, review) = ((1, "main".to_owned()), (1, "review-1".to_owned()));
  let mib = 1024 * 1024;
  assert_eq!(outbox.push_action(&review, text('r', 10)), Pushed::Queued);
  for _ in 0..8 {
    assert_eq!(outbox.push_action(&main, text('m', mib)), Pushed::Queued);
  }
  assert!(outbox.push(text('a', 3)));
  assert_eq!(outbox.push_action(&main, text('m', mib)), Pushed::Stale, "the ninth MiB passes the cap");
  assert!(outbox.is_stale(&main));
  assert_eq!(outbox.push_action(&main, text('m', 1)), Pushed::Stale, "a stale window waits for its resync");

  assert_eq!(outbox.next().await, Some(Next::Frame(text('r', 10))));
  assert_eq!(outbox.next().await, Some(Next::Frame(text('a', 3))), "the patches of the stale window are not sent");
  assert_eq!(outbox.next().await, Some(Next::Resync(vec![main.clone()])));
  outbox.resynced(&main);
  assert_eq!(outbox.push_action(&main, text('m', 2)), Pushed::Queued, "after the resync its actions are queued again");
  assert_eq!(outbox.next().await, Some(Next::Frame(text('m', 2))));
  assert!(STALE_CAP < 9 * mib);
}

/// An ended connection reads what was queued, then nothing, and queues nothing more
#[tokio::test]
async fn an_ended_connection_reads_what_was_queued_then_nothing() {
  let outbox = Outbox::default();
  assert!(outbox.push(text('a', 1)));
  outbox.close();
  assert!(!outbox.push(text('b', 1)));
  assert_eq!(outbox.push_action(&(1, "main".to_owned()), text('c', 1)), Pushed::Closed);
  assert_eq!(outbox.next().await, Some(Next::Frame(text('a', 1))));
  assert_eq!(outbox.next().await, None);
}
