//! The clock of the timers that decide what to ask and when to drop something (docs/design/rust-domain-authority.md,
//! "Факты", "Какие часы"): the life of a repository session without subscribers, the cadence of the polls of its facts
//! and the age of the registry's entries later. `SystemClock` is the monotonic clock of the process; `TestClock` stands
//! still until a test (or the E2E server, `POST /clock/advance`) moves it, so no test sleeps for a timer. What only
//! bounds the delay of a delivery (the 16 ms merge of the patches, the bounded wait of a file lock) runs on `Instant`
//! directly, never on this clock.
//!
//! A timer waits by `Clock::until`: the wall time to sleep on the system clock, a future that `TestClock::advance` wakes
//! on the test clock. The crate has no runtime (the settings and the tools crates, which reach no tokio, depend on it):
//! the sleep of the wall time is the caller's (`repository_model::runtime::sleep_until`).

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

/// A monotonic clock: `now` never goes back
pub trait Clock: Send + Sync {
  /// The time since the clock started
  fn now(&self) -> Duration;

  /// How a timer waits until `at` on this clock
  fn until(&self, at: Duration) -> Wait;
}

/// How a timer waits until a time of its clock
#[derive(Debug)]
pub enum Wait {
  /// The time has come
  Elapsed,
  /// This much wall time (the system clock)
  Wall(Duration),
  /// Until the test clock moved there
  Signal(TestWait),
}

/// The monotonic clock of the process
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
  start: Instant,
}

impl SystemClock {
  #[must_use]
  pub fn new() -> Self {
    Self { start: Instant::now() }
  }
}

impl Default for SystemClock {
  fn default() -> Self {
    Self::new()
  }
}

impl Clock for SystemClock {
  fn now(&self) -> Duration {
    self.start.elapsed()
  }

  fn until(&self, at: Duration) -> Wait {
    match at.checked_sub(self.now()) {
      Some(left) if !left.is_zero() => Wait::Wall(left),
      _ => Wait::Elapsed,
    }
  }
}

/// A clock that moves only by `advance`; its clones share the time and the timers that wait on it
#[derive(Clone, Default)]
pub struct TestClock {
  time: Arc<TestTime>,
}

/// The time of a test clock and the wakers of the timers that wait for it to move
#[derive(Default)]
struct TestTime {
  nanos: AtomicU64,
  waiters: Mutex<HashMap<u64, Waker>>,
  last_waiter: AtomicU64,
}

impl TestTime {
  fn now(&self) -> Duration {
    Duration::from_nanos(self.nanos.load(Ordering::SeqCst))
  }

  fn waiters(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Waker>> {
    self.waiters.lock().unwrap_or_else(PoisonError::into_inner)
  }
}

impl fmt::Debug for TestClock {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.debug_struct("TestClock").field("now", &self.time.now()).finish_non_exhaustive()
  }
}

impl TestClock {
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// Moves the clock forward by `by` (saturating at about 584 years) and wakes every timer that waits on it; each one
  /// looks at the time itself and waits on when its time has not come
  pub fn advance(&self, by: Duration) {
    let by = u64::try_from(by.as_nanos()).unwrap_or(u64::MAX);
    // `try_update` with a closure that always answers `Some` cannot fail
    let _ = self.time.nanos.try_update(Ordering::SeqCst, Ordering::SeqCst, |nanos| Some(nanos.saturating_add(by)));
    let waiters: Vec<Waker> = self.time.waiters().drain().map(|(_, waker)| waker).collect();
    for waker in waiters {
      waker.wake();
    }
  }
}

impl Clock for TestClock {
  fn now(&self) -> Duration {
    self.time.now()
  }

  fn until(&self, at: Duration) -> Wait {
    if self.now() >= at {
      return Wait::Elapsed;
    }
    let id = self.time.last_waiter.fetch_add(1, Ordering::SeqCst);
    Wait::Signal(TestWait {
      time: Arc::clone(&self.time),
      at,
      id,
    })
  }
}

/// A timer on a test clock: ready once the clock is at `at` or past it, woken by `TestClock::advance`
pub struct TestWait {
  time: Arc<TestTime>,
  at: Duration,
  /// Its waker's key among the clock's waiters
  id: u64,
}

impl fmt::Debug for TestWait {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.debug_struct("TestWait").field("at", &self.at).finish_non_exhaustive()
  }
}

impl Future for TestWait {
  type Output = ();

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
    if self.time.now() >= self.at {
      self.time.waiters().remove(&self.id);
      return Poll::Ready(());
    }
    self.time.waiters().insert(self.id, context.waker().clone());
    // an advance between the check and the insert woke nobody: look again
    if self.time.now() >= self.at {
      self.time.waiters().remove(&self.id);
      return Poll::Ready(());
    }
    Poll::Pending
  }
}

impl Drop for TestWait {
  fn drop(&mut self) {
    self.time.waiters().remove(&self.id);
  }
}
