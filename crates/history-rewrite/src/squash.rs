//! The squash of commits into the oldest of them, which keeps its place and its author (`git_ops::squash_commits`):
//! the app's and `bd squash`'s, whose commits `select_squash` finds among the commits ahead of the baseline; its trial
//! (`check_squash_commits_core`) and its undo, a move of the branch back. The squash and the undo compute before the
//! repository lock and hold it for the witness of HEAD and the move only.

use crate::commit_ids::resolve_commit_ids;
use crate::repository_lock::{LockWait, Prepared, RewriteOptions, Witness, lock_prepared};
use crate::reword::normalize_message;
use crate::snapshot::{CommitsAhead, read_grouping_inputs};
use anyhow::{Result, ensure};
use domain_types::error::ErrorCode;
use domain_types::rewrite::SquashCommitsResult;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::copy_commit::CopyCommitError;
use git_ops::history::History;
use git_ops::ref_transaction::RefTransaction;
use git_ops::revs::with_dirs_of;
use git_ops::rewrite_plan::PreparedRewrite;
use git_ops::squash_commits::{SquashGroup, SquashMessage, check_restore, prepare_squash};
use serde::Deserialize;
use std::collections::{BTreeSet, HashSet};
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
  let message = checked_message(params)?;
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

/// The trial of `squash_commits_core`: the same history, merges and new commits, then dropped, with no lock and no move.
/// Refused as the squash would be (`RewriteRefused`, `HeadMoved` on another HEAD than `options` expects); answers the
/// HEAD it was made on
#[instrument(skip(git_executor, params, options), fields(count = params.commit_ids.len()))]
pub fn check_squash_commits_core(git_executor: &GitCommandExecutor, params: &SquashCommitsParams, options: &RewriteOptions) -> Result<String> {
  let message = checked_message(params)?;
  let repo_path = &params.repository_path;
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
  let prepared = prepare_squash_commits(git_executor, params, &message, options)?;
  prepared.rewrite.discard(git_executor, repo_path);
  Ok(prepared.rewrite.old_head().to_string())
}

/// The message of `params`, normalized, after the checks of the selection: two commits or more, each once, a subject
fn checked_message(params: &SquashCommitsParams) -> Result<String> {
  let unique: HashSet<&str> = params.commit_ids.iter().map(String::as_str).collect();
  ensure!(unique.len() >= 2, "Select at least two commits to squash");
  ensure!(unique.len() == params.commit_ids.len(), "A commit is selected twice");
  let message = normalize_message(&params.message);
  ensure!(
    message.lines().next().is_some_and(|subject| !subject.trim().is_empty()),
    "The commit message must start with a subject line"
  );
  Ok(message)
}

/// The message a `bd squash` asks for the squashed commit
#[derive(Debug, Clone)]
pub enum SquashText {
  /// The message of the commit the others go into
  Target,
  /// A new subject line over the body of that commit
  Subject(String),
  /// The whole message
  Message(String),
}

/// The commits of a `bd squash` among the commits ahead of the baseline, by the ids `bd status` printed (abbreviated or
/// full): the commit the others go into, the oldest, and the message of the result
#[derive(Debug, Clone)]
pub struct SquashSelection {
  pub target: String,
  /// Oldest first
  pub folded: Vec<String>,
  pub message: String,
  /// The branch of the squashed commit, as the grouping names it; `None` for an unassigned one
  pub branch: Option<String>,
  /// The branches whose virtual branch the squash builds anew: the branch of the squashed commit and those of the
  /// commits between it and the newest folded one, whose files change. The copies of the later commits stay
  pub rebuilt_branches: Vec<String>,
}

/// A `bd squash` whose commits or message don't make one squash; nothing was read but the commits ahead
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SquashSelectionRefused {
  #[error("Ambiguous commit ids: {}", .0.join(", "))]
  Ambiguous(Vec<String>),
  #[error("Not commits ahead of the baseline: {}", .0.join(", "))]
  NotAhead(Vec<String>),
  #[error("Name the commit to squash into and at least one other commit, each once")]
  TooFew,
  #[error("Commit {into} is not the oldest one: commits are squashed into the oldest, {oldest}")]
  NotOldest { into: String, oldest: String },
  #[error("Commits {} are not in the branch of the commit they would go into ({}): move them first with `bd group`, or squash commits of one branch", .commits.join(", "), branch_name(.branch.as_deref()))]
  OtherBranch { commits: Vec<String>, branch: Option<String> },
  #[error("The message would move the squashed commit from {} to {}: keep its `(prefix)`, or change it afterwards with `bd reword`", branch_name(.from.as_deref()), branch_name(.to.as_deref()))]
  MovesBranch { from: Option<String>, to: Option<String> },
  #[error("The commit message must start with a subject line")]
  NoSubject,
}

fn branch_name(branch: Option<&str>) -> String {
  branch.map_or_else(|| "the unassigned commits".to_string(), |branch| format!("branch {branch}"))
}

impl SquashSelectionRefused {
  /// The code `bd` answers
  #[must_use]
  pub const fn code(&self) -> ErrorCode {
    match self {
      Self::NotAhead(_) => ErrorCode::NotAheadOfBaseline,
      _ => ErrorCode::InvalidArguments,
    }
  }

  /// The commits it names, as they were given
  #[must_use]
  pub fn commits(&self) -> &[String] {
    match self {
      Self::Ambiguous(commits) | Self::NotAhead(commits) | Self::OtherBranch { commits, .. } => commits,
      _ => &[],
    }
  }
}

/// The squash `bd squash <into> <commit>…` asks for: the commits ahead of the baseline and their grouping, read once
/// (`CommitsAhead`, the stores), with no lock; the squash itself reads its history again (`squash_commits_core`). The
/// folded commits must be in the branch of `into`, and the message must keep the squashed commit there: a squash never
/// moves work between branches
///
/// # Errors
/// `SquashSelectionRefused`, and the errors of the reads
#[instrument(skip(git_executor, text))]
pub fn select_squash(git_executor: &GitCommandExecutor, repo_path: &str, into: &str, commits: &[String], text: &SquashText) -> Result<SquashSelection> {
  let ahead = CommitsAhead::read(git_executor, repo_path)?.commits;
  let grouping = read_grouping_inputs(git_executor, repo_path)?;
  let candidates: Vec<&str> = ahead.iter().map(|commit| commit.id.as_str()).collect();
  let asked: Vec<String> = std::iter::once(into.to_string()).chain(commits.iter().cloned()).collect();
  let resolved = resolve_commit_ids(&asked, &candidates);
  if !resolved.ambiguous.is_empty() {
    return Err(SquashSelectionRefused::Ambiguous(resolved.ambiguous).into());
  }
  if !resolved.unknown.is_empty() {
    return Err(SquashSelectionRefused::NotAhead(resolved.unknown).into());
  }
  if resolved.full.len() < 2 || resolved.full.len() != asked.len() {
    return Err(SquashSelectionRefused::TooFew.into());
  }

  // oldest first, as `CommitsAhead` lists them
  let selected: Vec<(usize, &domain_types::commit::Commit)> = ahead.iter().enumerate().filter(|(_, commit)| resolved.full.contains(&commit.id)).collect();
  let (target_index, target) = selected[0];
  if !target.id.starts_with(into) {
    return Err(
      SquashSelectionRefused::NotOldest {
        into: into.to_string(),
        oldest: target.id.clone(),
      }
      .into(),
    );
  }
  let branch = grouping.branch_of(&target.subject);
  let elsewhere: Vec<String> = selected[1..]
    .iter()
    .filter(|(_, commit)| grouping.branch_of(&commit.subject) != branch)
    .map(|(_, commit)| commit.id.clone())
    .collect();
  if !elsewhere.is_empty() {
    return Err(SquashSelectionRefused::OtherBranch { commits: elsewhere, branch }.into());
  }

  let message = match text {
    SquashText::Target => target.message.clone(),
    SquashText::Subject(subject) => {
      let body = target.message.strip_prefix(target.subject.as_str()).unwrap_or_default();
      format!("{}{body}", subject.trim())
    }
    SquashText::Message(message) => message.clone(),
  };
  let message = normalize_message(&message);
  let Some(subject) = message.lines().next().filter(|subject| !subject.trim().is_empty()) else {
    return Err(SquashSelectionRefused::NoSubject.into());
  };
  let to = grouping.branch_of(subject);
  if to != branch {
    return Err(SquashSelectionRefused::MovesBranch { from: branch, to }.into());
  }

  let newest_index = selected.last().map_or(target_index, |(index, _)| *index);
  let mut rebuilt: BTreeSet<String> = branch.iter().cloned().collect();
  rebuilt.extend(
    ahead[target_index + 1..newest_index]
      .iter()
      .filter(|commit| !resolved.full.contains(&commit.id))
      .filter_map(|commit| grouping.branch_of(&commit.subject)),
  );
  Ok(SquashSelection {
    target: target.id.clone(),
    folded: selected[1..].iter().map(|(_, commit)| commit.id.clone()).collect(),
    message,
    branch,
    rebuilt_branches: rebuilt.into_iter().collect(),
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
  let history = History::read_to_with_notes(git_executor, repo_path, &ids)?;
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
