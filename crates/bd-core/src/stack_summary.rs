//! The stack branches in the output of `bd status` and `bd sync` (`stacks`): of each stack that forks, the branch of
//! all its commits, `{prefix}/stack/<root>` (spec/branch/whole-stack.spec.md); and the whole stack of a branch, which
//! `bd safe-push --whole-stack` lands and `bd robot --whole-stack` watches

use crate::branch_view::{block_reason_name, placement};
use crate::error::{CliError, ErrorCode};
use crate::output::ShortIds;
use crate::sync_command::{ConflictSummary, RemoteSummary};
use domain_types::GroupedBranchInfo;
use domain_types::stack::{StackBranchState, StackBranchUpdate};
use git_ops::branch_names::to_final_branch_name;
use git_ops::commit_list::Commit;
use schemars::JsonSchema;
use serde::Serialize;

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

/// The whole stack of a branch, as the sync placed it
pub enum WholeStack<'a> {
  /// A stack that forks: its stack branch, as the sync reported it
  Forking(&'a StackBranchUpdate),
  /// A straight chain, a branch alone included: its last branch carries every commit of it
  Chain { root: String, leaf: String },
}

impl WholeStack<'_> {
  /// The top-level root of the stack
  pub fn root(&self) -> &str {
    match self {
      Self::Forking(update) => &update.root,
      Self::Chain { root, .. } => root,
    }
  }

  pub fn summary(&self, prefix: &str) -> WholeStackSummary {
    match self {
      Self::Forking(update) => WholeStackSummary {
        root: update.root.clone(),
        source: update.branch_name.clone(),
        state: Some(update.state.into()),
      },
      Self::Chain { root, leaf } => WholeStackSummary {
        root: root.clone(),
        source: to_final_branch_name(prefix, leaf).unwrap_or_default(),
        state: None,
      },
    }
  }
}

/// The whole stack of `name` among the grouped `branches`: the stack that forks it is a branch of (the `members` of a
/// stack branch of `stacks`), else the straight chain from its top-level root down, by the parent each branch is built
/// on or blocked by. No git process
pub fn whole_stack_of<'a>(branches: &[GroupedBranchInfo], stacks: &'a [StackBranchUpdate], name: &str) -> Result<WholeStack<'a>, CliError> {
  if !branches.iter().any(|branch| branch.name == name) {
    return Err(CliError::new(ErrorCode::UnknownBranch, format!("No branch named {name:?}")));
  }
  if let Some(update) = stacks.iter().find(|stack| stack.members.iter().any(|member| member == name)) {
    return Ok(WholeStack::Forking(update));
  }
  let parent_of =
    |branch: &str| placement(branches, branch).and_then(|stack| stack.effective_base.as_deref().or_else(|| stack.blocked_by.as_ref().map(|block| block.parent.as_str())));
  // bounded: a broken stack may have a cycle
  let mut root = name;
  for _ in 0..branches.len() {
    match parent_of(root) {
      Some(parent) if parent != name => root = parent,
      _ => break,
    }
  }
  let mut leaf = root;
  for _ in 0..branches.len() {
    let children: Vec<&str> = branches
      .iter()
      .map(|branch| branch.name.as_str())
      .filter(|branch| parent_of(branch) == Some(leaf))
      .collect();
    match children.as_slice() {
      [] => break,
      [only] => leaf = only,
      _ => {
        return Err(CliError::new(
          ErrorCode::StackNotBuilt,
          format!("The stack of {root} forks at {leaf}, but no stack branch of it is known; `bd sync` builds it"),
        ));
      }
    }
  }
  Ok(WholeStack::Chain {
    root: root.to_string(),
    leaf: leaf.to_string(),
  })
}
