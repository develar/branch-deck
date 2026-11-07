//! The commits ahead of the baseline and what groups them, read once at the start of an operation: the whole `Snapshot`
//! of a regroup and a check, or the parts a read needs (`CommitsAhead`, `read_grouping_inputs`).

use crate::baseline::{NoBaseline, detect_baseline_branch};
use crate::commit_grouper::{CommitGrouper, GroupingInputs};
use crate::issue_groups::ISSUE_GROUPS;
use crate::needs::NEEDS;
use crate::parked::PARKED;
use crate::regroup::RegroupError;
use crate::stack::StackForest;
use crate::stack_parents::STACK_PARENTS;
use crate::store_lines::read_line_stores;
use anyhow::Result;
use domain_types::commit::Commit;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::get_commit_list;
use git_ops::revs::{DETACHED, DetachedHead, read_head};
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
    let baseline = detect_baseline_branch(git_executor, repository_path)?;
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
  /// The issue groups, the stack parents and the parked branches
  pub grouping: GroupingInputs,
  /// The branch the commits are ahead of, e.g. `origin/master`
  pub baseline: String,
  /// Oldest first
  pub commits: Vec<Commit>,
  /// Commit id -> its group now
  pub groups: HashMap<String, Option<String>>,
  /// How the branches stack now
  pub forest: StackForest,
}

impl Snapshot {
  /// The snapshot after the guards every regroup, dry run included, passes: no operation in progress, HEAD on a branch
  pub fn read(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Self, RegroupError> {
    Self::read_for(git_executor, repository_path, None)
  }

  /// `read`; with `lease_home` (`RegroupOptions::lease_home`), the commits ahead of that ref instead of the baseline,
  /// and a detached HEAD is the branch [`DETACHED`] instead of `DetachedHead`
  pub fn read_for(git_executor: &GitCommandExecutor, repository_path: &str, lease_home: Option<&str>) -> Result<Self, RegroupError> {
    let commits_ahead = |_: &str, _: &str| {
      let ahead = match lease_home {
        Some(home) => CommitsAhead {
          baseline: home.to_string(),
          commits: get_commit_list(git_executor, repository_path, home)?,
        },
        None => CommitsAhead::read(git_executor, repository_path)?,
      };
      Ok((ahead, ()))
    };
    let (snapshot, ()) = Self::read_guarded(git_executor, repository_path, lease_home.is_some(), commits_ahead)?;
    Ok(snapshot)
  }

  /// `read` with the commits ahead of the baseline from `commits_ahead` of HEAD and the checked out branch, after the
  /// guards; with what else it read
  pub fn read_with<T>(
    git_executor: &GitCommandExecutor,
    repository_path: &str,
    commits_ahead: impl FnOnce(&str, &str) -> Result<(CommitsAhead, T), RegroupError>,
  ) -> Result<(Self, T), RegroupError> {
    Self::read_guarded(git_executor, repository_path, false, commits_ahead)
  }

  /// `read_with`; with `detached`, a detached HEAD is the branch [`DETACHED`]
  fn read_guarded<T>(
    git_executor: &GitCommandExecutor,
    repository_path: &str,
    detached: bool,
    commits_ahead: impl FnOnce(&str, &str) -> Result<(CommitsAhead, T), RegroupError>,
  ) -> Result<(Self, T), RegroupError> {
    ensure_no_operation_in_progress(git_executor, repository_path).map_err(|e| RegroupError::OperationInProgress(format!("{e:#}")))?;
    let (head, branch) = read_head(git_executor, repository_path)?.ok_or_else(|| anyhow::Error::new(NoBaseline::NoCommits))?;
    let branch = match branch {
      Some(branch) => branch,
      None if detached => DETACHED.to_string(),
      None => return Err(RegroupError::DetachedHead(DetachedHead.to_string())),
    };
    let grouping = read_grouping_inputs(git_executor, repository_path)?;
    let (CommitsAhead { baseline, commits }, read) = commits_ahead(&head, &branch)?;
    let Grouped { groups, forest, .. } = group_commits(&commits, &grouping);
    let snapshot = Self {
      head,
      branch,
      grouping,
      baseline,
      commits,
      groups,
      forest,
    };
    Ok((snapshot, read))
  }
}

/// The issue groups and the stack parents: what decides the groups besides the subjects, and the parked branches. One
/// `cat-file --batch` for the three maps
#[instrument(skip(git_executor))]
pub fn read_grouping_inputs(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<GroupingInputs> {
  let mut maps = read_line_stores(git_executor, repository_path, &[&ISSUE_GROUPS, &STACK_PARENTS, &PARKED, &NEEDS])?.into_iter();
  Ok(GroupingInputs {
    issue_groups: maps.next().unwrap_or_default(),
    stack_parents: maps.next().unwrap_or_default(),
    parked: maps.next().unwrap_or_default(),
    needs: maps.next().unwrap_or_default(),
  })
}

/// How commits group
pub struct Grouped {
  /// Commit id -> group
  pub groups: HashMap<String, Option<String>>,
  /// How the branches stack
  pub forest: StackForest,
  /// The keys of the issue groups that group no commit
  pub idle_issue_groups: Vec<String>,
}

/// How `commits` group with `grouping`, as the sync groups them
pub fn group_commits(commits: &[Commit], grouping: &GroupingInputs) -> Grouped {
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
