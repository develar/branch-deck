//! The commits ahead of the baseline and what groups them, read once at the start of an operation: the whole `Snapshot`
//! of a regroup and a check, or the parts a read needs (`CommitsAhead`, `read_grouping_inputs`).

use crate::baseline::{NoBaseline, detect_baseline_branch, is_unborn_head};
use crate::commit_grouper::{CommitGrouper, GroupingInputs};
use crate::issue_groups::ISSUE_GROUPS;
use crate::regroup::RegroupError;
use crate::stack::{StackForest, stacks_enabled};
use crate::stack_parents::STACK_PARENTS;
use crate::store_lines::read_line_stores;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::{Commit, get_commit_list};
use git_ops::revs::{DetachedHead, current_branch};
use git_ops::rewrite_guard::ensure_no_operation_in_progress;
use std::collections::HashMap;
use tracing::instrument;

/// The commits ahead of the baseline
pub struct CommitsAhead {
  /// The branch the commits are ahead of, as `detect_baseline_branch` names it, e.g. `origin/master`
  pub baseline: String,
  /// Oldest first
  pub commits: Vec<Commit>,
}

impl CommitsAhead {
  /// The baseline (`remote`, then the upstream with `rev-parse` or the candidates with `for-each-ref`) and one `log` of
  /// the commits ahead of it
  pub fn read(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Self> {
    let baseline = detect_baseline_branch(git_executor, repository_path, "master")?;
    let commits = get_commit_list(git_executor, repository_path, &baseline)?;
    Ok(Self { baseline, commits })
  }
}

/// The commits ahead of the baseline and how they group now, read once per operation
#[derive(Clone)]
pub struct Snapshot {
  pub head: String,
  /// The checked out branch
  pub branch: String,
  /// The stack setting, the issue groups and the stack parents
  pub grouping: GroupingInputs,
  /// The branch the commits are ahead of, e.g. `origin/master`
  pub baseline: String,
  /// Oldest first
  pub commits: Vec<Commit>,
  /// Commit id -> its group now
  pub groups: HashMap<String, Option<String>>,
  /// How the branches stack now; `None` with stacked branches off
  pub forest: Option<StackForest>,
}

impl Snapshot {
  /// The snapshot after the guards every regroup, dry run included, passes: no operation in progress, HEAD on a branch
  pub fn read(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Self, RegroupError> {
    ensure_no_operation_in_progress(git_executor, repository_path).map_err(|e| RegroupError::OperationInProgress(format!("{e:#}")))?;
    let branch = current_branch(git_executor, repository_path)?.ok_or_else(|| RegroupError::DetachedHead(DetachedHead.to_string()))?;
    let head = read_head(git_executor, repository_path)?;
    let grouping = read_grouping_inputs(git_executor, repository_path)?;
    let CommitsAhead { baseline, commits } = CommitsAhead::read(git_executor, repository_path)?;
    let Grouped { groups, forest, .. } = group_commits(&commits, &grouping);
    Ok(Self {
      head,
      branch,
      grouping,
      baseline,
      commits,
      groups,
      forest,
    })
  }
}

/// The id of HEAD; a branch without commits is `NoBaseline::NoCommits`
pub(crate) fn read_head(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<String, RegroupError> {
  let (output, exit_code) = git_executor.execute_command_with_status(&["rev-parse", "HEAD"], repository_path)?;
  if exit_code == 0 {
    return Ok(output.trim().to_string());
  }
  if is_unborn_head(git_executor, repository_path)? {
    return Err(RegroupError::Other(anyhow::Error::new(NoBaseline::NoCommits)));
  }
  Err(RegroupError::Other(anyhow::anyhow!("Cannot read HEAD: {output}")))
}

/// The stack setting, the issue groups and the stack parents: what decides the groups besides the subjects. Two git
/// processes: `git config` and one `cat-file --batch` for both maps
#[instrument(skip(git_executor))]
pub fn read_grouping_inputs(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<GroupingInputs> {
  read_grouping_inputs_with(git_executor, repository_path, stacks_enabled(git_executor, repository_path)?)
}

/// `read_grouping_inputs` with the stack setting read already (`ConfigSnapshot::stacks`): one `cat-file --batch`
pub fn read_grouping_inputs_with(git_executor: &GitCommandExecutor, repository_path: &str, stacks: bool) -> Result<GroupingInputs> {
  let mut maps = read_line_stores(git_executor, repository_path, &[&ISSUE_GROUPS, &STACK_PARENTS])?.into_iter();
  Ok(GroupingInputs {
    stacks,
    issue_groups: maps.next().unwrap_or_default(),
    stack_parents: maps.next().unwrap_or_default(),
  })
}

/// How commits group
pub(crate) struct Grouped {
  /// Commit id -> group
  pub(crate) groups: HashMap<String, Option<String>>,
  /// With stacked branches on
  pub(crate) forest: Option<StackForest>,
  /// The keys of the issue groups that group no commit
  pub(crate) idle_issue_groups: Vec<String>,
}

/// How `commits` group with `grouping`, as the sync groups them
pub(crate) fn group_commits(commits: &[Commit], grouping: &GroupingInputs) -> Grouped {
  let mut grouper = CommitGrouper::new_with(grouping);
  for commit in commits {
    grouper.add_commit(commit.clone());
  }
  let (grouped, forest) = grouper.finish_with_stacks();
  let mut groups: HashMap<String, Option<String>> = HashMap::with_capacity(commits.len());
  for (name, branch_commits) in &grouped.branches {
    for commit in branch_commits {
      groups.insert(commit.id.clone(), Some(name.clone()));
    }
  }
  for commit in &grouped.unassigned {
    groups.insert(commit.id.clone(), None);
  }
  Grouped {
    groups,
    forest,
    idle_issue_groups: grouped.idle_issue_groups,
  }
}
