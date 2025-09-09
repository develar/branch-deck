use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::sync::detect_baseline_branch;
use sync_core::unapply_branch::{UnapplyBranchParams, UnapplyBranchResult, unapply_branch_core};
use tauri::State;
use tokio::task;

#[tauri::command]
#[specta::specta]
pub async fn unapply_branch(git_executor: State<'_, GitCommandExecutor>, params: UnapplyBranchParams) -> Result<UnapplyBranchResult, String> {
  let git = (*git_executor).clone();

  task::spawn_blocking(move || {
    // Detect the baseline branch (prefer "master" as default)
    let baseline_branch = detect_baseline_branch(&git, &params.repository_path, "master").map_err(|e| e.to_string())?;

    unapply_branch_core(&git, params, &baseline_branch).map_err(|e| e.to_string())
  })
  .await
  .map_err(|e| format!("Task error: {}", e))?
}
