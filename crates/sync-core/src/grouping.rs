//! The grouping of the commits ahead of the baseline as a sync groups them, without the builds and the events
//! (`BranchGrouping`): what `bd status`, `bd robot` and the other reads of `bd` start from.

use crate::baseline::BaselineUpstream;
use crate::repo_facts::{BaselineListing, landing_remote_of, read_baseline_listing};
use crate::sync::prepare_branches_for_ui;
use anyhow::Result;
use branch_integration::common::ListedBranch;
use domain_types::GroupedBranchInfo;
use domain_types::commit::Commit;
use domain_types::landing_route::LandingRemote;
use domain_types::stack::BranchStack;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::get_commit_list;
use git_ops::revs::{read_head, ref_tips, tips_under};
use history_rewrite::baseline::NoBaseline;
use history_rewrite::commit_grouper::{CommitGrouper, GroupedCommits, GroupingInputs};
use history_rewrite::snapshot::read_grouping_inputs_with;
use history_rewrite::stack::{StackForest, empty_parent_states_listed, place_branches, read_only_detection, stacks_enabled};
use indexmap::IndexMap;
use std::collections::HashMap;
use tracing::instrument;

/// The commits ahead of the baseline, grouped into virtual branches as a sync groups them, without syncing
pub struct BranchGrouping {
  /// The commit HEAD is at
  pub head: String,
  /// The checked out branch; `None` for a detached HEAD
  pub head_branch: Option<String>,
  pub baseline_branch: String,
  /// The remote branch behind the baseline; `None` for a local baseline
  pub upstream: Option<BaselineUpstream>,
  /// The branches under the prefix by name, with their tips: virtual, archived, unapplied and stack; the listing reads
  /// no commit
  pub listed: Vec<ListedBranch>,
  /// Full name -> tip of its remote branch on `origin`, of the branches under the prefix
  pub remote_tips: HashMap<String, String>,
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
  /// What the route into master is decided on of git, from the push URLs of the listing (`landing_remote_of`), without
  /// the setting, which is the config of the caller's command
  pub landing: LandingRemote,
}

/// Group the commits ahead of the baseline by prefix, issue group or issue key: the first phase of a sync without its
/// events, six git processes: the stack setting, HEAD and its branch (`rev-parse`), the baseline and the branches under
/// `branch_prefix` as a sync lists them (`remote`, one `for-each-ref`), one `log`, and one `cat-file` for the issue
/// groups and the stack parents; with stacked branches one more for placing them (the detection notes of parents
/// without commits). Integration detection runs only for a parent whose virtual branch the next sync archives, and
/// writes no note. No update check, no network
#[instrument(skip(git_executor))]
pub fn group_commits_ahead_of_baseline(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str) -> Result<BranchGrouping> {
  let stacks = stacks_enabled(git_executor, repository_path)?;
  group_commits_ahead_of_baseline_with(git_executor, repository_path, branch_prefix, stacks)
}

/// `group_commits_ahead_of_baseline` with the stack setting read already (`ConfigSnapshot::stacks`): one `git config`
/// fewer
#[instrument(skip(git_executor))]
pub fn group_commits_ahead_of_baseline_with(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str, stacks: bool) -> Result<BranchGrouping> {
  let (head, head_branch) = read_head(git_executor, repository_path)?.ok_or(NoBaseline::NoCommits)?;
  let BaselineListing {
    baseline: baseline_branch,
    upstream,
    branches: listed,
    remote_tips,
    push_urls,
    ..
  } = read_baseline_listing(git_executor, repository_path, head_branch.as_deref(), Some(branch_prefix), false)?;
  let commits = get_commit_list(git_executor, repository_path, &baseline_branch)?;
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
    Some(forest) if !branches.is_empty() => Some(place_branches(
      forest,
      &empty_parent_states_listed(git_executor, repository_path, branch_prefix, &listed, forest, read_only_detection(&baseline_branch))?,
    )),
    _ => None,
  };
  let landing = landing_remote_of(&push_urls, upstream.as_ref(), None);
  Ok(BranchGrouping {
    landing,
    head,
    head_branch,
    // bd reads the whole descriptions where it prints them
    branches: prepare_branches_for_ui(&branches, &branch_emails, placements.as_ref(), &HashMap::new()),
    baseline_branch,
    upstream,
    listed,
    remote_tips,
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

#[cfg(test)]
#[path = "grouping_test.rs"]
mod grouping_test;
