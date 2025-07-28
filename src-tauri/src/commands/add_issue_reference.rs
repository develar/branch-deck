use branch_sync::add_issue_reference::{AddIssueReferenceParams, AddIssueReferenceResult, add_issue_reference_to_commits_core};
use git_ops::git_command::GitCommandExecutor;
use tauri::State;
use tracing::instrument;

/// Adds an issue reference to commits in a branch that don't already have one.
/// Updates commit messages from "(branch-name) message" to "(branch-name) ISSUE-123 message"
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn add_issue_reference_to_commits(git_executor: State<'_, GitCommandExecutor>, params: AddIssueReferenceParams) -> Result<AddIssueReferenceResult, String> {
  add_issue_reference_to_commits_core(&git_executor, params).await
}
