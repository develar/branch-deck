use crate::command_error::CommandError;
use crate::commands::github::GitHubState;
use crate::commands::pull_requests::github_repository;
use crate::operation::{operation, run_blocking};
use crate::repository_state::RepositoryStateCache;
use crate::window_requests::WindowRequests;
use domain_types::pull_request::PullRequestAvailability;
use domain_types::review_request::{FetchReviewHeadParams, GetRangeParams, GetReviewRequestsParams, PruneReviewRefsParams, ReviewRange, ReviewRequestsResult};
use file_diffs::range_files::RangeFilesCache;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::{CommitFiles, DiffSubject};
use remote_facts::review_requests::list_review_requests;
use std::sync::Arc;
use sync_core::review_fetch::{fetch_review_head as core_fetch_review_head, prune_review_refs as core_prune_review_refs};
use sync_core::transfer_progress::TransferProgress;
use tauri::ipc::Channel;
use tauri::{State, Webview};
use tracing::{instrument, warn};

/// The latest review fetch of each window: a newer call of the window, `cancel_review_fetch` of it, or its close kills
/// the one in flight. The review window fetches the pull request it shows; windows fetch apart.
#[derive(Default)]
pub struct ReviewFetchState(pub WindowRequests);

/// The pull requests of others that wait for the user's review (requested by name), one GraphQL search; when GitHub
/// can't answer, the availability says why. A known `github_repository` skips resolving it.
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github))]
#[specta::specta]
pub async fn get_review_requests(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: GetReviewRequestsParams,
) -> Result<ReviewRequestsResult, CommandError> {
  operation(async move {
    let Some(repository) = github_repository(&git_executor, &repository_state, &params.repository_path, params.github_repository.as_deref()).await? else {
      return Ok(ReviewRequestsResult::unavailable(PullRequestAvailability::NotGitHub));
    };
    Ok(list_review_requests(&github.service, &params.repository_path, &repository).await)
  })
  .await
}

/// Fetches the head (and, if missing, the base) of a pull request into `refs/branch-deck/review/<n>/…` unless both
/// commits are local, and returns the range to review; `progress` receives how far the fetch is. A newer call of the
/// same window, `cancel_review_fetch` of it, or its close kills the fetch.
#[tauri::command]
#[instrument(skip(webview, state, git_executor, progress))]
#[specta::specta]
pub async fn fetch_review_head(
  webview: Webview,
  state: State<'_, ReviewFetchState>,
  git_executor: State<'_, GitCommandExecutor>,
  params: FetchReviewHeadParams,
  progress: Channel<TransferProgress>,
) -> Result<ReviewRange, CommandError> {
  let request = state.0.begin(webview.label());
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
      &|| request.is_superseded(),
    )
  })
  .await
}

/// Kills the review fetch of the calling window in flight, if any
#[tauri::command]
#[instrument(skip(webview, state))]
#[specta::specta]
pub async fn cancel_review_fetch(webview: Webview, state: State<'_, ReviewFetchState>) -> Result<(), CommandError> {
  operation(async move {
    state.0.cancel(webview.label());
    Ok(())
  })
  .await
}

/// Deletes the review refs of the pull requests not in `keep` (one CAS transaction)
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, keep = params.keep.len()))]
#[specta::specta]
pub async fn prune_review_refs(git_executor: State<'_, GitCommandExecutor>, params: PruneReviewRefsParams) -> Result<(), CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || core_prune_review_refs(&git, &params.repository_path, &params.keep)).await
}

/// The changed files of `base..head` with their line counts, from one `diff-tree` of the two commits
/// (`RangeFilesCache::read`), kept in memory
#[tauri::command]
#[instrument(skip(git_executor, range_files))]
#[specta::specta]
pub async fn get_range_files(
  git_executor: State<'_, GitCommandExecutor>,
  range_files: State<'_, RangeFilesCache>,
  params: GetRangeParams,
) -> Result<Arc<CommitFiles>, CommandError> {
  let git = (*git_executor).clone();
  let range_files = (*range_files).clone();
  run_blocking(move || range_files.read(&git, &params.repository_path, &range_subject(params.base, params.head))).await
}

const fn range_subject(base: String, head: String) -> DiffSubject {
  DiffSubject::Range { base, head }
}
