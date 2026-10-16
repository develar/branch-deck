use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use crate::commands::github::GitHubState;
use crate::commands::pull_requests::github_repository;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::{CommitFiles, CommitPatch, DiffSubject, PATCH_LIMITS, read_diff_files, read_patch};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use sync_core::review_fetch::{fetch_review_head as core_fetch_review_head, prune_review_refs as core_prune_review_refs};
use sync_core::review_requests::list_review_requests;
use sync_core::transfer_progress::TransferProgress;
use sync_types::pull_request::PullRequestAvailability;
use sync_types::review_request::{FetchReviewHeadParams, GetRangeParams, GetReviewRequestsParams, PruneReviewRefsParams, ReviewRange, ReviewRequestsResult};
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};

/// The latest review fetch: an older one still running is killed, as is the latest after `cancel_review_fetch`
#[derive(Default)]
pub struct ReviewFetchState {
  latest_request: Arc<AtomicU64>,
}

/// The pull requests of others that wait for the user's review (requested by name), one GraphQL search; when GitHub
/// can't answer, the availability says why. A known `github_repository` skips resolving it.
#[tauri::command]
#[specta::specta]
pub async fn get_review_requests(
  git_executor: State<'_, GitCommandExecutor>,
  github: State<'_, GitHubState>,
  params: GetReviewRequestsParams,
) -> Result<ReviewRequestsResult, CommandError> {
  let Some(repository) = github_repository(&git_executor, &params.repository_path, params.github_repository.as_deref()).await? else {
    return Ok(ReviewRequestsResult::unavailable(PullRequestAvailability::NotGitHub));
  };
  Ok(list_review_requests(&github.service, &params.repository_path, &repository).await)
}

/// Fetches the head (and, if missing, the base) of a pull request into `refs/branch-deck/review/<n>/…` unless both
/// commits are local, and returns the range to review; `progress` receives how far the fetch is. A newer call or
/// `cancel_review_fetch` kills the fetch.
#[tauri::command]
#[instrument(skip(state, git_executor, progress))]
#[specta::specta]
pub async fn fetch_review_head(
  state: State<'_, ReviewFetchState>,
  git_executor: State<'_, GitCommandExecutor>,
  params: FetchReviewHeadParams,
  progress: Channel<TransferProgress>,
) -> Result<ReviewRange, CommandError> {
  let latest_request = state.latest_request.clone();
  let request = latest_request.fetch_add(1, Ordering::SeqCst) + 1;
  let git = (*git_executor).clone();
  run_blocking(move || {
    core_fetch_review_head(
      &git,
      &params,
      &mut |fetch_progress| {
        if let Err(e) = progress.send(fetch_progress) {
          warn!(error = %e, "failed to report the review fetch progress");
        }
      },
      &|| latest_request.load(Ordering::SeqCst) != request,
    )
  })
  .await
}

/// Kills the review fetch in flight, if any
#[tauri::command]
#[specta::specta]
pub async fn cancel_review_fetch(state: State<'_, ReviewFetchState>) -> Result<(), CommandError> {
  state.latest_request.fetch_add(1, Ordering::SeqCst);
  Ok(())
}

/// Deletes the review refs of the pull requests not in `keep` (one CAS transaction)
#[tauri::command]
#[specta::specta]
pub async fn prune_review_refs(git_executor: State<'_, GitCommandExecutor>, params: PruneReviewRefsParams) -> Result<(), CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || core_prune_review_refs(&git, &params.repository_path, &params.keep)).await
}

/// The changed files of `base..head` with their line counts: one `diff-tree` of the two commits, not cached
#[tauri::command]
#[specta::specta]
pub async fn get_range_files(git_executor: State<'_, GitCommandExecutor>, params: GetRangeParams) -> Result<CommitFiles, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || read_diff_files(&git, &params.repository_path, &range_subject(params.base, params.head))).await
}

/// The patch of `base..head`: one `diff-tree -p` of the two commits, cut into files
#[tauri::command]
#[specta::specta]
pub async fn get_range_patch(git_executor: State<'_, GitCommandExecutor>, params: GetRangeParams) -> Result<CommitPatch, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || read_patch(&git, &params.repository_path, &range_subject(params.base, params.head), &PATCH_LIMITS)).await
}

fn range_subject(base: String, head: String) -> DiffSubject {
  DiffSubject::Range { base, head }
}
