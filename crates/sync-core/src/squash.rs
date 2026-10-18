//! The app's squash of commits into the oldest of them, which keeps its place and its author
//! (`git_ops::squash_commits`), and its undo, a move of the branch back; both under the repository lock.

use crate::repository_lock::{DEFAULT_LOCK_TIMEOUT, RewriteOptions, lock_repository};
use crate::reword_commit::normalize_message;
use anyhow::{Result, ensure};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::history::History;
use git_ops::ref_transaction::RefTransaction;
use git_ops::squash_commits::{SquashGroup, SquashMessage, prepare_squash, restore_branch_head};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SquashCommitsResult {
  /// The squashed commit, in place of the oldest selected one
  pub new_commit_id: String,
  /// HEAD after the squash and before it, for an undo
  pub new_head: String,
  pub previous_head: String,
  pub squashed_count: u32,
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
/// One history rewrite that keeps HEAD's tree, so the index and the working tree are not touched, under the repository
/// lock. The next sync rebuilds the virtual branches.
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
  let _lock = options.lock(git_executor, repo_path)?;
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
    message: SquashMessage::Text(message),
  };

  let squashed_count = params.commit_ids.len();
  let prepared = prepare_squash(git_executor, repo_path, &history, &[group])?;
  prepared.apply(git_executor, repo_path, RefTransaction::new(), &format!("squash {squashed_count} commits"))?;

  let new_commit_id = prepared.replacement_of(&target).unwrap_or_else(|| prepared.new_head()).to_string();
  info!(squashed_count, %new_commit_id, "Squashed commits");
  Ok(SquashCommitsResult {
    new_commit_id,
    new_head: prepared.new_head().to_string(),
    previous_head: prepared.old_head().to_string(),
    squashed_count: u32::try_from(squashed_count)?,
  })
}

/// Undoes a squash: moves the checked out branch back to the commit it pointed at before, under the repository lock
#[instrument(skip(git_executor))]
pub fn restore_branch_head_core(git_executor: &GitCommandExecutor, params: &RestoreBranchHeadParams) -> Result<()> {
  let _lock = lock_repository(git_executor, &params.repository_path, DEFAULT_LOCK_TIMEOUT)?;
  restore_branch_head(git_executor, &params.repository_path, &params.expected_head, &params.previous_head, "undo squash")?;
  info!("Restored the branch head");
  Ok(())
}

#[cfg(test)]
#[path = "squash_test.rs"]
mod squash_test;
