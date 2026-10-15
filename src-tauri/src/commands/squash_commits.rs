use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::squash::{RestoreBranchHeadParams, SquashCommitsParams, SquashCommitsResult, restore_branch_head_core, squash_commits_core};
use tauri::State;
use tracing::instrument;

/// Squashes commits of the current branch into the oldest of them in one history rewrite that keeps HEAD's tree; the
/// index and the working tree are not touched.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn squash_commits(git_executor: State<'_, GitCommandExecutor>, params: SquashCommitsParams) -> Result<SquashCommitsResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || squash_commits_core(&git, &params)).await
}

/// Undoes a squash: moves the current branch back to its previous commit, if it was not moved since
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn restore_branch_head(git_executor: State<'_, GitCommandExecutor>, params: RestoreBranchHeadParams) -> Result<(), CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || restore_branch_head_core(&git, &params)).await
}
