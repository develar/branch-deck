use crate::commands::branch_prefix::PrefixInputs;
use crate::lock_wait::{WaitChannel, Waits, run_waiting};
use crate::operation::run_blocking;
use crate::repository_state::RepositoryStateCache;
use app_api::api_error::undo_error;
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use domain_types::rewrite::MoveCommitsResult;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::stack_operations::{
  RenameBranchParams, RestackBranchParams, UndoRegroupParams, rename_branch as rename_branch_core, restack_branch as restack_branch_core, undo_regroup as undo_regroup_core,
};
use tauri::State;
use tracing::instrument;
use trial_check::check::{StackIndependence, StackIndependenceParams, get_stack_independence as get_stack_independence_core};

/// Renames a branch, and its name in the prefixes of the branches stacked on it and in the issue groups, in one history
/// rewrite.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, lock_waits, wait))]
pub async fn rename_branch(
  git_executor: State<'_, GitCommandExecutor>,
  lock_waits: Waits<'_>,
  params: RenameBranchParams,
  wait: WaitChannel,
) -> Result<MoveCommitsResult, ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |lock_wait| rename_branch_core(&git, &params, lock_wait)).await
}

/// Stacks a branch on another one, or makes it a root (`parent: null`), in one history rewrite.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, lock_waits, wait))]
pub async fn restack_branch(
  git_executor: State<'_, GitCommandExecutor>,
  lock_waits: Waits<'_>,
  params: RestackBranchParams,
  wait: WaitChannel,
) -> Result<MoveCommitsResult, ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |lock_wait| restack_branch_core(&git, &params, lock_wait)).await
}

/// Whether a stacked branch builds on the baseline without its parent: a trial build of it as a root, objects only,
/// through the object cache of the repository.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, repository_state, settings, prefixes))]
pub async fn get_stack_independence(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: StackIndependenceParams,
) -> Result<StackIndependence, ApiError> {
  let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
  let repository_state = (*repository_state).clone();
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    let cache = repository_state.object_cache(&inputs.git, &params.repository_path);
    get_stack_independence_core(&inputs.git, &cache, &branch_prefix, &params)
  })
  .await
}

/// Moves back what a move, a rename or a restack wrote (its `undo_token`): HEAD and the issue groups in one ref
/// transaction with compare-and-swap.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, lock_waits, wait))]
pub async fn undo_regroup(git_executor: State<'_, GitCommandExecutor>, lock_waits: Waits<'_>, params: UndoRegroupParams, wait: WaitChannel) -> Result<(), ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |lock_wait| undo_regroup_core(&git, &params, lock_wait).map_err(undo_error)).await
}
