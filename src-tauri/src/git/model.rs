use anyhow::ensure;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
pub enum BranchSyncStatus {
  Created,
  Updated,
  Unchanged,
  Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CommitDetail {
  pub original_hash: String,
  pub hash: String,
  pub is_new: bool,
  pub message: String,
  pub time: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BranchInfo {
  pub name: String,
  pub sync_status: BranchSyncStatus,
  pub commit_count: u32,
  pub commit_details: Vec<CommitDetail>,
  pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncBranchResult {
  pub branches: Vec<BranchInfo>,
}

/// Information about a commit needed for parallel processing
/// Contains only the essential data that can be safely sent across threads
#[derive(Debug, Clone)]
pub struct CommitInfo {
  pub message: String,
  pub id: git2::Oid,
  pub time: u32,
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
      is_new: true,
      message: "Test commit".to_string(),
      time: 1_234_567_890,
    };

    assert_eq!(commit.original_hash, "abc123");
    assert_eq!(commit.hash, "def456");
    assert!(commit.is_new);
    assert_eq!(commit.message, "Test commit");
    assert_eq!(commit.time, 1_234_567_890);
  }

  #[test]
  fn test_branch_info_creation() {
    let commit = CommitDetail {
      original_hash: "abc123".to_string(),
      hash: "def456".to_string(),
      is_new: true,
      message: "Test commit".to_string(),
      time: 1_234_567_890,
    };

    let branch = BranchInfo {
      name: "feature/auth".to_string(),
      sync_status: BranchSyncStatus::Created,
      commit_count: 1,
      commit_details: vec![commit],
      error: None,
    };

    assert_eq!(branch.name, "feature/auth");
    assert_eq!(branch.sync_status, BranchSyncStatus::Created);
    assert_eq!(branch.commit_count, 1);
    assert_eq!(branch.commit_details.len(), 1);
    assert!(branch.error.is_none());
  }

  #[test]
  fn test_branch_info_with_error() {
    let branch = BranchInfo {
      name: "feature/broken".to_string(),
      sync_status: BranchSyncStatus::Error,
      commit_count: 0,
      commit_details: vec![],
      error: Some("Failed to create branch".to_string()),
    };

    assert_eq!(branch.sync_status, BranchSyncStatus::Error);
    assert!(branch.error.is_some());
    assert_eq!(branch.error.unwrap(), "Failed to create branch");
  }

  #[test]
  fn test_sync_branch_result_creation() {
    let branch1 = BranchInfo {
      name: "feature/auth".to_string(),
      sync_status: BranchSyncStatus::Created,
      commit_count: 2,
      commit_details: vec![],
      error: None,
    };

    let branch2 = BranchInfo {
      name: "bugfix/login".to_string(),
      sync_status: BranchSyncStatus::Updated,
      commit_count: 1,
      commit_details: vec![],
      error: None,
    };

    let result = SyncBranchResult { branches: vec![branch1, branch2] };

    assert_eq!(result.branches.len(), 2);
    assert_eq!(result.branches[0].name, "feature/auth");
    assert_eq!(result.branches[1].name, "bugfix/login");
  }

  #[test]
  fn test_serialization_deserialization() {
    let branch = BranchInfo {
      name: "test/branch".to_string(),
      sync_status: BranchSyncStatus::Created,
      commit_count: 1,
      commit_details: vec![],
      error: None,
    };

    // Test that the struct can be serialized and deserialized
    let json = serde_json::to_string(&branch).unwrap();
    let deserialized: BranchInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(branch.name, deserialized.name);
    assert_eq!(branch.sync_status, deserialized.sync_status);
    assert_eq!(branch.commit_count, deserialized.commit_count);
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
      id: git2::Oid::from_str("1234567890abcdef1234567890abcdef12345678").unwrap(),
      time: 1_234_567_890,
    };

    assert_eq!(commit_info.message, "Test commit message");
    assert_eq!(commit_info.id.to_string(), "1234567890abcdef1234567890abcdef12345678");
    assert_eq!(commit_info.time, 1_234_567_890);
  }

  #[test]
  fn test_commit_info_clone() {
    let commit_info = CommitInfo {
      message: "Original message".to_string(),
      id: git2::Oid::from_str("abcdef1234567890abcdef1234567890abcdef12").unwrap(),
      time: 1_000_000_000,
    };

    let cloned = commit_info.clone();
    assert_eq!(cloned.message, commit_info.message);
    assert_eq!(cloned.id, commit_info.id);
    assert_eq!(cloned.time, commit_info.time);
  }
}
