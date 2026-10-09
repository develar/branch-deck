use branch_integration::archive::get_archived_branch_commits as get_commits;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::command_error::CommandError;
use git_ops::commit_list::Commit;
use sync_core::delete_archived_branch::{DeleteArchivedBranchParams, delete_archived_branch_core};
use sync_core::sync::detect_baseline_branch;

#[tauri::command]
#[specta::specta]
pub async fn get_archived_branch_commits(git_executor: tauri::State<'_, GitCommandExecutor>, repository_path: String, branch_name: String) -> Result<Vec<Commit>, CommandError> {
  // Detect the baseline branch (prefer "master" as default)
  let baseline_branch = detect_baseline_branch(&git_executor, &repository_path, "master")?;

  Ok(get_commits(&git_executor, &repository_path, &branch_name, &baseline_branch)?)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_archived_branch(git_executor: tauri::State<'_, GitCommandExecutor>, params: DeleteArchivedBranchParams) -> Result<(), CommandError> {
  delete_archived_branch_core(&git_executor, params)?;
  Ok(())
}
