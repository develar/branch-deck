use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::{CommitFiles, CommitPatch};
use sync_core::transfer_progress::TransferProgress;
use sync_types::review_request::{FetchReviewHeadParams, GetRangeParams, GetReviewRequestsParams, PruneReviewRefsParams, ReviewRange, ReviewRequestsResult};
use tauri::State;
use tauri::ipc::Channel;

// CONTRACT STUBS: the signatures are final (the TypeScript bindings are generated from them); the bodies are filled in
// by the backend work package.

/// The pull requests of others that wait for the user's review (requested by name), through `gh api graphql`; when `gh`
/// can't answer, the availability says why
#[tauri::command]
#[specta::specta]
pub async fn get_review_requests(git_executor: State<'_, GitCommandExecutor>, params: GetReviewRequestsParams) -> Result<ReviewRequestsResult, CommandError> {
  let _ = (git_executor, params);
  Err(CommandError::new("not implemented"))
}

/// Fetches the head (and, if missing, the base) of a pull request into `refs/branch-deck/review/<n>/…` unless both
/// commits are local, and returns the range to review; `progress` receives how far the fetch is. A newer call or
/// `cancel_review_fetch` kills the fetch.
#[tauri::command]
#[specta::specta]
pub async fn fetch_review_head(
  git_executor: State<'_, GitCommandExecutor>,
  params: FetchReviewHeadParams,
  progress: Channel<TransferProgress>,
) -> Result<ReviewRange, CommandError> {
  let _ = (git_executor, params, progress);
  Err(CommandError::new("not implemented"))
}

/// Kills the review fetch in flight, if any
#[tauri::command]
#[specta::specta]
pub async fn cancel_review_fetch() -> Result<(), CommandError> {
  Err(CommandError::new("not implemented"))
}

/// Deletes the review refs of the pull requests not in `keep` (one CAS transaction)
#[tauri::command]
#[specta::specta]
pub async fn prune_review_refs(git_executor: State<'_, GitCommandExecutor>, params: PruneReviewRefsParams) -> Result<(), CommandError> {
  let _ = (git_executor, params);
  Err(CommandError::new("not implemented"))
}

/// The changed files of `base..head` with their line counts: one `diff-tree` of the two commits, not cached
#[tauri::command]
#[specta::specta]
pub async fn get_range_files(git_executor: State<'_, GitCommandExecutor>, params: GetRangeParams) -> Result<CommitFiles, CommandError> {
  let _ = (git_executor, params);
  Err(CommandError::new("not implemented"))
}

/// The patch of `base..head`: one `diff-tree -p` of the two commits, cut into files
#[tauri::command]
#[specta::specta]
pub async fn get_range_patch(git_executor: State<'_, GitCommandExecutor>, params: GetRangeParams) -> Result<CommitPatch, CommandError> {
  let _ = (git_executor, params);
  Err(CommandError::new("not implemented"))
}
