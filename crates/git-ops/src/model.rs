use anyhow::ensure;
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
pub enum BranchSyncStatus {
  Created,
  Updated,
  Unchanged,
  Error,
  MergeConflict,
  AnalyzingConflict,
}

/// Status of a commit synchronization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
pub enum CommitSyncStatus {
  Pending,
  Created,
  Unchanged,
  Error,
  Blocked,
}

/// Represents details of a conflict during a cherry-pick operation.
///
/// Includes the path of the conflicted file, its status, and the diff details for the conflict.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictDetail {
  pub file: String,
  pub status: String,                                          // "modified", "added", "deleted"
  pub file_diff: crate::conflict_analysis::FileDiff,           // Diff showing conflict content with markers vs original (for "Conflicts only" view)
  pub base_file: Option<crate::conflict_analysis::FileInfo>,   // Base version (common ancestor)
  pub target_file: Option<crate::conflict_analysis::FileInfo>, // Target branch version (ours)
  pub cherry_file: Option<crate::conflict_analysis::FileInfo>, // Cherry-pick version (theirs)
  pub base_to_target_diff: crate::conflict_analysis::FileDiff, // Base -> Target diff with hunks (for 3-way view)
  pub base_to_cherry_diff: crate::conflict_analysis::FileDiff, // Base -> Cherry diff with hunks (for 3-way view)
}

/// Details about a merge conflict encountered during a cherry-pick operation.
///
/// Contains information about the conflicting files, associated commit details, and conflict analysis results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct MergeConflictInfo {
  pub commit_message: String,
  pub commit_hash: String,
  pub commit_author_time: u32,
  pub commit_committer_time: u32,
  // Original parent of the cherry-picked commit
  pub original_parent_message: String,
  pub original_parent_hash: String,
  pub original_parent_author_time: u32,
  pub original_parent_committer_time: u32,
  // Target branch HEAD where we're trying to apply the commit
  pub target_branch_message: String,
  pub target_branch_hash: String,
  pub target_branch_author_time: u32,
  pub target_branch_committer_time: u32,
  pub conflicting_files: Vec<ConflictDetail>,
  pub conflict_analysis: crate::conflict_analysis::ConflictAnalysis,
  // Map of commit hashes to their info for conflict markers (shared across all files)
  pub conflict_marker_commits: std::collections::HashMap<String, ConflictMarkerCommitInfo>,
}
/// Information about a commit referenced in conflict markers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictMarkerCommitInfo {
  pub hash: String,
  pub message: String,
  pub author: String,
  pub author_time: u32,
  pub committer_time: u32,
}

/// Branch operation errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
pub enum BranchError {
  Generic(String),
  MergeConflict(Box<MergeConflictInfo>),
}

pub fn to_final_branch_name(branch_prefix: &str, branch_name: &str) -> anyhow::Result<String> {
  let prefix = branch_prefix.trim_end_matches('/').trim();
  ensure!(!prefix.is_empty(), "branch prefix cannot be blank");

  let name = branch_name.trim_end_matches('/').trim();
  ensure!(!name.is_empty(), "branch name cannot be blank");

  // Sanitize branch name to make it valid for Git references
  let sanitized_name = sanitize_branch_name(name);

  Ok(format!("{prefix}/virtual/{sanitized_name}"))
}

/// Extract the simple branch name from a full virtual branch name
/// e.g., "user/virtual/feature-auth" -> "feature-auth"
pub fn extract_branch_name_from_final(full_branch_name: &str, branch_prefix: &str) -> Option<String> {
  let prefix = format!("{}/virtual/", branch_prefix.trim_end_matches('/'));
  full_branch_name.strip_prefix(&prefix).map(|s| s.to_string())
}

/// Sanitizes a branch name to make it valid for Git references
/// Git reference names cannot contain spaces, certain special characters, etc.
pub(crate) fn sanitize_branch_name(name: &str) -> String {
  name
    // Replace spaces with hyphens
    .replace(' ', "-")
    // Replace other problematic characters with hyphens
    .replace(['~', '^', ':', '?', '*', '[', ']', '\\'], "-")
    // Remove leading/trailing dots and slashes
    .trim_matches('.')
    .trim_matches('/')
    // Replace multiple consecutive hyphens with a single hyphen
    .chars()
    .fold(String::new(), |mut acc, c| {
      if c == '-' && acc.ends_with('-') {
        // Skip consecutive hyphens
        acc
      } else {
        acc.push(c);
        acc
      }
    })
    // Ensure it doesn't start or end with a hyphen
    .trim_matches('-')
    .to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_to_final_branch_name_valid_inputs() {
    let result = to_final_branch_name("feature", "auth").unwrap();
    assert_eq!(result, "feature/virtual/auth");
  }

  #[test]
  fn test_to_final_branch_name_with_trailing_slashes() {
    let result = to_final_branch_name("feature/", "auth/").unwrap();
    assert_eq!(result, "feature/virtual/auth");
  }

  #[test]
  fn test_to_final_branch_name_with_spaces() {
    let result = to_final_branch_name("  feature  ", "  auth  ").unwrap();
    assert_eq!(result, "feature/virtual/auth");
  }

  #[test]
  fn test_to_final_branch_name_empty_prefix() {
    let result = to_final_branch_name("", "auth");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "branch prefix cannot be blank");
  }

  #[test]
  fn test_to_final_branch_name_whitespace_prefix() {
    let result = to_final_branch_name("   ", "auth");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "branch prefix cannot be blank");
  }

  #[test]
  fn test_to_final_branch_name_empty_name() {
    let result = to_final_branch_name("feature", "");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "branch name cannot be blank");
  }

  #[test]
  fn test_to_final_branch_name_whitespace_name() {
    let result = to_final_branch_name("feature", "   ");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "branch name cannot be blank");
  }

  #[test]
  fn test_branch_sync_status_equality() {
    assert_eq!(BranchSyncStatus::Created, BranchSyncStatus::Created);
    assert_ne!(BranchSyncStatus::Created, BranchSyncStatus::Updated);
  }

  #[test]
  fn test_sanitize_branch_name_with_spaces() {
    assert_eq!(sanitize_branch_name("ui dispatcher"), "ui-dispatcher");
    assert_eq!(sanitize_branch_name("hello world test"), "hello-world-test");
  }

  #[test]
  fn test_sanitize_branch_name_with_special_chars() {
    assert_eq!(sanitize_branch_name("test~branch"), "test-branch");
    assert_eq!(sanitize_branch_name("test^branch"), "test-branch");
    assert_eq!(sanitize_branch_name("test:branch"), "test-branch");
    assert_eq!(sanitize_branch_name("test?branch"), "test-branch");
    assert_eq!(sanitize_branch_name("test*branch"), "test-branch");
    assert_eq!(sanitize_branch_name("test[branch]"), "test-branch");
    assert_eq!(sanitize_branch_name("test\\branch"), "test-branch");
  }

  #[test]
  fn test_sanitize_branch_name_consecutive_hyphens() {
    assert_eq!(sanitize_branch_name("test--branch"), "test-branch");
    assert_eq!(sanitize_branch_name("test   branch"), "test-branch");
    assert_eq!(sanitize_branch_name("test-~-branch"), "test-branch");
  }

  #[test]
  fn test_sanitize_branch_name_edge_cases() {
    assert_eq!(sanitize_branch_name("-test-"), "test");
    assert_eq!(sanitize_branch_name(".test."), "test");
    assert_eq!(sanitize_branch_name("/test/"), "test");
    assert_eq!(sanitize_branch_name("---test---"), "test");
  }

  #[test]
  fn test_to_final_branch_name_with_sanitization() {
    let result = to_final_branch_name("develar", "ui dispatcher").unwrap();
    assert_eq!(result, "develar/virtual/ui-dispatcher");

    let result = to_final_branch_name("feature", "test~branch").unwrap();
    assert_eq!(result, "feature/virtual/test-branch");

    let result = to_final_branch_name("bugfix", "hello world test").unwrap();
    assert_eq!(result, "bugfix/virtual/hello-world-test");
  }
}
