use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::add_issue_reference::{AddIssueReferenceParams, AddIssueReferenceResult, add_issue_reference_to_commits_core};
use sync_core::repository_lock::RewriteOptions;
use tauri::State;
use tracing::instrument;

/// Adds an issue reference to commits in a branch that don't already have one.
/// Updates commit messages from "(branch-name) message" to "(branch-name) ISSUE-123 message"
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn add_issue_reference_to_commits(git_executor: State<'_, GitCommandExecutor>, params: AddIssueReferenceParams) -> Result<AddIssueReferenceResult, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || add_issue_reference_to_commits_core(&git, params, &RewriteOptions::default())).await
}
