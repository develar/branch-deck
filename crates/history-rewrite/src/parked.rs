//! Parked branches: the branches whose commits stay on the local branch and are never pushed or landed, until they are
//! unparked (spec/branch/parked-branches.spec.md). The store is the ref `refs/branch-deck/parked`: a commit whose tree
//! holds the one file `parked`, sorted `branch<TAB>note` lines (`store_lines.rs`), the note may be empty. The key is the
//! branch as the grouping names it (the leaf with stacked branches on), so a change of the branch prefix keeps it. Park
//! and unpark write it under the repository lock with compare-and-swap; a rename carries the line in the transaction of
//! the regroup (`regroup.rs`); a sync only drops the lines of the branches whose archived branch is integrated
//! (`drop_integrated_parked`). The ref is shared by every branch and worktree, so a write changes only the line it names.

use crate::move_commits::validate_branch_name;
use crate::repository_lock::{RepositoryLocked, RewriteOptions};
use crate::snapshot::{CommitsAhead, group_commits, read_grouping_inputs};
use crate::store_lines::{LineStore, StoreMoved, drop_store_lines, format_store_lines};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_writer::write_store_updates;
use tracing::{info, instrument};

pub const PARKED_REF: &str = "refs/branch-deck/parked";
/// The file of the store's tree
pub const PARKED_FILE: &str = "parked";
/// The message of a store commit of a sync that only drops the lines of integrated branches: an undo of a regroup
/// passes over such commits (`store_lines::undo_over_drops`)
pub const PARKED_DROP_MESSAGE: &str = "Parked branches of Branch Deck: integrated branches dropped";
/// The longest note a line keeps, in characters
pub const MAX_NOTE_LENGTH: usize = 500;

/// The branch -> note map. A line whose key is not a branch name, and a branch listed twice (neither entry is used), are
/// problems
pub const PARKED: LineStore = LineStore {
  store_ref: PARKED_REF,
  file: PARKED_FILE,
  name: "parked branches",
  message: "Parked branches of Branch Deck",
  shape: "branch<TAB>note",
  validate: validate_line,
};

fn validate_line(branch: &str, _note: &str) -> Result<(), String> {
  validate_branch_name(branch).map_err(|e| format!("{e:#}"))
}

/// A note as a line keeps it: trimmed, one line, at most `MAX_NOTE_LENGTH` characters
pub fn normalize_note(note: &str) -> Result<String, ParkError> {
  let note = note.trim();
  if note.contains(['\n', '\r']) {
    return Err(ParkError::InvalidNote("The note must be one line".to_string()));
  }
  if note.chars().count() > MAX_NOTE_LENGTH {
    return Err(ParkError::InvalidNote(format!("The note is longer than {MAX_NOTE_LENGTH} characters")));
  }
  Ok(note.to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum ParkError {
  /// No commit ahead of the baseline is grouped into the branch
  #[error("No branch named {0:?} among the commits ahead of the baseline")]
  UnknownBranch(String),
  /// Unpark of a branch that is not parked
  #[error("{0} is not parked")]
  NotParked(String),
  #[error("{0}")]
  InvalidNote(String),
  /// The parked branches changed since they were read
  #[error(transparent)]
  Moved(StoreMoved),
  #[error(transparent)]
  Locked(RepositoryLocked),
  #[error("{0:#}")]
  Other(anyhow::Error),
}

impl From<anyhow::Error> for ParkError {
  fn from(error: anyhow::Error) -> Self {
    match (error.downcast_ref::<StoreMoved>().cloned(), error.downcast_ref::<RepositoryLocked>().cloned()) {
      (Some(moved), _) => Self::Moved(moved),
      (None, Some(locked)) => Self::Locked(locked),
      (None, None) => Self::Other(error),
    }
  }
}

/// What a park, a change of its note or an unpark did
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParkOutcome {
  pub branch: String,
  /// The note before; `None` when the branch was not parked
  pub previous_note: Option<String>,
  /// The note afterwards; `None` when the branch is not parked
  pub note: Option<String>,
  /// The commit of the store written; `None` when nothing changed
  pub new_commit: Option<String>,
}

/// Park `branch` with `note` (empty for none), or change the note of a parked branch: the branch must group a commit
/// ahead of the baseline. The repository lock, the commits ahead and the grouping (`CommitsAhead`, `read_grouping_inputs`),
/// then one `fast-import` and one `update-ref --stdin` with compare-and-swap on the store; nothing when the line is the
/// same
#[instrument(skip(git_executor, options))]
pub fn park_core(git_executor: &GitCommandExecutor, repository_path: &str, branch: &str, note: &str, options: &RewriteOptions) -> Result<ParkOutcome, ParkError> {
  let note = normalize_note(note)?;
  let _lock = options.lock(git_executor, repository_path)?;
  let grouping = read_grouping_inputs(git_executor, repository_path)?;
  let ahead = CommitsAhead::read(git_executor, repository_path)?;
  let grouped = group_commits(&ahead.commits, &grouping);
  if !grouped.groups.values().any(|group| group.as_deref() == Some(branch)) {
    return Err(ParkError::UnknownBranch(branch.to_string()));
  }
  write_line(git_executor, repository_path, &grouping.parked, branch, Some(note))
}

/// Unpark `branch`: drop its line, also of a branch that no longer groups a commit. The repository lock, one
/// `cat-file --batch`, then one `fast-import` and one `update-ref --stdin` with compare-and-swap on the store
#[instrument(skip(git_executor, options))]
pub fn unpark_core(git_executor: &GitCommandExecutor, repository_path: &str, branch: &str, options: &RewriteOptions) -> Result<ParkOutcome, ParkError> {
  let _lock = options.lock(git_executor, repository_path)?;
  let parked = PARKED.read(git_executor, repository_path)?;
  if !parked.entries.contains_key(branch) && !parked.problems.iter().any(|problem| problem.key.as_deref() == Some(branch)) {
    return Err(ParkError::NotParked(branch.to_string()));
  }
  write_line(git_executor, repository_path, &parked, branch, None)
}

/// Set the line of `branch` to `note`, or drop it with `None`, on the store as `read`; the unused lines of other keys
/// stay as they are
fn write_line(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  read: &crate::store_lines::StoreLines,
  branch: &str,
  note: Option<String>,
) -> Result<ParkOutcome, ParkError> {
  let previous_note = read.entries.get(branch).cloned();
  let had_problem = read.problems.iter().any(|problem| problem.key.as_deref() == Some(branch));
  let mut outcome = ParkOutcome {
    branch: branch.to_string(),
    previous_note,
    note: note.clone(),
    new_commit: None,
  };
  if outcome.previous_note == note && !had_problem {
    return Ok(outcome);
  }
  let mut entries = read.entries.clone();
  match note {
    Some(note) => entries.insert(branch.to_string(), note),
    None => entries.remove(branch),
  };
  let kept: Vec<String> = read
    .problems
    .iter()
    .filter(|problem| problem.key.as_deref() != Some(branch))
    .map(|problem| problem.line.clone())
    .collect();
  let content = format_store_lines(&entries, &kept);
  let written = write_store_updates(
    git_executor,
    repository_path,
    &[PARKED.update(read.commit.as_deref(), &content)],
    None,
    if outcome.note.is_some() { "park" } else { "unpark" },
  )
  .map_err(|error| {
    // a ref that moved between the read and the transaction explains the failure
    match crate::store_lines::read_store_head(git_executor, repository_path, PARKED_REF) {
      Ok(actual) if actual.as_ref().map(|(commit, _)| commit) != read.commit.as_ref() => ParkError::Moved(PARKED.moved(read.commit.clone(), actual.map(|(commit, _)| commit))),
      _ => ParkError::from(error),
    }
  })?;
  outcome.new_commit = written.into_iter().next();
  info!(branch, parked = outcome.note.is_some(), "Wrote the parked branches");
  Ok(outcome)
}

/// Drop the lines of `integrated` branches, whose archived branch is integrated into the baseline
/// (`stack_parents::integrated_children` finds them among the idle lines): their commits are on the baseline, so no
/// worktree parks them again. Best effort, as `drop_store_lines`
pub fn drop_integrated_parked(git_executor: &GitCommandExecutor, repository_path: &str, integrated: &[&str]) {
  drop_store_lines(git_executor, repository_path, &PARKED, PARKED_DROP_MESSAGE, integrated);
}
