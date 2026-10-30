//! The plan of a review in its window (spec/branch/review-plan.spec.md, Status: Experimental): what changed since the
//! review of the branch, the API it changes with the usages of each declaration, the facts of each file, the links
//! between the files and the dependency order of the files (`review_plan::review_plan_core`, the one entry of `bd review-plan` too). The fan-in asks the index
//! the host has (`SymbolIndexes`: accio in the app, the fixture of a test repository in the e2e server). One plan per
//! window (`Windows::begin_plan`): a newer one of the window or its close supersedes the one in flight, whose queries of
//! the index are killed, and it answers `canceled`; a symbol search of the window goes on beside it

use crate::context::AppContext;
use crate::operation::run_blocking;
use crate::transport::Call;
use domain_types::error::ApiError;
use file_diffs::accio::SearchCanceled;
use review_plan::{PlanSources, ReviewPlan, ReviewPlanParams, review_plan_core};
use std::sync::Arc;
use tracing::instrument;

/// The plan of `params.subject` (`review_plan_core`), with the branches named by the prefix Rust resolves for the
/// repository (`PrefixInputs`), through its object cache, as the plan of the window of `call`
#[instrument(skip_all, fields(repository_path = %params.repository_path, subject = ?params.subject, since_reviewed = params.since_reviewed))]
pub async fn get_review_plan(ctx: &AppContext, call: &Call, params: ReviewPlanParams) -> Result<ReviewPlan, ApiError> {
  let request = ctx.windows.begin_plan(&call.window);
  let (inputs, repository_state, file_diffs, indexes) = (ctx.prefix_inputs(), ctx.repository_state.clone(), ctx.file_diffs.clone(), Arc::clone(&ctx.symbol_indexes));
  run_blocking(move || -> anyhow::Result<ReviewPlan> {
    let canceled = || request.is_superseded();
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    let cache = repository_state.object_cache(&inputs.git, &params.repository_path);
    let index = indexes.search(&canceled);
    let sources = PlanSources {
      cache: &cache,
      file_diffs: &file_diffs,
      index: index.as_deref(),
    };
    let plan = review_plan_core(&inputs.git, sources, &branch_prefix, &params)?;
    // a plan the window asked again meanwhile is of no use, with or without an index to kill
    if canceled() {
      return Err(SearchCanceled.into());
    }
    Ok(plan)
  })
  .await
}
