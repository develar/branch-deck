//! Checks and index handling shared by every rewrite of the checked out branch (reword, move, amend, drop).
//! See docs/design/git-operations.md: the working tree is never written, and the user's index changes only for an
//! amend, which stages the files it took, keeping whatever is staged on the other paths.

use crate::index_lock::{INDEX_LOCK_WAIT, IndexLock};
use crate::ref_transaction::RefTransaction;
use crate::revs::{DETACHED, checked_out_ref, checked_out_target, known_dirs};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::path::Path;
use tracing::instrument;

/// The checked out branch about to be rewritten, read once: the branch is moved only if it still points at `head`
#[derive(Debug, Clone)]
pub struct RewriteTarget {
  /// Short branch name, e.g. `main`; [`DETACHED`] for a detached HEAD, which only a caller that allows it gets (a
  /// reword in a lease of the pool)
  pub branch: String,
  pub head: String,
}

impl RewriteTarget {
  /// The ref the rewrite moves: `refs/heads/<branch>`, or `HEAD` itself when it is detached
  #[must_use]
  pub fn full_ref(&self) -> String {
    checked_out_ref(&self.branch)
  }

  /// Add the move of the target from `head` to `new_head` to `transaction`, with compare-and-swap: a detached HEAD moves
  /// itself (`option no-deref`), so it stays detached even when a `git switch` attached it meanwhile
  pub fn add_move(&self, transaction: &mut RefTransaction, new_head: &str) {
    if self.branch == DETACHED {
      transaction.update_no_deref(DETACHED, new_head, Some(&self.head));
    } else {
      transaction.update(&self.full_ref(), new_head, Some(&self.head));
    }
  }
}

/// Check that the commits of `HEAD ^exclude...` on the checked out branch can be rewritten:
/// no rebase, merge, cherry-pick or revert in progress, HEAD on a branch (or, with `detached`, a detached HEAD) and no
/// merge commit in the range. Merges are refused rather than flattened: neither `git replay` nor our commit writer
/// recreate them. A rewrite of commits it read checks them with `History::check_rewrite`, which needs no process for
/// the merges
#[instrument(skip(git_executor))]
pub fn check_rewrite_range(git_executor: &GitCommandExecutor, repo_path: &str, exclude: &[&str], detached: bool) -> Result<RewriteTarget> {
  ensure_no_operation_in_progress(git_executor, repo_path)?;
  let (branch, head) = checked_out_target(git_executor, repo_path, detached)?;
  ensure_no_merges(git_executor, repo_path, &head, exclude)?;
  Ok(RewriteTarget { branch, head })
}

/// Fails if `head ^exclude...` contains a merge commit
pub fn ensure_no_merges(git_executor: &GitCommandExecutor, repo_path: &str, head: &str, exclude: &[&str]) -> Result<()> {
  let exclude: Vec<String> = exclude.iter().map(|rev| format!("^{rev}")).collect();
  let mut args = vec!["rev-list", "--min-parents=2", "-n1", head];
  args.extend(exclude.iter().map(String::as_str));
  args.push("--");
  let merge = git_executor.execute_command(&args, repo_path)?;
  if !merge.is_empty() {
    return Err(merge_refusal(&merge));
  }
  Ok(())
}

/// The refusal of a rewrite whose range has the merge commit `id`
pub(crate) fn merge_refusal(id: &str) -> anyhow::Error {
  anyhow!(
    "Cannot rewrite history that contains merge commit {}: Branch Deck rewrites only linear history",
    crate::revs::short_id(id)
  )
}

/// The files git keeps in the worktree's own git directory while it is in the middle of an operation, each with the
/// operation; a sync's inputs stat them too (`sync_core::sync_inputs`), so the end of one is a change
pub const OPERATION_MARKERS: [(&str, &str); 6] = [
  ("rebase-merge", "a rebase"),
  ("rebase-apply", "a rebase"),
  ("MERGE_HEAD", "a merge"),
  ("CHERRY_PICK_HEAD", "a cherry-pick"),
  ("REVERT_HEAD", "a revert"),
  ("sequencer", "a cherry-pick or revert"),
];

/// Fails if git is in the middle of a rebase, merge, cherry-pick or revert: rewriting history then would corrupt it.
pub fn ensure_no_operation_in_progress(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<()> {
  match find_operation_in_progress(git_executor, repo_path)? {
    Some(operation) => Err(anyhow!("Cannot rewrite commits while {operation} is in progress")),
    None => Ok(()),
  }
}

/// The operation git is in the middle of ("a rebase", "a merge", "a cherry-pick", "a revert"), from the files it keeps in
/// the git directory; one `rev-parse` for all of them
pub fn find_operation_in_progress(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Option<&'static str>> {
  // the markers live in the worktree's own git directory: none to read when the operation knows it
  if let Some(dirs) = known_dirs(git_executor, repo_path) {
    return Ok(OPERATION_MARKERS.iter().find(|(marker, _)| dirs.git_path(marker).exists()).map(|(_, operation)| *operation));
  }
  let mut args = vec!["rev-parse"];
  for (marker, _) in OPERATION_MARKERS {
    args.push("--git-path");
    args.push(marker);
  }
  // one path per marker, relative to the repository directory (or absolute in a linked worktree)
  let paths = git_executor.execute_command_lines(&args, repo_path)?;
  Ok(
    OPERATION_MARKERS
      .iter()
      .zip(paths)
      .find(|(_, path)| Path::new(repo_path).join(path).exists())
      .map(|((_, operation), _)| *operation),
  )
}

/// HEAD is not the commit a rewrite was computed for: read the history again and retry
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}; read it again and retry", self.what_moved())]
pub struct HeadMoved {
  pub branch: String,
  pub expected: String,
  pub actual: String,
}

impl HeadMoved {
  /// The words of an undo that this move refuses: the same undo can never succeed, so no advice to retry it
  pub fn undo_message(&self) -> String {
    format!("{}; it can't be undone any more", self.what_moved())
  }

  fn what_moved(&self) -> String {
    format!(
      "Branch {} moved: HEAD is {}, not {}",
      self.branch,
      crate::revs::short_id(&self.actual),
      crate::revs::short_id(&self.expected)
    )
  }
}

/// A copy of the user's index in a temporary directory, for a dry run with `GIT_INDEX_FILE`; removed on drop
pub struct IndexCopy {
  _dir: tempfile::TempDir,
  path: String,
}

impl IndexCopy {
  /// A copy of the index file at `index_path`, which a caller that knows the git dir of the worktree names
  pub(crate) fn of(index_path: &Path) -> Result<Self> {
    let dir = tempfile::Builder::new().prefix("branchdeck-index-").tempdir()?;
    let copy = dir.path().join("index");
    if index_path.exists() {
      std::fs::copy(index_path, &copy).map_err(|e| anyhow!("Failed to copy the index: {e}"))?;
    }
    let path = copy.to_str().ok_or_else(|| anyhow!("Temporary index path is not valid UTF-8"))?.to_string();
    Ok(Self { _dir: dir, path })
  }

  #[must_use]
  pub fn path(&self) -> &str {
    &self.path
  }
}

/// How the user's index follows the branch after a rewrite that touches no file: a rewrite whose files change (a drop)
/// moves the branch with its files and its index (`update_branch`), never through this
#[derive(Debug)]
pub enum IndexUpdate {
  /// HEAD keeps its tree (squash, fold): the index stays as it is. `rewrite_plan::prepare_plan` refuses a plan that
  /// would change the tree (`RewriteRefused`), so no rewrite of this kind leaves the index behind its HEAD
  Unchanged,
  /// HEAD now has the working-tree versions of these files (amend): stage exactly them, like `git commit -- <paths>`
  /// does. Other staged changes stay staged.
  FromWorktree(Vec<String>),
}

impl IndexUpdate {
  /// Update the user's index after the branch moved, waiting for the index lock of another process
  /// (`IndexLock::retry`); an error is returned, never ignored
  #[instrument(skip(self, git_executor))]
  pub fn apply(&self, git_executor: &GitCommandExecutor, repo_path: &str) -> Result<()> {
    let Self::FromWorktree(files) = self else { return Ok(()) };
    let lock = IndexLock::of(git_executor, repo_path);
    // `update-index --add` hashes the staged files and records their stat data, and no other entry changes: no refresh
    lock
      .retry(INDEX_LOCK_WAIT, || stage_worktree_files(git_executor, repo_path, &[], files))
      .map_err(|e| anyhow!("The branch was rewritten, but the index could not follow it; run `git reset -q` to reset the index to HEAD: {e:#}"))
  }
}

/// Stage the working-tree versions of `files` (a missing file is removed) into the index of `env`: the user's, or a
/// temporary one by `GIT_INDEX_FILE`. One `update-index --add --remove -z --stdin`
pub(crate) fn stage_worktree_files(git_executor: &GitCommandExecutor, repo_path: &str, env: &[(&str, &str)], files: &[String]) -> Result<()> {
  let mut input = String::new();
  for file in files {
    input.push_str(file);
    input.push('\0');
  }
  git_executor
    .cmd(&["update-index", "--add", "--remove", "-z", "--stdin"], repo_path)
    .env(env)
    .stdin(input.as_bytes())
    .text()?;
  Ok(())
}
