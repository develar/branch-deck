use crate::baseline::detect_baseline_branch;
use crate::commit_grouper::{CommitGrouper, GroupedCommits, GroupingInputs};
use crate::issue_groups::read_grouping_inputs;
use crate::stack::{StackForest, empty_parent_states, place_branches, read_only_detection};
use crate::sync::prepare_branches_for_ui;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::{Commit, get_commit_list_with_handler};
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
  /// The stack setting and the issue groups the commits were grouped with
  pub grouping: GroupingInputs,
  /// The keys of the issue groups that group no commit ahead of the baseline on this HEAD; they may group the commits
  /// of another branch or worktree
  pub idle_issue_groups: Vec<String>,
  /// How the branches stack; `None` with stacked branches off
  pub forest: Option<StackForest>,
  /// With stacked branches on, where each branch with commits goes, as a sync places it
  pub placements: Option<IndexMap<String, BranchStack>>,
}

/// Group the commits ahead of the baseline by prefix, issue group or issue key: the first phase of a sync without its
/// events, three to six git processes (the stack setting, the issue groups, baseline detection and one `log`), with
/// stacked branches two more for placing them (the branches under `branch_prefix` and the detection notes of parents
/// without commits). Integration
/// detection runs only for a parent whose virtual branch the next sync archives, and writes no note. No update check,
/// no network
#[instrument(skip(git_executor))]
pub fn group_commits_ahead_of_baseline(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str) -> Result<BranchGrouping> {
  let baseline_branch = detect_baseline_branch(git_executor, repository_path, "master")?;

  let grouping = read_grouping_inputs(git_executor, repository_path)?;
  let mut grouper = CommitGrouper::new_with(&grouping);
  let mut commit_ids = Vec::new();
  get_commit_list_with_handler(git_executor, repository_path, &baseline_branch, |commit| {
    commit_ids.push(commit.id.clone());
    grouper.add_commit(commit);
    Ok(())
  })?;

  let base = grouper.first_parent_id.take();
  let (
    GroupedCommits {
      branches,
      unassigned,
      branch_emails,
      idle_issue_groups,
    },
    forest,
  ) = grouper.finish_with_stacks();
  let placements = match &forest {
    Some(forest) if !branches.is_empty() => {
      let branch_refs = branch_tips(git_executor, repository_path, branch_prefix)?;
      Some(place_branches(
        forest,
        &empty_parent_states(git_executor, repository_path, branch_prefix, &branch_refs, forest, read_only_detection(&baseline_branch))?,
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
    forest,
    placements,
  })
}

/// Full name (without `refs/heads/`) -> tip of the branches under `{branch_prefix}/`
pub fn branch_tips(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str) -> Result<HashMap<String, String>> {
  let namespace = format!("refs/heads/{}/", branch_prefix.trim_end_matches('/'));
  let refs = git_executor.execute_command_lines(&["for-each-ref", "--format=%(refname:lstrip=2)%00%(objectname)", &namespace], repository_path)?;
  Ok(
    refs
      .into_iter()
      .filter_map(|line| line.split_once('\0').map(|(name, tip)| (name.to_string(), tip.to_string())))
      .collect(),
  )
}
