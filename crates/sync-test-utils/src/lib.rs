//! Test utilities for branch-sync and integration-detection testing.

use anyhow::Result;
use std::sync::{Arc, Mutex};
use sync_types::{ProgressReporter, SyncEvent};

/// Progress reporter that captures sync events for verification.
#[derive(Clone)]
pub struct TestReporter {
  events: Arc<Mutex<Vec<SyncEvent>>>,
}

impl TestReporter {
  pub fn new() -> Self {
    Self {
      events: Arc::new(Mutex::new(Vec::new())),
    }
  }

  /// Get all events that have been sent
  pub fn get_events(&self) -> Vec<SyncEvent> {
    self.events.lock().unwrap().clone()
  }
}

impl Default for TestReporter {
  fn default() -> Self {
    Self::new()
  }
}

impl ProgressReporter for TestReporter {
  fn send(&self, event: SyncEvent) -> Result<()> {
    self.events.lock().unwrap().push(event);
    Ok(())
  }
}
