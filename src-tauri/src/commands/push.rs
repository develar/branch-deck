use crate::command_error::CommandError;
use crate::commands::patronus::{PatronusState, remote_actions};
use crate::operation::{operation, run_blocking};
use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use repository_model::views::RepositoryViews;
use std::path::PathBuf;
use sync_core::patronus::SpaceRepository;
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
  operation(async {
    let (common_dir, space) = push_inputs(&git_executor, &repository_state, &patronus, &params.repository_path).await?;
    let actions = remote_actions(&patronus, space.as_ref(), &git_executor, &views);
    let pushed = actions.push_branch(common_dir, &params, move |push_progress| send_progress(&progress, push_progress)).await;
    pushed.map_err(CommandError::from)
  })
  .await
}

/// Pushes the branches of a stack to the remote in one atomic push, all of them or none, each replacing its remote
/// branch only as the user saw it, and returns their new remote statuses; a remote branch that moved is named, with the
/// statuses fetched after it. Under the repository lock, refused in Rust as `push_branch` is for any of its branches; the
/// stack branch of a whole stack also while a member is parked, every member landed whole, its robot runs on it or it is
/// not built; an unapplied branch takes only the lock. `progress` receives how far the push is.
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
  operation(async {
    let (common_dir, space) = push_inputs(&git_executor, &repository_state, &patronus, &params.repository_path).await?;
    let actions = remote_actions(&patronus, space.as_ref(), &git_executor, &views);
    let pushed = actions
      .push_branches(common_dir, params, move |push_progress| send_progress(&progress, push_progress))
      .await;
    pushed.map_err(CommandError::from)
  })
  .await
}

/// What the push of the repository at `repository_path` needs of the app: the `$GIT_COMMON_DIR` the app keeps per
/// repository (where the lock lives; one `rev-parse` on the first call, in a blocking step), and its Space repository
/// (none off Space, or when it can't be read: no robot holds, logged), for `RemoteActions`
async fn push_inputs(
  git_executor: &GitCommandExecutor,
  repository_state: &RepositoryStateCache,
  patronus: &PatronusState,
  repository_path: &str,
) -> Result<(PathBuf, Option<SpaceRepository>), CommandError> {
  let (git, state, path) = (git_executor.clone(), repository_state.clone(), repository_path.to_string());
  let common_dir = run_blocking(move || state.git_common_dir(&git, &path)).await?;
  let space = patronus.space_repository(git_executor, repository_path).await.unwrap_or_else(|error| {
    warn!(error = ?error, "no robots for the checks of the push: the Space repository is unknown");
    None
  });
  Ok((common_dir, space))
}

fn send_progress(progress: &Channel<TransferProgress>, push_progress: TransferProgress) {
  if let Err(e) = progress.send(push_progress) {
    warn!(error = %e, "failed to report the push progress");
  }
}
