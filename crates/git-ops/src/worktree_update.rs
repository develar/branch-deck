//! The one place Branch Deck writes the user's working tree (docs/design/git-operations.md, the exception to
//! invariant 1): Update Project moves the checked out branch from tree `old` to tree `new`, and the files follow with
//! a two-way `read-tree -m -u`. It updates only the paths whose content differs between the two trees, keeps local
//! changes on every other path, and refuses as a whole, before writing anything, if a path it would update has
//! uncommitted or staged changes or is untracked. The refresh of the index before the move and the `read-tree` after it
//! wait for the index lock of another process (`index_lock`). `crates/git-executor/src/worktree_safety_test.rs` allows `-u` in this
//! file only, and counts the occurrences. The functions are `pub(crate)`: only `update_branch.rs` calls them, for
//! Update Project (the app and `bd update`) and its undo (`bd undo update:`), and through its moves for the finish of a
//! resolve session, `bd pick` and the unapply of a virtual branch (`MOVE_CALLERS` of the scan).

use crate::index_lock::{INDEX_LOCK_WAIT, IndexLock};
use crate::rewrite_guard::IndexCopy;
use anyhow::{Result, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::{debug, instrument, warn};

/// Check that `read-tree -m -u <old_tree> <new_tree>` would succeed, on a copy of the index, so neither the index nor
/// the working tree is written. Returns git's message if the update is refused.
/// The copy is refreshed first: a file that was only touched (same content, new stat data) must not count as changed.
#[instrument(skip(git_executor))]
pub(crate) fn dry_run_worktree_update(git_executor: &GitCommandExecutor, repo_path: &str, old_tree: &str, new_tree: &str) -> Result<Option<String>> {
  let copy = IndexCopy::new(git_executor, repo_path)?;
  let env = [("GIT_INDEX_FILE", copy.path())];

  let refreshed = git_executor.cmd(&["update-index", "-q", "--refresh"], repo_path).env(&env).status()?;
  if refreshed.code != 0 {
    bail!("Failed to refresh a copy of the index: {}", refreshed.stderr);
  }
  let dry_run = git_executor
    .cmd(&["read-tree", "-m", "-u", "-n", "--no-recurse-submodules", old_tree, new_tree], repo_path)
    .env(&env)
    .status()?;
  if dry_run.code == 0 {
    return Ok(None);
  }
  debug!(output = %dry_run.stderr, "working tree update dry run refused");
  Ok(Some(dry_run.stderr))
}

/// Refresh the stat data of the user's index before a move whose files follow, so `read-tree -m -u` takes a file that
/// was only touched as unchanged: before the branch moves, so a wait for the index lock of another process
/// (`IndexLock::retry`) waits while nothing has moved
#[instrument(skip(git_executor, lock))]
pub(crate) fn prepare_worktree_update(git_executor: &GitCommandExecutor, repo_path: &str, lock: &IndexLock<'_>) -> Result<()> {
  lock.retry(INDEX_LOCK_WAIT, || refresh_index(git_executor, repo_path))
}

/// Update the user's index and working tree from `old_tree` (the tree of the commit HEAD pointed at) to `new_tree`
/// (HEAD's tree now; for an undo, the tree before the update). Must follow a passed `dry_run_worktree_update`,
/// `prepare_worktree_update` and the branch move: git checks every path again before it writes any file, so a file
/// changed since the dry run fails the update with nothing written. A lock of another process waits (`IndexLock::retry`)
#[instrument(skip(git_executor, lock))]
pub(crate) fn apply_worktree_update(git_executor: &GitCommandExecutor, repo_path: &str, old_tree: &str, new_tree: &str, lock: &IndexLock<'_>) -> Result<()> {
  lock.retry(INDEX_LOCK_WAIT, || {
    let (output, exit_code) = git_executor.execute_command_with_status(&["read-tree", "-m", "-u", "--no-recurse-submodules", old_tree, new_tree], repo_path)?;
    if exit_code != 0 {
      bail!("{output}");
    }
    Ok(())
  })?;
  // read-tree drops the stat data of the updated entries; restore it so `git status` doesn't rehash those files. The
  // files followed already: a failure (the lock of another process) is no reason to move the branch back, and the next
  // `git status` refreshes the index anyway
  if let Err(e) = refresh_index(git_executor, repo_path) {
    warn!(error = %format!("{e:#}"), "the files followed, but the index stat data was not refreshed");
  }
  Ok(())
}

/// `update-index -q --refresh`: with `-q`, modified files are not an error, only a failure to write the index is
fn refresh_index(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<()> {
  let (output, exit_code) = git_executor.execute_command_with_status(&["update-index", "-q", "--refresh"], repo_path)?;
  if exit_code != 0 {
    bail!("Failed to refresh the index: {output}");
  }
  Ok(())
}
