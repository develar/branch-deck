//! The clock of the timers that decide what to ask and when to drop something (docs/design/rust-domain-authority.md,
//! "Факты", "Какие часы"): the life of a repository session without subscribers today, the cadence of the polls and the
//! age of the registry's entries later. `SystemClock` is the monotonic clock of the process; `TestClock` stands still
//! until a test (or the E2E server) moves it, so no test sleeps for a timer. What only bounds the delay of a delivery
//! (the 16 ms merge of the patches, the bounded wait of a file lock) runs on `Instant` directly, never on this clock.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// A monotonic clock: `now` never goes back
pub trait Clock: Send + Sync {
  /// The time since the clock started
  fn now(&self) -> Duration;
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
}

/// A clock that moves only by `advance`; its clones share the time
#[derive(Debug, Clone, Default)]
pub struct TestClock {
  nanos: Arc<AtomicU64>,
}

impl TestClock {
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// Moves the clock forward by `by` (saturating at about 584 years)
  pub fn advance(&self, by: Duration) {
    let by = u64::try_from(by.as_nanos()).unwrap_or(u64::MAX);
    // `try_update` with a closure that always answers `Some` cannot fail
    let _ = self.nanos.try_update(Ordering::SeqCst, Ordering::SeqCst, |nanos| Some(nanos.saturating_add(by)));
  }
}

impl Clock for TestClock {
  fn now(&self) -> Duration {
    Duration::from_nanos(self.nanos.load(Ordering::SeqCst))
  }
}
