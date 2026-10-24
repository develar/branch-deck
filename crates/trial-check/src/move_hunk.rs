//! Moving the changes of one file from one commit ahead of the baseline into another (`bd move-hunk`): the checks of
//! the request against the commits ahead of the baseline and the blocks of the file in `from` (the ids that
//! `bd commits --patch` prints), under the repository lock, then the rewrite of `git_ops::move_hunk`, which keeps HEAD's
//! tree (docs/design/git-operations.md, "Move a hunk")

use crate::diff_scan::{Renames, blob_pair, read_commit_changes};
use file_diffs::patch::BlockPatch;
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::{get_frozen_commit_list, on_local_baseline};
use git_ops::move_hunk::{MoveHunkMode, MoveHunkRequest, MovedChange, move_hunk};
use git_ops::revs::{DetachedHead, current_branch, short_id};
use git_ops::rewrite_guard::ensure_no_operation_in_progress;
use history_rewrite::baseline::NoBaseline;
use history_rewrite::commit_ids::{IdMatch, match_commit_id};
use history_rewrite::regroup::RegroupError;
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, RepositoryLock, ensure_expected_head, lock_repository};
use object_cache::cache::ObjectCache;
use std::time::Duration;
use sync_core::repo_facts::{ListedBaseline, read_listed_baseline};
use text_diff::apply::{BlockId, LineBlock};
use tracing::instrument;

#[derive(Debug, Clone, Default)]
pub struct MoveHunkParams {
  /// The commit the change is in; full or abbreviated
  pub from: String,
  /// The commit it moves into
  pub to: String,
  /// The file, relative to the repository root, as `bd commits --patch` prints it
  pub path: String,
  /// The ids of the blocks of the file in `from` (`12,3:12,5@1a2b3c4d`, as `bd commits --patch` prints them); empty for
  /// the whole change of the file
  pub blocks: Vec<String>,
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
  /// The blocks that moved (every block of the file when the whole change moved; none for a binary file)
  pub blocks: Vec<BlockPatch>,
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
  /// symlink or submodule, blocks of a binary file, a block id that does not parse, a move that leaves a commit without
  /// changes
  #[error("{0}")]
  InvalidArguments(String),
  /// A block id names no block of the file in `from`: the file changed since the id was read; `blocks` are the ids of
  /// the file's blocks now
  #[error("{message}")]
  BlockNotFound { message: String, blocks: Vec<String> },
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

/// Move the change of `params.path` (all of it, or the blocks `params.blocks`) from commit `from` into commit `to`, both
/// ahead of the baseline on the checked out branch, in one rewrite that keeps HEAD's tree. `cache` gives the blocks
/// that the diff views showed, so that the ids are the ones they printed
#[instrument(skip(git_executor, cache, options), fields(mode = ?options.mode))]
pub fn move_hunk_core(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  cache: &ObjectCache,
  params: &MoveHunkParams,
  options: &MoveHunkOptions,
) -> Result<MoveHunkResult, MoveHunkError> {
  ensure_no_operation_in_progress(git_executor, repository_path).map_err(|e| RegroupError::OperationInProgress(format!("{e:#}")))?;
  let branch = current_branch(git_executor, repository_path)?.ok_or_else(|| RegroupError::DetachedHead(DetachedHead.to_string()))?;
  let _lock = if options.mode != MoveHunkMode::Apply || options.held_lock.is_some() {
    None
  } else {
    Some(lock_repository(git_executor, repository_path, options.lock_timeout)?)
  };
  // the baseline with its commit and HEAD's tip from one listing, the commits between them from one `log`
  let ListedBaseline { name: baseline, commit, head, .. } = read_listed_baseline(git_executor, repository_path, &branch, None)?;
  let head = head.ok_or_else(|| anyhow::Error::new(NoBaseline::NoCommits))?;
  let commits = get_frozen_commit_list(git_executor, repository_path, &commit, &head, on_local_baseline(&baseline, &branch))?;
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
  let blocks = file_blocks(git_executor, repository_path, cache, &from, path)?;
  let selected = selected_blocks(&params.blocks, blocks.as_deref(), path, &from)?;
  let all = blocks.unwrap_or_default();
  let partial = !selected.is_empty() && selected.len() < all.len();
  let ranges: Vec<LineBlock> = selected.iter().map(|&index| all[index].id.block).collect();
  let change = if partial { MovedChange::Blocks(&ranges) } else { MovedChange::WholeFile };
  let moved_blocks: Vec<BlockPatch> = if partial { selected.iter().map(|&index| all[index].clone()).collect() } else { all };

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
    blocks: moved_blocks,
    commits: outcome.commits,
    trees: outcome.trees,
    changed: outcome.changed,
  })
}

/// The blocks of `path` in `from`; `None` for a file without text
fn file_blocks(git_executor: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, from: &str, path: &str) -> Result<Option<Vec<BlockPatch>>, MoveHunkError> {
  let files = read_commit_changes(git_executor, repository_path, &[from.to_string()], Renames::Detect)?
    .pop()
    .map_or_default(|diff| diff.files);
  let change = files
    .iter()
    .find(|change| change.path == path || change.old_path.as_deref() == Some(path))
    .ok_or_else(|| MoveHunkError::InvalidArguments(format!("Commit {} doesn't change {path}", short_id(from))))?;
  if let Some(old_path) = &change.old_path {
    return Err(MoveHunkError::InvalidArguments(format!(
      "Commit {} renames {old_path} to {}: only the changes of a file that keeps its path move",
      short_id(from),
      change.path
    )));
  }
  // a symlink, a submodule and a type change have no blobs to compare; git-ops refuses them
  if change.status == 'T' || (change.old_blob.is_none() && change.new_blob.is_none()) {
    return Ok(None);
  }
  Ok(FileDiffs::default().patches_of(git_executor, repository_path, cache, &[blob_pair(change)])?.pop().flatten())
}

/// The indices of the blocks that `ids` name, in the order of the file, each once; empty for none named
fn selected_blocks(ids: &[String], blocks: Option<&[BlockPatch]>, path: &str, from: &str) -> Result<Vec<usize>, MoveHunkError> {
  if ids.is_empty() {
    return Ok(Vec::new());
  }
  let Some(blocks) = blocks.filter(|blocks| !blocks.is_empty()) else {
    let what = if blocks.is_none() {
      "a file without text (binary)"
    } else {
      "changed without blocks (the mode or the line ends only)"
    };
    return Err(MoveHunkError::InvalidArguments(format!(
      "{path} is {what} in commit {}: omit --block to move its whole change",
      short_id(from)
    )));
  };
  let mut selected = Vec::with_capacity(ids.len());
  for id in ids {
    let parsed = BlockId::parse(id).ok_or_else(|| MoveHunkError::InvalidArguments(format!("{id} is no block id: `bd commits --patch` prints them, as 12,3:12,5@1a2b3c4d")))?;
    let Some(index) = blocks.iter().position(|block| block.id == parsed) else {
      return Err(MoveHunkError::BlockNotFound {
        message: format!(
          "{path} has no block {id} in commit {}: the file changed since the id was read; `blocks` are its ids now",
          short_id(from)
        ),
        blocks: blocks.iter().map(|block| block.id.to_string()).collect(),
      });
    };
    selected.push(index);
  }
  selected.sort_unstable();
  selected.dedup();
  Ok(selected)
}
