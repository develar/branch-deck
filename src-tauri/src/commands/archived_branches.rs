use branch_integration::archive::get_archived_branch_commits as get_commits;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::Commit;
use model_tauri::blocking::run_blocking;
use model_tauri::command_error::CommandError;
use sync_core::baseline::detect_baseline_branch;
use sync_core::delete_archived_branch::{DeleteArchivedBranchParams, delete_archived_branch_core};

#[tauri::command]
#[specta::specta]
pub async fn get_archived_branch_commits(git_executor: tauri::State<'_, GitCommandExecutor>, repository_path: String, branch_name: String) -> Result<Vec<Commit>, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || {
    // Detect the baseline branch (prefer "master" as default)
    let baseline_branch = detect_baseline_branch(&git, &repository_path, "master")?;
    get_commits(&git, &repository_path, &branch_name, &baseline_branch)
  })
  .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_archived_branch(git_executor: tauri::State<'_, GitCommandExecutor>, params: DeleteArchivedBranchParams) -> Result<(), CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || delete_archived_branch_core(&git, params)).await
}
