use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use crate::commands::commit_files::read_patch;
use crate::commands::github::GitHubState;
use crate::commands::pull_requests::github_repository;
use crate::patch_cache::PatchCache;
use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::{CommitFiles, CommitPatch, DiffSubject, read_diff_files};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use sync_core::review_fetch::{fetch_review_head as core_fetch_review_head, prune_review_refs as core_prune_review_refs};
use sync_core::review_requests::list_review_requests;
use sync_core::transfer_progress::TransferProgress;
use sync_types::pull_request::PullRequestAvailability;
use sync_types::review_request::{FetchReviewHeadParams, GetRangeParams, GetReviewRequestsParams, PruneReviewRefsParams, ReviewRange, ReviewRequestsResult};
use tauri::ipc::Channel;
use tauri::{State, Webview};
use tracing::{instrument, warn};

/// The latest review fetch of each window (by webview label): an older one of the same window still running is killed,
/// as is the latest after `cancel_review_fetch` or when its window goes. The review window fetches the pull request it
/// shows; windows fetch apart, one never kills the fetch of another.
#[derive(Default)]
pub struct ReviewFetchState {
  latest_requests: Mutex<HashMap<String, Arc<AtomicU64>>>,
}

impl ReviewFetchState {
  fn counter(&self, label: &str) -> Arc<AtomicU64> {
    let mut latest = self.latest_requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    Arc::clone(latest.entry(label.to_string()).or_default())
  }

  /// Starts a fetch of the window: the counter and the number of this fetch; a newer number means it is killed
  pub fn begin(&self, label: &str) -> (Arc<AtomicU64>, u64) {
    let counter = self.counter(label);
    let request = counter.fetch_add(1, Ordering::SeqCst) + 1;
    (counter, request)
  }

  /// Kills the fetch of the window in flight, if any
  pub fn cancel(&self, label: &str) {
    self.counter(label).fetch_add(1, Ordering::SeqCst);
  }

  /// The window went: its fetch is killed and forgotten
  pub fn forget(&self, label: &str) {
    let removed = self.latest_requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(label);
    if let Some(counter) = removed {
      counter.fetch_add(1, Ordering::SeqCst);
    }
  }
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
  let (latest_request, request) = state.begin(webview.label());
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

/// Kills the review fetch of the calling window in flight, if any
#[tauri::command]
#[specta::specta]
pub async fn cancel_review_fetch(webview: Webview, state: State<'_, ReviewFetchState>) -> Result<(), CommandError> {
  state.cancel(webview.label());
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

/// The patch of `base..head`: from the memory cache, the object cache, or one `diff-tree -p` of the two commits, cut
/// into files
#[tauri::command]
#[specta::specta]
pub async fn get_range_patch(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  patch_cache: State<'_, PatchCache>,
  params: GetRangeParams,
) -> Result<Arc<CommitPatch>, CommandError> {
  read_patch(
    &git_executor,
    &repository_state,
    &patch_cache,
    params.repository_path,
    range_subject(params.base, params.head),
  )
  .await
}

const fn range_subject(base: String, head: String) -> DiffSubject {
  DiffSubject::Range { base, head }
}

#[cfg(test)]
mod tests {
  use super::ReviewFetchState;
  use std::sync::atomic::Ordering;

  fn is_killed(counter: &std::sync::atomic::AtomicU64, request: u64) -> bool {
    counter.load(Ordering::SeqCst) != request
  }

  #[test]
  fn a_window_kills_only_its_own_fetch() {
    let state = ReviewFetchState::default();
    let (main, main_request) = state.begin("main");
    let (window, window_request) = state.begin("pull-request-review");

    // a newer fetch of the window, then a cancel of main: each kills only its own window's fetch
    let (_, newer_request) = state.begin("pull-request-review");
    assert!(is_killed(&window, window_request));
    assert!(!is_killed(&main, main_request));
    state.cancel("main");
    assert!(is_killed(&main, main_request));
    assert!(!is_killed(&window, newer_request));
  }

  #[test]
  fn a_closed_window_kills_its_fetch() {
    let state = ReviewFetchState::default();
    let (window, request) = state.begin("pull-request-review");
    state.forget("pull-request-review");
    assert!(is_killed(&window, request));
    // a window opened again under the label starts afresh
    let (again, again_request) = state.begin("pull-request-review");
    assert!(!is_killed(&again, again_request));
  }
}
