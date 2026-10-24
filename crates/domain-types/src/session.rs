//! What the session of a repository sends a window (docs/design/rust-domain-authority.md, "Представление: снимок и
//! патчи"): a snapshot of the branch table (`TableView`) and the patches that follow it (`ViewPatch`). The table is cut
//! into parts that change for different reasons: of each branch its head and its commits (with a grouping), its sync
//! state (with its status and progress), the copies of its commits (with each commit the sync reports) and its remote
//! status; the stack branches by root; the sections below the table. A patch carries a part only when it differs from the one sent last, so the page
//! keeps the identity of every part it did not get.
//!
//! The names on the wire are the ones the page reads today (`branch-sync.ts`). The statuses are data: the texts, the
//! marks of what to expand and the badges are the page's presentation of them. A fact a service or a computation
//! answers is a `Fact` with its `FactState` (the rule of a fact without an answer).

use crate::branch_integration::BranchIntegrationInfo;
use crate::commit::Commit;
use crate::conflict::WordResolutionSummary;
use crate::error::ApiError;
use crate::issue_navigation::IssueNavigationConfig;
use crate::landed::LandedBranch;
use crate::stack::{BranchStack, StackBranchUpdate};
use crate::update_project::UpdateCheck;
use crate::{BranchError, BranchOrigin, BranchSyncStatus, CommitSyncStatus, RemoteStatusUpdate, UnappliedBranchInfo};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The version of what a window holds of a session. `epoch` is random per process and session, `seq` grows by one with
/// each patch. A patch applies only to the view at its `base`; any other view (a lost patch, another epoch) asks for a
/// snapshot. Both are `u32`, a number in TypeScript
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Version {
  pub epoch: u32,
  pub seq: u32,
}

impl Version {
  /// The version after this one, in the same epoch
  #[must_use]
  pub const fn next(self) -> Self {
    Self {
      epoch: self.epoch,
      seq: self.seq.wrapping_add(1),
    }
  }
}

/// A fact of a session that a service or a computation answers (docs/design/rust-domain-authority.md, "Факты"): the
/// value of the last answer, how it stands now, the number of the observation that answered it (`seq`, from the one
/// counter of the session: an answer applies only when its observation is newer) and when it answered (Unix seconds).
/// `value` is `None` until the first answer
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Fact<T> {
  pub value: Option<T>,
  pub state: FactState,
  pub seq: u32,
  pub observed_at: u32,
}

/// How a fact stands. A fact without an answer is never "no": a read that fails is `Unavailable` with its reason, never
/// an empty value, and never a `Loading` that lasts
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FactState {
  /// No answer yet: from the open of the session (or a change of what it is of) until the first one
  #[default]
  Loading,
  /// The value is the answer of the newest observation
  Fresh,
  /// The value is an earlier answer: the newest read failed (`reason`), or what the fact is of changed and it is being
  /// read or computed again (`None`); nothing waits for it
  Stale {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(optional))]
    reason: Option<ApiError>,
  },
  /// No answer: the read failed before any answer came
  Unavailable { reason: ApiError },
  /// Nothing asks for it (no window demands it)
  Unknown,
}

/// Why a session syncs (`repository_model::sync_scheduler` decides what runs)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SyncReason {
  /// ⌘R, the Sync button: always runs
  Manual,
  /// A window gained focus: runs when the inputs changed
  Focus,
  /// A rewrite, its undo or a fetch changed the branches: always runs, as a quiet follow-up while one is in flight
  AfterRewrite,
  /// A window opened the session: runs when the inputs changed, or joins the run in flight
  Open,
}

/// How the request of a sync ended, when it did not fail
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SyncOutcome {
  /// A run served the request and succeeded
  Synced,
  /// Nothing a sync reads changed since the last successful sync: no run
  Unchanged,
  /// A focus of a session that no window shows: nothing ran
  Skipped,
}

/// The answer of a request of a sync: how it ended, and the version of the view after the last patch of the run that
/// served it. The patch and the answer travel apart, so a window whose view is older waits for the patch that brings
/// it to `version`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SyncAnswer {
  pub outcome: SyncOutcome,
  pub version: Version,
}

/// The answer of a command whose result the session of its repository applied to the table (a push): its own outcome,
/// and the version of the view after the patch of that change, which a window waits for as it waits for the last patch
/// of a sync (`SyncAnswer`); `None` when no session of the repository is open (no window shows it). A command with no
/// outcome of its own (the deletion of an archived or unapplied branch) answers the version alone
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ViewAnswer<T> {
  pub outcome: T,
  pub version: Option<Version>,
}

impl<T> ViewAnswer<T> {
  /// The answer with the outcome `convert` makes of this one's, at the same version
  ///
  /// # Errors
  /// What `convert` failed with
  pub fn try_map<U, E>(self, convert: impl FnOnce(T) -> Result<U, E>) -> Result<ViewAnswer<U>, E> {
    Ok(ViewAnswer {
      outcome: convert(self.outcome)?,
      version: self.version,
    })
  }
}

/// A window's subscription to the patches of a session: its id (`unsubscribe_view`) and the view at the subscription,
/// which the first patch on the channel follows. A patch may reach the window before this answer: the window keeps it
/// until it has the view
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ViewSubscription {
  pub id: u32,
  pub view: TableView,
}

/// Whether a sync runs, and what it does
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SyncState {
  #[default]
  Idle,
  Running {
    step: SyncStep,
  },
  /// The last sync failed; the table is the one of the sync before
  Failed {
    error: ApiError,
  },
}

/// The step of a running sync, as its events tell it
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SyncStep {
  /// The commits ahead of the baseline are read and grouped
  Grouping,
  /// The branches are grouped and are being built
  Building,
  /// The branches and the stack branches are built; the detection of the archived branches or the update check still
  /// runs (both run beside the build, and no event says which one is left)
  Detection,
}

/// The snapshot of the branch table of a session
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TableView {
  pub version: Version,
  pub sync: SyncState,
  pub header: TableHeader,
  /// The names of the branches in the order of the table, as the sync grouped them (newest first)
  pub order: Vec<String>,
  pub branches: BTreeMap<String, BranchView>,
  /// The stack branches the last sync reported, by the root of their stack
  pub stacks: BTreeMap<String, StackBranchUpdate>,
  pub sections: TableSections,
}

/// What the table as a whole was grouped and checked with
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TableHeader {
  /// The baseline the sync compared with (`origin/master`); empty before the first grouping
  pub baseline_branch: String,
  /// Stacked branches are on (`branchdeck.stacks`)
  pub stacks_enabled: bool,
  /// Issue key -> branch name: the issue groups the commits were grouped with
  pub issue_groups: BTreeMap<String, String>,
  /// Child -> parent: the stack parents the branches were placed with
  pub stack_parents: BTreeMap<String, String>,
  /// Branch -> note: the parked branches
  pub parked: BTreeMap<String, String>,
  /// The issue links of the IntelliJ IDEA configuration, the last a sync found
  pub issue_navigation_config: Option<IssueNavigationConfig>,
}

/// The check of the checked out branch against the last fetched baseline
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BaselineStatus {
  pub baseline_branch: String,
  /// `None` for a local baseline, which has nothing to update from
  pub update: Option<UpdateCheck>,
  /// Why the check failed, e.g. a rebase in progress
  pub check_error: Option<String>,
}

/// A branch of the table, in the parts that change for different reasons
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchView {
  pub head: BranchHead,
  /// Newest first, as the sync grouped them; they change only with a grouping
  pub commits: Vec<Commit>,
  pub sync: BranchSync,
  /// What the sync made of each commit, in the order of `commits`
  pub copies: Vec<CommitSync>,
  pub remote: BranchRemote,
}

/// What the grouping says of a branch; a field without a value is absent
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchHead {
  pub name: String,
  pub latest_commit_time: u32,
  pub summary: String,
  /// The title of the branch's description (`bd describe`); `None` without one
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub description_title: Option<String>,
  pub all_commits_have_issue_references: bool,
  /// Most frequent author email in this branch's commits
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub my_email: Option<String>,
  pub origin: BranchOrigin,
  /// The issue the branch belongs to
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub issue_key: Option<String>,
  /// Where the branch sits in its stack; `None` without stacked branches
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stack: Option<BranchStack>,
}

/// What the sync made of a commit: its virtual copy; the fields are absent until the sync reported the commit
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CommitSync {
  /// The virtual copy the sync wrote or kept
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub hash: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub status: Option<CommitSyncStatus>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub error: Option<BranchError>,
  /// The conflict of git that the copy resolved by words (`branchdeck.resolveByWords`)
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub resolved: Option<WordResolutionSummary>,
}

/// The status of a branch in the table: `Syncing` until the sync reports how it ended
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum BranchStatus {
  Syncing,
  Created,
  Updated,
  Unchanged,
  Error,
  MergeConflict,
  AnalyzingConflict,
  Blocked,
}

impl From<BranchSyncStatus> for BranchStatus {
  fn from(status: BranchSyncStatus) -> Self {
    match status {
      BranchSyncStatus::Created => Self::Created,
      BranchSyncStatus::Updated => Self::Updated,
      BranchSyncStatus::Unchanged => Self::Unchanged,
      BranchSyncStatus::Error => Self::Error,
      BranchSyncStatus::MergeConflict => Self::MergeConflict,
      BranchSyncStatus::AnalyzingConflict => Self::AnalyzingConflict,
      BranchSyncStatus::Blocked => Self::Blocked,
    }
  }
}

/// What the sync made of a branch
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchSync {
  pub status: BranchStatus,
  /// The commits the sync reported while the branch was `Syncing` (the progress of its row)
  pub processed_count: u32,
  pub has_error: bool,
  /// The error of the branch or of its last failed commit
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub error_details: Option<BranchError>,
  /// Why a `Blocked` branch was not rebuilt (its parent can't be built on); `None` otherwise
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub blocked_reason: Option<String>,
}

impl BranchSync {
  /// The state of a branch the sync has not reported yet
  pub const SYNCING: Self = Self {
    status: BranchStatus::Syncing,
    processed_count: 0,
    has_error: false,
    error_details: None,
    blocked_reason: None,
  };
}

/// What the remote has of a branch
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchRemote {
  /// The last remote status a sync computed; `None` until one did, kept while the branch is rebuilt
  pub status: Option<RemoteStatusUpdate>,
}

/// The sections below the branch table
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TableSections {
  /// The commits no prefix and no issue key groups
  pub unassigned: Vec<Commit>,
  pub archived: ArchivedSection,
  /// The unapplied branches, newest first
  pub unapplied: Vec<UnappliedBranchInfo>,
  pub landed: LandedSection,
}

/// The archived branches: their names as the sync found them, and the integration each detection reported
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ArchivedSection {
  /// In the order the sync reported them (the page sorts them by the integration date)
  pub names: Vec<String>,
  /// The branches a sync of this session archived and that are still archived, oldest mark first
  pub newly_archived: Vec<String>,
  /// The integration of the archived branches whose detection reported
  pub integration: BTreeMap<String, BranchIntegrationInfo>,
}

/// The check of the checked out branch against the last fetched baseline and the commits that landed or are landing
/// with its update, which the sync reports together from the update check it runs beside its build
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandedSection {
  /// Whether the checked out branch can be updated onto the last fetched baseline; `None` until a sync reported it
  pub baseline: Option<BaselineStatus>,
  /// Local commits that the fetched baseline already has: the update drops them
  pub landing: Vec<LandedBranch>,
  /// Commits that landed with an update and that the user has not dismissed yet
  pub landed: Vec<LandedBranch>,
}

/// The change of a view from `base` to `next` (`next` is `base.next()`): the parts that differ from the ones sent last,
/// each whole. A window applies it only to the view at `base`
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ViewPatch {
  pub base: Version,
  pub next: Version,
  pub changes: TableChanges,
}

/// The parts of a `TableView` a patch replaces; a part absent is unchanged. The keyed parts (branches, stacks, the
/// integration of the archived branches) carry only the keys that changed and the keys removed
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TableChanges {
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub sync: Option<SyncState>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub header: Option<TableHeader>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub order: Option<Vec<String>>,
  /// The branches whose parts changed. A branch the view lacks comes with its head and its commits; without its sync
  /// part it is `BranchSync::SYNCING`, without its copies none of its commits is reported, without its remote part it
  /// has no remote status
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub branches: BTreeMap<String, BranchChange>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_branches: Vec<String>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stacks: BTreeMap<String, StackBranchUpdate>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_stacks: Vec<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub unassigned: Option<Vec<Commit>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub archived: Option<ArchivedChanges>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub unapplied: Option<Vec<UnappliedBranchInfo>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub landed: Option<LandedSection>,
}

impl TableChanges {
  /// Nothing changed
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self == &Self::default()
  }
}

/// The parts of one branch that changed
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchChange {
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub head: Option<BranchHead>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub commits: Option<Vec<Commit>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub sync: Option<BranchSync>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub copies: Option<Vec<CommitSync>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub remote: Option<BranchRemote>,
}

/// The parts of the archived section that changed
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ArchivedChanges {
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub names: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub newly_archived: Option<Vec<String>>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub integration: BTreeMap<String, BranchIntegrationInfo>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_integration: Vec<String>,
}
