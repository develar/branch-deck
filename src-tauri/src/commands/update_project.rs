use git_executor::git_command_executor::GitCommandExecutor;
use model_tauri::blocking::run_blocking;
use model_tauri::command_error::CommandError;
use sync_core::update_project::{UpdatePhase, UpdateProjectParams, UpdateProjectResult, update_project_core};
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};

/// Fetches the baseline and brings the current branch onto it, keeping unrelated local changes; a conflict or a block
/// is a result with nothing changed. `progress` receives the step the update is at.
#[tauri::command]
#[instrument(skip(git_executor, progress))]
#[specta::specta]
pub async fn update_project(git_executor: State<'_, GitCommandExecutor>, params: UpdateProjectParams, progress: Channel<UpdatePhase>) -> Result<UpdateProjectResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || {
    update_project_core(&git, &params, &|phase| {
      if let Err(e) = progress.send(phase) {
        warn!(error = %e, "failed to report the update phase");
      }
    })
  })
  .await
}
