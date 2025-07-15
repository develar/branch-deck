use crate::git::model::{BranchError, CommitDetail};
use serde::Serialize;
use specta::Type;

#[derive(Clone, Serialize, Type)]
#[serde(tag = "type", content = "data")]
pub enum SyncEvent {
  /// Initial progress message
  Progress { message: String, index: i16 },
  /// Sent immediately after grouping commits
  BranchesGrouped { branches: Vec<GroupedBranchInfo> },
  /// Sent for commits that don't match any prefix pattern
  UnassignedCommits { commits: Vec<CommitDetail> },
  /// Sent when a commit is successfully cherry-picked
  CommitSynced {
    branch_name: String,
    commit_hash: String,
    new_hash: String,
    status: crate::git::model::CommitSyncStatus,
  },
  /// Sent when a commit fails to cherry-pick
  CommitError { branch_name: String, commit_hash: String, error: BranchError },
  /// Sent to mark commits as blocked due to earlier error
  CommitsBlocked { branch_name: String, blocked_commit_hashes: Vec<String> },
  /// Sent when a branch status changes (including during processing and completion)
  BranchStatusUpdate {
    branch_name: String,
    status: crate::git::model::BranchSyncStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<BranchError>,
  },
  /// Final completion event
  Completed,
}

#[derive(Clone, Serialize, Type)]
pub struct GroupedBranchInfo {
  pub name: String,
  pub commits: Vec<CommitDetail>,
}
