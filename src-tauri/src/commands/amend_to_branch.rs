use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::amend_to_branch::{AmendCommandResult, AmendUncommittedToBranchParams, amend_uncommitted_to_branch_core};
use tauri::State;
use tracing::instrument;

/// Amend uncommitted changes to the original commit corresponding to a virtual branch tip.
/// This operation modifies the main branch history and requires a sync afterward to recreate virtual branches.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn amend_uncommitted_to_branch(git_executor: State<'_, GitCommandExecutor>, params: AmendUncommittedToBranchParams) -> Result<AmendCommandResult, String> {
  let git = (*git_executor).clone();

  tokio::task::spawn_blocking(move || amend_uncommitted_to_branch_core(&git, params))
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}
