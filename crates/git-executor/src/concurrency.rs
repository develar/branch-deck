use std::sync::{Arc, Condvar, Mutex, PoisonError};

/// Upper bound for git processes one operation runs at the same time (per-branch sync, integration detection).
/// Git work is mostly process startup and disk I/O, so more processes than cores only add contention.
pub fn max_parallel_git_processes() -> usize {
  std::thread::available_parallelism().map_or(4, std::num::NonZero::get).min(8)
}

/// The slots for the parallel git work of one operation, shared by all its parts: a sync builds its branches and
/// detects the integration of the archived ones at the same time, and both take their slots from one `ProcessPermits`,
/// so together they stay within `max_parallel_git_processes()`. A slot is one task's git work (a build, a detection
/// job), not one process. Blocking: a thread waits for a free slot
#[derive(Debug, Clone)]
pub struct ProcessPermits {
  inner: Arc<Permits>,
}

#[derive(Debug)]
struct Permits {
  capacity: usize,
  free: Mutex<usize>,
  released: Condvar,
}

impl ProcessPermits {
  /// `capacity` slots, at least one
  pub fn new(capacity: usize) -> Self {
    let capacity = capacity.max(1);
    Self {
      inner: Arc::new(Permits {
        capacity,
        free: Mutex::new(capacity),
        released: Condvar::new(),
      }),
    }
  }

  /// The slots of one operation: `max_parallel_git_processes()`
  pub fn for_operation() -> Self {
    Self::new(max_parallel_git_processes())
  }

  /// How many slots there are, free or not
  pub fn capacity(&self) -> usize {
    self.inner.capacity
  }

  /// Waits for a free slot; it is free again when the permit drops
  pub fn acquire(&self) -> ProcessPermit {
    let mut free = self.inner.free.lock().unwrap_or_else(PoisonError::into_inner);
    while *free == 0 {
      free = self.inner.released.wait(free).unwrap_or_else(PoisonError::into_inner);
    }
    *free -= 1;
    ProcessPermit { permits: self.clone() }
  }
}

/// A slot of `ProcessPermits`, held while the task runs its git work
#[derive(Debug)]
pub struct ProcessPermit {
  permits: ProcessPermits,
}

impl Drop for ProcessPermit {
  fn drop(&mut self) {
    let inner = &self.permits.inner;
    *inner.free.lock().unwrap_or_else(PoisonError::into_inner) += 1;
    inner.released.notify_one();
  }
}

#[cfg(test)]
#[path = "concurrency_test.rs"]
mod concurrency_test;
