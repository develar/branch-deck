use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::amend_operations::{AmendToCommitParams, amend_to_commit_in_main};
use git_ops::copy_commit::CopyCommitError;
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::instrument;

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
  BranchError(git_ops::model::BranchError),
}

/// Amend uncommitted changes to the original commit corresponding to a virtual branch tip.
/// This operation modifies the main branch history and requires a sync afterward to recreate virtual branches.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn amend_uncommitted_to_branch(git_executor: State<'_, GitCommandExecutor>, params: AmendUncommittedToBranchParams) -> Result<AmendCommandResult, String> {
  let git = (*git_executor).clone();

  tokio::task::spawn_blocking(move || {
    let AmendUncommittedToBranchParams {
      repository_path,
      branch_name: _,
      original_commit_id,
      main_branch,
    } = params;

    // Perform the amend operation (includes conflict checking)
    let amend_params = AmendToCommitParams { original_commit_id };

    // Perform the amend operation and handle errors properly
    match amend_to_commit_in_main(&git, &repository_path, &main_branch, amend_params) {
      Ok(result) => Ok(AmendCommandResult::Ok(AmendResult {
        amended_commit_id: result.amended_commit_id,
        rebased_to_commit: result.rebased_to_commit,
      })),
      Err(CopyCommitError::BranchError(branch_error)) => {
        // Return structured BranchError so frontend can display MergeConflictViewer
        Ok(AmendCommandResult::BranchError(branch_error))
      }
      Err(CopyCommitError::Other(other_err)) => {
        // Non-branch errors are still returned as string errors
        Err(format!("Failed to amend commit: {}", other_err))
      }
    }
  })
  .await
  .map_err(|e| format!("Task failed: {}", e))?
}
