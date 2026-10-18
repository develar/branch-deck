//! Stacked branches as a sync reports them (`sync_core::stack` computes them)
use serde::Serialize;

/// Why a branch of a stack can't be placed; it never fails the other branches
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum StackProblem {
  /// Commits declare different parents, in first-declared order
  ConflictingParents(Vec<DeclaredParent>),
  /// Following the declared parents leads back to the branch: the branches of the cycle, starting with this one
  Cycle(Vec<String>),
  /// `(a/a)`: the commits that declare the branch as its own parent
  SelfParent(Vec<String>),
  /// Another branch has the same name ignoring case (`Login` and `login`)
  LeafCollision(String),
  /// `(a//b)`: the commits whose prefix has an empty level
  Syntax(Vec<String>),
}

/// A parent and the commits that declare it
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DeclaredParent {
  pub parent: String,
  pub commits: Vec<String>,
}

/// The state of a declared parent; a label only: a parent without commits, whatever its state, doesn't block its children
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ParentState {
  /// The parent has commits
  Active,
  /// No commit is grouped into the parent and its archived branch is integrated into the baseline
  Integrated,
  /// The parent was unapplied
  Unapplied,
  /// No commit is grouped into the parent
  Missing,
}

/// Where the declared parent of a branch comes from
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ParentSource {
  /// The prefixes of its commits declare it (`(parent/name)`)
  Prefix,
  /// Its line in the stack parents (`refs/branch-deck/stack-parents`), because no prefix declares one
  Store,
}

/// Why a branch is not rebuilt: its parent can't be built on
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum BlockReason {
  /// The parent can't be placed (a stack problem) or failed to build
  Error,
  /// A commit of the parent has a merge conflict
  Conflict,
  /// The parent is blocked itself
  Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackBlock {
  pub parent: String,
  pub reason: BlockReason,
}

/// Where a virtual branch sits in its stack; sent only with stacked branches on
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchStack {
  pub declared_parent: Option<String>,
  /// Where `declared_parent` comes from
  pub parent_source: Option<ParentSource>,
  pub parent_state: Option<ParentState>,
  /// The branch the virtual branch is built on; `None` for the baseline
  pub effective_base: Option<String>,
  /// Why the branch is not rebuilt this sync, known before the sync starts: its parent can't be built (a problem, a
  /// conflict, a failure or a block); a parent that fails while building blocks its descendants with a `Blocked`
  /// branch status
  pub blocked_by: Option<StackBlock>,
  /// Why the branch itself can't be placed; it is not rebuilt either
  pub problems: Vec<StackProblem>,
  /// The declared parent has no commits, so the branch is built on its nearest active ancestor or the baseline and the
  /// base of its pull request differs from the declared parent
  pub pr_base_changed: bool,
  /// A commit of the branch comes before every commit of its parent on the current branch
  pub child_precedes_parent: bool,
}

/// How the stack branch of a stack (`{prefix}/stack/<root>`) stands after a sync
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum StackBranchState {
  /// The ref is the build of the stack's commits as the sync grouped them: the only state whose ref may be pushed
  Built,
  /// A member failed or is blocked (`StackBranchUpdate::blocked_by`): the stack branch is not rebuilt
  Blocked,
  /// A commit conflicts with the commits of the stack before it (`StackBranchUpdate::conflict`): not rebuilt
  Conflict,
  /// Not rebuilt for another reason (`StackBranchUpdate::error`): the stack no longer forks and the ref stays for its
  /// remote branch, another process moved the ref, or the build failed
  Stale,
}

/// The member of a stack that keeps its stack branch from being built, and why: `Error` for a stack problem or a failed
/// build, `Conflict` for a merge conflict of its own branch, `Blocked` when its parent can't be built on
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackMemberBlock {
  pub member: String,
  pub reason: BlockReason,
}

/// The commit of a stack branch whose copy conflicts with the commits of the stack before it, and the conflicting
/// paths
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackBranchConflict {
  /// The original commit, as in `GroupedBranchInfo::commits`
  pub commit: String,
  pub files: Vec<String>,
}

/// The stack branch of a stack after a sync: all commits of a stack that forks, in the order of the checked out
/// branch's history, on the base of the sync, as one more virtual branch (`{prefix}/stack/<root>`), which a Safe Push
/// of the whole stack lands in one robot. Sent once per stack branch the sync reports (`BranchesGrouped::stack_branches`)
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackBranchUpdate {
  /// The root branch of the stack, which names the stack branch
  pub root: String,
  /// The full branch name, `{prefix}/stack/<root>`
  pub branch_name: String,
  /// The branches of the stack, the root first, each after its parent; empty for a stack branch whose stack no longer
  /// forks
  pub members: Vec<String>,
  /// The commits of the stack: every commit of its members
  pub commit_count: u32,
  pub state: StackBranchState,
  /// The tip of the ref after the sync: the build for `Built`, else the tip the ref kept, which no push may send; `None`
  /// without a ref
  pub tip: Option<String>,
  /// `Blocked`: the member that failed or is blocked
  pub blocked_by: Option<StackMemberBlock>,
  /// `Conflict`: the commit that conflicts and its files
  pub conflict: Option<StackBranchConflict>,
  /// `Stale`: why the ref was not rebuilt
  pub error: Option<String>,
  /// The remote status of `tip` against its remote branch on `origin`, by content as for a virtual branch; `None`
  /// without a ref or when it could not be computed
  pub remote: Option<crate::RemoteStatusUpdate>,
}
