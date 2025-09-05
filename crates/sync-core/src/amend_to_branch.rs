use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::amend_operations::{AmendToCommitParams, amend_to_commit_in_main};
use git_ops::copy_commit::CopyCommitError;
use git_ops::model::BranchError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AmendUncommittedToBranchParams {
  pub repository_path: String,
  pub branch_name: String,
  pub original_commit_id: String,
  pub main_branch: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AmendResult {
  pub amended_commit_id: String,
  pub rebased_to_commit: String,
}

/// Result type for amend command that can be properly serialized by Tauri
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "status", content = "data", rename_all = "camelCase")]
pub enum AmendCommandResult {
  Ok(AmendResult),
  BranchError(BranchError),
}

/// Core function for amending uncommitted changes to a branch
pub fn amend_uncommitted_to_branch_core(git_executor: &GitCommandExecutor, params: AmendUncommittedToBranchParams) -> Result<AmendCommandResult, String> {
  let AmendUncommittedToBranchParams {
    repository_path,
    branch_name: _,
    original_commit_id,
    main_branch,
  } = params;

  // Perform the amend operation
  let amend_params = AmendToCommitParams { original_commit_id };

  match amend_to_commit_in_main(git_executor, &repository_path, &main_branch, amend_params) {
    Ok(result) => Ok(AmendCommandResult::Ok(AmendResult {
      amended_commit_id: result.amended_commit_id,
      rebased_to_commit: result.rebased_to_commit,
    })),
    Err(CopyCommitError::BranchError(branch_error)) => Ok(AmendCommandResult::BranchError(branch_error)),
    Err(CopyCommitError::Other(other_err)) => Err(format!("Failed to amend commit: {}", other_err)),
  }
}
