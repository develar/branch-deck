//! The app's squash of commits into the oldest of them, which keeps its place and its author
//! (`git_ops::squash_commits`), and its undo, a move of the branch back; both compute before the repository lock and
//! hold it for the witness of HEAD and the move only.

use crate::repository_lock::{LockWait, Prepared, RewriteOptions, Witness, lock_prepared};
use crate::reword::normalize_message;
use anyhow::{Result, ensure};
use domain_types::rewrite::SquashCommitsResult;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::copy_commit::CopyCommitError;
use git_ops::history::History;
use git_ops::ref_transaction::RefTransaction;
use git_ops::revs::with_dirs_of;
use git_ops::rewrite_plan::PreparedRewrite;
use git_ops::squash_commits::{SquashGroup, SquashMessage, check_restore, prepare_squash};
use serde::Deserialize;
use std::collections::HashSet;
use tracing::{info, instrument};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SquashCommitsParams {
  pub repository_path: String,
  /// Commits on the current branch (the original commits of virtual branch commits), in any order
  pub commit_ids: Vec<String>,
  /// The whole message of the squashed commit, the grouping prefix `(branch)` included
  pub message: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RestoreBranchHeadParams {
  pub repository_path: String,
  /// HEAD the squash left; the undo is refused if the branch moved since
  pub expected_head: String,
  pub previous_head: String,
}

/// Squashes the commits into the oldest of them, which keeps its place and its author; the others leave the history.
/// One history rewrite that keeps HEAD's tree, so the index and the working tree are not touched. The history, the
/// merges and the new commits come before the repository lock; under it only the witness of HEAD and the
/// compare-and-swap move of the branch (`RewriteOptions::lock_prepared`). The next sync rebuilds the virtual branches.
#[instrument(skip(git_executor, params, options), fields(count = params.commit_ids.len()))]
pub fn squash_commits_core(git_executor: &GitCommandExecutor, params: &SquashCommitsParams, options: &RewriteOptions) -> Result<SquashCommitsResult> {
  let unique: HashSet<&str> = params.commit_ids.iter().map(String::as_str).collect();
  ensure!(unique.len() >= 2, "Select at least two commits to squash");
  ensure!(unique.len() == params.commit_ids.len(), "A commit is selected twice");
  let message = normalize_message(&params.message);
  ensure!(
    message.lines().next().is_some_and(|subject| !subject.trim().is_empty()),
    "The commit message must start with a subject line"
  );

  let repo_path = &params.repository_path;
  // the guards and the lock take the directories read here
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
  let prepare = || prepare_squash_commits(git_executor, params, &message, options);
  let (prepared, _lock) = options.lock_prepared(git_executor, repo_path, prepare()?, prepare)?;
  let squashed_count = params.commit_ids.len();
  let rewrite = &prepared.rewrite;
  rewrite.apply(git_executor, repo_path, RefTransaction::new(), &format!("squash {squashed_count} commits"))?;

  let new_commit_id = rewrite.replacement_of(&prepared.target).unwrap_or_else(|| rewrite.new_head()).to_string();
  info!(squashed_count, %new_commit_id, "Squashed commits");
  Ok(SquashCommitsResult {
    new_commit_id,
    new_head: rewrite.new_head().to_string(),
    previous_head: rewrite.old_head().to_string(),
    squashed_count: u32::try_from(squashed_count)?,
  })
}

/// A squash computed before the repository lock: its new commits, and the commit the others go into
struct PreparedSquash {
  rewrite: PreparedRewrite,
  target: String,
}

impl Prepared for PreparedSquash {
  fn witness(&self) -> Witness {
    self.rewrite.witness()
  }

  fn discard(&self, git_executor: &GitCommandExecutor, repository_path: &str) {
    self.rewrite.discard(git_executor, repository_path);
  }
}

/// The history down to the oldest commit of `params`, on the HEAD `options` expects, and the squashed commits written on
/// temporary refs (`prepare_squash`): the branch is not moved
fn prepare_squash_commits(git_executor: &GitCommandExecutor, params: &SquashCommitsParams, message: &str, options: &RewriteOptions) -> Result<PreparedSquash> {
  let repo_path = &params.repository_path;
  let ids: Vec<&str> = params.commit_ids.iter().map(String::as_str).collect();
  let history = History::read_to(git_executor, repo_path, &ids)?;
  options.ensure_head(git_executor, repo_path, history.head().unwrap_or_default())?;
  let mut by_position = params.commit_ids.iter().map(|id| Ok((history.position(id)?, id.clone()))).collect::<Result<Vec<_>>>()?;
  // oldest first
  by_position.sort_unstable_by_key(|(position, _)| std::cmp::Reverse(*position));
  let mut ids = by_position.into_iter().map(|(_, id)| id);
  let target = ids.next().expect("at least two commits");
  let group = SquashGroup {
    target: target.clone(),
    folded: ids.collect(),
    message: SquashMessage::Text(message.to_string()),
  };
  let rewrite = prepare_squash(git_executor, repo_path, &history, &[group])?;
  Ok(PreparedSquash { rewrite, target })
}

/// Undoes a squash: moves the checked out branch back to the commit it pointed at before. Its checks (the branch at
/// the squash's head, no merge, the same tree) come before the repository lock, waited for as `lock_wait` says; under it
/// only the witness of HEAD and the compare-and-swap move
#[instrument(skip(git_executor))]
pub fn restore_branch_head_core(git_executor: &GitCommandExecutor, params: &RestoreBranchHeadParams, lock_wait: LockWait) -> Result<()> {
  let repo_path = &params.repository_path;
  // the guards and the lock take the directories read here
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
  let check = || check_restore(git_executor, repo_path, &params.expected_head, &params.previous_head).map_err(CopyCommitError::into_anyhow);
  let (restore, _lock) = lock_prepared((lock_wait, None), git_executor, repo_path, check()?, check)?;
  restore
    .apply(git_executor, repo_path, "undo squash", &RefTransaction::new())
    .map_err(CopyCommitError::into_anyhow)?;
  info!("Restored the branch head");
  Ok(())
}

#[cfg(test)]
#[path = "squash_test.rs"]
mod squash_test;
