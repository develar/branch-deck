use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::move_commits::{MoveCommitsParams, MoveCommitsResult, move_commits_to_branch_core};
use tauri::State;
use tracing::instrument;

/// Moves commits to another branch by setting (or removing) the `(branch)` prefix of their subjects in one history
/// rewrite; the working tree is not touched.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn move_commits_to_branch(git_executor: State<'_, GitCommandExecutor>, params: MoveCommitsParams) -> Result<MoveCommitsResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || move_commits_to_branch_core(&git, &params)).await
}
