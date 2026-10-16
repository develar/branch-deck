//! The git side of the code-owner review check before a Safe Push, shared by the app's Safe Push form and
//! `bd safe-push`: the commits of the branch as the pre-check reads them (one `git log`), then the paths of the groups
//! Patronus names without them (one `cat-file --batch` of the `OWNERSHIP` files). Both are blocking; the check itself
//! is `PatronusService::review_requirement` in between.

use crate::patronus::{ReviewCommit, attribute_paths_by_ownership, needs_ownership_paths, read_review_commits, review_paths};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::ownership::read_ownership;
use patronus::client::PreMergeCommitDto;
use sync_types::patronus::PatronusReviewRequirement;
use sync_utils::timestamp::{format_utc_timestamp, parse_rfc3339_timestamp};

/// The commits of a branch for the pre-check and every path they change
pub struct ReviewInput {
  /// Newest first
  pub commits: Vec<PreMergeCommitDto>,
  /// Sorted, distinct
  pub paths: Vec<String>,
}

impl ReviewInput {
  /// The oldest commit: with the tip it names the commits checked
  pub fn base(&self) -> &str {
    self.commits.last().map_or("", |commit| commit.hash.as_str())
  }
}

/// The commits of `branch_ref` that `baseline_branch` lacks; None for a branch too long to check, which is not checked
/// (`skipped_review`)
pub fn read_review_input(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str, branch_ref: &str) -> anyhow::Result<Option<ReviewInput>> {
  let Some(commits) = read_review_commits(git_executor, repository_path, baseline_branch, branch_ref)? else {
    return Ok(None);
  };
  let paths = review_paths(&commits);
  Ok(Some(ReviewInput {
    commits: commits.into_iter().map(to_pre_merge_commit).collect(),
    paths,
  }))
}

/// The requirement of a branch with too many commits to check, as the IDE skips it
pub fn skipped_review() -> PatronusReviewRequirement {
  PatronusReviewRequirement {
    required: false,
    merge_request: None,
    groups: Vec::new(),
    skipped: true,
  }
}

/// Give the groups Patronus sent without paths the changed paths their `OWNERSHIP` entries at `baseline_branch` own (the
/// code owners of the target branch decide, as for Patronus). Without the files the groups are still right.
pub fn attribute_ownership_paths(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str, requirement: &mut PatronusReviewRequirement, paths: &[String]) {
  if !needs_ownership_paths(requirement) {
    return;
  }
  match read_ownership(git_executor, repository_path, baseline_branch, paths) {
    Ok(rules) => attribute_paths_by_ownership(requirement, &rules, paths),
    Err(error) => tracing::warn!(error = %error, "cannot read the OWNERSHIP files"),
  }
}

/// The commit as the pre-check reads it: the dates in UTC
fn to_pre_merge_commit(commit: ReviewCommit) -> PreMergeCommitDto {
  let utc = |date: &str| parse_rfc3339_timestamp(date).map_or_else(|| date.to_string(), format_utc_timestamp);
  PreMergeCommitDto {
    author_date: utc(&commit.author_date),
    committer_date: utc(&commit.committer_date),
    hash: commit.hash,
    is_merge_commit: commit.is_merge,
    message: commit.message,
    author_name: commit.author_name,
    author_email: commit.author_email,
    commiter_name: commit.committer_name,
    commiter_email: commit.committer_email,
    changed_paths: commit.changed_paths,
  }
}
