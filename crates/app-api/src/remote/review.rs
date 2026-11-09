//! The review window (spec/review/review-window.spec.md): what it shows of its target, through the memo of the app
//! (`RemoteMemory::reviews`), and its read ahead while the window opens

use crate::context::AppContext;
use crate::operation::run_blocking;
use domain_types::error::ApiError;
use domain_types::review::{GetReviewParams, TargetReview};
use git_ops::commit_files::DiffSubject;
use serde::Deserialize;
use specta::Type;
use std::sync::Arc;
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
/// (`RangeFilesCache::read`): it answers once both are read, an answer the page never waits for, so a host that waits
/// for the commands of a repository waits for the read too (the e2e server before it deletes one: a read that outlived
/// its command failed on the objects of a deleted repository); a failure is the window's to tell when it reads them
/// itself. A list of commits has no range: its files are those of its first commit, which the window reads itself
#[instrument(skip_all, fields(repository_path = %params.review.repository_path, target = ?params.review.target, part = ?params.part))]
pub async fn prefetch_review(ctx: &AppContext, params: PrefetchReviewParams) -> Result<(), ApiError> {
  let (inputs, memo, range_files) = (ctx.prefix_inputs(), ctx.remote.reviews.clone(), ctx.file_diffs.range_files.clone());
  run_blocking(move || {
    let review = match inputs
      .branch_prefix(&params.review.repository_path)
      .and_then(|branch_prefix| memo.get(&inputs.git, &branch_prefix, &params.review))
    {
      Ok(review) => review,
      Err(e) => {
        debug!(error = %e, "cannot prefetch the review");
        return Ok::<_, ApiError>(());
      }
    };
    let segments = review.segments();
    let (Some(first), Some(last)) = (segments.first(), segments.last()) else {
      return Ok(());
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
    Ok(())
  })
  .await
}
