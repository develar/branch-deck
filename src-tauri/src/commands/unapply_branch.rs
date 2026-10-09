use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::command_error::CommandError;
use sync_core::sync::detect_baseline_branch;
use sync_core::unapply_branch::{UnapplyBranchParams, UnapplyBranchResult, unapply_branch_core};
use tauri::State;
use tokio::task;

#[tauri::command]
#[specta::specta]
pub async fn unapply_branch(git_executor: State<'_, GitCommandExecutor>, params: UnapplyBranchParams) -> Result<UnapplyBranchResult, CommandError> {
  let git = (*git_executor).clone();

  task::spawn_blocking(move || {
    // Detect the baseline branch (prefer "master" as default)
    let baseline_branch = detect_baseline_branch(&git, &params.repository_path, "master")?;

    unapply_branch_core(&git, params, &baseline_branch)
  })
  .await
  .map_err(|e| CommandError::new(format!("Task failed: {e}")))?
  .map_err(CommandError::from)
}
