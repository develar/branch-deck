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

pub fn to_final_branch_name(
  branch_prefix: &str,
  branch_name: &str,
) -> Result<String, String> {
  let prefix = branch_prefix.trim_end_matches('/').trim();
  let name = branch_name.trim_end_matches('/').trim();
  if prefix.is_empty() {
    return Err("branch prefix cannot be blank".to_string());
  }
  if name.is_empty() {
    return Err("branch name cannot be blank".into());
  }
  Ok(format!("{prefix}/virtual/{name}"))
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
    assert_eq!(result.unwrap_err(), "branch prefix cannot be blank");
  }

  #[test]
  fn test_to_final_branch_name_whitespace_prefix() {
    let result = to_final_branch_name("   ", "auth");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "branch prefix cannot be blank");
  }

  #[test]
  fn test_to_final_branch_name_empty_name() {
    let result = to_final_branch_name("feature", "");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "branch name cannot be blank");
  }

  #[test]
  fn test_to_final_branch_name_whitespace_name() {
    let result = to_final_branch_name("feature", "   ");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "branch name cannot be blank");
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

    let result = SyncBranchResult {
      branches: vec![branch1, branch2],
    };

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
}
