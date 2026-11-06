//! The drop of commits ahead of the baseline from the checked out branch, for good, the files following (Drop Commit,
//! `brd drop`; spec/commit/drop.spec.md): the plan and the dry run of the files (`git_ops::unapply::check_drop`) before
//! the repository lock, under it only the witness of HEAD and the guarded move (`RewriteOptions::lock_prepared_move`),
//! as the unapply does; and its undo, the move back with the files (`lock_checked_undo`).

use crate::commit_ids::resolve_commit_ids;
use crate::repository_lock::{LockWait, RewriteOptions, WithFacts, lock_checked_undo};
use crate::snapshot::{CommitsAhead, read_grouping_inputs};
use anyhow::Result;
use domain_types::error::ErrorCode;
use domain_types::rewrite::{DropCommitsResult, RemovedCommit, UndoneDrop};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::{is_object_id, with_dirs_of};
use git_ops::unapply::{DropKind, DropPlan, apply_drop, apply_drop_undo, check_drop, check_drop_undo};
use git_ops::update_branch::UndoCheck;
use serde::Deserialize;
use std::collections::BTreeSet;
use tracing::{info, instrument};

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DropCommitsParams {
  pub repository_path: String,
  /// Commits ahead of the baseline (the original commits of virtual branch commits), full or abbreviated, in any order
  pub commit_ids: Vec<String>,
  /// Compute the drop and the dry run of its files, then drop them: no ref, note or file changes
  #[serde(default)]
  pub dry_run: bool,
}

/// A drop whose commits are not commits ahead of the baseline; nothing was read but the commits ahead
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DropSelectionRefused {
  #[error("Ambiguous commit ids: {}", .0.join(", "))]
  Ambiguous(Vec<String>),
  #[error("Not commits ahead of the baseline: {}", .0.join(", "))]
  NotAhead(Vec<String>),
  #[error("Name at least one commit to drop")]
  NoCommit,
}

impl DropSelectionRefused {
  /// The code `bd` and the app answer
  #[must_use]
  pub const fn code(&self) -> ErrorCode {
    match self {
      Self::NotAhead(_) => ErrorCode::NotAheadOfBaseline,
      Self::Ambiguous(_) | Self::NoCommit => ErrorCode::InvalidArguments,
    }
  }

  /// The commits it names, as they were given
  #[must_use]
  pub fn commits(&self) -> &[String] {
    match self {
      Self::Ambiguous(commits) | Self::NotAhead(commits) => commits,
      Self::NoCommit => &[],
    }
  }
}

/// The undo of a drop: `drop:<previous head>..<new head>`, two full ids of one length
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropUndoToken {
  pub previous_head: String,
  pub new_head: String,
}

const DROP_TOKEN: &str = "drop:";

impl DropUndoToken {
  /// A token as `Display` prints it; anything else, abbreviated ids included, is `None`
  #[must_use]
  pub fn parse(text: &str) -> Option<Self> {
    let (previous_head, new_head) = text.strip_prefix(DROP_TOKEN)?.split_once("..")?;
    let full = |id: &str| is_object_id(id) && id.len() == previous_head.len();
    (full(previous_head) && full(new_head) && previous_head != new_head).then(|| Self {
      previous_head: previous_head.to_string(),
      new_head: new_head.to_string(),
    })
  }

  /// Whether `text` claims to be a drop token, valid or not
  #[must_use]
  pub fn is_drop_token(text: &str) -> bool {
    text.starts_with(DROP_TOKEN)
  }
}

impl std::fmt::Display for DropUndoToken {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{DROP_TOKEN}{}..{}", self.previous_head, self.new_head)
  }
}

/// Drop the commits of `params` from the checked out branch, the files following: the commits ahead of the baseline
/// with their grouping (one `log`, the stores), the drop replayed on objects, the commit count (a later commit that
/// would become empty refuses it) and the dry run of the files (`check_drop`) before the repository lock, the carried
/// mapping notes written there; under it only the witness of HEAD and the mark of an interrupted move, and the guarded
/// move (`branch-deck: drop`). A dry run stops before the lock and drops what it wrote. Refused, with nothing changed:
/// `DropSelectionRefused`, `DropRefused` (a later commit depends on a dropped one, would become empty, or the root),
/// `MoveBlocked` (local changes on a path the drop writes), `HeadMoved` (another HEAD than `options` expects)
#[instrument(skip(git_executor, options), fields(repo = %params.repository_path, count = params.commit_ids.len()))]
pub fn drop_commits_core(git_executor: &GitCommandExecutor, params: &DropCommitsParams, options: &RewriteOptions) -> Result<DropCommitsResult> {
  let repo = params.repository_path.as_str();
  // the guards, the lock and the mark take the directories read here
  let git_executor = &with_dirs_of(git_executor, repo)?;
  let prepare = || prepare_drop(git_executor, params, options);
  if params.dry_run {
    let prepared = prepare()?;
    prepared.prepared.discard(git_executor, repo);
    return Ok(result_of(&prepared, prepared.prepared.head(), None));
  }
  let (prepared, _lock) = options.lock_prepared_move(git_executor, repo, prepare(), prepare)?;
  let plan = &prepared.prepared;
  apply_drop(git_executor, repo, plan)?;
  let token = DropUndoToken {
    previous_head: plan.head().to_string(),
    new_head: plan.new_head().to_string(),
  };
  info!(dropped = prepared.facts.dropped.len(), files_changed = plan.files_change(), "dropped the commits");
  Ok(result_of(&prepared, plan.new_head(), Some(token.to_string())))
}

/// What a drop read for its answer, beside its checked plan: the commits it drops and what the next sync builds anew
struct DropFacts {
  dropped: Vec<RemovedCommit>,
  rebuilt_branches: Vec<String>,
  emptied_branches: Vec<String>,
}

/// A drop computed before the repository lock
type PreparedDrop = WithFacts<DropPlan, DropFacts>;

/// The answer of `prepared`, HEAD at `new_head`
fn result_of(prepared: &PreparedDrop, new_head: &str, undo_token: Option<String>) -> DropCommitsResult {
  let WithFacts { prepared: plan, facts } = prepared;
  DropCommitsResult {
    dropped: facts.dropped.clone(),
    previous_head: plan.head().to_string(),
    new_head: new_head.to_string(),
    files_changed: plan.files_change(),
    rebuilt_branches: facts.rebuilt_branches.clone(),
    emptied_branches: facts.emptied_branches.clone(),
    undo_token,
  }
}

/// Everything a drop reads and computes before the lock: the commits it names among the commits ahead of the baseline,
/// the checked drop (its notes written, unless a dry run), the branches the next sync builds anew and those it empties
fn prepare_drop(git_executor: &GitCommandExecutor, params: &DropCommitsParams, options: &RewriteOptions) -> Result<PreparedDrop> {
  let repo = params.repository_path.as_str();
  if params.commit_ids.is_empty() {
    return Err(DropSelectionRefused::NoCommit.into());
  }
  let ahead = CommitsAhead::read(git_executor, repo)?.commits;
  // the newest commit ahead is HEAD
  if let Some(head) = ahead.last() {
    options.ensure_head(git_executor, repo, &head.id)?;
  }
  let candidates: Vec<&str> = ahead.iter().map(|commit| commit.id.as_str()).collect();
  let resolved = resolve_commit_ids(&params.commit_ids, &candidates);
  if !resolved.ambiguous.is_empty() {
    return Err(DropSelectionRefused::Ambiguous(resolved.ambiguous).into());
  }
  if !resolved.unknown.is_empty() {
    return Err(DropSelectionRefused::NotAhead(resolved.unknown).into());
  }
  let grouping = read_grouping_inputs(git_executor, repo)?;
  // oldest first, as `CommitsAhead` lists them
  let ids: Vec<String> = ahead.iter().filter(|commit| resolved.full.contains(&commit.id)).map(|commit| commit.id.clone()).collect();
  let plan = check_drop(git_executor, repo, &ids, DropKind::Drop)?;
  if let Err(e) = options.ensure_head(git_executor, repo, plan.head()) {
    plan.discard(git_executor, repo);
    return Err(e.into());
  }
  if !params.dry_run {
    plan.write_carried_notes(git_executor, repo);
  }

  let dropped: Vec<RemovedCommit> = ahead
    .iter()
    .filter(|commit| resolved.full.contains(&commit.id))
    .map(|commit| RemovedCommit {
      commit: commit.id.clone(),
      subject: commit.subject.clone(),
      branch: grouping.branch_of(&commit.subject),
    })
    .collect();
  let positions: Vec<usize> = ahead
    .iter()
    .enumerate()
    .filter(|(_, commit)| resolved.full.contains(&commit.id))
    .map(|(index, _)| index)
    .collect();
  let (oldest, newest) = (positions.first().copied().unwrap_or_default(), positions.last().copied().unwrap_or_default());
  // a kept commit between the dropped ones changes its files; one after them only when the drop changes HEAD's files
  let end = if plan.files_change() { ahead.len() } else { newest };
  let mut rebuilt: BTreeSet<String> = dropped.iter().filter_map(|commit| commit.branch.clone()).collect();
  rebuilt.extend(
    ahead[oldest..end]
      .iter()
      .filter(|commit| !resolved.full.contains(&commit.id))
      .filter_map(|commit| grouping.branch_of(&commit.subject)),
  );
  let kept_branches: BTreeSet<String> = ahead
    .iter()
    .filter(|commit| !resolved.full.contains(&commit.id))
    .filter_map(|commit| grouping.branch_of(&commit.subject))
    .collect();
  let emptied: BTreeSet<String> = dropped
    .iter()
    .filter_map(|commit| commit.branch.clone())
    .filter(|branch| !kept_branches.contains(branch))
    .collect();
  Ok(WithFacts {
    prepared: plan,
    facts: DropFacts {
      dropped,
      rebuilt_branches: rebuilt.into_iter().collect(),
      emptied_branches: emptied.into_iter().collect(),
    },
  })
}

/// Undo a drop: HEAD moves back from the token's new head to its previous head with the files (`check_drop_undo`, the
/// trees swapped), checked before the repository lock, which holds only the witness and the move (`lock_checked_undo`).
/// `HeadMoved` when HEAD moved on, `NotAnUpdate` when the last move of the branch is not this drop, `UndoBlocked` when
/// local changes sit on a path the undo writes. Undoing again is no error: `already_undone`. The dropped commits come
/// back with their ids and their mapping notes
#[instrument(skip(git_executor))]
pub fn undo_drop_core(git_executor: &GitCommandExecutor, repo_path: &str, token: &DropUndoToken, lock_wait: LockWait) -> Result<UndoneDrop> {
  // the guards, the lock, the mark and the move take the directories read here
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
  let (undo, _lock) = lock_checked_undo(lock_wait, git_executor, repo_path, |git| {
    check_drop_undo(git, repo_path, DropKind::Drop, &token.previous_head, &token.new_head)
  })?;
  let UndoCheck::Ready(undo) = undo else {
    return Ok(UndoneDrop { already_undone: true });
  };
  apply_drop_undo(git_executor, repo_path, &undo)?;
  info!("undid the drop");
  Ok(UndoneDrop { already_undone: false })
}

#[cfg(test)]
#[path = "drop_commits_test.rs"]
mod drop_commits_test;
