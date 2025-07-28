use git_ops::git_command::GitCommandExecutor;
use tauri::State;
use tracing::instrument;

// Re-export types from branch-sync for backward compatibility
pub use branch_sync::create_branch::{CreateBranchFromCommitsParams, RewordResult};

/// Assigns commits to a branch by prepending a branch prefix to their messages.
/// Uses git plumbing commands to efficiently rewrite commit messages without touching the working directory.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn create_branch_from_commits(git_executor: State<'_, GitCommandExecutor>, params: CreateBranchFromCommitsParams) -> Result<RewordResult, String> {
  branch_sync::create_branch::do_create_branch_from_commits(&git_executor, params).await
}
