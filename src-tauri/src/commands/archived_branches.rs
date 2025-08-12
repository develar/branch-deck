use branch_integration::archive::get_archived_branch_commits as get_commits;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::Commit;
use sync_core::delete_archived_branch::{DeleteArchivedBranchParams, delete_archived_branch_core};
use sync_core::sync::detect_baseline_branch;

#[tauri::command]
#[specta::specta]
pub async fn get_archived_branch_commits(git_executor: tauri::State<'_, GitCommandExecutor>, repository_path: String, branch_name: String) -> Result<Vec<Commit>, String> {
  // Detect the baseline branch (prefer "master" as default)
  let baseline_branch = detect_baseline_branch(&git_executor, &repository_path, "master").map_err(|e| e.to_string())?;

  get_commits(&git_executor, &repository_path, &branch_name, &baseline_branch).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_archived_branch(git_executor: tauri::State<'_, GitCommandExecutor>, repository_path: String, branch_name: String, branch_prefix: String) -> Result<(), String> {
  let params = DeleteArchivedBranchParams {
    repository_path,
    branch_name,
    branch_prefix,
  };

  delete_archived_branch_core(&git_executor, params).map_err(|e| e.to_string())?;
  Ok(())
}
