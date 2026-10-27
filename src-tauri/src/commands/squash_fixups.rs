use crate::operation::run_blocking;
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::autosquash::{AutosquashPlan, GetAutosquashPlanParams, get_autosquash_plan_core};
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
