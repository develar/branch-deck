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

/// A grouped branch without commits, placed on `effective_base` and declaring
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

/// The time of the commits of `repo_with_word_changes`: two repositories made alike have the same commit ids
pub const WORD_CONFLICT_TIME: i64 = 1_700_000_000;

/// The baseline (`origin/master`) has `files`; alpha changes them by `alpha`, then beta by `beta`. Beta is copied onto
/// the baseline without alpha, so git conflicts wherever beta changes a line alpha changed
pub fn repo_with_word_changes(files: &[(&str, &str)], alpha: &[(&str, &str)], beta: &[(&str, &str)]) -> TestRepo {
  let repo = TestRepo::new();
  for (index, (path, content)) in files.iter().enumerate() {
    repo.create_commit_with_timestamp(&format!("Add {path}"), path, content, Some(WORD_CONFLICT_TIME + i64::try_from(index).unwrap()));
  }
  repo.git(&["remote", "add", "origin", "."]);
  let initial = repo.head();
  repo.git(&["update-ref", "refs/remotes/origin/master", &initial]);
  commit_files(&repo, "(alpha) Change the call", alpha, WORD_CONFLICT_TIME + 100);
  commit_files(&repo, "(beta) Change the call again", beta, WORD_CONFLICT_TIME + 200);
  repo
}

/// A commit of `files` written over the working tree with `message`, authored and committed at `time`; its id
pub fn commit_files(repo: &TestRepo, message: &str, files: &[(&str, &str)], time: i64) -> String {
  for (path, content) in files {
    std::fs::write(repo.path().join(path), content).unwrap();
  }
  let date = format!("{time} +0000");
  repo
    .git_executor()
    .cmd(&["commit", "-qam", message], repo.path_str())
    .env(&[("GIT_AUTHOR_DATE", date.as_str()), ("GIT_COMMITTER_DATE", date.as_str())])
    .text()
    .unwrap();
  repo.head()
}

/// alpha changes the first argument, beta the third: one conflict of git that the words resolve
pub fn repo_with_a_word_conflict() -> TestRepo {
  repo_with_word_changes(
    &[("Call.kt", "call(a, b, c)\nend\n")],
    &[("Call.kt", "call(A, b, c)\nend\n")],
    &[("Call.kt", "call(A, b, C)\nend\n")],
  )
}

/// The mapping note of `commit` with `version` in its resolve line (`v-commit-resolved-v1:<version> <conflicts>
/// <files>`) and, when `copy` is given, that commit as its copy: the note an older resolve by words wrote
pub fn with_resolve_version(repo: &TestRepo, commit: &str, version: u32, copy: Option<&str>) {
  let note = repo.git(&["notes", "show", commit]);
  let lines: Vec<String> = note
    .lines()
    .map(|line| {
      if let Some(counts) = line.strip_prefix("v-commit-resolved-v1:") {
        let (_, counts) = counts.split_once(' ').expect("a resolve line with its counts");
        format!("v-commit-resolved-v1:{version} {counts}")
      } else if let (Some(copy), true) = (copy, line.starts_with("v-commit-v1:")) {
        format!("v-commit-v1:{copy}")
      } else {
        line.to_string()
      }
    })
    .collect();
  assert!(lines.iter().any(|line| line.starts_with("v-commit-resolved-v1:")), "a note of a resolve by words: {note}");
  repo.git(&["notes", "add", "-f", "-m", &lines.join("\n"), commit]);
}
