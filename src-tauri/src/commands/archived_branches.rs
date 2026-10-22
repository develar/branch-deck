use crate::command_error::CommandError;
use crate::operation::run_blocking;
use branch_integration::archive::{get_archived_branch_commits as get_commits, get_unapplied_branch_commits as get_unapplied_commits};
use domain_types::commit::Commit;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::baseline::detect_baseline_branch;
use sync_core::delete_archived_branch::{DeleteArchivedBranchParams, delete_archived_branch_core, delete_unapplied_branch_core};
use tracing::instrument;

#[tauri::command]
#[instrument(skip(git_executor))]
#[specta::specta]
pub async fn get_archived_branch_commits(
  git_executor: tauri::State<'_, GitCommandExecutor>,
  repository_path: String,
  branch_prefix: String,
  branch_name: String,
) -> Result<Vec<Commit>, CommandError> {
  run_blocking(own_commits(&git_executor, repository_path, branch_prefix, branch_name, get_commits)).await
}

/// The step that reads the own commits of the kept branch `branch_name` by `read`, from the baseline of the repository
fn own_commits(
  git_executor: &GitCommandExecutor,
  repository_path: String,
  branch_prefix: String,
  branch_name: String,
  read: fn(&GitCommandExecutor, &str, &str, &str, &str) -> anyhow::Result<Vec<Commit>>,
) -> impl FnOnce() -> anyhow::Result<Vec<Commit>> + Send + 'static {
  let git = git_executor.clone();
  move || {
    let baseline_branch = detect_baseline_branch(&git, &repository_path)?;
    read(&git, &repository_path, &branch_prefix, &branch_name, &baseline_branch)
  }
}

#[tauri::command]
#[instrument(skip(git_executor))]
#[specta::specta]
pub async fn delete_archived_branch(git_executor: tauri::State<'_, GitCommandExecutor>, params: DeleteArchivedBranchParams) -> Result<(), CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || delete_archived_branch_core(&git, params)).await
}

#[tauri::command]
#[instrument(skip(git_executor))]
#[specta::specta]
pub async fn get_unapplied_branch_commits(
  git_executor: tauri::State<'_, GitCommandExecutor>,
  repository_path: String,
  branch_prefix: String,
  branch_name: String,
) -> Result<Vec<Commit>, CommandError> {
  run_blocking(own_commits(&git_executor, repository_path, branch_prefix, branch_name, get_unapplied_commits)).await
}

#[tauri::command]
#[instrument(skip(git_executor))]
#[specta::specta]
pub async fn delete_unapplied_branch(git_executor: tauri::State<'_, GitCommandExecutor>, params: DeleteArchivedBranchParams) -> Result<(), CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || delete_unapplied_branch_core(&git, params)).await
}
