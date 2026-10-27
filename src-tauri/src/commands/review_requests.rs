use crate::commands::branch_prefix::{PrefixInputs, Prefixes, Settings};
use crate::commands::session::{AppServices, SessionsState, session_key};
use crate::operation::{operation, run_blocking};
use crate::window_requests::WindowRequests;
use domain_types::error::ApiError;
use domain_types::review_request::{
  FetchReviewHeadParams, GetRangeParams, LinkPullRequestParams, LinkedPullRequestAnswer, PruneReviewRefsParams, ReviewRange, UnlinkPullRequestParams,
};
use file_diffs::range_files::RangeFilesCache;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::{CommitFiles, DiffSubject};
use remote_facts::reviews::{link_pull_request as core_link_pull_request, unlink_pull_request as core_unlink_pull_request};
use std::sync::Arc;
use sync_core::review_fetch::{fetch_review_head as core_fetch_review_head, prune_review_refs as core_prune_review_refs};
use sync_core::transfer_progress::TransferProgress;
use sync_utils::timestamp::unix_now;
use tauri::ipc::Channel;
use tauri::{AppHandle, State, Webview};
use tracing::{instrument, warn};

/// The latest review fetch of each window: a newer call of the window, `cancel_review_fetch` of it, or its close kills
/// the one in flight. The review window fetches the pull request it shows; windows fetch apart.
#[derive(Default)]
pub struct ReviewFetchState(pub WindowRequests);

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
) -> Result<ReviewRange, ApiError> {
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
pub async fn cancel_review_fetch(webview: Webview, state: State<'_, ReviewFetchState>) -> Result<(), ApiError> {
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
pub async fn prune_review_refs(git_executor: State<'_, GitCommandExecutor>, params: PruneReviewRefsParams) -> Result<(), ApiError> {
  let git = (*git_executor).clone();
  run_blocking(move || core_prune_review_refs(&git, &params.repository_path, &params.keep)).await
}

/// The changed files of `base..head` with their line counts, from one `diff-tree` of the two commits
/// (`RangeFilesCache::read`), kept in memory
#[tauri::command]
#[instrument(skip(git_executor, range_files))]
#[specta::specta]
pub async fn get_range_files(git_executor: State<'_, GitCommandExecutor>, range_files: State<'_, RangeFilesCache>, params: GetRangeParams) -> Result<Arc<CommitFiles>, ApiError> {
  let git = (*git_executor).clone();
  let range_files = (*range_files).clone();
  run_blocking(move || range_files.read(&git, &params.repository_path, &range_subject(params.base, params.head))).await
}

const fn range_subject(base: String, head: String) -> DiffSubject {
  DiffSubject::Range { base, head }
}

/// Adds a pull request to Reviews by its link, `#number` or number (`remote_facts::reviews::link_pull_request`): GitHub
/// is asked for it, an open one is kept for the repository, and the session reads Reviews again with it. Answers the
/// pull request, which the page opens in the review window
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path))]
#[specta::specta]
pub async fn link_pull_request(
  app: AppHandle,
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  state: State<'_, SessionsState>,
  params: LinkPullRequestParams,
) -> Result<LinkedPullRequestAnswer, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let key = session_key(inputs, &params.repository_path).await?;
    let (entry, kept) = core_link_pull_request(&AppServices { app }, state.sessions(), &key, &params.input, unix_now()).await?;
    Ok(LinkedPullRequestAnswer { entry, kept })
  })
  .await
}

/// Removes a pull request the user added by its link from Reviews, and the session reads Reviews again
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, number = params.number))]
#[specta::specta]
pub async fn unlink_pull_request(
  app: AppHandle,
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  state: State<'_, SessionsState>,
  params: UnlinkPullRequestParams,
) -> Result<(), ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    core_unlink_pull_request(&AppServices { app }, state.sessions(), &session_key(inputs, &params.repository_path).await?, params.number).await
  })
  .await
}
