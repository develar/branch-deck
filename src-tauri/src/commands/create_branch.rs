use git_executor::git_command_executor::GitCommandExecutor;
use tauri::State;
use tracing::instrument;

use sync_core::create_branch::{CreateBranchFromCommitsParams, RewordResult};

/// Assigns commits to a branch by prepending a branch prefix to their messages.
/// Uses git plumbing commands to efficiently rewrite commit messages without touching the working directory.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn create_branch_from_commits(git_executor: State<'_, GitCommandExecutor>, params: CreateBranchFromCommitsParams) -> Result<RewordResult, String> {
  // Clone the executor since spawn_blocking requires 'static lifetime
  let git = (*git_executor).clone();
  tokio::task::spawn_blocking(move || sync_core::create_branch::do_create_branch_from_commits(&git, params))
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}
