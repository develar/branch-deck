use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, RewriteOptions};
use sync_core::reapply_branch::{ReapplyBranchParams, ReapplyBranchResult, ReapplyUndoToken, UndoReapplyParams, UndoneReapply, reapply_branch_core, undo_reapply_core};
use sync_core::unapply_branch::{UnapplyBranchParams, UnapplyBranchResult, UnapplyUndoToken, UndoUnapplyParams, UndoneUnapply, unapply_branch_core, undo_unapply_core};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn unapply_branch(git_executor: State<'_, GitCommandExecutor>, params: UnapplyBranchParams) -> Result<UnapplyBranchResult, CommandError> {
  let git = (*git_executor).clone();

  run_blocking(move || unapply_branch_core(&git, params, &RewriteOptions::default())).await
}

#[tauri::command]
#[specta::specta]
pub async fn undo_unapply_branch(git_executor: State<'_, GitCommandExecutor>, params: UndoUnapplyParams) -> Result<UndoneUnapply, CommandError> {
  let git = (*git_executor).clone();

  run_blocking(move || {
    let token = UnapplyUndoToken::parse(&params.undo_token).ok_or_else(|| anyhow::anyhow!("Not an undo token of an unapply: {}", params.undo_token))?;
    undo_unapply_core(&git, &params.repository_path, &token, DEFAULT_LOCK_TIMEOUT)
  })
  .await
}

#[tauri::command]
#[specta::specta]
pub async fn reapply_branch(git_executor: State<'_, GitCommandExecutor>, params: ReapplyBranchParams) -> Result<ReapplyBranchResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || reapply_branch_core(&git, &params, &RewriteOptions::default())).await
}

#[tauri::command]
#[specta::specta]
pub async fn undo_reapply_branch(git_executor: State<'_, GitCommandExecutor>, params: UndoReapplyParams) -> Result<UndoneReapply, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || {
    let token = ReapplyUndoToken::parse(&params.undo_token).ok_or_else(|| anyhow::anyhow!("Not an undo token of a reapply: {}", params.undo_token))?;
    undo_reapply_core(&git, &params.repository_path, &token, DEFAULT_LOCK_TIMEOUT)
  })
  .await
}
