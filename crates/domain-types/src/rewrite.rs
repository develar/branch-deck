//! The requests and the answers of the rewrites of the checked out branch and of the refs of Branch Deck
//! (docs/design/git-operations.md, "Operations"): what an intent of the family `Rewrite` names
//! (`operation::RewriteIntent`: the texts of a reword, a regroup plan, the scope of a squash of fixups, the branches to
//! park) and what its core answered (`operation::RewriteResult`), as the cores of `history-rewrite`, `sync-core`,
//! `trial-check` and `update-core` build them, `bd` prints them and the page reads them.

use crate::BranchError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// The new text of a commit; `{"message": …}` or `{"subject": …}` in the app's request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum RewordText {
  /// The whole message, the `(prefix)` of the subject included
  Message(String),
  /// The subject line, the `(prefix)` included; the body stays
  Subject(String),
}

/// A commit ahead of the baseline (abbreviated ids are fine) and its new text
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CommitReword {
  pub commit: String,
  pub text: RewordText,
}

/// Where commits go: `{"branches": {"auth/login": [ids]}, "unassign": [ids], "issue_groups": {"name": [keys]},
/// "ungroup_issues": [keys], "unstack": [branches]}`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct RegroupPlan {
  /// Branch path -> commits: a flat name, or `parent/leaf` with stacked branches on, which stacks `leaf` on `parent` in
  /// the stack parents and gives a commit that needs a prefix the bare `(leaf)`. Ids may be abbreviated
  #[serde(default)]
  pub branches: BTreeMap<String, Vec<String>>,
  /// Commits whose prefix is removed: they fall back to their issue group or issue branch, or become unassigned
  #[serde(default)]
  pub unassign: Vec<String>,
  /// Issue group name (a branch name without `/`) -> issue keys: the commits without a prefix that carry one of the
  /// keys are grouped into the name, without a rewrite
  #[serde(default)]
  pub issue_groups: BTreeMap<String, Vec<String>>,
  /// Issue keys whose issue group is removed: their commits fall back to the issue branch of the key
  #[serde(default)]
  pub ungroup_issues: Vec<String>,
  /// Branches made roots: their stack parent is removed, and their commits whose prefix declares a parent get the bare
  /// `(leaf)`. Needs stacked branches on
  #[serde(default)]
  pub unstack: Vec<String>,
}

/// Which planned groups a squash takes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum AutosquashScope {
  All,
  /// The fixups whose target is grouped into this virtual branch
  Branch(String),
  /// The fixups whose target is unassigned
  Unassigned,
  /// Only this fixup commit, into its target
  Fixup(String),
}

/// A branch to park and its note
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ParkBranchLine {
  /// The branch (a leaf with stacked branches on)
  pub branch_name: String,
  /// Why it is parked, one line; empty for none. Parking a parked branch changes its note
  pub note: String,
}

/// How a move changes a commit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum MoveChange {
  /// Its message got the prefix of the target
  Prefixed,
  /// Its message lost its prefix: its issue key puts it where it goes
  PrefixRemoved,
  /// The issue groups moved it, its message stays
  Regrouped,
  /// It is where it goes already
  Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MovedCommit {
  /// The request commit id
  pub old_id: String,
  /// The id after the rewrite; `old_id` for a commit older than every rewritten one and for a dry run
  pub new_id: String,
  pub change: MoveChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MoveCommitsResult {
  /// The requested commits in request order, then the other commits that moved: the fixups that followed them, the
  /// commits of a renamed branch
  pub commits: Vec<MovedCommit>,
  /// The issue keys whose commits the issue groups now put into another branch, without a rewrite
  pub grouped_issues: Vec<String>,
  /// The issue keys whose issue group was removed: their commits are in the issue branch of the key again
  pub ungrouped_issues: Vec<String>,
  /// The branch the commits are in afterwards (the leaf with stacked branches on); `None` for a move back
  #[cfg_attr(feature = "specta", specta(optional))]
  pub target: Option<String>,
  /// HEAD afterwards
  pub head: String,
  /// With stacked branches on, the branches whose parent the operation emptied: they stay stacked on it and are built on
  /// the nearest ancestor with commits, or the baseline, until it has commits again
  pub orphaned_children: Vec<String>,
  /// What `undo_regroup` moves back; `None` for a dry run and when nothing changed
  #[cfg_attr(feature = "specta", specta(optional))]
  pub undo_token: Option<String>,
}

impl MoveCommitsResult {
  /// Whether the operation moves anything
  pub fn moves(&self) -> bool {
    self.commits.iter().any(|commit| commit.change != MoveChange::Unchanged)
  }
}

/// What a park or an unpark did to one branch
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ParkedBranchChange {
  pub branch_name: String,
  /// The note afterwards; `None` when the branch is not parked any more
  pub note: Option<String>,
  /// The note before; `None` when it was not parked
  pub previous_note: Option<String>,
}

/// What a park or an unpark did
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ParkResult {
  /// One per branch named, in the order named
  pub branches: Vec<ParkedBranchChange>,
  /// Whether the store changed
  pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UnapplyBranchResult {
  /// The full name of the unapplied branch: `{prefix}/unapplied/<name>`, with a `-N` suffix when the name is taken
  pub unapplied_branch_name: String,
  /// The original commits that left HEAD, oldest first
  pub commits_removed: Vec<String>,
  /// HEAD before the unapply
  pub previous_head: String,
  /// HEAD after it
  pub new_head: String,
  /// The undo: `UnapplyUndoToken` as it prints
  pub undo_token: String,
}

/// What the undo of an unapply did
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UndoneUnapply {
  /// It was undone before: nothing changed now
  pub already_undone: bool,
}

/// A commit of the unapplied branch that the reapply put on top of HEAD
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReappliedCommit {
  /// The commit of the unapplied branch (or of its restored chain)
  pub commit: String,
  /// Its copy on HEAD (with a dry run: the one the reapply writes)
  pub new_id: String,
  pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReapplyBranchResult {
  /// `{prefix}/unapplied/<name>`, deleted now (with a dry run: would be)
  pub unapplied_branch_name: String,
  /// The branch its commits group into again
  pub branch: String,
  /// Oldest first
  pub reapplied: Vec<ReappliedCommit>,
  /// The commits HEAD has already, by their patch or their content: left out
  pub present: Vec<String>,
  /// How many messages got the grouping prefix back (`(<name>) `): a virtual branch keeps none, so a commit whose
  /// subject groups elsewhere gets it
  pub prefix_restored: u32,
  pub previous_head: String,
  pub new_head: String,
  /// `ReapplyUndoToken` as it prints; `None` for a dry run
  pub undo_token: Option<String>,
}

/// What the undo of a reapply did
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UndoneReapply {
  /// It was undone before: nothing changed now
  pub already_undone: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SquashCommitsResult {
  /// The squashed commit, in place of the oldest selected one
  pub new_commit_id: String,
  /// HEAD after the squash and before it, for an undo
  pub new_head: String,
  pub previous_head: String,
  pub squashed_count: u32,
}

/// A fixup without a target among the older commits of the range: it is not squashed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct OrphanFixup {
  pub commit_id: String,
  pub subject: String,
  /// The virtual branch the fixup itself is grouped into; `None` if it is unassigned
  pub branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SquashedTarget {
  pub target: String,
  pub new_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SquashFixupsResult {
  /// One entry per target, oldest first
  pub squashed: Vec<SquashedTarget>,
  /// Number of fixup commits folded into their targets
  pub squashed_count: u32,
  /// Fixups of the scope left as they are: no target in this range
  pub orphans: Vec<OrphanFixup>,
  /// HEAD after the squash and before it, for an undo
  pub new_head: String,
  pub previous_head: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RewordResult {
  /// Request commit id -> new commit id; a commit that was not recreated maps to its own (full) id
  pub new_commit_ids: HashMap<String, String>,
  pub previous_head: String,
  /// `previous_head` when no message changed
  pub new_head: String,
  /// The token of `undo_regroup` (`UndoToken`); `None` when nothing changed
  #[cfg_attr(feature = "specta", specta(optional))]
  pub undo_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AmendResult {
  pub amended_commit_id: String,
  pub rebased_to_commit: String,
}

/// Result type for amend command that can be properly serialized by Tauri
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "status", content = "data", rename_all = "camelCase")]
pub enum AmendCommandResult {
  Ok(AmendResult),
  BranchError(BranchError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AddIssueReferenceResult {
  pub success: bool,
  pub updated_count: u32,
  pub skipped_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AppliedConflictFix {
  /// The regroup is written; `result` as a move reports it, with the commits that changed
  Applied { result: MoveCommitsResult },
  /// HEAD, the issue groups or the stack parents moved since the fix was found: look for it again
  Stale,
}

/// A commit Pick Remote Commits took in: the remote commit and its copy on the checked out branch
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PickedRemoteCommit {
  pub commit: String,
  pub new_id: String,
  /// The subject of the copy, with the prefix of the branch
  pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PickRemoteCommitsResult {
  pub previous_head: String,
  pub new_head: String,
  pub picked: Vec<PickedRemoteCommit>,
  /// `pick:<previous>..<new>` (`bd undo`); none when nothing moved
  pub undo_token: Option<String>,
}
