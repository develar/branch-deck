//! Fixtures of the sync and detection tests (sync-core, repository-model, branch-integration): a progress reporter
//! that records the sync events, a grouped branch placed in a stack, a local clone of an upstream, and the integration
//! results a detection reports or a detection note holds.

use crate::git_test_utils::TestRepo;
use crate::history::{HistoryCommit, HistoryStream};
use anyhow::Result;
use domain_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus, IntegrationConfidence};
use domain_types::stack::BranchStack;
use domain_types::{BranchOrigin, GroupedBranchInfo, ProgressReporter, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
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

/// An upstream repository with `main` at its initial commit and a local clone of it, so the tests run with a real
/// remote
pub fn setup_test_repos() -> (TestRepo, TestRepo, GitCommandExecutor) {
  let git_executor = GitCommandExecutor::new();

  let upstream_repo = TestRepo::new();
  // `master` and `main` at the initial commit from one stream: `fast-import` and `read-tree` where `add`, `commit` and
  // `branch` were three processes
  let mut history = HistoryStream::new("refs/heads/master");
  history.commit(&HistoryCommit::new("Initial commit", &[("README.md", "# Project")]));
  history.reset_to_root("refs/heads/main");
  history.write(upstream_repo.path()).unwrap();

  let local_repo = TestRepo::new_empty();
  local_repo.clone_from(upstream_repo.path()).unwrap();

  (upstream_repo, local_repo, git_executor)
}

/// Integration info of the detected branch `name`, from its `BranchIntegrationDetected` event
pub fn detected_branch_info<'a>(events: &'a [SyncEvent], name: &str) -> &'a BranchIntegrationInfo {
  events
    .iter()
    .find_map(|e| match e {
      SyncEvent::BranchIntegrationDetected { info } if info.name == name => Some(info),
      _ => None,
    })
    .expect("Should find integration info")
}

/// Integration info of a fully integrated branch, as a detection note holds it
pub fn integrated_branch_info() -> BranchIntegrationInfo {
  BranchIntegrationInfo {
    name: "test-branch".to_string(),
    summary: String::new(),
    status: BranchIntegrationStatus::Integrated {
      integrated_at: Some(0),
      confidence: IntegrationConfidence::High,
      commit_count: 1,
      commit: None,
    },
  }
}

/// Integration info of a branch whose commits are partly or not at all integrated, as a detection note holds it
pub fn not_integrated_branch_info(total_commit_count: u32, integrated_count: u32, orphaned_count: u32) -> BranchIntegrationInfo {
  BranchIntegrationInfo {
    name: "test-branch".to_string(),
    summary: String::new(),
    status: BranchIntegrationStatus::NotIntegrated {
      total_commit_count,
      integrated_count,
      orphaned_count,
      integrated_at: None,
    },
  }
}
