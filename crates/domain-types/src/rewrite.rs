//! The rewrites of the checked out branch and of the refs of Branch Deck as operations (docs/design/git-operations.md,
//! "Rewrite in the engine"): what the user asks (`RewriteIntent`, the intent `Rewrite` of `operation::Intent`) with the
//! table it was asked on (`TableWitness`), how its outcome is undone (`UndoToken`), and what its core answered
//! (`RewriteResult`); with the requests the intents name (the texts of a reword, a regroup plan, the scope of a squash
//! of fixups, the branches to park) and the answers of the cores, as the cores of `history-rewrite`, `sync-core`,
//! `trial-check` and `update-core` build them, `bd` prints them and the page reads them.

use crate::BranchError;
use crate::commit::{CommitInfo, SubjectParts};
use crate::session::StoreTips;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// A rewrite of the checked out branch or of the refs of Branch Deck (the family `Rewrite`, `operations::rewrite`): what
/// its core takes but the repository, which its session names, and the table its window showed (`witness`), which its
/// commits and branches are named on. The undo (`Undo`) names no table: its token holds what it moves back, each ref
/// with compare-and-swap
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "rewrite", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RewriteIntent {
  /// The commits (original ids) go into `target_branch`, existing or new, or back out of their prefix branch or issue
  /// group (`None`): Move to…, a drag of commits
  Move {
    commit_ids: Vec<String>,
    target_branch: Option<String>,
    witness: TableWitness,
  },
  /// Every commit of the branch goes back where its key groups it (Ungroup)
  Ungroup {
    branch_name: String,
    witness: TableWitness,
  },
  /// The branch and the branches stacked on it get `new_name` in the prefix of their commits, its issue groups too
  Rename {
    branch_name: String,
    new_name: String,
    witness: TableWitness,
  },
  /// The branch is stacked on `parent` (Stack On)
  Restack {
    branch_name: String,
    parent: String,
    witness: TableWitness,
  },
  /// The branch is made a root (Unstack)
  Unstack {
    branch_name: String,
    witness: TableWitness,
  },
  /// The branches are parked with their notes, or the notes of parked ones change
  Park {
    branches: Vec<ParkBranchLine>,
    witness: TableWitness,
  },
  Unpark {
    branch_names: Vec<String>,
    witness: TableWitness,
  },
  /// The branch and its files leave HEAD for `{prefix}/unapplied/<name>`; `original_commit_ids`: the commits its row
  /// showed, which it must still have
  Unapply {
    branch_name: String,
    original_commit_ids: Option<Vec<String>>,
    witness: TableWitness,
  },
  /// The unapplied branch of that name comes back on top of HEAD
  Reapply {
    branch_name: String,
    witness: TableWitness,
  },
  /// The commits become one with `message`, in place of the oldest of them
  Squash {
    commit_ids: Vec<String>,
    message: String,
    witness: TableWitness,
  },
  /// The fixups of `scope` fold into their targets
  SquashFixups {
    scope: AutosquashScope,
    witness: TableWitness,
  },
  /// The texts of commits ahead of the baseline change
  Reword {
    rewords: Vec<CommitReword>,
    witness: TableWitness,
  },
  /// The uncommitted changes of `files` go into the commit `original_commit_id` of the branch
  Amend {
    branch_name: String,
    original_commit_id: String,
    files: Vec<String>,
    witness: TableWitness,
  },
  /// `issue_reference` goes into the subjects of `commits` of the branch
  AddIssueReference {
    branch_name: String,
    commits: Vec<CommitInfo>,
    issue_reference: String,
    witness: TableWitness,
  },
  /// The regroup the conflict fix found and built (`fingerprint` of its trial) is written
  ApplyConflictFix {
    fingerprint: String,
    plan: RegroupPlan,
    witness: TableWitness,
  },
  /// The archived branch of that name is deleted
  DeleteArchived {
    branch_name: String,
    witness: TableWitness,
  },
  /// The unapplied branch of that name is deleted
  DeleteUnapplied {
    branch_name: String,
    witness: TableWitness,
  },
  /// The commits someone else pushed to the remote branch of the virtual branch go onto HEAD with the prefix of the
  /// branch (Pick Remote Commits); `expected_remote`: the remote tip the row showed
  PickRemote {
    branch_name: String,
    parent_branch: Option<String>,
    my_email: Option<String>,
    expected_remote: Option<String>,
    witness: TableWitness,
  },
  /// The rewrite whose outcome gave `token` (`Outcome::undo`) is undone
  Undo {
    token: UndoToken,
  },
  /// The regroup of a plan (`bd group --plan`): only the record of a run of `bd` in the operations registry of the
  /// repository, which the view and the journal show; the engine of the app runs none
  Group {
    plan: RegroupPlan,
    witness: TableWitness,
  },
  /// A hunk of `path` moves from the commit `from` into the commit `to` (`bd move-hunk`): only the record of a run of
  /// `bd`, as `Group`
  MoveHunk {
    from: String,
    to: String,
    path: String,
    witness: TableWitness,
  },
}

impl RewriteIntent {
  /// The table it was asked on; none for an undo
  #[must_use]
  pub const fn witness(&self) -> Option<&TableWitness> {
    match self {
      Self::Move { witness, .. }
      | Self::Ungroup { witness, .. }
      | Self::Rename { witness, .. }
      | Self::Restack { witness, .. }
      | Self::Unstack { witness, .. }
      | Self::Park { witness, .. }
      | Self::Unpark { witness, .. }
      | Self::Unapply { witness, .. }
      | Self::Reapply { witness, .. }
      | Self::Squash { witness, .. }
      | Self::SquashFixups { witness, .. }
      | Self::Reword { witness, .. }
      | Self::Amend { witness, .. }
      | Self::AddIssueReference { witness, .. }
      | Self::ApplyConflictFix { witness, .. }
      | Self::DeleteArchived { witness, .. }
      | Self::DeleteUnapplied { witness, .. }
      | Self::PickRemote { witness, .. }
      | Self::Group { witness, .. }
      | Self::MoveHunk { witness, .. } => Some(witness),
      Self::Undo { .. } => None,
    }
  }

  /// Whether only `bd` runs it (`Group`, `MoveHunk`): the engine of the app refuses it, its record in the operations
  /// registry is the view's and the journal's
  #[must_use]
  pub const fn recorded_only(&self) -> bool {
    matches!(self, Self::Group { .. } | Self::MoveHunk { .. })
  }

  /// The branch it is about, as the view keys a row: the target of a move, the branch of the others; none for a move
  /// back, a squash, a reword, a fix, an undo, a regroup of a plan, a move of a hunk
  #[must_use]
  pub fn subject(&self) -> Option<&str> {
    match self {
      Self::Move { target_branch, .. } => target_branch.as_deref(),
      Self::Park { branches, .. } => branches.first().map(|line| line.branch_name.as_str()),
      Self::Unpark { branch_names, .. } => branch_names.first().map(String::as_str),
      Self::SquashFixups {
        scope: AutosquashScope::Branch(branch),
        ..
      } => Some(branch),
      Self::Ungroup { branch_name, .. }
      | Self::Rename { branch_name, .. }
      | Self::Restack { branch_name, .. }
      | Self::Unstack { branch_name, .. }
      | Self::Unapply { branch_name, .. }
      | Self::Reapply { branch_name, .. }
      | Self::Amend { branch_name, .. }
      | Self::AddIssueReference { branch_name, .. }
      | Self::DeleteArchived { branch_name, .. }
      | Self::DeleteUnapplied { branch_name, .. }
      | Self::PickRemote { branch_name, .. } => Some(branch_name),
      Self::Squash { .. } | Self::SquashFixups { .. } | Self::Reword { .. } | Self::ApplyConflictFix { .. } | Self::Undo { .. } | Self::Group { .. } | Self::MoveHunk { .. } => {
        None
      }
    }
  }
}

/// The table a rewrite was asked on: HEAD and the tips of the refs of Branch Deck the rewrites read, as the last sync of
/// the window's session read them (`TableHeader::witness`); a patch of the facts of the remote never changes it
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TableWitness {
  /// The commit HEAD pointed at
  pub head: String,
  pub store_tips: StoreTips,
}

/// How the outcome of a rewrite is undone (`RewriteIntent::Undo`): the token its core printed, the same `bd undo` takes,
/// by the core that undoes it
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UndoToken {
  pub kind: UndoKind,
  pub token: String,
}

/// The core an undo token is for
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum UndoKind {
  /// A regroup (a move, an ungroup, a rename, a restack, an unstack, a reword, a fix) or a park: HEAD and the stores
  /// back (`<previous>..<new>`, `groups:`, `stacks:`, `parked:`, joined by `+`)
  Regroup,
  /// A squash or a squash of fixups: HEAD back from the new commit (`<previous>..<new>`)
  Squash,
  /// An unapply (`unapply:…`)
  Unapply,
  /// A reapply (`reapply:…`)
  Reapply,
  /// Pick Remote Commits (`pick:…`)
  Pick,
  /// Update Project (`update:<previous>..<new>`): the branch and the files back, as `bd undo` does
  Update,
}

/// What the core of a rewrite answered (`OperationResult::Rewrite`), by its kind
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "rewrite", content = "answer", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RewriteResult {
  /// A move, an ungroup, a rename, a restack, an unstack: the commits that moved and the branch they are in
  Moved(MoveCommitsResult),
  /// A park or an unpark
  Parked(ParkResult),
  Unapplied(UnapplyBranchResult),
  Reapplied(ReapplyBranchResult),
  Squashed(SquashCommitsResult),
  FixupsSquashed(SquashFixupsResult),
  Reworded(RewordResult),
  Amended(AmendCommandResult),
  IssueReferenceAdded(AddIssueReferenceResult),
  ConflictFixApplied(AppliedConflictFix),
  Picked(PickRemoteCommitsResult),
  /// An archived or an unapplied branch is gone
  Deleted,
  /// An undo; `already_undone`: an undo before had done it, nothing changed now (of an unapply, a reapply, a pick)
  Undone {
    already_undone: bool,
  },
}

impl RewriteResult {
  /// HEAD as the rewrite left it, when its core says; none for a rewrite that moves no HEAD and for an undo
  #[must_use]
  pub fn head(&self) -> Option<&str> {
    match self {
      Self::Moved(moved) | Self::ConflictFixApplied(AppliedConflictFix::Applied { result: moved }) => Some(&moved.head),
      Self::Unapplied(unapplied) => Some(&unapplied.new_head),
      Self::Reapplied(reapplied) => Some(&reapplied.new_head),
      Self::Squashed(squashed) => Some(&squashed.new_head),
      Self::FixupsSquashed(squashed) => Some(&squashed.new_head),
      Self::Reworded(reworded) => Some(&reworded.new_head),
      Self::Picked(picked) => Some(&picked.new_head),
      Self::Amended(AmendCommandResult::Ok(amended)) => Some(&amended.rebased_to_commit),
      Self::IssueReferenceAdded(added) => added.new_head.as_deref(),
      _ => None,
    }
  }

  /// Whether it changed anything a sync shows: a move that moved nothing, a park of the same notes, a squash of no
  /// fixups, a stale fix and an undo done before change nothing
  #[must_use]
  pub fn changes(&self) -> bool {
    match self {
      Self::Moved(moved) | Self::ConflictFixApplied(AppliedConflictFix::Applied { result: moved }) => {
        moved.moves() || !moved.grouped_issues.is_empty() || !moved.ungrouped_issues.is_empty() || moved.undo_token.is_some()
      }
      Self::Parked(parked) => parked.changed,
      Self::FixupsSquashed(squashed) => squashed.squashed_count > 0,
      Self::Reworded(reworded) => reworded.new_head != reworded.previous_head || reworded.undo_token.is_some(),
      Self::Amended(amended) => matches!(amended, AmendCommandResult::Ok(_)),
      Self::IssueReferenceAdded(added) => added.updated_count > 0,
      Self::ConflictFixApplied(AppliedConflictFix::Stale) => false,
      Self::Picked(picked) => picked.new_head != picked.previous_head,
      Self::Undone { already_undone } => !already_undone,
      Self::Unapplied(_) | Self::Reapplied(_) | Self::Squashed(_) | Self::Deleted => true,
    }
  }

  /// Commit id → its id after the rewrite, of the commits its core names (a move, a reword, the targets of the fixups);
  /// a commit it does not name may have a new id too (one on top of a rewritten one)
  #[must_use]
  pub fn rewritten(&self) -> BTreeMap<String, String> {
    let changed = |(old, new): (&String, &String)| (old != new).then(|| (old.clone(), new.clone()));
    match self {
      Self::Moved(moved) | Self::ConflictFixApplied(AppliedConflictFix::Applied { result: moved }) => {
        moved.commits.iter().filter_map(|commit| changed((&commit.old_id, &commit.new_id))).collect()
      }
      Self::Reworded(reworded) => reworded.new_commit_ids.iter().filter_map(changed).collect(),
      Self::FixupsSquashed(squashed) => squashed.squashed.iter().filter_map(|target| changed((&target.target, &target.new_id))).collect(),
      Self::IssueReferenceAdded(added) => added.new_commit_ids.iter().filter_map(changed).collect(),
      _ => BTreeMap::new(),
    }
  }
}

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

/// A commit copied onto HEAD by a reapply or a pick of remote commits: the commit it was copied from, its copy and the
/// subject of the copy
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CopiedCommit {
  /// The commit of the unapplied branch (or of its restored chain), or of the remote branch
  pub commit: String,
  /// Its copy on HEAD (a dry run of a reapply: the one it writes)
  pub new_id: String,
  /// The subject of the copy; of a pick with the prefix of the branch
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
  pub reapplied: Vec<CopiedCommit>,
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

// jscpd:ignore-start: plain data, whose derives and closing fields repeat those of the other answers
/// A fixup without a target among the older commits of the range: it is not squashed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct OrphanFixup {
  pub commit_id: String,
  pub subject: String,
  /// The parts of `subject` the page shows: its markers apart from the rest
  #[serde(default)]
  pub subject_parts: SubjectParts,
  /// The virtual branch the fixup itself is grouped into; `None` if it is unassigned
  pub branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SquashedTarget {
  /// The target before the squash
  pub target: String,
  pub subject: String,
  pub new_id: String,
  /// The fixups folded into it, oldest first
  pub fixups: Vec<String>,
}
// jscpd:ignore-end

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
  /// Old -> new full id of every commit the reword recreated, the ones after the oldest reworded one included; empty
  /// when nothing changed
  pub new_commit_ids: BTreeMap<String, String>,
  /// HEAD after the reword, when the caller named the HEAD it expected (`RewriteOptions::expect_head`) and something
  /// changed
  pub new_head: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PickRemoteCommitsResult {
  pub previous_head: String,
  pub new_head: String,
  pub picked: Vec<CopiedCommit>,
  /// `pick:<previous>..<new>` (`bd undo`); none when nothing moved
  pub undo_token: Option<String>,
}
