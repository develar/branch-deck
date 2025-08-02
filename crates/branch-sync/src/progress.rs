use crate::issue_navigation::IssueNavigationConfig;
use git_ops::commit_list::Commit;
use git_ops::model::{BranchError, BranchSyncStatus};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum SyncEvent {
  /// Sent at the beginning with issue navigation configuration if found
  IssueNavigationConfig { config: Option<IssueNavigationConfig> },
  /// Sent immediately after grouping commits
  BranchesGrouped { branches: Vec<GroupedBranchInfo> },
  /// Sent for commits that don't match any prefix pattern
  UnassignedCommits { commits: Vec<Commit> },
  /// Sent when a commit is successfully cherry-picked
  #[serde(rename_all = "camelCase")]
  CommitSynced {
    branch_name: String,
    commit_hash: String,
    new_hash: String,
    status: git_ops::CommitSyncStatus,
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
  /// Final completion event
  Completed,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GroupedBranchInfo {
  pub name: String,
  pub commits: Vec<Commit>,
  pub latest_commit_time: u32,
}

/// Progress reporter trait that abstracts away Tauri-specific channel
pub trait ProgressReporter: Send + Sync {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()>;
}
