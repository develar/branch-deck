use branch_sync::{AddIssueReferenceParams, AddIssueReferenceResult, add_issue_reference_to_commits_core};
use git_ops::git_command::GitCommandExecutor;
use tauri::State;
use tracing::instrument;

/// Adds an issue reference to commits in a branch that don't already have one.
/// Updates commit messages from "(branch-name) message" to "(branch-name) ISSUE-123 message"
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn add_issue_reference_to_commits(
  git_executor: State<'_, GitCommandExecutor>,
  repository_path: String,
  branch_name: String,
  commit_ids: Vec<String>,
  issue_reference: String,
) -> Result<AddIssueReferenceResult, String> {
  let params = AddIssueReferenceParams {
    repository_path,
    branch_name,
    commit_ids,
    issue_reference,
  };
  add_issue_reference_to_commits_core(&git_executor, params).await
}
