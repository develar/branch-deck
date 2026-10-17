//! A memo of three-way merges, commit trees and conflict views that outlives one operation, e.g. a disk cache keyed by
//! object ids: the same trees always merge the same way, a commit always has the same tree, and the cherry-pick of a
//! commit onto another always conflicts the same way

use crate::model::MergeConflictInfo;

/// The version of a remembered `MergeConflictInfo`: bump it when the type or how `build_merge_conflict_info` computes
/// it changes, so no view of the old analysis is read
pub const CONFLICT_INFO_VERSION: u32 = 1;

/// The trees of a cherry-pick: the commit's parent tree, the commit's tree and the tree it is applied onto
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MergeTrees {
  pub base: String,
  pub commit: String,
  pub target: String,
}

/// What `merge-tree --write-tree` produced for some trees
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoizedMerge {
  Clean {
    tree: String,
  },
  /// The conflicting paths
  Conflict {
    files: Vec<String>,
  },
}

/// The full ids of a cherry-pick: the commit, its parent and the commit it is applied onto. Its conflict view reads
/// these commits and what they reach, not only their trees
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CherryCommits<'a> {
  pub commit: &'a str,
  pub parent: &'a str,
  pub target: &'a str,
}

pub trait MergeMemo: Send + Sync {
  fn merge(&self, trees: &MergeTrees) -> Option<MemoizedMerge>;
  fn remember_merge(&self, trees: &MergeTrees, result: &MemoizedMerge);
  fn commit_tree(&self, commit: &str) -> Option<String>;
  fn remember_commit_tree(&self, commit: &str, tree: &str);
  /// Report a conflict with its files only (`MergeConflictInfo::light`), without the conflict analysis a conflict
  /// view needs: for a dry run that only asks whether branches build
  fn light_conflicts(&self) -> bool {
    false
  }
  /// The conflict view of a cherry-pick of `commits` (before the sync names the branches of its missing commits), when
  /// it is remembered
  fn conflict(&self, _commits: CherryCommits<'_>) -> Option<MergeConflictInfo> {
    None
  }
  fn remember_conflict(&self, _commits: CherryCommits<'_>, _info: &MergeConflictInfo) {}
}
