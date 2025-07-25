use anyhow::ensure;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Status of a branch synchronization operation.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
pub enum BranchSyncStatus {
  Created,
  Updated,
  Unchanged,
  Error,
  MergeConflict,
  AnalyzingConflict,
}

/// Details about a synchronized commit.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
  pub original_hash: String,
  pub hash: String,
  pub message: String,
  pub author: String,
  pub author_time: u32,
  pub committer_time: u32,
  pub status: CommitSyncStatus,
  pub error: Option<BranchError>,
}

/// Status of a commit synchronization.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictDetail {
  pub file: String,
  pub status: String,                                               // "modified", "added", "deleted"
  pub file_diff: crate::git::conflict_analysis::FileDiff,           // Diff showing conflict content with markers vs original (for "Conflicts only" view)
  pub base_file: Option<crate::git::conflict_analysis::FileInfo>,   // Base version (common ancestor)
  pub target_file: Option<crate::git::conflict_analysis::FileInfo>, // Target branch version (ours)
  pub cherry_file: Option<crate::git::conflict_analysis::FileInfo>, // Cherry-pick version (theirs)
  pub base_to_target_diff: crate::git::conflict_analysis::FileDiff, // Base -> Target diff with hunks (for 3-way view)
  pub base_to_cherry_diff: crate::git::conflict_analysis::FileDiff, // Base -> Cherry diff with hunks (for 3-way view)
}

/// Details about a merge conflict encountered during a cherry-pick operation.
///
/// Contains information about the conflicting files, associated commit details, and conflict analysis results.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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
  pub conflict_analysis: crate::git::conflict_analysis::ConflictAnalysis,
  // Map of commit hashes to their info for conflict markers (shared across all files)
  pub conflict_marker_commits: std::collections::HashMap<String, ConflictMarkerCommitInfo>,
}
/// Information about a commit referenced in conflict markers
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictMarkerCommitInfo {
  pub hash: String,
  pub message: String,
  pub author: String,
  pub author_time: u32,
  pub committer_time: u32,
}

/// Branch operation errors.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum BranchError {
  Generic(String),
  MergeConflict(Box<MergeConflictInfo>),
}

/// Information about a commit needed for parallel processing
/// Contains only the essential data that can be safely sent across threads
#[derive(Debug, Clone)]
pub struct CommitInfo {
  pub message: String,
  pub id: String,
  pub author_name: String,
  pub author_email: String,
  pub author_time: u32,
  pub committer_time: u32,
  pub parent_id: Option<String>,
  pub tree_id: String,
  pub mapped_commit_id: Option<String>, // Extracted from git note if present
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
  fn test_commit_detail_creation() {
    let commit = CommitDetail {
      original_hash: "abc123".to_string(),
      hash: "def456".to_string(),
      message: "Test commit".to_string(),
      author: "Test Author".to_string(),
      author_time: 1_234_567_890,
      committer_time: 1_234_567_890,
      status: CommitSyncStatus::Created,
      error: None,
    };

    assert_eq!(commit.original_hash, "abc123");
    assert_eq!(commit.hash, "def456");
    assert_eq!(commit.status, CommitSyncStatus::Created);
    assert_eq!(commit.message, "Test commit");
    assert_eq!(commit.author, "Test Author");
    assert_eq!(commit.author_time, 1_234_567_890);
    assert_eq!(commit.committer_time, 1_234_567_890);
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

  #[test]
  fn test_commit_info_creation() {
    let commit_info = CommitInfo {
      message: "Test commit message".to_string(),
      id: "1234567890abcdef1234567890abcdef12345678".to_string(),
      author_name: "Test Author".to_string(),
      author_email: "test@example.com".to_string(),
      author_time: 1_234_567_890,
      committer_time: 1_234_567_890,
      parent_id: None,
      tree_id: "tree123".to_string(),
      mapped_commit_id: None,
    };

    assert_eq!(commit_info.message, "Test commit message");
    assert_eq!(commit_info.id, "1234567890abcdef1234567890abcdef12345678");
    assert_eq!(commit_info.author_time, 1_234_567_890);
    assert_eq!(commit_info.committer_time, 1_234_567_890);
    assert_eq!(commit_info.mapped_commit_id, None);
  }

  #[test]
  fn test_commit_info_clone() {
    let commit_info = CommitInfo {
      message: "Original message".to_string(),
      id: "abcdef1234567890abcdef1234567890abcdef12".to_string(),
      author_name: "Test Author".to_string(),
      author_email: "test@example.com".to_string(),
      author_time: 1_000_000_000,
      committer_time: 1_000_000_000,
      parent_id: Some("parent123".to_string()),
      tree_id: "tree456".to_string(),
      mapped_commit_id: Some("fedcba9876543210".to_string()),
    };

    let cloned = commit_info.clone();
    assert_eq!(cloned.message, commit_info.message);
    assert_eq!(cloned.id, commit_info.id);
    assert_eq!(cloned.author_time, commit_info.author_time);
    assert_eq!(cloned.committer_time, commit_info.committer_time);
    assert_eq!(cloned.mapped_commit_id, commit_info.mapped_commit_id);
  }
}
