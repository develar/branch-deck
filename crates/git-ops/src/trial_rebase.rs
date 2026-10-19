//! The rebase that Patronus runs before its checks, tried on objects only: the commits of a branch that the target
//! lacks, replayed onto the target tip with `git replay`, as a plain `git rebase` replays them. Nothing resolves a
//! conflict by words, because Patronus cannot (docs/design/git-operations.md, "Target conflict check").

use crate::paths::{COMMIT_FORMAT, commits_touching, parse_commits};
use crate::replay::{conflicting_files, first_conflict, replay_range};
use crate::revs::merge_base;
use crate::update_branch::UpdateCommit;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use sync_utils::list_cut::kept;
use tracing::{debug, instrument};

/// The newest commits of the target that change the conflicting files, at most this many
pub const MAX_TARGET_COMMITS: usize = 10;

/// The conflicting files a conflict names, at most this many (all of them when only one more)
const MAX_CONFLICT_FILES: usize = 50;

/// What the rebase of a branch onto the target tip does
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrialRebase {
  /// Every commit applies
  Clean,
  Conflict(Box<RebaseConflict>),
}

/// The first commit of the branch whose replay onto the target conflicts
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebaseConflict {
  /// The commit of the branch; its `files` are empty
  pub commit: UpdateCommit,
  /// The conflicting files, at most `MAX_CONFLICT_FILES`
  pub files: Vec<String>,
  /// How many files conflict
  pub file_count: u32,
  /// The merge base of the branch and the target tip: where the branch left the target
  pub base: Option<String>,
  /// The commits of the target after `base` that change `files`, the newest `MAX_TARGET_COMMITS`, oldest first, each
  /// with the files of `files` it changes
  pub target_commits: Vec<UpdateCommit>,
}

/// Replays the commits of `tip` that `onto` lacks onto `onto` (one `replay`). On a conflict it finds the first
/// conflicting commit by a binary search over the prefixes (`first_conflict`), the files of its merge onto the longest
/// clean prefix, and the commits of the target that change them. It writes objects only, never a ref
#[instrument(skip(git_executor))]
pub fn trial_rebase(git_executor: &GitCommandExecutor, repo_path: &str, onto: &str, tip: &str) -> Result<TrialRebase> {
  let negated = format!("^{onto}");
  if replay_range(git_executor, repo_path, onto, &[tip, &negated])?.is_some() {
    return Ok(TrialRebase::Clean);
  }
  // the commits a rebase replays, oldest first
  let output = git_executor.execute_command(&["log", "--reverse", "--no-merges", COMMIT_FORMAT, "--end-of-options", tip, &negated, "--"], repo_path)?;
  let commits = parse_commits(&output);
  let last = commits
    .len()
    .checked_sub(1)
    .ok_or_else(|| anyhow!("git replay of {tip} onto {onto} conflicts, but no commit is ahead"))?;
  let (index, clean_tip) = first_conflict(0, last, onto.to_string(), |end| {
    Ok(replay_range(git_executor, repo_path, onto, &[&commits[end].id, &negated])?)
  })?;
  let commit = commits[index].clone();
  let mut files = conflicting_files(git_executor, repo_path, &clean_tip, &commit.id)?;
  let file_count = u32::try_from(files.len()).unwrap_or(u32::MAX);
  files.truncate(kept(files.len(), MAX_CONFLICT_FILES));
  let base = merge_base(git_executor, repo_path, tip, onto)?;
  let target_commits = match &base {
    Some(base) if !files.is_empty() => commits_touching(git_executor, repo_path, &[onto, &format!("^{base}")], &files, Some(MAX_TARGET_COMMITS))?,
    _ => Vec::new(),
  };
  debug!(commit = %commit.id, file_count, target_commits = target_commits.len(), "the rebase onto the target conflicts");
  Ok(TrialRebase::Conflict(Box::new(RebaseConflict {
    commit,
    files,
    file_count,
    base,
    target_commits,
  })))
}
