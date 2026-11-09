//! What a review shows (openspec/specs/review/review-window/spec.md): a target, whose commits and ranges the resolver reads
//! (`sync_core::review_target`), and what it answers. A pull request is no target: its range comes from its fetch
//! (`sync_core::review_fetch`), in the same shape of a range.

use crate::review_request::ReviewCommit;
use serde::{Deserialize, Serialize};

/// An object with changes the review reads by its refs or ids
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReviewTarget {
  /// A virtual branch with its stack ancestors: the branches from the root of the stack to the reviewed branch, without
  /// the `{prefix}/virtual/` namespace; one name for a branch outside a stack
  Branch { chain: Vec<String> },
  /// The whole stack of `root`: its stack branch `{prefix}/stack/<root>` (openspec/specs/branch/whole-stack/spec.md), one part
  /// named by the root
  WholeStack { root: String },
  /// A branch kept outside HEAD by its full name, `{prefix}/unapplied/<name>` or `{prefix}/archived/<date>/<name>`: its
  /// own commits, from its stack base, else from where it left the baseline (`own_commits_start`)
  Kept { name: String },
  /// Commits of a list no branch owns (unassigned, landed, the missing commits of a conflict), in the order of the list,
  /// each against its first parent
  Commits { ids: Vec<String> },
}

/// A part of a reviewed range: a branch of a stack on its parent, the whole stack, a kept branch on its base
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewSegment {
  /// The branch, without its namespace (`auth`, the `docs` of `jane/unapplied/docs`)
  pub name: String,
  /// The head of the parent, the commit the root is built on, or where a kept branch starts
  pub base: String,
  pub head: String,
  /// Newest first; the commits of a pull request are of the same type, a review shows both alike
  pub commits: Vec<ReviewCommit>,
}

/// What the resolver answers for a target
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TargetReview {
  /// The parts of a range, in the order of the chain: the root first
  Range { segments: Vec<ReviewSegment> },
  /// The commits of a list, in its order
  Commits { commits: Vec<ReviewCommit> },
}

impl TargetReview {
  /// The parts of a range; none for a list of commits
  #[must_use]
  pub fn segments(&self) -> &[ReviewSegment] {
    match self {
      Self::Range { segments } => segments,
      Self::Commits { .. } => &[],
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetReviewParams {
  pub repository_path: String,
  /// As the sync detected it, e.g. `origin/master`
  pub baseline_branch: String,
  pub target: ReviewTarget,
}
