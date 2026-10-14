//! A memo of three-way merges and commit trees that outlives one operation, e.g. a disk cache keyed by object ids:
//! the same trees always merge the same way, and a commit always has the same tree

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
}
