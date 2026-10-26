use crate::commands::branch_prefix::PrefixInputs;
use crate::commands::patronus::PatronusState;
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
use history_rewrite::repository_lock::DEFAULT_LOCK_TIMEOUT;
use operations::events::NoEvents;
use operations::push::PushHost;
use repository_model::session::SessionKey;
use std::path::PathBuf;
use std::sync::Arc;
use sync_core::patronus::SpaceRepository;
use sync_core::push::{PushBranchesOutcome, PushBranchesParams};
use sync_core::transfer_progress::TransferProgress;
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};

/// Pushes the branches of a stack to the remote in one atomic push, all of them or none, each replacing its remote
/// branch only as the user saw it, and answers their new remote statuses; a remote branch that moved is named, with the
/// statuses fetched after it. An operation of the session of the repository (`operations::push`), which gets the
/// outcome, so no sync follows: the answer names the version of the view whose patch has it. Decided under the
/// repository lock and refused in Rust (`push_guard`) for a branch the last sync did not build, a parked branch or one
/// built on parked, one a merged Safe Push landed whole, one a running Safe Push holds (the robots as Rust reads them);
/// the stack branch of a whole stack also while a member is parked, every member landed whole, its robot runs on it or
/// it is not built. `progress` receives how far the push is, and whom it waits for while another holder has the lock,
/// until `cancel_lock_wait` of its id. A branch alone and an unapplied branch are pushed by `execute` of the intent
/// `Push` (`operations::push::execute_push`); the pushes of a stack move there in s9-13.
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branches = params.branches.len()))]
#[specta::specta]
#[expect(clippy::too_many_arguments, reason = "the states of the app a Tauri command takes one by one")]
pub async fn push_branches(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  patronus: State<'_, PatronusState>,
  sessions: State<'_, SessionsState>,
  lock_waits: State<'_, LockWaits>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: PushBranchesParams,
  progress: Channel<TransferProgress>,
) -> Result<ViewAnswer<PushBranchesOutcome>, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let states = PushStates::of(&repository_state, &patronus, &sessions, &lock_waits, inputs);
    states.push(params, progress).await
  })
  .await
}

/// The state of the app a push needs, with what it resolves the branch prefix of the repository with (and its git)
struct PushStates<'a> {
  repository_state: &'a RepositoryStateCache,
  patronus: &'a PatronusState,
  sessions: &'a SessionsState,
  lock_waits: &'a LockWaits,
  prefix_inputs: PrefixInputs,
}

impl<'a> PushStates<'a> {
  const fn of(
    repository_state: &'a RepositoryStateCache,
    patronus: &'a PatronusState,
    sessions: &'a SessionsState,
    lock_waits: &'a LockWaits,
    prefix_inputs: PrefixInputs,
  ) -> Self {
    Self {
      repository_state,
      patronus,
      sessions,
      lock_waits,
      prefix_inputs,
    }
  }

  /// The push of `params` (`operations::push::PushHost::push_branches`) under the prefix resolved for its repository, in
  /// the session of that key, its progress and its wait for another holder of the lock sent to `progress`, the wait
  /// registered by the channel's id (`cancel_lock_wait`)
  async fn push(self, params: PushBranchesParams, progress: Channel<TransferProgress>) -> Result<ViewAnswer<PushBranchesOutcome>, ApiError> {
    let git_executor = self.prefix_inputs.git.clone();
    let branch_prefix = self.prefix_inputs.resolve(&params.repository_path).await?;
    let (git_common_dir, space) = push_inputs(&git_executor, self.repository_state, self.patronus, &params.repository_path).await?;
    let host = PushHost {
      git: git_executor,
      git_common_dir,
      registry: self.sessions.registry().clone(),
      key: SessionKey {
        worktree: params.repository_path.clone(),
        prefix: branch_prefix,
      },
      patronus: space.map(|space| (Arc::clone(&self.patronus.service), space.name)),
      events: Arc::new(NoEvents),
      unknown_holder_timeout: DEFAULT_LOCK_TIMEOUT,
    };
    let ticket = register(self.lock_waits, progress.id(), |_| {});
    let canceled = canceled_by(&ticket);
    let pushed = host.push_branches(params, move |push_progress| send_progress(&progress, push_progress), canceled).await;
    canceled_code(pushed.map_err(api_error), &ticket)
  }
}

/// What the push of the repository at `repository_path` needs of the app: the `$GIT_COMMON_DIR` the app keeps per
/// repository (where the lock lives; one `rev-parse` on the first call, in a blocking step), and its Space repository
/// (none off Space, or when it can't be read: no robot holds, logged), for the robots of the checks
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
