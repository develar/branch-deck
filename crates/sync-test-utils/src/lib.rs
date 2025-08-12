//! Test utilities for branch-sync and integration-detection testing.

use anyhow::Result;
use sync_types::{ProgressReporter, SyncEvent};
use test_utils::progress_reporter::TestProgressReporter;

/// Wrapper around TestProgressReporter that implements ProgressReporter.
/// This avoids the orphan rule since TestReporter is local to this crate.
#[derive(Clone)]
pub struct TestReporter(TestProgressReporter<SyncEvent>);

impl TestReporter {
  /// Create a new test reporter
  pub fn new() -> Self {
    Self(TestProgressReporter::new())
  }

  /// Get all events that have been sent
  pub fn get_events(&self) -> Vec<SyncEvent> {
    self.0.get_events()
  }

  /// Get the number of events
  pub fn event_count(&self) -> usize {
    self.0.event_count()
  }

  /// Clear all events
  pub fn clear_events(&self) {
    self.0.clear_events()
  }
}

impl Default for TestReporter {
  fn default() -> Self {
    Self::new()
  }
}

impl ProgressReporter for TestReporter {
  fn send(&self, event: SyncEvent) -> Result<()> {
    self.0.push_event(event);
    Ok(())
  }
}
