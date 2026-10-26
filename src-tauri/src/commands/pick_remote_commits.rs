use crate::commands::branch_prefix::{PrefixInputs, Prefixes, Settings, run_waiting_with_prefix};
use crate::lock_wait::{WaitChannel, Waits, run_waiting};
use crate::repository_state::RepositoryStateCache;
use app_api::api_error::undo_error;
use domain_types::error::ApiError;
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, RewriteOptions};
use tauri::State;
use tracing::instrument;
use update_core::pick::{PickRemoteCommitsParams, PickRemoteCommitsResult, UndoPickRemoteCommitsParams, pick_remote_commits_core, undo_pick_remote_commits_core};

/// Pick Remote Commits: fetches the remote branch of a virtual branch someone else pushed to and picks the commits a
/// push would drop onto the checked out branch, with the prefix of the branch (`bd pick --remote`); the branch is named
/// with the prefix Rust resolves for the repository (`PrefixInputs`)
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = %params.branch_name))]
#[specta::specta]
#[expect(
  clippy::too_many_arguments,
  reason = "the states of the app a pick reads (git, the object cache, the diffs, the prefix, the lock waits) and its params"
)]
pub async fn pick_remote_commits(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  file_diffs: State<'_, FileDiffs>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  lock_waits: Waits<'_>,
  params: PickRemoteCommitsParams,
  wait: WaitChannel,
) -> Result<PickRemoteCommitsResult, ApiError> {
  let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
  let (repository_state, file_diffs) = ((*repository_state).clone(), (*file_diffs).clone());
  let repository_path = params.repository_path.clone();
  run_waiting_with_prefix(inputs, &lock_waits, wait, &repository_path, move |git, branch_prefix, lock_wait| {
    let cache = repository_state.object_cache(git, &params.repository_path);
    pick_remote_commits_core(git, &params, branch_prefix, &RewriteOptions::waiting(lock_wait), &cache, &file_diffs)
  })
  .await
}

/// The undo of Pick Remote Commits: the branch and its files back from the copies to the head before the pick
#[tauri::command]
#[instrument(skip(git_executor, lock_waits, wait))]
#[specta::specta]
pub async fn undo_pick_remote_commits(
  git_executor: State<'_, GitCommandExecutor>,
  lock_waits: Waits<'_>,
  params: UndoPickRemoteCommitsParams,
  wait: WaitChannel,
) -> Result<(), ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |_lock_wait| {
    undo_pick_remote_commits_core(&git, &params, DEFAULT_LOCK_TIMEOUT).map_err(undo_error)
  })
  .await
}
