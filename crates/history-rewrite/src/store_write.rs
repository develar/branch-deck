//! The write of named lines of one `LineStore` outside a regroup: a park and an unpark (`parked.rs`), a need and its
//! removal (`needs.rs`). The store commit is written on a temporary ref before the repository lock; under it one
//! `update-ref --stdin` moves the store with compare-and-swap on the commit it was read at, which is its witness: a
//! store that moved after the read (a sync dropped the lines of integrated branches) is read and written once more,
//! under the lock now (docs/design/git-operations.md, "Lock spans").

use crate::baseline::NoBaseline;
use crate::commit_grouper::GroupingInputs;
use crate::repository_lock::{MovedHead, Prepared, RepositoryLocked, RewriteOptions, Witness};
use crate::snapshot::{CommitsAhead, group_commits, read_grouping_inputs};
use crate::store_lines::{LineStore, StoreLines, StoreMoved, format_store_lines, read_store_head};
use anyhow::Result;
use domain_types::error::ErrorCode;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_signing::Signing;
use git_ops::commit_writer::{WrittenCommits, write_commits_with_notes};
use git_ops::ref_transaction::RefTransaction;
use git_ops::revs::with_dirs_of;
use std::collections::BTreeSet;
use tracing::info;

/// What fails a write of a store but its refusals (`ParkError::Write`, `NeedError::Write`)
#[derive(Debug, thiserror::Error)]
pub enum StoreWriteError {
  /// The store changed since it was read, also after it was read once more under the lock
  #[error(transparent)]
  Moved(StoreMoved),
  #[error(transparent)]
  Locked(RepositoryLocked),
  #[error("{0:#}")]
  Other(anyhow::Error),
}

impl StoreWriteError {
  /// The code of the failure, the same in `bd` and in the app; `moved` the code of a store that moved
  /// (`ErrorCode::ParkedMoved`); `None` for a failure without a code of its own
  pub fn code(&self, moved: ErrorCode) -> Option<ErrorCode> {
    match self {
      Self::Moved(_) => Some(moved),
      Self::Locked(_) => Some(ErrorCode::Locked),
      Self::Other(error) if error.downcast_ref::<NoBaseline>().is_some() => Some(ErrorCode::NoBaseline),
      Self::Other(_) => None,
    }
  }
}

/// A write of a store reads no HEAD: only a git failure could carry `HeadMoved`
impl MovedHead for StoreWriteError {
  fn head_moved(&self) -> bool {
    matches!(self, Self::Other(error) if error.head_moved())
  }
}

impl From<anyhow::Error> for StoreWriteError {
  fn from(error: anyhow::Error) -> Self {
    match (error.downcast_ref::<StoreMoved>().cloned(), error.downcast_ref::<RepositoryLocked>().cloned()) {
      (Some(moved), _) => Self::Moved(moved),
      (None, Some(locked)) => Self::Locked(locked),
      (None, None) => Self::Other(error),
    }
  }
}

/// The error of the core of a write of a store: its refusals, and `StoreWriteError` (`store_error!`)
pub trait StoreError: From<anyhow::Error> + MovedHead {
  fn write_error(&self) -> Option<&StoreWriteError>;
}

/// `StoreError` of an error enum whose variant `Write` holds the `StoreWriteError`: a git failure converts into it, and
/// only it can carry `HeadMoved`
macro_rules! store_error {
  ($error:ty) => {
    impl $crate::repository_lock::MovedHead for $error {
      fn head_moved(&self) -> bool {
        matches!(self, Self::Write(error) if $crate::repository_lock::MovedHead::head_moved(error))
      }
    }

    impl From<anyhow::Error> for $error {
      fn from(error: anyhow::Error) -> Self {
        Self::Write(error.into())
      }
    }

    impl $crate::store_write::StoreError for $error {
      fn write_error(&self) -> Option<&$crate::store_write::StoreWriteError> {
        match self {
          Self::Write(error) => Some(error),
          _ => None,
        }
      }
    }
  };
}
pub(crate) use store_error;

/// The inputs of the grouping and the branches that group a commit ahead of the baseline: a park and a need name only
/// those (`CommitsAhead`, `read_grouping_inputs`, before the repository lock)
pub fn grouped_branches(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<(GroupingInputs, BTreeSet<String>)> {
  let grouping = read_grouping_inputs(git_executor, repository_path)?;
  let ahead = CommitsAhead::read(git_executor, repository_path)?;
  let branches = group_commits(&ahead.commits, &grouping).groups.into_values().flatten().collect();
  Ok((grouping, branches))
}

/// A write of named lines computed before the repository lock: what the core answers (`outcome`) and the store commit
/// on a temporary ref, none when nothing changes
pub struct PreparedLines<O> {
  pub outcome: O,
  store: &'static LineStore,
  /// The store commit it was computed on; `None` when there was no store
  previous: Option<String>,
  written: Option<WrittenCommits>,
  /// The operation of the reflog message (`branch-deck: park`)
  operation: &'static str,
}

impl<O> PreparedLines<O> {
  /// Set the line of each key of `changes` to its value, or drop it (`None`), on the store as `read`, in one commit
  /// written on a temporary ref: the lines of other keys stay, and so do the lines it can not read of keys it does not
  /// name. A key named twice gets its last change; nothing is written when nothing changes
  pub fn new(
    git_executor: &GitCommandExecutor,
    repository_path: &str,
    store: &'static LineStore,
    read: &StoreLines,
    changes: &[(&str, Option<String>)],
    operation: &'static str,
    outcome: O,
  ) -> Result<Self> {
    let mut prepared = Self {
      outcome,
      store,
      previous: read.commit.clone(),
      written: None,
      operation,
    };
    let named = |key: &str| changes.iter().any(|(name, _)| *name == key);
    let mut entries = read.entries.clone();
    for (key, value) in changes {
      match value {
        Some(value) => entries.insert((*key).to_string(), value.clone()),
        None => entries.remove(*key),
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
    let update = store.update(read.commit.as_deref(), &content);
    prepared.written = Some(write_commits_with_notes(
      git_executor,
      repository_path,
      &[],
      None,
      std::slice::from_ref(&update.commit),
      None,
      Signing::Off,
    )?);
    Ok(prepared)
  }

  /// Move the store from the commit it was read at to the new one, deleting the temporary ref, in one transaction: the
  /// outcome and the new commit, `None` when nothing changed. A store that moved since the read is `StoreMoved`
  fn write(mut self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<(O, Option<String>)> {
    let Some(written) = self.written.take() else {
      return Ok((self.outcome, None));
    };
    let store_ref = self.store.store_ref;
    let new_commit = written.store_commits.first().cloned().unwrap_or_default();
    let mut transaction = RefTransaction::new();
    match &self.previous {
      Some(previous) => transaction.update(store_ref, &new_commit, Some(previous)),
      None => transaction.create(store_ref, &new_commit),
    };
    written.delete_temp_refs(&mut transaction);
    if let Err(error) = transaction.commit(git_executor, repository_path, self.operation) {
      written.discard(git_executor, repository_path);
      // a ref that moved between the read and the transaction explains the failure
      return Err(match read_store_head(git_executor, repository_path, store_ref) {
        Ok(actual) if actual.as_ref().map(|(commit, _)| commit) != self.previous.as_ref() => anyhow::Error::new(self.store.moved(self.previous, actual.map(|(commit, _)| commit))),
        _ => error,
      });
    }
    info!(store = self.store.name, operation = self.operation, "Wrote the store");
    Ok((self.outcome, Some(new_commit)))
  }
}

impl<O> Prepared for PreparedLines<O> {
  /// No read: the compare-and-swap of the one store it writes is its witness
  fn witness(&self) -> Witness {
    Witness::default()
  }

  fn discard(&self, git_executor: &GitCommandExecutor, repository_path: &str) {
    if let Some(written) = &self.written {
      written.discard(git_executor, repository_path);
    }
  }
}

/// The write `prepare` computes over the executor it gets (the directories of the repository read once, for the guards
/// and the lock): at once when nothing changes (no lock), else the store commit computed before the repository lock and
/// moved under it; a store that moved after the read (`StoreWriteError::Moved`) is computed and written once more under
/// the lock. The outcome and the new commit, `None` when nothing changed
pub fn write_prepared_lines<O, E: StoreError>(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  options: &RewriteOptions,
  prepare: impl Fn(&GitCommandExecutor) -> Result<PreparedLines<O>, E>,
) -> Result<(O, Option<String>), E> {
  let git_executor = &with_dirs_of(git_executor, repository_path)?;
  let prepare = || prepare(git_executor);
  let moved = |error: &E| matches!(error.write_error(), Some(StoreWriteError::Moved(_)));
  let prepared = match prepare() {
    Ok(prepared) if prepared.written.is_none() => return Ok((prepared.outcome, None)),
    prepared => prepared,
  };
  let (prepared, lock) = options.lock_prepared(git_executor, repository_path, prepared, &prepare)?;
  match prepared.write(git_executor, repository_path).map_err(E::from) {
    Err(error) if moved(&error) && lock.is_some() => Ok(prepare()?.write(git_executor, repository_path)?),
    written => written,
  }
}
