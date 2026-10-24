use crate::commands::branch_prefix::PrefixInputs;
use crate::commands::patronus::{PatronusState, remote_actions};
use crate::commands::session::SessionsState;
use crate::lock_wait::{canceled_by, canceled_code, register};
use crate::operation::{operation, run_blocking};
use crate::repository_state::RepositoryStateCache;
use app_api::api_error::api_error;
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use domain_types::session::ViewAnswer;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::lock_wait::LockWaits;
use repository_model::session::Sessions;
use repository_model::views::RepositoryViews;
use std::path::PathBuf;
use sync_core::patronus::SpaceRepository;
use sync_core::push::{PushBranchParams, PushBranchesOutcome, PushBranchesParams, PushOutcome};
use sync_core::transfer_progress::TransferProgress;
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};

/// Pushes a branch to the remote, replacing the remote branch only as the user saw it, and answers its new remote
/// status; a remote that moved is fetched and answered instead. The session of the repository gets the outcome
/// (`Sessions::push`): the answer names the version of the view whose patch has it. Under the repository lock, and
/// refused in Rust as `bd push` refuses it (`push_guard`): a branch the last sync did not build, a parked branch or one
/// built on parked, one a merged Safe Push landed whole, one a running Safe Push holds (the robots as Rust reads them).
/// `progress` receives how far the push is, and whom it waits for while another holder has the repository lock (as long
/// as it lives); `cancel_lock_wait` of the channel's id ends that wait, with nothing pushed.
#[tauri::command]
#[instrument(skip(git_executor, repository_state, patronus, views, sessions, lock_waits, settings, prefixes, progress))]
#[specta::specta]
#[expect(clippy::too_many_arguments, reason = "the states of the app a Tauri command takes one by one")]
pub async fn push_branch(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  patronus: State<'_, PatronusState>,
  views: State<'_, RepositoryViews>,
  sessions: State<'_, SessionsState>,
  lock_waits: State<'_, LockWaits>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: PushBranchParams,
  progress: Channel<TransferProgress>,
) -> Result<ViewAnswer<PushOutcome>, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let host = PushHost::of(&repository_state, &patronus, &views, sessions.sessions(), &lock_waits, inputs);
    host.push(params.as_stack(), progress, PushOutcome::of_stack).await
  })
  .await
}

/// Pushes the branches of a stack to the remote in one atomic push, all of them or none, each replacing its remote
/// branch only as the user saw it, and answers their new remote statuses; a remote branch that moved is named, with the
/// statuses fetched after it. The session gets the outcome as from `push_branch`, an unapplied branch's too, so no
/// sync follows. Under the repository lock, refused in Rust as `push_branch` is for any of its branches; the
/// stack branch of a whole stack also while a member is parked, every member landed whole, its robot runs on it or it is
/// not built; an unapplied branch takes only the lock. `progress` receives how far the push is, and whom it waits for
/// while another holder has the lock, until `cancel_lock_wait` of its id.
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branches = params.branches.len()))]
#[specta::specta]
#[expect(clippy::too_many_arguments, reason = "the states of the app a Tauri command takes one by one")]
pub async fn push_branches(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  patronus: State<'_, PatronusState>,
  views: State<'_, RepositoryViews>,
  sessions: State<'_, SessionsState>,
  lock_waits: State<'_, LockWaits>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: PushBranchesParams,
  progress: Channel<TransferProgress>,
) -> Result<ViewAnswer<PushBranchesOutcome>, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let host = PushHost::of(&repository_state, &patronus, &views, sessions.sessions(), &lock_waits, inputs);
    host.push(params, progress, Ok).await
  })
  .await
}

/// The state of the app a push needs, with what it resolves the branch prefix of the repository with (and its git)
struct PushHost<'a> {
  repository_state: &'a RepositoryStateCache,
  patronus: &'a PatronusState,
  views: &'a RepositoryViews,
  sessions: &'a Sessions,
  lock_waits: &'a LockWaits,
  prefix_inputs: PrefixInputs,
}

impl<'a> PushHost<'a> {
  const fn of(
    repository_state: &'a RepositoryStateCache,
    patronus: &'a PatronusState,
    views: &'a RepositoryViews,
    sessions: &'a Sessions,
    lock_waits: &'a LockWaits,
    prefix_inputs: PrefixInputs,
  ) -> Self {
    Self {
      repository_state,
      patronus,
      views,
      sessions,
      lock_waits,
      prefix_inputs,
    }
  }

  /// The guarded push of `params` (`RemoteActions::push_branches`) under the prefix resolved for its repository, in the
  /// session of that key, its progress and its wait for another holder of the lock sent to `progress`, the wait
  /// registered by the channel's id (`cancel_lock_wait`); `outcome` makes the answer
  async fn push<T>(
    self,
    params: PushBranchesParams,
    progress: Channel<TransferProgress>,
    outcome: fn(PushBranchesOutcome) -> anyhow::Result<T>,
  ) -> Result<ViewAnswer<T>, ApiError> {
    let git_executor = self.prefix_inputs.git.clone();
    let branch_prefix = self.prefix_inputs.resolve(&params.repository_path).await?;
    let (common_dir, space) = push_inputs(&git_executor, self.repository_state, self.patronus, &params.repository_path).await?;
    let actions = remote_actions(self.patronus, space.as_ref(), &git_executor, self.views, &branch_prefix);
    let ticket = register(self.lock_waits, progress.id(), |_| {});
    let canceled = canceled_by(&ticket);
    let pushed = actions
      .push_branches(self.sessions, common_dir, params, move |push_progress| send_progress(&progress, push_progress), canceled)
      .await;
    canceled_code(pushed.and_then(|answer| answer.try_map(outcome)).map_err(api_error), &ticket)
  }
}

/// What the push of the repository at `repository_path` needs of the app: the `$GIT_COMMON_DIR` the app keeps per
/// repository (where the lock lives; one `rev-parse` on the first call, in a blocking step), and its Space repository
/// (none off Space, or when it can't be read: no robot holds, logged), for `RemoteActions`
async fn push_inputs(
  git_executor: &GitCommandExecutor,
  repository_state: &RepositoryStateCache,
  patronus: &PatronusState,
  repository_path: &str,
) -> Result<(PathBuf, Option<SpaceRepository>), ApiError> {
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
