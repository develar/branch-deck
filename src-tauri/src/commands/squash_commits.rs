use crate::lock_wait::{WaitChannel, Waits, run_waiting};
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::RewriteOptions;
use history_rewrite::squash::{RestoreBranchHeadParams, SquashCommitsParams, SquashCommitsResult, restore_branch_head_core, squash_commits_core};
use tauri::State;
use tracing::instrument;

/// Squashes commits of the current branch into the oldest of them in one history rewrite that keeps HEAD's tree; the
/// index and the working tree are not touched.
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, commits = params.commit_ids.len()))]
#[specta::specta]
pub async fn squash_commits(
  git_executor: State<'_, GitCommandExecutor>,
  lock_waits: Waits<'_>,
  params: SquashCommitsParams,
  wait: WaitChannel,
) -> Result<SquashCommitsResult, ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |lock_wait| squash_commits_core(&git, &params, &RewriteOptions::waiting(lock_wait))).await
}

/// Undoes a squash: moves the current branch back to its previous commit, if it was not moved since
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, lock_waits, wait))]
pub async fn restore_branch_head(git_executor: State<'_, GitCommandExecutor>, lock_waits: Waits<'_>, params: RestoreBranchHeadParams, wait: WaitChannel) -> Result<(), ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |lock_wait| {
    // also the undo of squash fixups; a branch moved since is `undo_stale`, as every undo answers it
    restore_branch_head_core(&git, &params, lock_wait).map_err(app_api::api_error::undo_error)
  })
  .await
}
