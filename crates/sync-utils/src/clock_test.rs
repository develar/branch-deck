use crate::clock::{Clock, SystemClock, TestClock, TestWait, Wait};
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

#[test]
fn a_test_clock_moves_only_by_advance_and_its_clones_share_the_time() {
  let clock = TestClock::new();
  let shared = clock.clone();
  assert_eq!(clock.now(), Duration::ZERO);
  clock.advance(Duration::from_secs(120));
  assert_eq!(shared.now(), Duration::from_secs(120));
  shared.advance(Duration::from_millis(5));
  assert_eq!(clock.now(), Duration::from_millis(120_005));
  clock.advance(Duration::MAX);
  assert_eq!(clock.now(), Duration::from_nanos(u64::MAX));
}

#[test]
fn the_system_clock_never_goes_back() {
  let clock = SystemClock::new();
  let first = clock.now();
  assert!(clock.now() >= first);
}

/// A waker that counts how often it was woken
#[derive(Default)]
struct Wakes(AtomicUsize);

impl Wake for Wakes {
  fn wake(self: Arc<Self>) {
    self.0.fetch_add(1, Ordering::SeqCst);
  }
}

/// The timer of `clock` until `at`, a time to come, with a waker that counts its wakes
fn waiting(clock: &TestClock, at: Duration) -> (TestWait, Arc<Wakes>, Waker) {
  let Wait::Signal(wait) = clock.until(at) else {
    panic!("a time to come is a signal of the test clock");
  };
  let wakes = Arc::new(Wakes::default());
  let waker = Waker::from(Arc::clone(&wakes));
  (wait, wakes, waker)
}

/// A timer on the test clock is pending until an advance of any clone of the clock brings the time to it, which wakes
/// it; an advance short of it wakes it to look and wait on. A time that has come needs no wait
#[test]
fn a_timer_of_the_test_clock_wakes_on_the_advance_that_reaches_it_not_before() {
  let clock = TestClock::new();
  clock.advance(Duration::from_secs(10));
  assert!(matches!(clock.until(Duration::from_secs(10)), Wait::Elapsed));
  let (wait, wakes, waker) = waiting(&clock, Duration::from_secs(70));
  let mut context = Context::from_waker(&waker);
  let mut wait = pin!(wait);
  assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);
  assert_eq!(wakes.0.load(Ordering::SeqCst), 0);

  let other = clock.clone();
  other.advance(Duration::from_secs(30));
  assert_eq!(wakes.0.load(Ordering::SeqCst), 1, "woken to look");
  assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending, "40 s of 70");
  other.advance(Duration::from_secs(30));
  assert_eq!(wakes.0.load(Ordering::SeqCst), 2);
  assert_eq!(wait.as_mut().poll(&mut context), Poll::Ready(()));
  clock.advance(Duration::from_secs(1));
  assert_eq!(wakes.0.load(Ordering::SeqCst), 2, "a timer that ended is no waiter");
}

/// A clock that runs out (`auto_advance`) wakes the timer that waits on it, which ends, and moves to the time of each
/// timer asked after it, never back; each of those yields once before it ends, and the clock counts the timers it moved
/// to: a wait with a deadline, as the E2E server's operations of a deleted repository wait, ends after its last look,
/// and a loop without one yields at every look and shows in the count
#[test]
fn a_clock_that_runs_out_moves_to_each_timer_and_wakes_those_that_wait() {
  let clock = TestClock::new();
  let (wait, wakes, waker) = waiting(&clock, Duration::from_secs(60));
  let mut context = Context::from_waker(&waker);
  let mut wait = pin!(wait);
  assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);

  let other = clock.clone();
  other.auto_advance();
  assert_eq!(wakes.0.load(Ordering::SeqCst), 1, "woken to look");
  assert_eq!(wait.as_mut().poll(&mut context), Poll::Ready(()));
  assert_eq!((clock.now(), clock.run_out_waits()), (Duration::from_secs(60), 1), "at the time of the timer");
  ends_after_one_yield(&clock, Duration::from_secs(90));
  assert!(matches!(clock.until(Duration::from_secs(30)), Wait::Elapsed), "a time that came");
  assert_eq!((clock.now(), clock.run_out_waits()), (Duration::from_secs(90), 2), "never back");

  // a poll every 10 s until a deadline 60 s on looks six times
  let deadline = clock.now() + Duration::from_secs(60);
  let mut looks = 0;
  while clock.now() + Duration::from_secs(10) <= deadline {
    looks += 1;
    ends_after_one_yield(&clock, clock.now() + Duration::from_secs(10));
  }
  assert_eq!((looks, clock.run_out_waits()), (6, 8));
}

/// A timer until `at` of a clock that ran out: the clock is at `at` at once, and the timer ends at its second poll,
/// its first woke the task to poll it again
fn ends_after_one_yield(clock: &TestClock, at: Duration) {
  let Wait::Signal(wait) = clock.until(at) else {
    panic!("a timer of a clock that ran out yields once");
  };
  assert_eq!(clock.now(), at);
  let wakes = Arc::new(Wakes::default());
  let waker = Waker::from(Arc::clone(&wakes));
  let mut context = Context::from_waker(&waker);
  let mut wait = pin!(wait);
  assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);
  assert_eq!(wakes.0.load(Ordering::SeqCst), 1, "it asks to be polled again");
  assert_eq!(wait.as_mut().poll(&mut context), Poll::Ready(()));
}

/// The system clock waits the wall time left, and nothing for a time that has come
#[test]
fn the_system_clock_waits_the_wall_time_left() {
  let clock = SystemClock::new();
  assert!(matches!(clock.until(Duration::ZERO), Wait::Elapsed));
  let Wait::Wall(left) = clock.until(clock.now() + Duration::from_secs(60)) else {
    panic!("a time to come is wall time on the system clock");
  };
  assert!(left <= Duration::from_secs(60) && left > Duration::from_secs(59), "{left:?}");
}
