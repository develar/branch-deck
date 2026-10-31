use crate::single_flight::{Begin, NextFlight, SingleFlight};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Semaphore;

/// A read that counts itself and answers once `gate` lets it
async fn counted_read(reads: &AtomicUsize, gate: &Semaphore) -> usize {
  let number = reads.fetch_add(1, Ordering::SeqCst) + 1;
  drop(gate.acquire().await.unwrap());
  number
}

#[tokio::test]
async fn callers_that_ask_while_a_read_runs_get_its_answer() {
  let flights = SingleFlight::<&str, usize>::default();
  let (reads, gate) = (AtomicUsize::new(0), Semaphore::new(0));
  let ask = || flights.run("gh", || counted_read(&reads, &gate));
  let release = async {
    // every caller is in the flight before the read answers: the first leads, the others joined it
    while reads.load(Ordering::SeqCst) == 0 {
      tokio::task::yield_now().await;
    }
    gate.add_permits(1);
  };
  let (answers, ()) = tokio::join!(async { tokio::join!(ask(), ask(), ask(), ask(), ask()) }, release);
  assert_eq!(<[usize; 5]>::from(answers), [1; 5]);
  assert_eq!(reads.load(Ordering::SeqCst), 1);

  // the flight ended with its answer: a later caller reads again
  gate.add_permits(1);
  assert_eq!(flights.run("gh", || counted_read(&reads, &gate)).await, 2);
}

#[tokio::test]
async fn a_leader_that_went_away_is_replaced_by_a_caller_that_waited() {
  let flights = SingleFlight::<&str, usize>::default();
  let Begin::Lead(lead) = flights.begin("gh") else {
    panic!("the first caller leads");
  };
  let Begin::Join(join) = flights.begin("gh") else {
    panic!("a second caller joins");
  };
  let waiting = tokio::spawn(join);
  // the joiner waits before the leader goes away (one thread: the spawned task runs at this yield)
  tokio::task::yield_now().await;
  assert!(!waiting.is_finished());
  drop(lead);
  assert_eq!(waiting.await.unwrap(), None, "no answer from a leader that went away");
  // the next caller leads its own flight
  assert_eq!(flights.run("gh", || async { 7 }).await, 7);
}

#[tokio::test]
async fn a_caller_after_forget_starts_a_new_flight_and_the_old_one_still_answers_its_callers() {
  let flights = SingleFlight::<&str, usize>::default();
  let Begin::Lead(old) = flights.begin("gh") else {
    panic!("the first caller leads");
  };
  let Begin::Join(joined) = flights.begin("gh") else {
    panic!("a second caller joins");
  };
  flights.forget();
  let Begin::Lead(new) = flights.begin("gh") else {
    panic!("a caller after forget leads a new flight");
  };
  old.finish(1);
  assert_eq!(joined.await, Some(1));
  // the old leader removed nothing of the new flight
  let Begin::Join(on_new) = flights.begin("gh") else {
    panic!("the new flight is still in the air");
  };
  new.finish(2);
  assert_eq!(on_new.await, Some(2));
}

#[tokio::test]
async fn flights_of_other_keys_do_not_wait_for_each_other() {
  let flights = SingleFlight::<&str, usize>::default();
  let Begin::Lead(_slow) = flights.begin("git acme/one") else {
    panic!("the first caller leads");
  };
  assert_eq!(flights.run("git acme/other", || async { 3 }).await, 3);
}

/// A read that counts itself and answers once `gate` gives it a permit of its own
async fn one_permit_read(reads: &AtomicUsize, gate: &Semaphore) -> usize {
  let number = reads.fetch_add(1, Ordering::SeqCst) + 1;
  gate.acquire().await.unwrap().forget();
  number
}

/// A caller that asks while a read runs waits for it to end and gets the answer of the next read, which every caller
/// that asked meanwhile shares; a caller while none runs leads at once
#[tokio::test]
async fn callers_that_ask_while_a_read_runs_share_the_read_after_it() {
  let flight = NextFlight::<usize>::default();
  let (reads, gate) = (AtomicUsize::new(0), Semaphore::new(0));
  let ask = || flight.run(|| one_permit_read(&reads, &gate));
  let first = ask();
  tokio::pin!(first);
  // the first read runs, held by the gate
  assert!(futures_poll(&mut first).await.is_none());
  assert_eq!(reads.load(Ordering::SeqCst), 1);
  let release = async {
    // both later callers wait before the first read answers; then the follow-up runs once
    tokio::task::yield_now().await;
    gate.add_permits(1);
    while reads.load(Ordering::SeqCst) < 2 {
      tokio::task::yield_now().await;
    }
    gate.add_permits(1);
  };
  let (first, second, third, ()) = tokio::join!(first, ask(), ask(), release);
  assert_eq!((first, second, third), (1, 2, 2), "the later callers share the read after the first");
  assert_eq!(reads.load(Ordering::SeqCst), 2);

  // nothing runs: a caller leads at once
  gate.add_permits(1);
  assert_eq!(ask().await, 3);
}

/// A leader of a follow-up that went away is replaced by a caller that waited for it, with its own read
#[tokio::test]
async fn a_follow_up_whose_leader_went_away_is_read_by_a_caller_that_waited() {
  let flight = NextFlight::<usize>::default();
  let (reads, gate) = (AtomicUsize::new(0), Semaphore::new(0));
  let first = flight.run(|| one_permit_read(&reads, &gate));
  tokio::pin!(first);
  assert!(futures_poll(&mut first).await.is_none());
  // boxed, so its drop ends it
  let mut gone = Box::pin(flight.run(|| one_permit_read(&reads, &gate)));
  let waiting = flight.run(|| one_permit_read(&reads, &gate));
  tokio::pin!(waiting);
  assert!(futures_poll(&mut gone).await.is_none());
  assert!(futures_poll(&mut waiting).await.is_none());
  gate.add_permits(1);
  assert_eq!(first.await, 1);
  // the follow-up starts with the first caller that polls: `gone`, which then goes away
  assert!(futures_poll(&mut gone).await.is_none());
  assert_eq!(reads.load(Ordering::SeqCst), 2);
  drop(gone);
  gate.add_permits(1);
  assert_eq!(waiting.await, 3, "the waiting caller read the follow-up itself");
}

/// One poll of `future`: its answer, or None while it waits
async fn futures_poll<F: Future + Unpin>(future: &mut F) -> Option<F::Output> {
  std::future::poll_fn(|cx| {
    std::task::Poll::Ready(match std::pin::Pin::new(&mut *future).poll(cx) {
      std::task::Poll::Ready(answer) => Some(answer),
      std::task::Poll::Pending => None,
    })
  })
  .await
}
