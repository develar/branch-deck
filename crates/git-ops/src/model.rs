//! The types a sync reports and the app shows: the statuses of branches and commits, conflicts, `BranchError`. The names
//! of the branches under the prefix are `branch_names.rs`.

use crate::commit_list::Commit;
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;

/// Simple commit information with hash and message.
/// Used for passing commit data between frontend and backend.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
pub struct CommitInfo {
  pub hash: String,
  pub message: String,
}

/// Status of a branch synchronization operation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
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
#[cfg_attr(feature = "specta", derive(Type))]
pub enum CommitSyncStatus {
  Pending,
  Created,
  Unchanged,
  Error,
  Blocked,
}

/// A copy whose cherry-pick conflicted in git and resolved by words (`word_resolve`): what its mapping note records and
/// the commit row says ("Resolved 3 conflicts by words")
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct WordResolutionSummary {
  /// The conflicts of git that the words resolved: the `<<<<<<<` lines git would have left
  pub conflicts: u32,
  /// The resolved files
  pub file_count: u32,
  /// The resolved files with their conflicts, in git's order; empty for a copy reused by its mapping note, which records
  /// the counts only. The conflict view merges the commit again for them (`get_file_merges`)
  pub files: Vec<ResolvedFileSummary>,
}

/// A file of `WordResolutionSummary`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ResolvedFileSummary {
  pub path: String,
  pub conflicts: u32,
}

/// A conflicting file of a cherry-pick, as a conflict is reported: what the view lists before it reads the contents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
  pub file: String,
  /// The conflicts of the merged file, its `<<<<<<<` marker lines (`count_conflicts`); 0 when one side deleted the
  /// file and the other changed it (a modify/delete conflict)
  pub conflict_count: u32,
}

/// A merge conflict of a cherry-pick, as a sync reports it: the commits, the conflicting files with their conflict
/// counts and the conflict analysis, without file contents and diffs, which the views read from `file_diffs`
/// (`get_file_merges`, `get_file_diffs`): a conflict in big files is megabytes, and the sync reports it at every run
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct MergeConflictInfo {
  /// Commit being applied (cherry-picked)
  pub commit: ConflictCommit,
  /// Original parent of the cherry-picked commit
  pub original_parent: ConflictCommit,
  /// Target branch HEAD where we're trying to apply the commit
  pub target_branch: ConflictCommit,
  pub conflicting_files: Vec<ConflictFile>,
  pub conflict_analysis: crate::conflict_analysis::ConflictAnalysis,
}

impl MergeConflictInfo {
  /// A conflict known only by its commits and files, without conflict counts or analysis (`MergeMemo::light_conflicts`)
  pub fn light(commit: &str, original_parent: &str, target: &str, files: Vec<String>) -> Self {
    let commit_of = |hash: &str| ConflictCommit {
      hash: hash.to_string(),
      message: String::new(),
      author: String::new(),
      author_time: 0,
      committer_time: 0,
    };
    Self {
      commit: commit_of(commit),
      original_parent: commit_of(original_parent),
      target_branch: commit_of(target),
      conflicting_files: files.into_iter().map(|file| ConflictFile { file, conflict_count: 0 }).collect(),
      conflict_analysis: crate::conflict_analysis::ConflictAnalysis::default(),
    }
  }
}

/// A commit involved in a merge conflict: the applied commit, its original parent, the target, or one referenced in conflict markers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictCommit {
  pub hash: String,
  pub message: String,
  pub author: String,
  pub author_time: u32,
  pub committer_time: u32,
}

impl From<&Commit> for ConflictCommit {
  fn from(commit: &Commit) -> Self {
    Self {
      hash: commit.id.clone(),
      message: commit.subject.clone(),
      author: commit.author_name.clone(),
      author_time: commit.author_timestamp,
      committer_time: commit.committer_timestamp,
    }
  }
}

/// Branch operation errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
pub enum BranchError {
  Generic(String),
  MergeConflict(Box<MergeConflictInfo>),
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_branch_sync_status_equality() {
    assert_eq!(BranchSyncStatus::Created, BranchSyncStatus::Created);
    assert_ne!(BranchSyncStatus::Created, BranchSyncStatus::Updated);
  }
}
