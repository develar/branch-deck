//! The latest request of one kind in each window (by webview label): an older one of the same window still running is
//! superseded, as is the latest after a cancel of the window or when the window goes. Windows ask apart: one never
//! cancels the request of another. The review fetch and the symbol search each keep one.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct WindowRequests {
  latest_requests: Mutex<HashMap<String, Arc<AtomicU64>>>,
}

/// A request of a window: superseded once the window starts a newer one, cancels it, or goes
pub struct WindowRequest {
  counter: Arc<AtomicU64>,
  request: u64,
}

impl WindowRequest {
  pub fn is_superseded(&self) -> bool {
    self.counter.load(Ordering::SeqCst) != self.request
  }
}

impl WindowRequests {
  fn counter(&self, label: &str) -> Arc<AtomicU64> {
    let mut latest = self.latest_requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    Arc::clone(latest.entry(label.to_string()).or_default())
  }

  /// Starts a request of the window, which supersedes the one before
  pub fn begin(&self, label: &str) -> WindowRequest {
    let counter = self.counter(label);
    let request = counter.fetch_add(1, Ordering::SeqCst) + 1;
    WindowRequest { counter, request }
  }

  /// Supersedes the request of the window in flight, if any
  pub fn cancel(&self, label: &str) {
    self.counter(label).fetch_add(1, Ordering::SeqCst);
  }

  /// The window went: its request is superseded and forgotten
  pub fn forget(&self, label: &str) {
    let removed = self.latest_requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(label);
    if let Some(counter) = removed {
      counter.fetch_add(1, Ordering::SeqCst);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::WindowRequests;

  #[test]
  fn a_window_supersedes_only_its_own_request() {
    let requests = WindowRequests::default();
    let main = requests.begin("main");
    let window = requests.begin("pull-request-review");

    // a newer request of the window, then a cancel of main: each supersedes only its own window's request
    let newer = requests.begin("pull-request-review");
    assert!(window.is_superseded());
    assert!(!main.is_superseded());
    requests.cancel("main");
    assert!(main.is_superseded());
    assert!(!newer.is_superseded());
  }

  #[test]
  fn a_closed_window_supersedes_its_request() {
    let requests = WindowRequests::default();
    let window = requests.begin("pull-request-review");
    requests.forget("pull-request-review");
    assert!(window.is_superseded());
    // a window opened again under the label starts afresh
    let again = requests.begin("pull-request-review");
    assert!(!again.is_superseded());
  }
}
