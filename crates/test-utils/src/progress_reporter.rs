use std::sync::{Arc, Mutex};

/// Test progress reporter that captures events for verification
/// This replaces the duplicated TestProgressReporter in various test files
#[derive(Clone)]
pub struct TestProgressReporter<T> {
  events: Arc<Mutex<Vec<T>>>,
}

impl<T> TestProgressReporter<T> {
  pub fn new() -> Self {
    Self {
      events: Arc::new(Mutex::new(Vec::new())),
    }
  }

  pub fn get_events(&self) -> Vec<T>
  where
    T: Clone,
  {
    self.events.lock().unwrap().clone()
  }

  pub fn push_event(&self, event: T) {
    self.events.lock().unwrap().push(event);
  }

  pub fn event_count(&self) -> usize {
    self.events.lock().unwrap().len()
  }

  pub fn clear_events(&self) {
    self.events.lock().unwrap().clear();
  }
}

impl<T> Default for TestProgressReporter<T> {
  fn default() -> Self {
    Self::new()
  }
}
