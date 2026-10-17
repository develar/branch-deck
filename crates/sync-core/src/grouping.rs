use crate::commit_grouper::{CommitGrouper, GroupedCommits, GroupingInputs};
use crate::snapshot::{CommitsAhead, read_grouping_inputs_with};
use crate::stack::{StackForest, empty_parent_states_listed, place_branches, read_only_detection, stacks_enabled};
use crate::sync::prepare_branches_for_ui;
use anyhow::Result;
use branch_integration::common::list_branches;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::Commit;
use git_ops::revs::{ref_tips, tips_under};
use indexmap::IndexMap;
use std::collections::HashMap;
use sync_types::GroupedBranchInfo;
use sync_types::stack::BranchStack;
use tracing::instrument;

/// The commits ahead of the baseline, grouped into virtual branches as a sync groups them, without syncing
pub struct BranchGrouping {
  pub baseline_branch: String,
  /// Ordered as the UI shows them, newest first, each with its commits newest first
  pub branches: Vec<GroupedBranchInfo>,
  /// Commits without a group, newest first
  pub unassigned: Vec<Commit>,
  /// The ids of every commit ahead of the baseline, oldest first
  pub commit_ids: Vec<String>,
  /// The parent of the oldest commit, which the virtual branches are rebuilt on; `None` without commits
  pub base: Option<String>,
  /// The stack setting, the issue groups and the stack parents the commits were grouped with
  pub grouping: GroupingInputs,
  /// The keys of the issue groups that group no commit ahead of the baseline on this HEAD; they may group the commits
  /// of another branch or worktree
  pub idle_issue_groups: Vec<String>,
  /// With stacked branches on, the children of the stack parents that no branch on this HEAD reaches; they may stack a
  /// branch of another worktree
  pub idle_stack_parents: Vec<String>,
  /// How the branches stack; `None` with stacked branches off
  pub forest: Option<StackForest>,
  /// With stacked branches on, where each branch with commits goes, as a sync places it
  pub placements: Option<IndexMap<String, BranchStack>>,
}

/// Group the commits ahead of the baseline by prefix, issue group or issue key: the first phase of a sync without its
/// events, three to six git processes (the stack setting, one `cat-file` for the issue groups and the stack parents,
/// baseline detection and one `log`), with stacked branches two more for placing them (the branches under
/// `branch_prefix` and the detection notes of parents without commits). Integration detection runs only for a parent
/// whose virtual branch the next sync archives, and writes no note. No update check, no network
#[instrument(skip(git_executor))]
pub fn group_commits_ahead_of_baseline(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str) -> Result<BranchGrouping> {
  let stacks = stacks_enabled(git_executor, repository_path)?;
  group_commits_ahead_of_baseline_with(git_executor, repository_path, branch_prefix, stacks)
}

/// `group_commits_ahead_of_baseline` with the stack setting read already (`ConfigSnapshot::stacks`): one `git config`
/// fewer
#[instrument(skip(git_executor))]
pub fn group_commits_ahead_of_baseline_with(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str, stacks: bool) -> Result<BranchGrouping> {
  let CommitsAhead {
    baseline: baseline_branch,
    commits,
  } = CommitsAhead::read(git_executor, repository_path)?;
  let grouping = read_grouping_inputs_with(git_executor, repository_path, stacks)?;
  let mut grouper = CommitGrouper::new_with(&grouping);
  let commit_ids = commits.iter().map(|commit| commit.id.clone()).collect();
  for commit in commits {
    grouper.add_commit(commit);
  }

  let base = grouper.first_parent_id.take();
  let (
    GroupedCommits {
      branches,
      unassigned,
      branch_emails,
      idle_issue_groups,
      idle_stack_parents,
    },
    forest,
  ) = grouper.finish_with_stacks();
  let placements = match &forest {
    Some(forest) if !branches.is_empty() => {
      let listed = list_branches(git_executor, repository_path, branch_prefix)?;
      Some(place_branches(
        forest,
        &empty_parent_states_listed(git_executor, repository_path, branch_prefix, &listed, forest, read_only_detection(&baseline_branch))?,
      ))
    }
    _ => None,
  };
  Ok(BranchGrouping {
    // bd reads the whole descriptions where it prints them
    branches: prepare_branches_for_ui(&branches, &branch_emails, placements.as_ref(), &HashMap::new()),
    baseline_branch,
    unassigned: unassigned.into_iter().rev().collect(),
    commit_ids,
    base,
    grouping,
    idle_issue_groups,
    idle_stack_parents,
    forest,
    placements,
  })
}

/// Full name (without `refs/heads/`) -> tip of the branches under `{branch_prefix}/`, from one `for-each-ref`
pub fn branch_tips(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str) -> Result<HashMap<String, String>> {
  let namespace = format!("refs/heads/{}/", branch_prefix.trim_end_matches('/'));
  Ok(tips_under(&ref_tips(git_executor, repository_path, &[&namespace])?, "refs/heads/"))
}
