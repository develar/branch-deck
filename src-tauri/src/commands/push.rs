use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::push::{PushBranchParams, PushBranchesOutcome, PushBranchesParams, PushOutcome, push_branch as core_push_branch, push_branches as core_push_branches};
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
  run_blocking(move || core_push_branch(&git, &params, &mut |push_progress| send_progress(&progress, push_progress))).await
}

/// Pushes the branches of a stack to the remote in one atomic push, all of them or none, each replacing its remote
/// branch only as the user saw it, and returns their new remote statuses; a remote branch that moved is named, with the
/// statuses fetched after it. `progress` receives how far the push is.
#[tauri::command]
#[instrument(skip(git_executor, progress))]
#[specta::specta]
pub async fn push_branches(
  git_executor: State<'_, GitCommandExecutor>,
  params: PushBranchesParams,
  progress: Channel<TransferProgress>,
) -> Result<PushBranchesOutcome, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || core_push_branches(&git, &params, &mut |push_progress| send_progress(&progress, push_progress))).await
}

fn send_progress(progress: &Channel<TransferProgress>, push_progress: TransferProgress) {
  if let Err(e) = progress.send(push_progress) {
    warn!(error = %e, "failed to report the push progress");
  }
}
