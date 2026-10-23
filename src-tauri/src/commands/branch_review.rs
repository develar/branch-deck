use crate::operation::{operation, run_blocking};
use domain_types::error::ApiError;
use file_diffs::range_files::RangeFilesCache;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::DiffSubject;
use git_ops::ownership::{FileOwner, GetFileOwnersParams, read_file_owners};
use serde::Deserialize;
use specta::Type;
use std::sync::Arc;
use sync_core::branch_review::{BranchReview, BranchReviewMemo, GetBranchReviewParams};
use sync_utils::span::in_current_span;
use tauri::State;
use tracing::{debug, instrument};

/// The part of a stack a review of a branch shows first
#[derive(Debug, Clone, Copy, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum BranchReviewPart {
  /// The branch on its parent
  Own,
  /// The stack from the baseline
  All,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PrefetchBranchReviewParams {
  pub review: GetBranchReviewParams,
  pub part: BranchReviewPart,
}

/// The parts of a branch for its review window: each branch of its stack from the root, with its base, head and
/// commits, from one `for-each-ref` and one `git log`; the `log` only for tips and a baseline the memo hasn't seen
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, baseline_branch = %params.baseline_branch, chain = params.chain.len(), whole_stack = params.whole_stack))]
#[specta::specta]
pub async fn get_branch_review(
  git_executor: State<'_, GitCommandExecutor>,
  memo: State<'_, BranchReviewMemo>,
  params: GetBranchReviewParams,
) -> Result<Arc<BranchReview>, ApiError> {
  let git = (*git_executor).clone();
  let memo = memo.inner().clone();
  run_blocking(move || memo.get(&git, &params)).await
}

/// The code owners of paths by the `OWNERSHIP` files of the baseline, from one `cat-file --batch`; a path without an
/// owner is left out
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, paths = params.paths.len()))]
#[specta::specta]
pub async fn get_file_owners(git_executor: State<'_, GitCommandExecutor>, params: GetFileOwnersParams) -> Result<Vec<FileOwner>, ApiError> {
  let git = (*git_executor).clone();
  run_blocking(move || read_file_owners(&git, &params.repository_path, &params.baseline_branch, &params.paths)).await
}

/// Reads in the background what the review window of a branch asks for first: its parts (`get_branch_review`) and the
/// files of `part` (one `diff-tree`, `RangeFilesCache::read`), so the window finds them in the caches. Answers at once;
/// a failure is the window's to tell
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.review.repository_path, chain = params.review.chain.len(), part = ?params.part))]
#[specta::specta]
pub async fn prefetch_branch_review(
  git_executor: State<'_, GitCommandExecutor>,
  memo: State<'_, BranchReviewMemo>,
  range_files: State<'_, RangeFilesCache>,
  params: PrefetchBranchReviewParams,
) -> Result<(), ApiError> {
  operation(async move {
    let git = (*git_executor).clone();
    let memo = memo.inner().clone();
    let range_files = (*range_files).clone();
    // in the background: the window tells a failure
    tokio::task::spawn_blocking(in_current_span(move || {
      let review = match memo.get(&git, &params.review) {
        Ok(review) => review,
        Err(e) => {
          debug!(error = %e, "cannot prefetch the review of a branch");
          return;
        }
      };
      let (Some(first), Some(last)) = (review.segments.first(), review.segments.last()) else {
        return;
      };
      let base = match params.part {
        BranchReviewPart::Own => &last.base,
        BranchReviewPart::All => &first.base,
      };
      let subject = DiffSubject::Range {
        base: base.clone(),
        head: last.head.clone(),
      };
      if let Err(e) = range_files.read(&git, &params.review.repository_path, &subject) {
        debug!(error = %e, "cannot prefetch the changes of a branch");
      }
    }));
    Ok(())
  })
  .await
}
