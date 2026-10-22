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
  operation(guarded(&git_executor, &repository_state, &patronus, &views, params, progress, push_branch_guarded)).await
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
  operation(guarded(&git_executor, &repository_state, &patronus, &views, params, progress, push_guarded)).await
}

/// The parameters of a push name the repository and the branch prefix whose robots and views the guard reads
trait PushTarget: Send + 'static {
  fn repository_path(&self) -> &str;
  fn branch_prefix(&self) -> &str;
}

impl PushTarget for PushBranchParams {
  fn repository_path(&self) -> &str {
    &self.repository_path
  }

  fn branch_prefix(&self) -> &str {
    &self.branch_prefix
  }
}

impl PushTarget for PushBranchesParams {
  fn repository_path(&self) -> &str {
    &self.repository_path
  }

  fn branch_prefix(&self) -> &str {
    &self.branch_prefix
  }
}

/// A guard of `push_guard`: the lock, the checks and the push of the parameters, reporting its progress
type Guard<P, T> = fn(&GitCommandExecutor, &Path, &RepositoryViews, Vec<PatronusRobot>, &P, &mut dyn FnMut(TransferProgress)) -> anyhow::Result<T>;

/// The whole body of a push command, so the command is one operation: the user's robots as Rust reads them
/// (`robots_for_checks`), then, in a blocking step, the `$GIT_COMMON_DIR` of the repository (where the lock lives) and
/// `push`, its progress sent to `progress`
async fn guarded<P: PushTarget, T: Send + 'static>(
  git_executor: &GitCommandExecutor,
  repository_state: &RepositoryStateCache,
  patronus: &PatronusState,
  views: &RepositoryViews,
  params: P,
  progress: Channel<TransferProgress>,
  push: Guard<P, T>,
) -> Result<T, CommandError> {
  let robots = patronus.robots_for_checks(git_executor, params.repository_path(), params.branch_prefix()).await;
  let (git, state, views) = (git_executor.clone(), repository_state.clone(), views.clone());
  run_blocking(move || {
    let common_dir = state.git_common_dir(&git, params.repository_path())?;
    push(&git, &common_dir, &views, robots, &params, &mut |push_progress| send_progress(&progress, push_progress))
  })
  .await
}

fn send_progress(progress: &Channel<TransferProgress>, push_progress: TransferProgress) {
  if let Err(e) = progress.send(push_progress) {
    warn!(error = %e, "failed to report the push progress");
  }
}
