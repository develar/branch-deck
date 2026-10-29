//! Parked branches: the branches whose commits stay on the local branch and are never pushed or landed, until they are
//! unparked (spec/branch/parked-branches.spec.md). The store is the ref `refs/branch-deck/parked`: a commit whose tree
//! holds the one file `parked`, sorted `branch<TAB>note` lines (`store_lines.rs`), the note may be empty. The key is the
//! branch as the grouping names it (the leaf with stacked branches on), so a change of the branch prefix keeps it. Park
//! and unpark compute it before the repository lock and move it under the lock with compare-and-swap; a rename carries the line in the transaction of
//! the regroup (`regroup.rs`); a sync only drops the lines of the branches whose archived branch is integrated
//! (`drop_integrated_parked`). The ref is shared by every branch and worktree, so a write changes only the lines it names.

use crate::baseline::NoBaseline;
use crate::move_commits::validate_branch_name;
use crate::repository_lock::{MovedHead, Prepared, RepositoryLocked, RewriteOptions, Witness};
use crate::snapshot::{CommitsAhead, group_commits, read_grouping_inputs};
use crate::store_lines::{LineStore, StoreMoved, drop_store_lines, format_store_lines};
use anyhow::Result;
use domain_types::error::ErrorCode;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_writer::{WrittenCommits, write_commits_with_notes};
use git_ops::ref_transaction::RefTransaction;
use git_ops::revs::with_dirs_of;
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

impl ParkError {
  /// The code of a refused or failed park, the same in `bd` and in the app (as `RegroupError::code`); `None` for a
  /// failure without a code of its own
  pub fn code(&self) -> Option<ErrorCode> {
    Some(match self {
      Self::UnknownBranch(_) => ErrorCode::UnknownBranch,
      Self::NotParked(_) => ErrorCode::NotParked,
      Self::InvalidNote(_) => ErrorCode::InvalidArguments,
      Self::Moved(_) => ErrorCode::ParkedMoved,
      Self::Locked(_) => ErrorCode::Locked,
      Self::Other(error) if error.downcast_ref::<NoBaseline>().is_some() => ErrorCode::NoBaseline,
      Self::Other(_) => return None,
    })
  }
}

/// A park reads no HEAD: only a git failure could carry `HeadMoved`
impl MovedHead for ParkError {
  fn head_moved(&self) -> bool {
    matches!(self, Self::Other(error) if error.head_moved())
  }
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
  // the guards and the lock take the directories read here
  let git_executor = &with_dirs_of(git_executor, repository_path)?;
  write_prepared(git_executor, repository_path, options, || {
    let grouping = read_grouping_inputs(git_executor, repository_path)?;
    let ahead = CommitsAhead::read(git_executor, repository_path)?;
    let grouped = group_commits(&ahead.commits, &grouping);
    if let Some(unknown) = lines.iter().find(|line| !grouped.groups.values().any(|group| group.as_deref() == Some(line.branch))) {
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
  // the guards and the lock take the directories read here
  let git_executor = &with_dirs_of(git_executor, repository_path)?;
  let changes: Vec<(&str, Option<String>)> = branches.iter().map(|branch| (*branch, None)).collect();
  write_prepared(git_executor, repository_path, options, || {
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

/// The store commit `prepare` computed and wrote before the repository lock, moved under it. A park or an unpark writes
/// the one store it read, so the old value of its compare-and-swap is its witness: a store that moved after the read
/// (a sync dropped the lines of integrated branches) is read and written once more, under the lock now
fn write_prepared(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  options: &RewriteOptions,
  prepare: impl Fn() -> Result<PreparedLines, ParkError>,
) -> Result<ParkOutcome, ParkError> {
  let prepared = match prepare() {
    Ok(prepared) if prepared.written.is_none() => return Ok(prepared.outcome),
    prepared => prepared,
  };
  let (prepared, lock) = options.lock_prepared(git_executor, repository_path, prepared, &prepare)?;
  match prepared.write(git_executor, repository_path) {
    Err(ParkError::Moved(_)) if lock.is_some() => prepare()?.write(git_executor, repository_path),
    written => written,
  }
}

/// A park or an unpark computed before the repository lock: its outcome and the store commit on a temporary ref
struct PreparedLines {
  outcome: ParkOutcome,
  /// The store commit and its temporary ref; `None` when nothing changes
  written: Option<WrittenCommits>,
  parks: bool,
}

impl Prepared for PreparedLines {
  /// No read: the compare-and-swap of the one store it writes is its witness (`write_prepared`)
  fn witness(&self) -> Witness {
    Witness::default()
  }

  fn discard(&self, git_executor: &GitCommandExecutor, repository_path: &str) {
    if let Some(written) = &self.written {
      written.discard(git_executor, repository_path);
    }
  }
}

impl PreparedLines {
  /// Move the store from the commit it was read at to the new one, deleting the temporary ref, in one transaction
  fn write(mut self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<ParkOutcome, ParkError> {
    let Some(written) = self.written.take() else {
      return Ok(self.outcome);
    };
    let previous = self.outcome.previous_commit.clone();
    let new_commit = written.store_commits.first().cloned().unwrap_or_default();
    let mut transaction = RefTransaction::new();
    match &previous {
      Some(previous) => transaction.update(PARKED_REF, &new_commit, Some(previous)),
      None => transaction.create(PARKED_REF, &new_commit),
    };
    written.delete_temp_refs(&mut transaction);
    if let Err(error) = transaction.commit(git_executor, repository_path, if self.parks { "park" } else { "unpark" }) {
      written.discard(git_executor, repository_path);
      // a ref that moved between the read and the transaction explains the failure
      return Err(match crate::store_lines::read_store_head(git_executor, repository_path, PARKED_REF) {
        Ok(actual) if actual.as_ref().map(|(commit, _)| commit) != previous.as_ref() => ParkError::Moved(PARKED.moved(previous, actual.map(|(commit, _)| commit))),
        _ => ParkError::from(error),
      });
    }
    self.outcome.new_commit = Some(new_commit);
    info!(branches = self.outcome.branches.len(), parks = self.parks, "Wrote the parked branches");
    Ok(self.outcome)
  }
}

/// Set the line of each branch of `changes` to its note, or drop it with `None`, on the store as `read`, in one commit
/// written on a temporary ref; the unused lines of other keys stay as they are. A branch named twice gets its last change
fn prepare_lines(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  read: &crate::store_lines::StoreLines,
  changes: &[(&str, Option<String>)],
) -> Result<PreparedLines, ParkError> {
  let named = |branch: &str| changes.iter().any(|(name, _)| *name == branch);
  let parks = changes.iter().any(|(_, note)| note.is_some());
  let mut prepared = PreparedLines {
    outcome: ParkOutcome {
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
    },
    written: None,
    parks,
  };
  let mut entries = read.entries.clone();
  for (branch, note) in changes {
    match note {
      Some(note) => entries.insert((*branch).to_string(), note.clone()),
      None => entries.remove(*branch),
    };
  }
  let had_problem = read.problems.iter().any(|problem| problem.key.as_deref().is_some_and(named));
  if entries == read.entries && !had_problem {
    return Ok(prepared);
  }
  let kept: Vec<String> = read
    .problems
    .iter()
    .filter(|problem| !problem.key.as_deref().is_some_and(named))
    .map(|problem| problem.line.clone())
    .collect();
  let content = format_store_lines(&entries, &kept);
  let update = PARKED.update(read.commit.as_deref(), &content);
  prepared.written = Some(write_commits_with_notes(
    git_executor,
    repository_path,
    &[],
    None,
    std::slice::from_ref(&update.commit),
    None,
  )?);
  Ok(prepared)
}

/// Drop the lines of `integrated` branches, whose archived branch is integrated into the baseline
/// (`stack_parents::integrated_children` finds them among the idle lines): their commits are on the baseline, so no
/// worktree parks them again. Best effort, as `drop_store_lines`
pub fn drop_integrated_parked(git_executor: &GitCommandExecutor, repository_path: &str, integrated: &[&str]) {
  drop_store_lines(git_executor, repository_path, &PARKED, PARKED_DROP_MESSAGE, integrated);
}
