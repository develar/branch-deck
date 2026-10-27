use crate::commands::branch_prefix::{PrefixInputs, Prefixes, Settings, run_waiting_with_prefix};
use crate::lock_wait::{WaitChannel, Waits, run_waiting};
use app_api::api_error::undo_error;
use domain_types::error::ApiError;
use domain_types::rewrite::{ReapplyBranchResult, UnapplyBranchResult, UndoneReapply, UndoneUnapply};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::RewriteOptions;
use sync_core::reapply_branch::{ReapplyBranchParams, ReapplyUndoToken, UndoReapplyParams, reapply_branch_core, undo_reapply_core};
use sync_core::unapply_branch::{UnapplyBranchParams, UnapplyUndoToken, UndoUnapplyParams, unapply_branch_core, undo_unapply_core};
use tauri::State;
use tracing::instrument;

/// Unapplies a virtual branch of the repository, named with the prefix Rust resolves for it (`PrefixInputs`), the one a
/// sync's view of the repository has
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = %params.branch_name, original_commits = ?params.original_commit_ids.as_ref().map(Vec::len)))]
#[specta::specta]
pub async fn unapply_branch(
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  lock_waits: Waits<'_>,
  params: UnapplyBranchParams,
  wait: WaitChannel,
) -> Result<UnapplyBranchResult, ApiError> {
  let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
  let repository_path = params.repository_path.clone();
  run_waiting_with_prefix(inputs, &lock_waits, wait, &repository_path, move |git, branch_prefix, lock_wait| {
    unapply_branch_core(git, params, branch_prefix, &RewriteOptions::waiting(lock_wait))
  })
  .await
}

#[tauri::command]
#[instrument(skip(git_executor, lock_waits, wait))]
#[specta::specta]
pub async fn undo_unapply_branch(
  git_executor: State<'_, GitCommandExecutor>,
  lock_waits: Waits<'_>,
  params: UndoUnapplyParams,
  wait: WaitChannel,
) -> Result<UndoneUnapply, ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |lock_wait| {
    let token = UnapplyUndoToken::parse(&params.undo_token)
      .ok_or_else(|| anyhow::anyhow!("Not an undo token of an unapply: {}", params.undo_token))
      .map_err(undo_error)?;
    undo_unapply_core(&git, &params.repository_path, &token, lock_wait).map_err(undo_error)
  })
  .await
}

/// Reapplies an unapplied branch of the repository, named with the prefix Rust resolves for it (`PrefixInputs`)
#[tauri::command]
#[instrument(skip(git_executor, settings, prefixes, lock_waits, wait))]
#[specta::specta]
pub async fn reapply_branch(
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  lock_waits: Waits<'_>,
  params: ReapplyBranchParams,
  wait: WaitChannel,
) -> Result<ReapplyBranchResult, ApiError> {
  let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
  let repository_path = params.repository_path.clone();
  run_waiting_with_prefix(inputs, &lock_waits, wait, &repository_path, move |git, branch_prefix, lock_wait| {
    reapply_branch_core(git, &params, branch_prefix, &RewriteOptions::waiting(lock_wait))
  })
  .await
}

#[tauri::command]
#[instrument(skip(git_executor, lock_waits, wait))]
#[specta::specta]
pub async fn undo_reapply_branch(
  git_executor: State<'_, GitCommandExecutor>,
  lock_waits: Waits<'_>,
  params: UndoReapplyParams,
  wait: WaitChannel,
) -> Result<UndoneReapply, ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |lock_wait| {
    let token = ReapplyUndoToken::parse(&params.undo_token)
      .ok_or_else(|| anyhow::anyhow!("Not an undo token of a reapply: {}", params.undo_token))
      .map_err(undo_error)?;
    undo_reapply_core(&git, &params.repository_path, &token, lock_wait).map_err(undo_error)
  })
  .await
}
