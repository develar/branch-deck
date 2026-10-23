use crate::lock_wait::{WaitChannel, Waits, run_waiting};
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::RewriteOptions;
use sync_core::amend_to_branch::{AmendCommandResult, AmendUncommittedToBranchParams, amend_uncommitted_to_branch_core};
use tauri::State;
use tracing::instrument;

/// Amend uncommitted changes to the original commit corresponding to a virtual branch tip.
/// This operation modifies the main branch history and requires a sync afterward to recreate virtual branches.
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = %params.branch_name, files = params.files.len()))]
#[specta::specta]
pub async fn amend_uncommitted_to_branch(
  git_executor: State<'_, GitCommandExecutor>,
  lock_waits: Waits<'_>,
  params: AmendUncommittedToBranchParams,
  wait: WaitChannel,
) -> Result<AmendCommandResult, ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |lock_wait| {
    amend_uncommitted_to_branch_core(&git, params, &RewriteOptions::waiting(lock_wait))
  })
  .await
}
