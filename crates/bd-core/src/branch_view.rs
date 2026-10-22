//! The branches as the sync places them, read by most commands: the grouping of the commits ahead of the baseline,
//! the stack ancestors and the parent of a branch, and its stack problems and blocks as the outputs print them

use crate::context::RepoContext;
use crate::error::{CliError, ErrorCode};
use crate::output::ShortIds;
use domain_types::GroupedBranchInfo;
use domain_types::stack::{BlockReason, BranchStack, ParentSource};
use history_rewrite::regroup::problem_commits;
use history_rewrite::stack::describe_problem;
use schemars::JsonSchema;
use serde::Serialize;
use sync_core::grouping::{BranchGrouping, group_commits_ahead_of_baseline_with};

/// Where the parent of a branch comes from
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParentSourceName {
  /// A prefix of its commits declares it (`(parent/name)`)
  Prefix,
  /// Its line in the stack parents (`refs/branch-deck/stack-parents`)
  Store,
}

impl From<ParentSource> for ParentSourceName {
  fn from(source: ParentSource) -> Self {
    match source {
      ParentSource::Prefix => Self::Prefix,
      ParentSource::Store => Self::Store,
    }
  }
}

/// Why a branch is not built: its parent can't be built on
#[derive(Debug, Serialize, JsonSchema)]
pub struct StackBlockSummary {
  pub parent: String,
  /// `problem` (the parent has a stack problem), `error` (its build failed), `conflict` or `blocked`
  pub reason: String,
}

impl StackBlockSummary {
  pub fn of(parent: &str, reason: BlockReason) -> Self {
    Self {
      parent: parent.to_string(),
      reason: block_reason_name(reason).to_string(),
    }
  }
}

/// `problem`, `error`, `conflict` or `blocked`
pub const fn block_reason_name(reason: BlockReason) -> &'static str {
  match reason {
    BlockReason::Problem => "problem",
    BlockReason::Error => "error",
    BlockReason::Conflict => "conflict",
    BlockReason::Blocked => "blocked",
  }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StackIssue {
  pub message: String,
  /// The commits that declare it
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub commits: Vec<String>,
}

/// The stack problems of a branch, with their commits
pub fn stack_issues(branch: &str, stack: &BranchStack, short: &ShortIds) -> Vec<StackIssue> {
  stack
    .problems
    .iter()
    .map(|problem| StackIssue {
      message: describe_problem(branch, problem),
      commits: problem_commits(&problem.problem).iter().map(|id| short.get(id).to_string()).collect(),
    })
    .collect()
}

/// Group the commits ahead of the baseline; a missing baseline is `no_baseline`, a `branch` that is not a group is
/// `unknown_branch`
pub fn read_grouping(context: &RepoContext, branch: Option<&str>) -> Result<BranchGrouping, CliError> {
  // the stack setting of the command's config
  let stacks = context.config()?.stacks()?;
  let grouping = group_commits_ahead_of_baseline_with(&context.git, &context.repository, &context.prefix, stacks)?;
  if let Some(name) = branch
    && !grouping.branches.iter().any(|candidate| candidate.name == name)
  {
    return Err(CliError::new(
      ErrorCode::UnknownBranch,
      format!("No branch named {name:?} among the commits ahead of {}", grouping.baseline_branch),
    ));
  }
  Ok(grouping)
}

/// Where the sync places `name` among `branches` (`GroupedBranchInfo::stack`); `None` with stacked branches off
pub fn placement<'a>(branches: &'a [GroupedBranchInfo], name: &str) -> Option<&'a BranchStack> {
  branches.iter().find(|branch| branch.name == name)?.stack.as_ref()
}
