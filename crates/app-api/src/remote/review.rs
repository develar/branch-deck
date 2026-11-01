//! The review window (spec/review/review-window.spec.md): what it shows of its target, through the memo of the app
//! (`RemoteMemory::reviews`), and its read ahead while the window opens

use crate::context::AppContext;
use crate::operation::{operation, run_blocking};
use domain_types::error::ApiError;
use domain_types::review::{GetReviewParams, TargetReview};
use git_ops::commit_files::DiffSubject;
use serde::Deserialize;
use specta::Type;
use std::sync::Arc;
use sync_utils::span::in_current_span;
use tracing::{debug, instrument};

/// The part of a range a review shows first
#[derive(Debug, Clone, Copy, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ReviewPart {
  /// The last part: a branch on its parent, a whole stack, a kept branch
  Own,
  /// Every part: a stack from the baseline
  All,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PrefetchReviewParams {
  pub review: GetReviewParams,
  pub part: ReviewPart,
}

/// The parts of the target from the memo, its `log` only for tips and a baseline it hasn't seen (`ReviewMemo`)
#[instrument(skip_all, fields(repository_path = %params.repository_path, baseline_branch = %params.baseline_branch, target = ?params.target))]
pub async fn get_review(ctx: &AppContext, params: GetReviewParams) -> Result<Arc<TargetReview>, ApiError> {
  let (inputs, memo) = (ctx.prefix_inputs(), ctx.remote.reviews.clone());
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    memo.get(&inputs.git, &branch_prefix, &params)
  })
  .await
}

/// The parts of the target into the memo and the files of `part` into the memory of the ranges
/// (`RangeFilesCache::read`), in a blocking task that outlives the command: it answers at once, and a failure is the
/// window's to tell when it reads them itself. A list of commits has no range: its files are those of its first commit,
/// which the window reads itself
#[instrument(skip_all, fields(repository_path = %params.review.repository_path, target = ?params.review.target, part = ?params.part))]
pub async fn prefetch_review(ctx: &AppContext, params: PrefetchReviewParams) -> Result<(), ApiError> {
  operation(async move {
    let (inputs, memo, range_files) = (ctx.prefix_inputs(), ctx.remote.reviews.clone(), ctx.file_diffs.range_files.clone());
    tokio::task::spawn_blocking(in_current_span(move || {
      let review = match inputs
        .branch_prefix(&params.review.repository_path)
        .and_then(|branch_prefix| memo.get(&inputs.git, &branch_prefix, &params.review))
      {
        Ok(review) => review,
        Err(e) => {
          debug!(error = %e, "cannot prefetch the review");
          return;
        }
      };
      let segments = review.segments();
      let (Some(first), Some(last)) = (segments.first(), segments.last()) else {
        return;
      };
      let base = match params.part {
        ReviewPart::Own => &last.base,
        ReviewPart::All => &first.base,
      };
      let subject = DiffSubject::Range {
        base: base.clone(),
        head: last.head.clone(),
      };
      if let Err(e) = range_files.read(&inputs.git, &params.review.repository_path, &subject) {
        debug!(error = %e, "cannot prefetch the changes of the review");
      }
    }));
    Ok(())
  })
  .await
}
