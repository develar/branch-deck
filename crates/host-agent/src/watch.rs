//! The wait of a follow for a change in its run: a write to `events.log`, or a new file in the run directory, such as
//! `exit.json`. On macOS a kqueue watches both, with `EVFILT_VNODE` (`sys::kqueue`). Elsewhere, and after any kqueue
//! error, the wait is a poll of [`tuning::watch_poll`].
//!
//! A follow makes its watch before its first read of the run files. A change after the watch exists ends the next wait,
//! also when it came before that wait started

use crate::sys::kqueue::Queue;
use crate::tuning;
use std::path::Path;
use std::thread;
use std::time::Duration;

/// The watch of one run
#[derive(Debug)]
pub(crate) struct Watch {
  /// `None` for a poll
  queue: Option<Queue>,
}

impl Watch {
  /// Watches the files `paths`. Without a kqueue, or when a file cannot be opened, the watch polls
  pub(crate) fn new(paths: &[&Path]) -> Self {
    Self { queue: Queue::open(paths).ok() }
  }

  /// Whether a change ends a wait at once. `false` means a poll
  #[cfg(test)]
  pub(crate) const fn is_pushed(&self) -> bool {
    self.queue.is_some()
  }

  /// Waits for a change, up to `timeout`. A poll waits up to [`tuning::watch_poll`]. A kqueue error turns this watch into a poll
  pub(crate) fn wait(&mut self, timeout: Duration) {
    if let Some(queue) = &self.queue {
      if queue.wait(timeout).is_ok() {
        return;
      }
      self.queue = None;
    }
    thread::sleep(timeout.min(tuning::watch_poll()));
  }
}

#[cfg(test)]
#[path = "watch_test.rs"]
mod watch_test;
