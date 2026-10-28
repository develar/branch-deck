//! The types that cross a process or window boundary: what the cores hand to `bd` (JSON) and to the app (Tauri
//! commands and events, `src/lib/bindings.ts`), and what the page sends back. Data only, no IO: serde, and behind the
//! features of the same names `specta` (the bindings of the app) and `schemars` (`bd schema`, which `bd-core` enables).
//! `git-ops` depends on this crate, never the other way round: it reads and reports these types, the functions that
//! build them stay there.
//!
//! - the crate root: the events of a sync (`SyncEvent`, `ProgressReporter`), the statuses of its branches and commits
//!   (`BranchSyncStatus`, `CommitSyncStatus`, `BranchError`), its grouped branches, the remote status of a branch and
//!   what a push of it does (`PushAction`), the unapplied branches, the dropped commits and the landed remote commits
//! - `actions`: the actions of a subject by their ids and whether each can run now, as `domain_policy::actions` decides
//! - `activity`: what the page records in the journal of the app data directory (`PageEntry`), and the log Activity
//!   reads of it and of the journals of the repositories (`ActivityLog`)
//! - `branch_integration`: the integration of a branch into the baseline (rebase, merge, squash)
//! - `calendar`: the local weekday, hour and week of a moment, in a time zone the host names
//! - `branch_names`: the names of the branches under the prefix (`{prefix}/virtual/<name>`, `BranchKind`), the prefix,
//!   a group name valid in a ref
//! - `branch_name_suggestion`: branch names suggested by the user's Claude Code
//! - `cli_tools`: the `teamcity` and `youtrack` CLIs of Settings → Tools
//! - `commit`: a commit of the branch as `git-ops` reads it (`Commit`, its mapping note), and as the page names it
//! - `conflict`: the merge conflict of a sync, its analysis, and the conflicts a copy resolved by words
//! - `fixtures` (the feature of the same name, for the tests of other crates): builders of robots
//! - `error`: the stable error codes of `bd` and the app (`ErrorCode`), their exit codes and retries, the error a command
//!   of the app answers (`ApiError`)
//! - `issue_navigation`: the issue links of the IntelliJ IDEA configuration
//! - `landed`: the landed commits and branches
//! - `landing`: what refuses a Safe Push or a Dry Run before the push, and the fix of each refusal
//! - `operation`: the operations of the remote: what is asked (`Intent`), what it would do (`Plan`), how it ended
//!   (`Outcome`), what its caller hears (`OperationEvent`), the operations of a session that run or wait
//!   (`RunningOperation`) with the resources they hold and the admission of a new one (`Admission`)
//! - `patronus`: the Patronus robots of the virtual branches, the parameters of their start, the review readiness
//! - `pull_request`: the pull requests of the branches, their checks and reviews
//! - `repository_view`: what Rust derives of the branch table for the page and `bd`
//! - `review_request`: the review requests of Your Turn
//! - `robot_failure`: why a robot failed and the commits it checked
//! - `robot_landing`: what a merged robot landed and what a running one lands
//! - `robot_stats`: the Safe Push statistics, the robots they keep and the command of the app
//! - `session`: what the session of a repository sends a window: the snapshot of the branch table, its parts and its
//!   versions, and the patches that follow it
//! - `settings`: the settings of the app that Rust reads (`AppSettings`), the branch prefix of git and the resolved one
//!   with its source
//! - `setup`: what in the setup of the machine waits for the user (the Setup dot)
//! - `stack`: the stacked branches as a sync reports them, the syntax of a stack prefix (`(auth/login)`)
//! - `update`: the update of the app
//! - `update_project`: Update Project, the check of the branch against its upstream and the applied update
//! - `upstream_commit`: a commit of the baseline that carries the user's work
//! - `youtrack`: the YouTrack sign-in and issues

use crate::commit::Commit;
use crate::conflict::{MergeConflictInfo, WordResolutionSummary};
use crate::issue_navigation::IssueNavigationConfig;
use crate::session::StoreTips;
use crate::update_project::UpdateCheck;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod actions;
pub mod activity;
pub mod branch_integration;
pub mod branch_name_suggestion;
pub mod branch_names;
pub mod calendar;
pub mod cli_tools;
pub mod commit;
pub mod commit_body;
pub mod conflict;
pub mod error;
#[cfg(feature = "fixtures")]
pub mod fixtures;
pub mod issue_navigation;
pub mod landed;
pub mod landing;
pub mod next_step;
pub mod operation;
pub mod patronus;
pub mod pull_request;
pub mod reason;
pub mod repository_view;
pub mod review_request;
pub mod rewrite;
pub mod robot_failure;
pub mod robot_landing;
pub mod robot_stats;
pub mod session;
pub mod settings;
pub mod setup;
pub mod stack;
pub mod update;
pub mod update_project;
pub mod upstream_commit;
pub mod youtrack;

/// Status of a branch synchronization operation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum BranchSyncStatus {
  Created,
  Updated,
  Unchanged,
  Error,
  MergeConflict,
  AnalyzingConflict,
  /// Not rebuilt: the parent branch of a stacked branch can't be built on
  Blocked,
}

/// Status of a commit synchronization.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum CommitSyncStatus {
  Pending,
  Created,
  Unchanged,
  Error,
  Blocked,
}

/// Branch operation errors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum BranchError {
  Generic(String),
  MergeConflict(Box<MergeConflictInfo>),
}

/// A branch an unapply kept: its commits are not in HEAD, it lands nothing until it is reapplied
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UnappliedBranchInfo {
  /// The full name, `{prefix}/unapplied/<name>`
  pub name: String,
  /// `<name>`: the branch it was, a `-N` suffix when that name was taken
  pub branch: String,
  pub tip: String,
  /// The subject of its tip
  pub subject: String,
  /// A worktree has it checked out
  pub checked_out: bool,
  /// Its remote status, when its remote branch exists (a plain push of it); `None` when it is not on the remote
  pub remote: Option<RemoteStatusUpdate>,
}

/// Remote branch status information, by content: the baseline and patch-equivalent (rebased) commits don't count
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
  /// Of the commits behind, the ones whose author is not the user (`my_email` of the status, the branch's author):
  /// someone else's work on the remote branch, which the branch should take in (Pick Remote Commits) rather than drop
  pub others_behind: u32,
  /// The newest of the commits behind, newest first, at most `remote_status::DROPPED_COMMITS_LIMIT`, all of them when
  /// only one more (`commits_behind` is the total)
  pub dropped_commits: Vec<DroppedCommit>,
  /// Commits of the remote that are older versions of commits amended here (author, author time and subject match an
  /// unpushed commit of the branch, the changes differ; a copy with the same changes on moved lines counts nowhere): a
  /// push replaces them
  pub commits_amended: u32,
  /// Commits of the remote that are older versions of commits of the branches a stacked branch is built on (they were
  /// amended or rebuilt since the branch was pushed): a push replaces them
  pub commits_restacked: u32,
  /// Commits of the remote that are older versions of commits the baseline has above the remote branch (author, author
  /// time and subject match a baseline commit, as Update Project finds landed commits; the changes may differ, as when a
  /// Safe Push landed them rebuilt on a newer target): a push replaces them
  pub commits_landed: u32,
  /// The newest of the landed commits, newest first, at most `remote_status::DROPPED_COMMITS_LIMIT`, all of them when
  /// only one more (`commits_landed` is the total)
  pub landed_commits: Vec<LandedRemoteCommit>,
  /// Commits of the branch the remote has by content in another version: their message or author changed here (a
  /// reword), so a push replaces the remote's copies, which a robot would land as they are
  pub commits_reworded: u32,
  /// Commits of the branches a stacked branch is built on whose content its remote branch lacks (a new commit of the
  /// parent since the branch was pushed): a Safe Push of the remote branch lands an older piece, so a push replaces it.
  /// Not among `unpushed_commits`, which are the branch's own; 0 for a branch without a remote branch
  pub ancestor_commits_ahead: u32,
  /// Of the unpushed commits, the ones authored by the current user
  pub my_unpushed_count: u32,
  /// The remote branch is an ancestor of the local one; false for a rebased branch, which a push overwrites
  pub fast_forward: bool,
  /// Last time this branch was pushed to the remote (Unix timestamp, 0 = never pushed)
  pub last_push_time: u32,
  /// What a push of the branch does over this status: `domain_policy::push::action` of the counts above, set where the
  /// status is computed (`sync_core::remote_status`), so the app and `bd` read one answer
  pub action: PushAction,
}

/// What a push of a branch does to its remote branch, by its remote status (`domain_policy::push::action`): the first
/// push creates it, a fast-forward pushes, any other push forces, and a remote that has the branch as it is, by content,
/// needs none
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PushAction {
  /// No remote branch: the push creates it
  Create,
  /// The remote branch is an ancestor of the branch and lacks some of its commits
  Push,
  /// Not a fast-forward: the branch was rebased, the remote has older versions of its commits (`replaces`: amended,
  /// reworded, restacked or landed ones, or an older piece of a stacked branch) or commits it lacks (`drops`: someone's
  /// work, which the push asks for first)
  Force { drops: bool, replaces: bool },
  /// The remote has the branch as it is, by content: nothing to push
  UpToDate,
}

/// A commit of the remote branch whose content the local branch lacks: a push drops it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DroppedCommit {
  pub id: String,
  pub subject: String,
  /// The author's name
  pub author: String,
  pub author_email: String,
  /// Unix timestamp
  pub author_time: u32,
}

/// A commit of the remote branch that is an older version of a commit of the baseline: a push replaces it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandedRemoteCommit {
  pub id: String,
  pub subject: String,
  /// The baseline commit with its identity
  pub baseline_commit: String,
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
    /// Branch -> note: the parked branches (`refs/branch-deck/parked`), kept local and never pushed; also the lines of
    /// branches that group no commit on this HEAD
    #[cfg_attr(feature = "specta", specta(optional))]
    parked: BTreeMap<String, String>,
    /// The roots of the stack branches this sync reports, each with one `StackBranchUpdate` later: the stacks that fork
    /// and the stack branches kept for their remote branch; any other stack branch is gone
    #[cfg_attr(feature = "specta", specta(optional))]
    stack_branches: Vec<String>,
    /// The branch of origin the baseline tracks, which the robots of Patronus push into (`master` for
    /// `origin/master`); none for a baseline that is no branch of origin
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(optional))]
    target_branch: Option<String>,
    /// The commit HEAD pointed at when the commits were grouped
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(optional))]
    head: String,
    /// The checked out branch
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(optional))]
    branch: String,
    /// The tips of the refs of Branch Deck the commits were grouped with
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(optional))]
    store_tips: StoreTips,
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
    /// The conflict of git that the copy resolved by words (`branchdeck.resolveByWords`); null for a copy without one
    #[cfg_attr(feature = "specta", specta(optional))]
    resolved: Option<WordResolutionSummary>,
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
  /// The unapplied branches (`{prefix}/unapplied/<name>`), newest first, with the remote status of those on the remote;
  /// sent once per sync, also empty, so the list clears
  #[serde(rename_all = "camelCase")]
  UnappliedBranchesFound { branches: Vec<UnappliedBranchInfo> },
  /// Sent when remote branch status is checked
  #[serde(rename_all = "camelCase")]
  RemoteStatusUpdate(RemoteStatusUpdate),
  /// Sent for each stack branch `BranchesGrouped` announced, once the branches of its stack are built: its state, its
  /// tip and its remote status
  #[serde(rename_all = "camelCase")]
  StackBranchUpdate(Box<stack::StackBranchUpdate>),
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

/// A shared reporter reports to the one it shares: a host that picks the reporter of a run at run time passes an
/// `Arc<dyn ProgressReporter>`
impl<T: ProgressReporter + ?Sized> ProgressReporter for std::sync::Arc<T> {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    (**self).send(event)
  }
}
