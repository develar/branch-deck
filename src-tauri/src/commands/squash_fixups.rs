use crate::lock_wait::{WaitChannel, Waits, run_waiting};
use crate::operation::run_blocking;
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::autosquash::{AutosquashPlan, GetAutosquashPlanParams, SquashFixupsParams, SquashFixupsResult, get_autosquash_plan_core, squash_fixup_commits_core};
use history_rewrite::repository_lock::RewriteOptions;
use tauri::State;
use tracing::instrument;

/// The `fixup!`, `squash!` and `amend!` commits ahead of the baseline, grouped by the commit each one is squashed into
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn get_autosquash_plan(git_executor: State<'_, GitCommandExecutor>, params: GetAutosquashPlanParams) -> Result<AutosquashPlan, ApiError> {
  let git = (*git_executor).clone();
  run_blocking(move || get_autosquash_plan_core(&git, &params.repository_path)).await
}

/// Squashes fixup commits into their targets in one history rewrite that keeps HEAD's tree; the index and the working
/// tree are not touched.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, lock_waits, wait))]
pub async fn squash_fixup_commits(
  git_executor: State<'_, GitCommandExecutor>,
  lock_waits: Waits<'_>,
  params: SquashFixupsParams,
  wait: WaitChannel,
) -> Result<SquashFixupsResult, ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |lock_wait| {
    squash_fixup_commits_core(&git, &params, &RewriteOptions::waiting(lock_wait))
  })
  .await
}
