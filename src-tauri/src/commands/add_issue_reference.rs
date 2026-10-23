use crate::lock_wait::{WaitChannel, Waits, run_waiting};
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::RewriteOptions;
use sync_core::add_issue_reference::{AddIssueReferenceParams, AddIssueReferenceResult, add_issue_reference_to_commits_core};
use tauri::State;
use tracing::instrument;

/// Adds an issue reference to commits in a branch that don't already have one.
/// Updates commit messages from "(branch-name) message" to "(branch-name) ISSUE-123 message"
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = %params.branch_name, commits = params.commits.len()))]
#[specta::specta]
pub async fn add_issue_reference_to_commits(
  git_executor: State<'_, GitCommandExecutor>,
  lock_waits: Waits<'_>,
  params: AddIssueReferenceParams,
  wait: WaitChannel,
) -> Result<AddIssueReferenceResult, ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |lock_wait| {
    add_issue_reference_to_commits_core(&git, params, &RewriteOptions::waiting(lock_wait))
  })
  .await
}
