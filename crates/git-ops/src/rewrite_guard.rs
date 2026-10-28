//! Checks and index handling shared by every rewrite of the checked out branch (reword, move, amend, drop).
//! See docs/design/git-operations.md: the working tree is never written, and the user's index changes only when the
//! tree of HEAD changes, keeping whatever is staged on paths the rewrite does not touch.

use crate::index_lock::{INDEX_LOCK_WAIT, IndexLock};
use crate::revs::{checked_out_head, known_dirs};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, instrument};

/// The checked out branch about to be rewritten, read once: the branch is moved only if it still points at `head`
#[derive(Debug, Clone)]
pub struct RewriteTarget {
  /// Short branch name, e.g. `main`
  pub branch: String,
  pub head: String,
}

impl RewriteTarget {
  #[must_use]
  pub fn full_ref(&self) -> String {
    format!("refs/heads/{}", self.branch)
  }
}

/// Check that the commits of `HEAD ^exclude...` on the checked out branch can be rewritten:
/// no rebase, merge, cherry-pick or revert in progress, HEAD on a branch and no merge commit in the range.
/// Merges are refused rather than flattened: neither `git replay` nor our commit writer recreate them. A rewrite of
/// commits it read checks them with `History::check_rewrite`, which needs no process for the merges
#[instrument(skip(git_executor))]
pub fn check_rewrite_range(git_executor: &GitCommandExecutor, repo_path: &str, exclude: &[&str]) -> Result<RewriteTarget> {
  ensure_no_operation_in_progress(git_executor, repo_path)?;
  let (branch, head) = checked_out_head(git_executor, repo_path)?;
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
  pub fn new(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Self> {
    let index_path = match known_dirs(git_executor, repo_path) {
      Some(dirs) => dirs.git_path("index").to_string_lossy().into_owned(),
      None => git_executor.execute_command(&["rev-parse", "--path-format=absolute", "--git-path", "index"], repo_path)?,
    };
    let dir = tempfile::Builder::new().prefix("branchdeck-index-").tempdir()?;
    let copy = dir.path().join("index");
    if Path::new(&index_path).exists() {
      std::fs::copy(&index_path, &copy).map_err(|e| anyhow!("Failed to copy the index: {e}"))?;
    }
    let path = copy.to_str().ok_or_else(|| anyhow!("Temporary index path is not valid UTF-8"))?.to_string();
    Ok(Self { _dir: dir, path })
  }

  #[must_use]
  pub fn path(&self) -> &str {
    &self.path
  }
}

/// How the user's index follows the branch after a rewrite
#[derive(Debug)]
pub enum IndexUpdate {
  /// HEAD's tree changes from `old_tree` to `new_tree` (drop). A two-way `read-tree -m` without `-u`: index entries
  /// that matched the old HEAD take the new HEAD's version, anything staged on other paths stays staged, and the
  /// working tree is not read or written. Refused if a staged change sits on a path the rewrite changes.
  TwoWay { old_tree: String, new_tree: String },
  /// HEAD now has the working-tree versions of these files (amend): stage exactly them, like `git commit -- <paths>`
  /// does. Other staged changes stay staged.
  FromWorktree(Vec<String>),
}

impl IndexUpdate {
  /// Check on a copy of the index that the update can't lose a staged change; run before any ref moves
  #[instrument(skip(self, git_executor))]
  pub fn preflight(&self, git_executor: &GitCommandExecutor, repo_path: &str) -> Result<()> {
    let Self::TwoWay { old_tree, new_tree } = self else { return Ok(()) };
    if old_tree == new_tree {
      return Ok(());
    }
    let copy = IndexCopy::new(git_executor, repo_path)?;
    let env = [("GIT_INDEX_FILE", copy.path())];
    let output = git_executor.cmd(&["read-tree", "-m", "-i", old_tree, new_tree], repo_path).env(&env).status()?;
    if output.code == 0 {
      return Ok(());
    }
    // the exit code decides; git's message is for people, so the paths come from diffs
    debug!(stderr = %output.stderr, "index update preflight failed");
    let paths = staged_overlap(git_executor, repo_path, &env, old_tree, new_tree)?;
    if paths.is_empty() {
      return Err(anyhow!("The index can't follow the rewritten files, resolve, unstage or commit the staged changes first"));
    }
    Err(anyhow!("Staged changes overlap the rewritten files, unstage or commit them first: {}", paths.join(", ")))
  }

  /// Update the user's index after the branch moved, waiting for the index lock of another process
  /// (`IndexLock::retry`); an error is returned, never ignored
  #[instrument(skip(self, git_executor))]
  pub fn apply(&self, git_executor: &GitCommandExecutor, repo_path: &str) -> Result<()> {
    let lock = IndexLock::of(git_executor, repo_path);
    let failed = |e: anyhow::Error| anyhow!("The branch was rewritten, but the index could not follow it; run `git reset -q` to reset the index to HEAD: {e:#}");
    match self {
      Self::TwoWay { old_tree, new_tree } if old_tree == new_tree => Ok(()),
      Self::TwoWay { old_tree, new_tree } => {
        lock
          .retry(INDEX_LOCK_WAIT, || {
            git_executor.execute_command(&["read-tree", "-m", "-i", old_tree, new_tree], repo_path).map(drop)
          })
          .map_err(failed)?;
        // read-tree drops the stat data of the replaced entries; refresh it so `git status` doesn't rehash those
        // files. Exits with 1 when files are modified, which is not an error here.
        git_executor.execute_command_with_status(&["update-index", "-q", "--refresh"], repo_path)?;
        Ok(())
      }
      // `update-index --add` hashes the staged files and records their stat data, and no other entry changes: no refresh
      Self::FromWorktree(files) => lock.retry(INDEX_LOCK_WAIT, || stage_worktree_files(git_executor, repo_path, &[], files)).map_err(failed),
    }
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

/// The paths a two-way `read-tree -m <old_tree> <new_tree>` of the index of `env` refuses: staged as neither tree has
/// them, where the trees differ. One `diff-index --cached` against the old tree and one `diff-tree` of the two trees,
/// each entry compared by mode and blob id
fn staged_overlap(git_executor: &GitCommandExecutor, repo_path: &str, env: &[(&str, &str)], old_tree: &str, new_tree: &str) -> Result<Vec<String>> {
  let staged = git_executor
    .cmd(&["diff-index", "--cached", "--raw", "-z", "--no-renames", old_tree], repo_path)
    .env(env)
    .bytes()?;
  let rewritten = git_executor
    .cmd(&["diff-tree", "-r", "--raw", "-z", "--no-renames", old_tree, new_tree], repo_path)
    .bytes()?;
  let new_entries: HashMap<&[u8], (&[u8], &[u8])> = raw_new_entries(&rewritten).collect();
  Ok(
    raw_new_entries(&staged)
      .filter(|(path, staged_entry)| new_entries.get(path).is_some_and(|new_entry| new_entry != staged_entry))
      .map(|(path, _)| String::from_utf8_lossy(path).into_owned())
      .collect(),
  )
}

/// `(path, (mode, id))` of the new side of each entry of `--raw -z --no-renames` output
/// (`:<old mode> <new mode> <old id> <new id> <status>\0<path>\0`); a missing file has mode `000000` and the null id
fn raw_new_entries(output: &[u8]) -> impl Iterator<Item = (&[u8], (&[u8], &[u8]))> {
  let mut fields = output.split(|&b| b == b'\0');
  std::iter::from_fn(move || {
    let meta = fields.next()?.strip_prefix(b":")?;
    let path = fields.next()?;
    let mut parts = meta.split(|&b| b == b' ').skip(1);
    let new_mode = parts.next()?;
    let new_id = parts.nth(1)?;
    Some((path, (new_mode, new_id)))
  })
}
