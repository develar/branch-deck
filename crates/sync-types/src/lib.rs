use crate::issue_navigation::IssueNavigationConfig;
use git_ops::commit_list::Commit;
use git_ops::model::{BranchError, BranchSyncStatus, CommitSyncStatus};
use git_ops::update_branch::UpdateCheck;
use serde::Serialize;
use std::collections::BTreeMap;

pub mod branch_integration;
pub mod branch_name_suggestion;
pub mod issue_navigation;
pub mod landed;
pub mod patronus;
pub mod pull_request;
pub mod review_request;
pub mod robot_failure;
pub mod stack;
pub mod upstream_commit;

/// Remote branch status information, by content: the baseline and patch-equivalent (rebased) commits don't count
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatusUpdate {
  pub branch_name: String,
  pub remote_exists: bool,
  /// The commit the remote branch points to, the one the status was computed against; `None` when it does not exist.
  /// A push leases on it, so it replaces the remote branch only as the status showed it
  pub remote_tip: Option<String>,
  /// Commits of the branch whose content the remote lacks, oldest first; all the commits of a branch not on the remote
  pub unpushed_commits: Vec<String>,
  /// Commits of the remote whose content the branch lacks and that are no older version of a commit it has: a push drops
  /// them
  pub commits_behind: u32,
  /// The newest of the commits behind, newest first, at most `remote_status::DROPPED_COMMITS_LIMIT`
  /// (`commits_behind` is the total)
  pub dropped_commits: Vec<DroppedCommit>,
  /// Commits of the remote that are older versions of commits amended here (author, author time and subject match an
  /// unpushed commit of the branch, the changes differ; a copy with the same changes on moved lines counts nowhere): a
  /// push replaces them
  pub commits_amended: u32,
  /// Commits of the remote that are older versions of commits of the branches a stacked branch is built on (they were
  /// amended or rebuilt since the branch was pushed): a push replaces them
  pub commits_restacked: u32,
  /// Of the unpushed commits, the ones authored by the current user
  pub my_unpushed_count: u32,
  /// The remote branch is an ancestor of the local one; false for a rebased branch, which a push overwrites
  pub fast_forward: bool,
  /// Last time this branch was pushed to the remote (Unix timestamp, 0 = never pushed)
  pub last_push_time: u32,
}

/// A commit of the remote branch whose content the local branch lacks: a push drops it
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DroppedCommit {
  pub id: String,
  pub subject: String,
  /// The author's name
  pub author: String,
  /// Unix timestamp
  pub author_time: u32,
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
    /// Stacked branches are on (`branchdeck.stacks`): prefixes are read as `(parent/leaf)`
    #[cfg_attr(feature = "specta", specta(optional))]
    stacks: bool,
    /// Issue key -> branch name: the issue groups (`refs/branch-deck/issue-groups`) the commits were grouped with
    #[cfg_attr(feature = "specta", specta(optional))]
    issue_groups: BTreeMap<String, String>,
    /// Child -> parent: the stack parents (`refs/branch-deck/stack-parents`) the branches were placed with; empty with
    /// stacked branches off
    #[cfg_attr(feature = "specta", specta(optional))]
    stack_parents: BTreeMap<String, String>,
    /// The roots of the stack branches this sync reports, each with one `StackBranchUpdate` later: the stacks that fork
    /// and the stack branches kept for their remote branch; any other stack branch is gone
    #[cfg_attr(feature = "specta", specta(optional))]
    stack_branches: Vec<String>,
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
    error: Option<BranchError>,
  },
  /// Unified per-branch detection event for any status (Integrated, Orphaned, `NotIntegrated`, Partial)
  #[serde(rename_all = "camelCase")]
  BranchIntegrationDetected { info: branch_integration::BranchIntegrationInfo },
  /// Sent immediately when archived branches are found (before expensive detection)
  #[serde(rename_all = "camelCase")]
  ArchivedBranchesFound {
    branch_names: Vec<String>,
    /// The branches this sync archived, a subset of `branch_names`
    newly_archived: Vec<String>,
  },
  /// Sent when remote branch status is checked
  #[serde(rename_all = "camelCase")]
  RemoteStatusUpdate(RemoteStatusUpdate),
  /// Sent for each stack branch `BranchesGrouped` announced, once the branches of its stack are built: its state, its
  /// tip and its remote status
  #[serde(rename_all = "camelCase")]
  StackBranchUpdate(stack::StackBranchUpdate),
  /// Sent once after `BranchesGrouped`: whether the checked out branch can be updated onto the last fetched baseline
  /// (checked without the network). Both fields `None` for a local baseline, which has nothing to update from.
  #[serde(rename_all = "camelCase")]
  BaselineStatus {
    baseline_branch: String,
    update: Option<Box<UpdateCheck>>,
    /// Why the check failed, e.g. a rebase in progress; the sync itself does not fail
    check_error: Option<String>,
    /// Local commits that the fetched baseline already has: the update drops them
    landing: Vec<landed::LandedBranch>,
    /// Commits that landed with an update (by Branch Deck or elsewhere) and that the user has not dismissed yet
    landed: Vec<landed::LandedBranch>,
  },
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GroupedBranchInfo {
  pub name: String,
  pub commits: Vec<Commit>,
  pub latest_commit_time: u32,
  pub summary: String,
  /// The title of the branch's description (`branch.<name>.description`, which `bd describe` writes), shown instead of
  /// the summary; `None` without one, and from `group_commits_ahead_of_baseline`, whose callers read the whole
  /// descriptions themselves. Optional in TypeScript, as `stack`
  #[cfg_attr(feature = "specta", specta(optional))]
  pub description_title: Option<String>,
  pub all_commits_have_issue_references: bool,
  /// Most frequent author email in this branch's commits
  pub my_email: Option<String>,
  pub origin: BranchOrigin,
  /// The issue the branch belongs to: the name of an issue branch, or the key every commit of a prefix branch carries
  pub issue_key: Option<String>,
  /// Where the branch sits in its stack; `None` without stacked branches (`branchdeck.stacks`). Optional in TypeScript, so
  /// code that builds a `GroupedBranchInfo` needn't know about stacks
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stack: Option<stack::BranchStack>,
}

/// What groups the commits of a branch
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum BranchOrigin {
  /// An explicit `(name)` subject prefix
  Prefix,
  /// The issue key of the subjects (`ABC-123: ...`), no explicit prefix
  Issue,
  /// The issue groups (`refs/branch-deck/issue-groups`) put the commits of other issue keys into the branch; no
  /// explicit prefix
  IssueGroup,
}

/// Progress reporter trait that abstracts away Tauri-specific channel
pub trait ProgressReporter: Send + Sync {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()>;
}
