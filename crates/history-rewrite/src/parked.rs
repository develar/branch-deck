//! Parked branches: the branches whose commits stay on the local branch and are never pushed or landed, until they are
//! unparked (spec/branch/parked-branches.spec.md). The store is the ref `refs/branch-deck/parked`: a commit whose tree
//! holds the one file `parked`, sorted `branch<TAB>note` lines (`store_lines.rs`), the note may be empty. The key is the
//! branch as the grouping names it (the leaf), so a change of the branch prefix keeps it. Park
//! and unpark compute it before the repository lock and move it under the lock with compare-and-swap; a rename carries the line in the transaction of
//! the regroup (`regroup.rs`); a sync only drops the lines of the branches whose archived branch is integrated
//! (`drop_integrated_parked`). The ref is shared by every branch and worktree, so a write changes only the lines it names.

use crate::move_commits::validate_branch_name;
use crate::repository_lock::RewriteOptions;
use crate::store_lines::{LineStore, StoreDrop, drop_store_lines};
use crate::store_write::{PreparedLines, StoreWriteError, grouped_branches, store_error, write_prepared_lines};
use anyhow::Result;
use domain_types::error::ErrorCode;
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::instrument;

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
  Write(StoreWriteError),
}

impl ParkError {
  /// The code of a refused or failed park, the same in `bd` and in the app (as `RegroupError::code`); `None` for a
  /// failure without a code of its own
  pub fn code(&self) -> Option<ErrorCode> {
    Some(match self {
      Self::UnknownBranch(_) => ErrorCode::UnknownBranch,
      Self::NotParked(_) => ErrorCode::NotParked,
      Self::InvalidNote(_) => ErrorCode::InvalidArguments,
      Self::Write(error) => return error.code(ErrorCode::ParkedMoved),
    })
  }
}

store_error!(ParkError);

/// A line a park writes: the branch and why it stays local (empty for none)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParkLine<'a> {
  pub branch: &'a str,
  pub note: &'a str,
}

/// What a park, a change of its note or an unpark did to one branch
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParkedChange {
  pub branch: String,
  /// The note before; `None` when the branch was not parked
  pub previous_note: Option<String>,
  /// The note afterwards; `None` when the branch is not parked
  pub note: Option<String>,
}

/// What a park or an unpark of one or more branches did
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParkOutcome {
  /// One per branch it named, in the order named
  pub branches: Vec<ParkedChange>,
  /// The commit of the store written, one for every branch; `None` when nothing changed
  pub new_commit: Option<String>,
  /// The commit of the store it was written on (the undo moves the store back to it); `None` when there was no store
  pub previous_commit: Option<String>,
}

/// Park the branches of `lines` with their notes (empty for none), or change the notes of parked branches: each must
/// group a commit ahead of the baseline, else nothing is written. The commits ahead and the grouping
/// (`CommitsAhead`, `read_grouping_inputs`) and the `fast-import` of the store commit come before the repository lock;
/// under it only one `update-ref --stdin` with compare-and-swap on the store, for every line at once (`write_prepared`);
/// nothing, and no lock, when every line is the same
#[instrument(skip(git_executor, options))]
pub fn park_core(git_executor: &GitCommandExecutor, repository_path: &str, lines: &[ParkLine<'_>], options: &RewriteOptions) -> Result<ParkOutcome, ParkError> {
  let changes = lines
    .iter()
    .map(|line| Ok((line.branch, Some(normalize_note(line.note)?))))
    .collect::<Result<Vec<_>, ParkError>>()?;
  write_prepared(git_executor, repository_path, options, |git_executor| {
    let (grouping, branches) = grouped_branches(git_executor, repository_path)?;
    if let Some(unknown) = lines.iter().find(|line| !branches.contains(line.branch)) {
      return Err(ParkError::UnknownBranch(unknown.branch.to_string()));
    }
    prepare_lines(git_executor, repository_path, &grouping.parked, &changes)
  })
}

/// Unpark `branches`: drop their lines, also of a branch that no longer groups a commit; each must be parked, else
/// nothing is written. One `cat-file --batch` and the `fast-import` of the store commit before the repository lock,
/// under it one `update-ref --stdin` with compare-and-swap on the store (`write_prepared`)
#[instrument(skip(git_executor, options))]
pub fn unpark_core(git_executor: &GitCommandExecutor, repository_path: &str, branches: &[&str], options: &RewriteOptions) -> Result<ParkOutcome, ParkError> {
  let changes: Vec<(&str, Option<String>)> = branches.iter().map(|branch| (*branch, None)).collect();
  write_prepared(git_executor, repository_path, options, |git_executor| {
    let parked = PARKED.read(git_executor, repository_path)?;
    if let Some(branch) = branches
      .iter()
      .find(|branch| !parked.entries.contains_key(**branch) && !parked.problems.iter().any(|problem| problem.key.as_deref() == Some(**branch)))
    {
      return Err(ParkError::NotParked((*branch).to_string()));
    }
    prepare_lines(git_executor, repository_path, &parked, &changes)
  })
}

/// The store commit `prepare` computed and wrote before the repository lock, moved under it (`write_prepared_lines`);
/// a store a sync dropped lines of after the read is read and written once more, under the lock now
fn write_prepared(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  options: &RewriteOptions,
  prepare: impl Fn(&GitCommandExecutor) -> Result<PreparedLines<ParkOutcome>, ParkError>,
) -> Result<ParkOutcome, ParkError> {
  let (mut outcome, new_commit) = write_prepared_lines(git_executor, repository_path, options, prepare)?;
  outcome.new_commit = new_commit;
  Ok(outcome)
}

/// Set the line of each branch of `changes` to its note, or drop it with `None`, on the store as `read`, in one commit
/// written on a temporary ref (`PreparedLines::new`)
fn prepare_lines(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  read: &crate::store_lines::StoreLines,
  changes: &[(&str, Option<String>)],
) -> Result<PreparedLines<ParkOutcome>, ParkError> {
  let parks = changes.iter().any(|(_, note)| note.is_some());
  let outcome = ParkOutcome {
    branches: changes
      .iter()
      .map(|(branch, note)| ParkedChange {
        branch: (*branch).to_string(),
        previous_note: read.entries.get(*branch).cloned(),
        note: note.clone(),
      })
      .collect(),
    new_commit: None,
    previous_commit: read.commit.clone(),
  };
  Ok(PreparedLines::new(
    git_executor,
    repository_path,
    &PARKED,
    read,
    changes,
    if parks { "park" } else { "unpark" },
    outcome,
  )?)
}

/// The drop of the lines of `integrated` branches from the parked store (`drop_integrated_parked`)
pub const fn parked_drop<'a>(integrated: &'a [&'a str]) -> StoreDrop<'a> {
  StoreDrop {
    store: &PARKED,
    message: PARKED_DROP_MESSAGE,
    keys: integrated,
    drop_values: None,
  }
}

/// Drop the lines of `integrated` branches, whose archived branch is integrated into the baseline
/// (`stack_parents::integrated_children` finds them among the idle lines): their commits are on the baseline, so no
/// worktree parks them again. Best effort, as `drop_store_lines`
pub fn drop_integrated_parked(git_executor: &GitCommandExecutor, repository_path: &str, integrated: &[&str]) {
  drop_store_lines(git_executor, repository_path, &[parked_drop(integrated)]);
}
