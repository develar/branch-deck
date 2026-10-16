use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::check::StackIndependence;
use sync_core::move_commits::MoveCommitsResult;
use sync_core::stack_operations::{
  RenameBranchParams, RestackBranchParams, StackIndependenceParams, UndoRegroupParams, get_stack_independence as get_stack_independence_core, rename_branch as rename_branch_core,
  restack_branch as restack_branch_core, undo_regroup as undo_regroup_core,
};
use tauri::State;
use tracing::instrument;

/// Renames a branch, and its name in the prefixes of the branches stacked on it and in the issue groups, in one history
/// rewrite.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn rename_branch(git_executor: State<'_, GitCommandExecutor>, params: RenameBranchParams) -> Result<MoveCommitsResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || rename_branch_core(&git, &params)).await
}

/// Stacks a branch on another one, or makes it a root (`parent: null`), in one history rewrite.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn restack_branch(git_executor: State<'_, GitCommandExecutor>, params: RestackBranchParams) -> Result<MoveCommitsResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || restack_branch_core(&git, &params)).await
}

/// Whether a stacked branch builds on the baseline without its parent: a trial build of it as a root, objects only,
/// through the object cache of the repository.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, repository_state))]
pub async fn get_stack_independence(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  params: StackIndependenceParams,
) -> Result<StackIndependence, CommandError> {
  let git = (*git_executor).clone();
  let repository_state = (*repository_state).clone();
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &params.repository_path);
    get_stack_independence_core(&git, &cache, &params)
  })
  .await
}

/// Moves back what a move, a rename or a restack wrote (its `undo_token`): HEAD and the issue groups in one ref
/// transaction with compare-and-swap.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn undo_regroup(git_executor: State<'_, GitCommandExecutor>, params: UndoRegroupParams) -> Result<(), CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || undo_regroup_core(&git, &params)).await
}
