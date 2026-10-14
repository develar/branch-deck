use git_executor::git_command_executor::GitCommandExecutor;
use model_tauri::blocking::run_blocking;
use model_tauri::command_error::CommandError;
use sync_core::push::{PushBranchParams, PushOutcome, push_branch as core_push_branch};
use sync_core::transfer_progress::TransferProgress;
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};

/// Pushes a branch to the remote, replacing the remote branch only as the user saw it, and returns its new remote
/// status; a remote that moved is fetched and returned instead. `progress` receives how far the push is.
#[tauri::command]
#[instrument(skip(git_executor, progress))]
#[specta::specta]
pub async fn push_branch(git_executor: State<'_, GitCommandExecutor>, params: PushBranchParams, progress: Channel<TransferProgress>) -> Result<PushOutcome, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || {
    core_push_branch(&git, &params, &mut |push_progress| {
      if let Err(e) = progress.send(push_progress) {
        warn!(error = %e, "failed to report the push progress");
      }
    })
  })
  .await
}
