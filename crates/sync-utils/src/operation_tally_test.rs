use crate::operation_tally::{OperationTally, current, git_process, http_request};
use crate::span::in_current_span;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

/// Polls `work` to its end on this thread
fn ready<F: Future>(work: F) -> F::Output {
  let mut work = pin!(work);
  let mut cx = Context::from_waker(Waker::noop());
  loop {
    if let Poll::Ready(output) = work.as_mut().poll(&mut cx) {
      return output;
    }
  }
}

#[test]
fn a_tally_counts_what_runs_inside_it_and_on_the_threads_it_hands_work_to() {
  let tally = OperationTally::begin();
  git_process("status");
  http_request();
  tally.scope(|| {
    git_process("rev-parse");
    http_request();
    // a closure handed to another thread takes the tally along
    std::thread::spawn(in_current_span(|| git_process("push"))).join().unwrap();
  });
  // nothing is current outside it
  assert!(current().is_none());
  git_process("status");

  assert_eq!(tally.git_processes(), BTreeMap::from([("push".to_string(), 1), ("rev-parse".to_string(), 1)]));
  assert_eq!(tally.http_requests(), 1);
}

#[test]
fn a_future_counts_only_while_it_is_polled() {
  let tally = OperationTally::begin();
  let mut polls = 0;
  let counted = tally.counting(std::future::poll_fn(|cx| {
    git_process("fetch");
    polls += 1;
    if polls < 3 {
      cx.waker().wake_by_ref();
      return Poll::Pending;
    }
    Poll::Ready(())
  }));
  ready(counted);
  git_process("fetch");

  assert_eq!(tally.git_processes(), BTreeMap::from([("fetch".to_string(), 3)]));
  assert!(current().is_none());
}

#[test]
fn the_tally_before_an_inner_one_is_current_again_after_it() {
  let (outer, inner) = (OperationTally::begin(), OperationTally::begin());
  outer.scope(|| {
    inner.scope(http_request);
    http_request();
    let _ = std::panic::catch_unwind(|| inner.scope(|| panic!("a step that panics")));
    http_request();
  });

  assert_eq!((outer.http_requests(), inner.http_requests()), (2, 1));
}
