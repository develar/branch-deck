use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::ownership::{FileOwner, GetFileOwnersParams, read_file_owners};
use sync_core::branch_review::{BranchReview, GetBranchReviewParams, get_branch_review as core_get_branch_review};
use tauri::State;

/// The parts of a branch for its review window: each branch of its stack from the root, with its base, head and
/// commits, from one `for-each-ref` and one `git log`
#[tauri::command]
#[specta::specta]
pub async fn get_branch_review(git_executor: State<'_, GitCommandExecutor>, params: GetBranchReviewParams) -> Result<BranchReview, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || core_get_branch_review(&git, &params)).await
}

/// The code owners of paths by the `OWNERSHIP` files of the baseline, from one `cat-file --batch`; a path without an
/// owner is left out
#[tauri::command]
#[specta::specta]
pub async fn get_file_owners(git_executor: State<'_, GitCommandExecutor>, params: GetFileOwnersParams) -> Result<Vec<FileOwner>, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || read_file_owners(&git, &params.repository_path, &params.baseline_branch, &params.paths)).await
}
