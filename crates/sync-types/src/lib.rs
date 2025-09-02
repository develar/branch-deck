use crate::issue_navigation::IssueNavigationConfig;
use git_ops::commit_list::Commit;
use git_ops::model::{BranchError, BranchSyncStatus, CommitSyncStatus};
use serde::Serialize;

pub mod branch_integration;
pub mod issue_navigation;

/// Remote branch status information
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatusUpdate {
  pub branch_name: String,
  pub remote_exists: bool,
  pub unpushed_commits: Vec<String>,
  pub commits_behind: u32,
  /// Number of commits ahead authored by the current user (derived during sync)
  pub my_unpushed_count: u32,
  /// Last time this branch was pushed to the remote (Unix timestamp, 0 = never pushed)
  pub last_push_time: u32,
}

/// Progress events for sync operations
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum SyncEvent {
  /// Sent at the beginning with issue navigation configuration if found
  IssueNavigationConfig { config: Option<IssueNavigationConfig> },
  /// Sent immediately after grouping commits
  #[serde(rename_all = "camelCase")]
  BranchesGrouped {
    branches: Vec<GroupedBranchInfo>,
    /// Repository's baseline branch (e.g., "origin/master", "master")
    baseline_branch: String,
  },
  /// Sent for commits that don't match any prefix pattern
  UnassignedCommits { commits: Vec<Commit> },
  /// Sent when a commit is successfully cherry-picked
  #[serde(rename_all = "camelCase")]
  CommitSynced {
    branch_name: String,
    commit_hash: String,
    new_hash: String,
    status: CommitSyncStatus,
  },
  /// Sent when a commit fails to cherry-pick
  #[serde(rename_all = "camelCase")]
  CommitError { branch_name: String, commit_hash: String, error: BranchError },
  /// Sent to mark commits as blocked due to earlier error
  #[serde(rename_all = "camelCase")]
  CommitsBlocked { branch_name: String, blocked_commit_hashes: Vec<String> },
  /// Sent when a branch status changes (including during processing and completion)
  #[serde(rename_all = "camelCase")]
  BranchStatusUpdate {
    branch_name: String,
    status: BranchSyncStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<BranchError>,
  },
  /// Unified per-branch detection event for any status (Integrated, Orphaned, NotIntegrated, Partial)
  #[serde(rename_all = "camelCase")]
  BranchIntegrationDetected { info: branch_integration::BranchIntegrationInfo },
  /// Sent immediately when archived branches are found (before expensive detection)
  #[serde(rename_all = "camelCase")]
  ArchivedBranchesFound { branch_names: Vec<String> },
  /// Sent when remote branch status is checked
  #[serde(rename_all = "camelCase")]
  RemoteStatusUpdate(RemoteStatusUpdate),
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GroupedBranchInfo {
  pub name: String,
  pub commits: Vec<Commit>,
  pub latest_commit_time: u32,
  pub summary: String,
  pub all_commits_have_issue_references: bool,
  /// Most frequent author email in this branch's commits
  pub my_email: Option<String>,
}

/// Progress reporter trait that abstracts away Tauri-specific channel
pub trait ProgressReporter: Send + Sync {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()>;
}

pub mod ordered_progress_reporter;
