use crate::command_error::CommandError;
use crate::operation::run_blocking;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::landed::dismiss_landed as dismiss_landed_core;
use sync_core::update_project::{UpdateProgress, UpdateProjectParams, UpdateProjectResult, update_project_core};
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};

/// Fetches the baseline and brings the current branch onto it, keeping unrelated local changes; a conflict or a block
/// is a result with nothing changed. `progress` receives the step the update is at and how far the fetch is.
#[tauri::command]
#[instrument(skip(git_executor, progress))]
#[specta::specta]
pub async fn update_project(
  git_executor: State<'_, GitCommandExecutor>,
  params: UpdateProjectParams,
  progress: Channel<UpdateProgress>,
) -> Result<UpdateProjectResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || {
    update_project_core(&git, &params, &|update_progress| {
      if let Err(e) = progress.send(update_progress) {
        warn!(error = %e, "failed to report the update progress");
      }
    })
  })
  .await
}

/// Clears the report of the commits that landed with an update; the next update starts a new one
#[tauri::command]
#[instrument(skip(git_executor))]
#[specta::specta]
pub async fn dismiss_landed(git_executor: State<'_, GitCommandExecutor>, repository_path: String) -> Result<(), CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || dismiss_landed_core(&git, &repository_path)).await
}
