use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::add_issue_reference::{AddIssueReferenceParams, AddIssueReferenceResult, add_issue_reference_to_commits_core};
use tauri::State;
use tracing::instrument;

/// Adds an issue reference to commits in a branch that don't already have one.
/// Updates commit messages from "(branch-name) message" to "(branch-name) ISSUE-123 message"
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn add_issue_reference_to_commits(git_executor: State<'_, GitCommandExecutor>, params: AddIssueReferenceParams) -> Result<AddIssueReferenceResult, String> {
  // Clone the executor since spawn_blocking requires 'static lifetime
  let git = (*git_executor).clone();
  tokio::task::spawn_blocking(move || add_issue_reference_to_commits_core(&git, params))
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}
