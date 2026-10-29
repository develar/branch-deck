//! `bd mark-reviewed`: keep the base and the tip a branch has now as its reviewed pair (`review_plan::mark_reviewed`,
//! spec/branch/review-plan.spec.md), which a later review of the branch compares it with. Writes only
//! `refs/branch-deck/reviewed/`, with compare-and-swap; the baseline is that of the checked out branch

use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use git_ops::reviewed_refs::ReviewedPair;
use review_plan::mark_reviewed::{MarkReviewedParams, NoCommonHistory, UnknownReviewBase, UnknownReviewBranch, mark_reviewed_core};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, clap::Args)]
pub struct MarkReviewedArgs {
  /// The branch: a virtual branch by its name (`auth`), else a local branch by its name, or `refs/heads/<name>`
  pub branch: String,
  /// The commit the branch is reviewed on, for a branch stacked on another: its parent (`auth` of `ui` on `auth`), so
  /// that the reviewed commits are the branch's own; without it, the merge base with the baseline
  #[arg(long)]
  pub base: Option<String>,
}

/// `bd mark-reviewed`
#[derive(Debug, Serialize, JsonSchema)]
pub struct MarkReviewedOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  /// The full name of the marked branch: `{prefix}/virtual/<name>` for a virtual branch
  pub branch: String,
  /// The marked branch, `refs/heads/<branch>`
  #[serde(rename = "ref")]
  pub reference: String,
  /// The merge base of the branch and `--base`, else of the branch and the baseline
  pub base: String,
  /// The tip of the branch, now its reviewed tip
  pub tip: String,
  /// The reviewed pair this mark replaced; `null` when the branch was never marked
  pub previous: Option<ReviewedPair>,
  /// Whether a ref moved: `false` when the branch was marked at this base and tip already
  pub changed: bool,
  /// How many reviewed pairs of branches that are gone this mark deleted
  pub pruned: u32,
}

/// The error of a mark as its code: an unknown branch is `unknown_branch`; a `--base` that names no commit or shares no
/// history with the branch is `invalid_arguments`; a baseline that shares none is `no_baseline`; the rest is git's
pub(crate) fn refused(error: anyhow::Error, base_given: bool) -> CliError {
  if let Some(unknown) = error.downcast_ref::<UnknownReviewBranch>() {
    return CliError::new(ErrorCode::UnknownBranch, unknown.to_string());
  }
  if let Some(unknown) = error.downcast_ref::<UnknownReviewBase>() {
    return CliError::new(ErrorCode::InvalidArguments, unknown.to_string());
  }
  if let Some(unrelated) = error.downcast_ref::<NoCommonHistory>() {
    let code = if base_given { ErrorCode::InvalidArguments } else { ErrorCode::NoBaseline };
    return CliError::new(code, unrelated.to_string());
  }
  CliError::from(error)
}

pub fn mark_reviewed(context: &RepoContext, args: &MarkReviewedArgs) -> Result<MarkReviewedOutput, CliError> {
  let params = MarkReviewedParams {
    repository_path: context.repository.clone(),
    branch: args.branch.clone(),
    base: args.base.clone(),
    baseline_branch: None,
  };
  let marked = mark_reviewed_core(&context.git, &context.prefix, &params).map_err(|error| refused(error, args.base.is_some()))?;
  let previous_ids = marked.previous.iter().flat_map(|pair| [pair.base.as_str(), pair.tip.as_str()]);
  let short = context.short_ids([marked.base.as_str(), marked.tip.as_str()].into_iter().chain(previous_ids));
  Ok(MarkReviewedOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    reference: format!("refs/heads/{}", marked.branch),
    base: short.get(&marked.base).to_string(),
    tip: short.get(&marked.tip).to_string(),
    previous: marked.previous.as_ref().map(|pair| ReviewedPair {
      base: short.get(&pair.base).to_string(),
      tip: short.get(&pair.tip).to_string(),
    }),
    changed: marked.changed,
    pruned: marked.pruned,
    branch: marked.branch,
  })
}
