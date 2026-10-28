//! A memo of three-way merges, commit trees and conflict views that outlives one operation, e.g. a disk cache keyed by
//! object ids: the same trees always merge the same way, a commit always has the same tree, and the cherry-pick of a
//! commit onto another always conflicts the same way

use crate::merge_tree_output::BatchMerge;
use domain_types::conflict::MergeConflictInfo;
use object_cache::merge_result::MergeResult;
use object_cache::word_resolution::WordResolutionResult;

/// The version of a remembered `MergeConflictInfo`: bump it when the type or how `build_merge_conflict_info` computes it
/// change, so no view of the old analysis is read
pub const CONFLICT_INFO_VERSION: u32 = 6;

/// The version of a remembered chain of copies of a conflicting branch (`sync_core::conflict_chain`): bump it when how a
/// commit is copied changes (`copy_commit`: its tree, message, author, committer or date), so no copy of the old way is
/// taken
pub const CONFLICT_CHAIN_VERSION: u32 = 3;

/// The trees of a cherry-pick: the commit's parent tree, the commit's tree and the tree it is applied onto
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MergeTrees {
  pub base: String,
  pub commit: String,
  pub target: String,
}

/// What `merge-tree --write-tree` produced for some trees: the result of a merge of the sync's `CommitChain`, and what
/// a memo remembers of it
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
  Clean {
    tree: String,
  },
  /// The conflicting paths, each once, in git's order
  Conflict {
    files: Vec<String>,
  },
}

impl From<BatchMerge> for MergeOutcome {
  /// A path of a conflict that is not UTF-8 gets U+FFFD for its bytes that are not
  fn from(merge: BatchMerge) -> Self {
    if merge.clean {
      return Self::Clean { tree: merge.output.tree };
    }
    let files = merge.output.conflicted_paths().into_iter().map(|path| String::from_utf8_lossy(path).into_owned()).collect();
    Self::Conflict { files }
  }
}

/// The merge as the object cache stores it (`merge_result`)
impl From<MergeResult> for MergeOutcome {
  fn from(result: MergeResult) -> Self {
    match result {
      MergeResult::Clean { tree } => Self::Clean { tree },
      MergeResult::Conflict { files } => Self::Conflict { files },
    }
  }
}

impl From<&MergeOutcome> for MergeResult {
  fn from(merge: &MergeOutcome) -> Self {
    match merge {
      MergeOutcome::Clean { tree } => Self::Clean { tree: tree.clone() },
      MergeOutcome::Conflict { files } => Self::Conflict { files: files.clone() },
    }
  }
}

/// The full ids of a cherry-pick: the commit, its parent and the commit it is applied onto. Its conflict view reads
/// these commits and what they reach, not only their trees
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CherryCommits<'a> {
  pub commit: &'a str,
  pub parent: &'a str,
  pub target: &'a str,
}

/// What a memo remembers; each kind it doesn't is left at its default, which remembers nothing
pub trait MergeMemo: Send + Sync {
  fn merge(&self, _trees: &MergeTrees) -> Option<MergeOutcome> {
    None
  }
  fn remember_merge(&self, _trees: &MergeTrees, _result: &MergeOutcome) {}
  fn commit_tree(&self, _commit: &str) -> Option<String> {
    None
  }
  fn remember_commit_tree(&self, _commit: &str, _tree: &str) {}
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
  /// Whether the conflicting cherry-pick of `trees` resolved by words (`word_resolve`), when it is remembered: a
  /// `Resolved` tree counts only while it exists
  fn word_resolution(&self, _trees: &MergeTrees) -> Option<WordResolutionResult> {
    None
  }
  fn remember_word_resolution(&self, _trees: &MergeTrees, _result: &WordResolutionResult) {}
}
