//! Moving the changes of one file from one commit ahead of the baseline into another (`bd move-hunk`): the checks of
//! the request against the commits ahead of the baseline and the patch of `from` (the hunks numbered as
//! `bd commits --patch` prints them), under the repository lock, then the rewrite of `git_ops::move_hunk`, which keeps
//! HEAD's tree (docs/design/git-operations.md, "Move a hunk")

use crate::commit_ids::{IdMatch, match_commit_id};
use crate::diff_scan::{PatchHunk, file_patch, read_commit_patches};
use crate::regroup::RegroupError;
use crate::repository_lock::{DEFAULT_LOCK_TIMEOUT, RepositoryLock, ensure_expected_head, lock_repository};
use crate::snapshot::CommitsAhead;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::move_hunk::{MoveHunkMode, MoveHunkRequest, MovedChange, move_hunk};
use git_ops::revs::{DetachedHead, current_branch, short_id};
use git_ops::rewrite_guard::ensure_no_operation_in_progress;
use object_cache::commit_diff::Hunk;
use std::collections::BTreeSet;
use std::time::Duration;
use tracing::instrument;

#[derive(Debug, Clone, Default)]
pub struct MoveHunkParams {
  /// The commit the change is in; full or abbreviated
  pub from: String,
  /// The commit it moves into
  pub to: String,
  /// The file, relative to the repository root, as `bd commits --patch` prints it
  pub path: String,
  /// The hunks of the file's patch in `from`, numbered from 1; empty for the whole change of the file
  pub hunks: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct MoveHunkOptions<'a> {
  /// The HEAD the request was made for; abbreviated ids are fine
  pub expect_head: Option<String>,
  /// `DryRun` computes and checks the rewrite, `Trial` also writes the commits (objects only); neither moves a ref or
  /// writes a note, nor takes the lock
  pub mode: MoveHunkMode,
  pub lock_timeout: Duration,
  /// The repository lock the caller holds, so that it can sync under it (`bd move-hunk --sync`); `None` takes the lock
  /// for the move only
  pub held_lock: Option<&'a RepositoryLock>,
}

impl Default for MoveHunkOptions<'_> {
  fn default() -> Self {
    Self {
      expect_head: None,
      mode: MoveHunkMode::Apply,
      lock_timeout: DEFAULT_LOCK_TIMEOUT,
      held_lock: None,
    }
  }
}

#[derive(Debug, Clone)]
pub struct MoveHunkResult {
  pub previous_head: String,
  /// HEAD afterwards; `previous_head` for a dry run
  pub new_head: String,
  /// Full ids
  pub from: String,
  pub to: String,
  pub path: String,
  /// The hunks that moved (every hunk of the file when the whole change moved; none for a binary file)
  pub hunks: Vec<PatchHunk>,
  /// Old -> new id of every recreated commit, oldest first; empty for a dry run
  pub commits: Vec<(String, String)>,
  /// The tree of each commit of `commits`, in their order
  pub trees: Vec<String>,
  /// The commits whose changes differ afterwards, oldest first, by their old ids
  pub changed: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum MoveHunkError {
  /// The request can't be applied: the same commit twice, a file the commit doesn't change, a rename, a mode change, a
  /// symlink or submodule, hunks of a binary file, a move that leaves a commit without changes
  #[error("{0}")]
  InvalidArguments(String),
  /// `--hunk` names a hunk the file's patch doesn't have
  #[error("{0}")]
  HunkNotFound(String),
  /// The change doesn't apply to `commit`, a commit between the two; nothing was written
  #[error("{message}")]
  Conflict { message: String, commit: String, path: String },
  /// What every rewrite of the checked out branch can meet, as a regroup names it: an id that is no commit ahead of the
  /// baseline, another HEAD, the lock held, an operation in progress, a detached HEAD, a git failure
  #[error(transparent)]
  Rewrite(#[from] RegroupError),
}

impl From<anyhow::Error> for MoveHunkError {
  fn from(error: anyhow::Error) -> Self {
    Self::Rewrite(RegroupError::from(error))
  }
}

/// Move the change of `params.path` (all of it, or the hunks `params.hunks`) from commit `from` into commit `to`, both
/// ahead of the baseline on the checked out branch, in one rewrite that keeps HEAD's tree
#[instrument(skip(git_executor, options), fields(mode = ?options.mode))]
pub fn move_hunk_core(git_executor: &GitCommandExecutor, repository_path: &str, params: &MoveHunkParams, options: &MoveHunkOptions) -> Result<MoveHunkResult, MoveHunkError> {
  ensure_no_operation_in_progress(git_executor, repository_path).map_err(|e| RegroupError::OperationInProgress(format!("{e:#}")))?;
  let branch = current_branch(git_executor, repository_path)?.ok_or_else(|| RegroupError::DetachedHead(DetachedHead.to_string()))?;
  let _lock = if options.mode != MoveHunkMode::Apply || options.held_lock.is_some() {
    None
  } else {
    Some(lock_repository(git_executor, repository_path, options.lock_timeout)?)
  };
  let CommitsAhead { baseline, commits } = CommitsAhead::read(git_executor, repository_path)?;
  let head = git_executor.execute_command(&["rev-parse", "--verify", "HEAD^{commit}"], repository_path)?;
  ensure_expected_head(options.expect_head.as_deref(), &head, || branch).map_err(RegroupError::HeadMoved)?;

  let resolve = |id: &str| match match_commit_id(id, commits.iter().map(|commit| commit.id.as_str())) {
    IdMatch::One(full) => Ok(full.to_string()),
    IdMatch::None => Err(MoveHunkError::from(RegroupError::NotAheadOfBaseline { commits: vec![id.to_string()] })),
    IdMatch::Ambiguous => Err(MoveHunkError::InvalidArguments(format!(
      "{id} names several commits ahead of the baseline; give more of the id"
    ))),
  };
  let (from, to) = (resolve(&params.from)?, resolve(&params.to)?);
  if from == to {
    return Err(MoveHunkError::InvalidArguments(format!("{} and {} are the same commit", params.from, params.to)));
  }

  let path = params.path.as_str();
  let patch = read_commit_patches(git_executor, repository_path, std::slice::from_ref(&from))?
    .pop()
    .map_or_default(|(_, patch)| patch);
  let file = file_patch(&patch, path).ok_or_else(|| MoveHunkError::InvalidArguments(format!("Commit {} doesn't change {path}", short_id(&from))))?;
  if file.renamed || file.copied {
    return Err(MoveHunkError::InvalidArguments(format!(
      "Commit {} {} {} to {}: only the changes of a file that keeps its path move",
      short_id(&from),
      if file.renamed { "renames" } else { "copies" },
      file.old_path.as_deref().unwrap_or_default(),
      file.new_path.as_deref().unwrap_or_default()
    )));
  }

  let selected: BTreeSet<usize> = params.hunks.iter().copied().collect();
  let count = file.hunks.len();
  if !selected.is_empty() && count == 0 {
    let what = if file.binary { "a binary file" } else { "changed without hunks" };
    return Err(MoveHunkError::InvalidArguments(format!(
      "{path} is {what} in commit {}: omit --hunk to move its whole change",
      short_id(&from)
    )));
  }
  if let Some(unknown) = selected.iter().find(|&&number| number == 0 || number > count) {
    let has = if count == 1 { "1 hunk".to_string() } else { format!("{count} hunks") };
    return Err(MoveHunkError::HunkNotFound(format!(
      "{path} has {has} in commit {}, no hunk {unknown}; `bd commits {} --patch` numbers them from 1",
      short_id(&from),
      short_id(&from)
    )));
  }
  let partial = !selected.is_empty() && selected.len() < count;
  let ranges: Vec<Hunk> = selected.iter().map(|&number| file.hunks[number - 1].range).collect();
  let change = if partial { MovedChange::Hunks(&ranges) } else { MovedChange::WholeFile };
  let moved_hunks: Vec<PatchHunk> = if partial {
    selected.iter().map(|&number| file.hunks[number - 1].clone()).collect()
  } else {
    file.hunks
  };

  let request = MoveHunkRequest {
    from: &from,
    to: &to,
    path,
    change,
    expected_head: &head,
    baseline_branch: &baseline,
    mode: options.mode,
  };
  let outcome = move_hunk(git_executor, repository_path, &request).map_err(|error| match error {
    git_ops::move_hunk::MoveHunkError::Refused(message) => MoveHunkError::InvalidArguments(message),
    git_ops::move_hunk::MoveHunkError::Conflict { ref commit, ref path } => MoveHunkError::Conflict {
      message: error.to_string(),
      commit: commit.clone(),
      path: path.clone(),
    },
    git_ops::move_hunk::MoveHunkError::HeadMoved(moved) => RegroupError::HeadMoved(moved).into(),
    git_ops::move_hunk::MoveHunkError::Other(error) => error.into(),
  })?;
  Ok(MoveHunkResult {
    previous_head: outcome.previous_head,
    new_head: outcome.new_head,
    from,
    to,
    path: path.to_string(),
    hunks: moved_hunks,
    commits: outcome.commits,
    trees: outcome.trees,
    changed: outcome.changed,
  })
}
