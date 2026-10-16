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
