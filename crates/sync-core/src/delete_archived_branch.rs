use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DeleteArchivedBranchParams {
  pub repository_path: String,
  pub branch_name: String,
  pub branch_prefix: String,
}

/// Core function to delete an archived branch
/// This function contains all the safety checks and deletion logic
#[instrument(skip(git_executor), fields(repo = %params.repository_path, branch = %params.branch_name))]
pub fn delete_archived_branch_core(git_executor: &GitCommandExecutor, params: DeleteArchivedBranchParams) -> Result<()> {
  let DeleteArchivedBranchParams {
    repository_path,
    branch_name,
    branch_prefix,
  } = params;

  // Safety checks: only allow deleting refs under <prefix>/archived/
  let required_prefix = format!("{}/archived/", branch_prefix);
  if !branch_name.starts_with(&required_prefix) {
    return Err(anyhow::anyhow!("Can only delete archived branches under the configured branch prefix"));
  }

  if branch_name.starts_with('-') || branch_name.contains("..") || branch_name.contains('\n') || branch_name.contains('\r') {
    return Err(anyhow::anyhow!("Invalid branch name"));
  }

  // Verify branch exists before attempting deletion
  let exists = git_executor
    .execute_command(&["show-ref", "--verify", &format!("refs/heads/{}", branch_name)], &repository_path)
    .is_ok();
  if !exists {
    return Err(anyhow::anyhow!("Branch does not exist"));
  }

  // Delete branch
  git_executor
    .execute_command(&["branch", "-D", &branch_name], &repository_path)
    .map_err(|e| anyhow::anyhow!("Failed to delete branch: {}", e))?;

  Ok(())
}
