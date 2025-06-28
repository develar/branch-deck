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