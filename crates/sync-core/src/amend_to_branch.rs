//! The app's amend of uncommitted changes into a commit of the checked out branch (`git_ops::amend_operations`), under
//! the repository lock; a refusal of the rewrite is a `BranchError` that the app shows.

use domain_types::BranchError;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::amend_operations::{AmendToCommitParams, apply_amend, prepare_amend};
use git_ops::copy_commit::CopyCommitError;
use history_rewrite::repository_lock::RewriteOptions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AmendUncommittedToBranchParams {
  pub repository_path: String,
  pub branch_name: String,
  pub original_commit_id: String,
  pub files: Vec<String>,
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

/// Amends uncommitted changes into a commit of the checked out branch (`amend_to_commit_in_main`), under the repository
/// lock; a refusal of the rewrite is a `BranchError`, the lock held or another HEAD an error with its message
pub fn amend_uncommitted_to_branch_core(git_executor: &GitCommandExecutor, params: AmendUncommittedToBranchParams, options: &RewriteOptions) -> Result<AmendCommandResult, String> {
  let AmendUncommittedToBranchParams {
    repository_path,
    branch_name: _,
    original_commit_id,
    files,
  } = params;

  let _lock = options.lock(git_executor, &repository_path).map_err(|e| format!("{e:#}"))?;
  let amend_params = AmendToCommitParams {
    original_commit_id: original_commit_id.clone(),
    files,
  };
  let prepared = match prepare_amend(git_executor, &repository_path, amend_params) {
    Ok(prepared) => prepared,
    Err(e) => return refused(e),
  };
  if let Err(moved) = options.ensure_head(git_executor, &repository_path, prepared.old_head()) {
    prepared.discard(git_executor, &repository_path);
    return Err(moved.to_string());
  }
  match apply_amend(git_executor, &repository_path, &prepared, &original_commit_id) {
    Ok(result) => Ok(AmendCommandResult::Ok(AmendResult {
      amended_commit_id: result.amended_commit_id,
      rebased_to_commit: result.rebased_to_commit,
    })),
    Err(e) => refused(e),
  }
}

/// A refusal of the rewrite as the page shows it (a conflict opens the conflict view), any other failure as an error
fn refused(error: CopyCommitError) -> Result<AmendCommandResult, String> {
  match error {
    CopyCommitError::BranchError(branch_error) => Ok(AmendCommandResult::BranchError(branch_error)),
    CopyCommitError::Other(other_err) => Err(format!("Failed to amend commit: {other_err}")),
  }
}
