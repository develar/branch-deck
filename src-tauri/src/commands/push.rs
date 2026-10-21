use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use repository_view::views::RepositoryViews;
use std::collections::HashMap;
use sync_core::push::{
  NOT_SYNCED_YET, PushBranchParams, PushBranchesOutcome, PushBranchesParams, PushOutcome, push_branch as core_push_branch, push_branches as core_push_branches,
};
use sync_core::transfer_progress::TransferProgress;
use sync_types::stack::BlockReason;
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};

/// Pushes a branch to the remote, replacing the remote branch only as the user saw it, and returns its new remote
/// status; a remote that moved is fetched and returned instead. A branch the last sync did not build is refused.
/// `progress` receives how far the push is.
#[tauri::command]
#[instrument(skip(git_executor, views, progress))]
#[specta::specta]
pub async fn push_branch(
  git_executor: State<'_, GitCommandExecutor>,
  views: State<'_, RepositoryViews>,
  params: PushBranchParams,
  progress: Channel<TransferProgress>,
) -> Result<PushOutcome, CommandError> {
  let git = (*git_executor).clone();
  let not_built = not_built(&views, &params.repository_path, &params.branch_prefix)?;
  run_blocking(move || core_push_branch(&git, &params, &not_built, &mut |push_progress| send_progress(&progress, push_progress))).await
}

/// Pushes the branches of a stack to the remote in one atomic push, all of them or none, each replacing its remote
/// branch only as the user saw it, and returns their new remote statuses; a remote branch that moved is named, with the
/// statuses fetched after it. A branch the last sync did not build refuses the push. `progress` receives how far the push
/// is.
#[tauri::command]
#[instrument(skip(git_executor, views, progress))]
#[specta::specta]
pub async fn push_branches(
  git_executor: State<'_, GitCommandExecutor>,
  views: State<'_, RepositoryViews>,
  params: PushBranchesParams,
  progress: Channel<TransferProgress>,
) -> Result<PushBranchesOutcome, CommandError> {
  let git = (*git_executor).clone();
  let not_built = not_built(&views, &params.repository_path, &params.branch_prefix)?;
  run_blocking(move || core_push_branches(&git, &params, &not_built, &mut |push_progress| send_progress(&progress, push_progress))).await
}

/// The branches the last sync did not build, from the table it left (`RepositoryViews::not_built`)
fn not_built(views: &RepositoryViews, repository_path: &str, branch_prefix: &str) -> Result<HashMap<String, BlockReason>, CommandError> {
  views.not_built(repository_path, branch_prefix).ok_or_else(|| CommandError::new(NOT_SYNCED_YET))
}

fn send_progress(progress: &Channel<TransferProgress>, push_progress: TransferProgress) {
  if let Err(e) = progress.send(push_progress) {
    warn!(error = %e, "failed to report the push progress");
  }
}
