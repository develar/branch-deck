//! The review window of a branch (spec/branch/branch-review.spec.md): the parts of its stack, through the memo of the
//! app (`RemoteMemory::branch_reviews`), and their read ahead while the window opens

use crate::context::AppContext;
use crate::operation::{operation, run_blocking};
use domain_types::error::ApiError;
use git_ops::commit_files::DiffSubject;
use serde::Deserialize;
use specta::Type;
use std::sync::Arc;
use sync_core::branch_review::{BranchReview, GetBranchReviewParams};
use sync_utils::span::in_current_span;
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

/// The parts of the branch from the memo, its `log` only for tips and a baseline it hasn't seen (`BranchReviewMemo`)
#[instrument(skip_all, fields(repository_path = %params.repository_path, baseline_branch = %params.baseline_branch, chain = params.chain.len(), whole_stack = params.whole_stack))]
pub async fn get_branch_review(ctx: &AppContext, params: GetBranchReviewParams) -> Result<Arc<BranchReview>, ApiError> {
  let (inputs, memo) = (ctx.prefix_inputs(), ctx.remote.branch_reviews.clone());
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    memo.get(&inputs.git, &branch_prefix, &params)
  })
  .await
}

/// The parts of the branch into the memo and the files of `part` into the memory of the ranges
/// (`RangeFilesCache::read`), in a blocking task that outlives the command: it answers at once, and a failure is the
/// window's to tell when it reads them itself
#[instrument(skip_all, fields(repository_path = %params.review.repository_path, chain = params.review.chain.len(), part = ?params.part))]
pub async fn prefetch_branch_review(ctx: &AppContext, params: PrefetchBranchReviewParams) -> Result<(), ApiError> {
  operation(async move {
    let (inputs, memo, range_files) = (ctx.prefix_inputs(), ctx.remote.branch_reviews.clone(), ctx.file_diffs.range_files.clone());
    tokio::task::spawn_blocking(in_current_span(move || {
      let review = match inputs
        .branch_prefix(&params.review.repository_path)
        .and_then(|branch_prefix| memo.get(&inputs.git, &branch_prefix, &params.review))
      {
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
      if let Err(e) = range_files.read(&inputs.git, &params.review.repository_path, &subject) {
        debug!(error = %e, "cannot prefetch the changes of a branch");
      }
    }));
    Ok(())
  })
  .await
}
