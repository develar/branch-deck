//! The git side of the code-owner review check before a Safe Push, shared by the app and `bd safe-push`: the commits of
//! the branches as the pre-check reads them (one `git log` of all of them, their paths from the object cache), then the
//! paths of the groups Patronus names without them (one `cat-file --batch` of the `OWNERSHIP` files). Both are
//! blocking; the check itself is `review_readiness` in between.

use domain_types::patronus::PatronusReviewRequirement;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::ownership::read_ownership;
use object_cache::cache::ObjectCache;
use patronus::client::PreMergeCommitDto;
use std::collections::{BTreeSet, HashMap};
use sync_core::patronus::{ReviewCommit, attribute_paths_by_ownership, fill_changed_paths, needs_ownership_paths, read_review_commits, review_paths};
use sync_utils::timestamp::{format_utc_timestamp, parse_rfc3339_timestamp};
use tracing::instrument;

/// The commits of a branch for the pre-check and every path they change
pub struct ReviewInput {
  /// Newest first
  pub commits: Vec<PreMergeCommitDto>,
  /// Sorted, distinct: what Patronus decides on
  pub paths: Vec<String>,
}

impl ReviewInput {
  /// The oldest commit: with the tip it names the commits checked
  pub fn base(&self) -> &str {
    self.commits.last().map_or("", |commit| commit.hash.as_str())
  }
}

/// The commits of each of `tips` that `baseline_branch` lacks, in the order of `tips`: one `git log` for all, and one
/// `diff-tree --stdin` for the commits whose paths the object cache misses. None for a branch too long to check, which
/// is not checked (`skipped_review`)
#[instrument(skip(git_executor, cache, tips), fields(tips = tips.len()))]
pub fn read_review_inputs(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  cache: &ObjectCache,
  baseline_branch: &str,
  tips: &[String],
) -> anyhow::Result<Vec<Option<ReviewInput>>> {
  let mut branches = read_review_commits(git_executor, repository_path, baseline_branch, tips)?;
  // the paths of every distinct commit at once: stacked branches share their parents' commits
  let mut seen = BTreeSet::new();
  let mut distinct: Vec<ReviewCommit> = branches.iter().flatten().flatten().filter(|commit| seen.insert(commit.hash.clone())).cloned().collect();
  fill_changed_paths(git_executor, repository_path, cache, &mut distinct)?;
  let paths: HashMap<String, Vec<String>> = distinct.into_iter().map(|commit| (commit.hash, commit.changed_paths)).collect();
  for commit in branches.iter_mut().flatten().flatten() {
    commit.changed_paths = paths.get(&commit.hash).cloned().unwrap_or_default();
  }
  Ok(
    branches
      .into_iter()
      .map(|commits| {
        commits.map(|commits| ReviewInput {
          paths: review_paths(&commits),
          commits: commits.into_iter().map(to_pre_merge_commit).collect(),
        })
      })
      .collect(),
  )
}

/// The requirement of a branch with too many commits to check, as the IDE skips it
pub const fn skipped_review() -> PatronusReviewRequirement {
  PatronusReviewRequirement {
    required: false,
    merge_request: None,
    groups: Vec::new(),
    skipped: true,
  }
}

/// Give the groups Patronus sent without paths the changed paths their `OWNERSHIP` entries at `baseline_branch` own (the
/// code owners of the target branch decide, as for Patronus): one read of the files for all `requirements`, each with
/// the paths of its branch. Without the files the groups are still right.
pub fn attribute_ownership_paths(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  baseline_branch: &str,
  requirements: &mut [(&mut PatronusReviewRequirement, &[String])],
) {
  let needing: Vec<&mut (&mut PatronusReviewRequirement, &[String])> = requirements.iter_mut().filter(|(requirement, _)| needs_ownership_paths(requirement)).collect();
  if needing.is_empty() {
    return;
  }
  let paths: Vec<String> = needing.iter().flat_map(|(_, paths)| paths.iter().cloned()).collect::<BTreeSet<_>>().into_iter().collect();
  match read_ownership(git_executor, repository_path, baseline_branch, &paths) {
    Ok(rules) => {
      for (requirement, paths) in needing {
        attribute_paths_by_ownership(requirement, &rules, paths);
      }
    }
    Err(error) => tracing::warn!(error = %error, "cannot read the OWNERSHIP files"),
  }
}

/// The commit as the pre-check reads it: the dates in UTC
fn to_pre_merge_commit(commit: ReviewCommit) -> PreMergeCommitDto {
  let utc = |date: &str| parse_rfc3339_timestamp(date).map_or_else(|| date.to_string(), format_utc_timestamp);
  PreMergeCommitDto {
    author_date: utc(&commit.author_date),
    committer_date: utc(&commit.committer_date),
    is_merge_commit: commit.is_merge(),
    hash: commit.hash,
    message: commit.message,
    author_name: commit.author_name,
    author_email: commit.author_email,
    commiter_name: commit.committer_name,
    commiter_email: commit.committer_email,
    changed_paths: commit.changed_paths,
  }
}
