use crate::lock_wait::{WaitChannel, Waits, run_waiting};
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::move_commits::{MoveCommitsParams, MoveCommitsResult, move_commits_to_branch_core};
use tauri::State;
use tracing::instrument;

/// Moves commits to another branch by setting (or removing) the `(branch)` prefix of their subjects in one history
/// rewrite; the working tree is not touched.
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, commits = params.commit_ids.len(), target_branch = ?params.target_branch, dry_run = params.dry_run))]
#[specta::specta]
pub async fn move_commits_to_branch(
  git_executor: State<'_, GitCommandExecutor>,
  lock_waits: Waits<'_>,
  params: MoveCommitsParams,
  wait: WaitChannel,
) -> Result<MoveCommitsResult, ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |lock_wait| move_commits_to_branch_core(&git, &params, lock_wait)).await
}
