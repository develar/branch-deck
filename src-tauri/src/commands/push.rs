use crate::command_error::CommandError;
use crate::commands::patronus::PatronusState;
use crate::operation::{operation, run_blocking};
use crate::repository_state::RepositoryStateCache;
use domain_types::patronus::PatronusRobot;
use git_executor::git_command_executor::GitCommandExecutor;
use repository_view::push_guard::{push_branch_guarded, push_guarded};
use repository_view::views::RepositoryViews;
use std::path::Path;
use sync_core::push::{PushBranchParams, PushBranchesOutcome, PushBranchesParams, PushOutcome};
use sync_core::transfer_progress::TransferProgress;
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};

/// Pushes a branch to the remote, replacing the remote branch only as the user saw it, and returns its new remote
/// status; a remote that moved is fetched and returned instead. Under the repository lock, and refused in Rust as `bd
/// push` refuses it (`push_guard`): a branch the last sync did not build, a parked branch or one built on parked, one a
/// merged Safe Push landed whole, one a running Safe Push holds (the robots as Rust reads them). `progress` receives how
/// far the push is.
#[tauri::command]
#[instrument(skip(git_executor, repository_state, patronus, views, progress))]
#[specta::specta]
pub async fn push_branch(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  patronus: State<'_, PatronusState>,
  views: State<'_, RepositoryViews>,
  params: PushBranchParams,
  progress: Channel<TransferProgress>,
) -> Result<PushOutcome, CommandError> {
  let target = (params.repository_path.clone(), params.branch_prefix.clone());
  operation(guarded(
    &git_executor,
    &repository_state,
    &patronus,
    &views,
    target,
    move |git, common_dir, views, robots| push_branch_guarded(git, common_dir, views, robots, &params, &mut |push_progress| send_progress(&progress, push_progress)),
  ))
  .await
}

/// Pushes the branches of a stack to the remote in one atomic push, all of them or none, each replacing its remote
/// branch only as the user saw it, and returns their new remote statuses; a remote branch that moved is named, with the
/// statuses fetched after it. Under the repository lock, refused in Rust as `push_branch` is for any of its branches; the
/// stack branch of a whole stack also while a member is parked, every member landed whole or its robot runs on it; an
/// unapplied branch takes only the lock. `progress` receives how far the push is.
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_prefix = %params.branch_prefix, branches = params.branches.len()))]
#[specta::specta]
pub async fn push_branches(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  patronus: State<'_, PatronusState>,
  views: State<'_, RepositoryViews>,
  params: PushBranchesParams,
  progress: Channel<TransferProgress>,
) -> Result<PushBranchesOutcome, CommandError> {
  let target = (params.repository_path.clone(), params.branch_prefix.clone());
  operation(guarded(
    &git_executor,
    &repository_state,
    &patronus,
    &views,
    target,
    move |git, common_dir, views, robots| push_guarded(git, common_dir, views, robots, &params, &mut |push_progress| send_progress(&progress, push_progress)),
  ))
  .await
}

/// A guarded push of the repository and branch prefix of `target`: the user's robots as Rust reads them
/// (`robots_for_checks`), then, in a blocking step, its `$GIT_COMMON_DIR` (where the lock lives) and `push`
async fn guarded<T: Send + 'static>(
  git_executor: &GitCommandExecutor,
  repository_state: &RepositoryStateCache,
  patronus: &PatronusState,
  views: &RepositoryViews,
  (repository_path, branch_prefix): (String, String),
  push: impl FnOnce(&GitCommandExecutor, &Path, &RepositoryViews, Vec<PatronusRobot>) -> anyhow::Result<T> + Send + 'static,
) -> Result<T, CommandError> {
  let robots = patronus.robots_for_checks(git_executor, &repository_path, &branch_prefix).await;
  let (git, state, views) = (git_executor.clone(), repository_state.clone(), views.clone());
  run_blocking(move || {
    let common_dir = state.git_common_dir(&git, &repository_path)?;
    push(&git, &common_dir, &views, robots)
  })
  .await
}

fn send_progress(progress: &Channel<TransferProgress>, push_progress: TransferProgress) {
  if let Err(e) = progress.send(push_progress) {
    warn!(error = %e, "failed to report the push progress");
  }
}
