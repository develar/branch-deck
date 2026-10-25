//! The stack branches in the output of `bd status` and `bd sync` (`stacks`): of each stack that forks, the branch of
//! all its commits, `{prefix}/stack/<root>` (spec/branch/whole-stack.spec.md); and the whole stack of a branch, which
//! `bd safe-push --whole-stack` lands and `bd robot --whole-stack` watches, by the rule of the app
//! (`domain_policy::stack::whole_stack_of`)

use crate::branch_view::block_reason_name;
use crate::error::CliError;
use crate::output::ShortIds;
use crate::sync_command::{ConflictSummary, RemoteSummary};
use domain_policy::facts::Repository;
use domain_policy::stack::{WholeStack, whole_stack_of};
use domain_types::GroupedBranchInfo;
use domain_types::branch_names::to_final_branch_name;
use domain_types::commit::Commit;
use domain_types::error::ErrorCode;
use domain_types::stack::{StackBranchState, StackBranchUpdate};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StackStateName {
  /// The ref is the build of the stack's commits: the only state whose ref a push may send
  Built,
  /// A member failed or is blocked (`member`): not rebuilt
  Blocked,
  /// A commit of the stack conflicts with the commits before it (`conflict`): not rebuilt
  Conflict,
  /// The ref is not the build of the stack's commits (`error` says why); never push it
  Stale,
}

impl From<StackBranchState> for StackStateName {
  fn from(state: StackBranchState) -> Self {
    match state {
      StackBranchState::Built => Self::Built,
      StackBranchState::Blocked => Self::Blocked,
      StackBranchState::Conflict => Self::Conflict,
      StackBranchState::Stale => Self::Stale,
    }
  }
}

/// The stack branch of a stack that forks, or one kept for its remote branch after its stack stopped forking
#[derive(Debug, Serialize, JsonSchema)]
pub struct StackSummary {
  /// The root branch of the stack
  pub root: String,
  /// The stack branch, `{prefix}/stack/<root>`
  pub branch: String,
  pub state: StackStateName,
  /// The tip of the stack branch; `null` without one. Only a `built` tip has the stack's commits as they are
  pub tip: Option<String>,
  /// `blocked`: the member that failed or is blocked, the first in stack order, a failure before a block
  #[serde(skip_serializing_if = "Option::is_none")]
  pub member: Option<String>,
  /// `blocked`: why `member` was not built: `error` (a stack problem or a failed build), `conflict` or `blocked` (its
  /// parent can't be built on)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
  /// `conflict`: the commit whose copy conflicts with the commits of the stack before it, and the files
  #[serde(skip_serializing_if = "Option::is_none")]
  pub conflict: Option<ConflictSummary>,
  /// `stale`: why the stack branch is not the build of the stack's commits
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<String>,
  /// How many commits it has: every commit of its members
  pub commits: u32,
  /// The branches of the stack, the root first, each after its parent; empty for one whose stack no longer forks
  pub members: Vec<String>,
  /// `bd sync`: what the stack branch has that its remote branch on `origin` lacks and the reverse; `bd status`
  /// omits it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub remote: Option<RemoteSummary>,
}

impl StackSummary {
  /// The summary of `update`; `commits` are the commits it may name (the grouped branches'), for the subject of a
  /// conflict
  pub fn new<'a>(update: &StackBranchUpdate, commits: impl IntoIterator<Item = &'a Commit>, short: &ShortIds) -> Self {
    let conflict = update.conflict.as_ref().map(|conflict| {
      let subject = commits
        .into_iter()
        .find(|commit| commit.id == conflict.commit)
        .map_or_else(String::new, |commit| commit.subject.clone());
      ConflictSummary {
        commit: short.get(&conflict.commit).to_string(),
        subject,
        files: conflict.files.clone(),
        missing_commits: Vec::new(),
      }
    });
    Self {
      root: update.root.clone(),
      branch: update.branch_name.clone(),
      state: update.state.into(),
      tip: update.tip.as_deref().map(|tip| short.get(tip).to_string()),
      member: update.blocked_by.as_ref().map(|block| block.member.clone()),
      reason: update.blocked_by.as_ref().map(|block| block_reason_name(block.reason).to_string()),
      conflict,
      error: update.error.clone(),
      commits: update.commit_count,
      members: update.members.clone(),
      remote: update
        .remote
        .as_ref()
        .map(|remote| RemoteSummary::new(remote, usize::try_from(update.commit_count).unwrap_or(usize::MAX), short)),
    }
  }

  /// The ids `new` shortens, for `RepoContext::short_ids`
  pub fn ids(update: &StackBranchUpdate) -> impl Iterator<Item = &str> {
    update
      .tip
      .as_deref()
      .into_iter()
      .chain(update.conflict.as_ref().map(|conflict| conflict.commit.as_str()))
      .chain(update.remote.iter().flat_map(RemoteSummary::ids))
  }
}

/// The whole stack a `--whole-stack` start lands
#[derive(Debug, Serialize, JsonSchema)]
pub struct WholeStackSummary {
  /// The top-level root of the stack
  pub root: String,
  /// The branch the robot starts from and the only one pushed: the stack branch `{prefix}/stack/<root>` of a stack that
  /// forks; for a straight chain, which has none, the virtual branch of its last branch
  pub source: String,
  /// The state of the stack branch as the sync left it, as `stacks` names it; a start needs `built`. Omitted for a
  /// straight chain
  #[serde(skip_serializing_if = "Option::is_none")]
  pub state: Option<StackStateName>,
}

/// The whole stack of `name` among the grouped `branches` (`domain_policy::stack::whole_stack_of`, D8): the stack of its
/// root by declared parent, its stack branch of `stacks` while it forks, else the straight chain from its root, which its
/// last branch not parked lands. A branch the table lacks and a fork no stack branch is known of are refused. No git
pub fn whole_stack<'a>(branches: &'a [GroupedBranchInfo], parked: &'a BTreeMap<String, String>, stacks: &[StackBranchUpdate], name: &str) -> Result<WholeStack<'a>, CliError> {
  let repo = Repository::of_grouping(branches, parked);
  match whole_stack_of(&repo, |root| stack_branch_of(stacks, root).is_some(), name) {
    None => Err(CliError::new(ErrorCode::UnknownBranch, format!("No branch named {name:?}"))),
    Some(WholeStack::Unbuilt { root, fork }) => Err(CliError::new(
      ErrorCode::StackNotBuilt,
      format!("The stack of {root} forks at {fork}, but no stack branch of it is known; `bd sync` builds it"),
    )),
    Some(whole) => Ok(whole),
  }
}

/// The stack branch of the stack of `root` that forks, as the sync reported it
pub fn stack_branch_of<'s>(stacks: &'s [StackBranchUpdate], root: &str) -> Option<&'s StackBranchUpdate> {
  stacks.iter().find(|stack| stack.root == root && !stack.members.is_empty())
}

/// What `--whole-stack` lands, as the outputs print it
pub fn whole_stack_summary(whole: &WholeStack<'_>, stacks: &[StackBranchUpdate], prefix: &str) -> WholeStackSummary {
  match (whole, stack_branch_of(stacks, whole.root())) {
    (WholeStack::Forking { .. }, Some(update)) => WholeStackSummary {
      root: update.root.clone(),
      source: update.branch_name.clone(),
      state: Some(update.state.into()),
    },
    (WholeStack::Chain { root, leaf }, _) => WholeStackSummary {
      root: (*root).to_string(),
      source: to_final_branch_name(prefix, leaf).unwrap_or_default(),
      state: None,
    },
    _ => WholeStackSummary {
      root: whole.root().to_string(),
      source: String::new(),
      state: None,
    },
  }
}
