use crate::baseline::detect_baseline_branch;
use crate::commit_grouper::{CommitGrouper, GroupedCommits};
use crate::sync::prepare_branches_for_ui;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::{Commit, get_commit_list_with_handler};
use sync_types::GroupedBranchInfo;
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
}

/// Group the commits ahead of the baseline by prefix or issue key: the first phase of a sync without its events, two
/// to four git processes (baseline detection and one `log`). No integration detection, no update check, no network
#[instrument(skip(git_executor))]
pub fn group_commits_ahead_of_baseline(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<BranchGrouping> {
  let baseline_branch = detect_baseline_branch(git_executor, repository_path, "master")?;

  let mut grouper = CommitGrouper::new();
  let mut commit_ids = Vec::new();
  get_commit_list_with_handler(git_executor, repository_path, &baseline_branch, |commit| {
    commit_ids.push(commit.id.clone());
    grouper.add_commit(commit);
    Ok(())
  })?;

  let GroupedCommits {
    branches,
    unassigned,
    branch_emails,
  } = grouper.finish();
  Ok(BranchGrouping {
    branches: prepare_branches_for_ui(&branches, &branch_emails),
    baseline_branch,
    unassigned: unassigned.into_iter().rev().collect(),
    commit_ids,
  })
}
