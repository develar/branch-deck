//! Fixtures of the sync tests (sync-core, repository-model, integration-detection-tests): a progress reporter that
//! records the sync events, and a grouped branch placed in a stack.

use anyhow::Result;
use domain_types::stack::BranchStack;
use domain_types::{BranchOrigin, GroupedBranchInfo, ProgressReporter, SyncEvent};
use std::sync::{Arc, Mutex};

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
    self.events.lock().map_err(|e| anyhow::anyhow!("the event collector is poisoned: {e}"))?.push(event);
    Ok(())
  }
}

/// A grouped branch without commits, placed on `effective_base` (with stacked branches on) and declaring
/// `declared_parent`
pub fn placed_branch(name: &str, effective_base: Option<&str>, declared_parent: Option<&str>) -> GroupedBranchInfo {
  GroupedBranchInfo {
    name: name.to_string(),
    commits: Vec::new(),
    latest_commit_time: 0,
    summary: String::new(),
    description_title: None,
    all_commits_have_issue_references: false,
    my_email: None,
    origin: BranchOrigin::Prefix,
    issue_key: None,
    stack: Some(BranchStack {
      declared_parent: declared_parent.map(str::to_string),
      effective_base: effective_base.map(str::to_string),
      ..BranchStack::default()
    }),
  }
}
