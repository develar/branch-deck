//! Mark Reviewed of the review window of a branch (spec/branch/review-plan.spec.md, Status: Experimental): the base and
//! the tip the branch has now become its reviewed pair, `refs/branch-deck/reviewed/<sha256>/{base,tip}`, through their
//! one writer (`review_plan::mark_reviewed::mark_reviewed_core`, `bd mark-reviewed` too): one transaction with CAS that
//! also deletes the pairs of the branches that are gone. No repository lock and no session: no sync reads these refs,
//! only the next plan of the branch (`get_review_plan`)

use crate::context::AppContext;
use crate::operation::run_blocking;
use domain_types::error::ApiError;
use review_plan::mark_reviewed::{MarkReviewedParams, ReviewedTip, mark_reviewed_core};
use tracing::instrument;

/// Keeps the base and the tip of the branch of `params` as its reviewed pair (`mark_reviewed_core`), the branch named
/// with the prefix Rust resolves for the repository (`PrefixInputs`)
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch = %params.branch))]
pub async fn mark_branch_reviewed(ctx: &AppContext, params: MarkReviewedParams) -> Result<ReviewedTip, ApiError> {
  let inputs = ctx.prefix_inputs();
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    mark_reviewed_core(&inputs.git, &branch_prefix, &params)
  })
  .await
}
