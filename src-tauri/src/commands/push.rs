use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::to_final_branch_name;
use serde::Deserialize;
use sync_core::remote_status::compute_remote_status_for_branch;
use sync_types::RemoteStatusUpdate;
use tauri::State;

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PushBranchParams {
  pub repository_path: String,
  pub branch_prefix: String,
  pub branch_name: String,
  pub total_commits: u32,
  pub my_email: Option<String>,
  pub baseline_branch: String,
}

/// Pushes a specific branch to the remote repository and returns updated remote status
#[tauri::command]
#[specta::specta]
pub async fn push_branch(git_executor: State<'_, GitCommandExecutor>, params: PushBranchParams) -> Result<RemoteStatusUpdate, String> {
  // Clone the executor since spawn_blocking requires 'static lifetime
  let git = (*git_executor).clone();

  tokio::task::spawn_blocking(move || {
    let repository_path = &params.repository_path;
    let branch_prefix = &params.branch_prefix;
    let branch_name = &params.branch_name;
    let final_branch_name = to_final_branch_name(branch_prefix, branch_name).map_err(|e| format!("{e:?}"))?;

    // Perform the push
    git
      .execute_command(
        &[
          "-c",
          "credential.helper=",
          "-c",
          "log.showSignature=false",
          "push",
          "--porcelain",
          "--force",
          "origin",
          &format!("refs/heads/{final_branch_name}:{final_branch_name}"),
        ],
        repository_path,
      )
      .map_err(|e| e.to_string())?;

    // Compute and return updated remote status
    let remote_status = compute_remote_status_for_branch(
      &git,
      repository_path,
      &final_branch_name,
      branch_name,
      params.my_email.as_deref(),
      params.total_commits,
      &params.baseline_branch,
    )
    .map_err(|e| format!("Failed to compute remote status: {}", e))?;

    Ok(remote_status)
  })
  .await
  .map_err(|e| format!("Task failed: {e}"))?
}
