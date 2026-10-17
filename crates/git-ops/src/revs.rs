//! Small rev and worktree facts shared by operations: the current worktree, ancestry and merge bases.

use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;

/// The id of the main worktree; a linked one is named by its directory under `$GIT_COMMON_DIR/worktrees`
const MAIN_WORKTREE: &str = "main";

/// The worktree a command runs in
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
  pub id: String,
  /// Absolute
  pub git_dir: String,
}

/// The worktree of `repo_path`: `main`, or the name of a linked worktree
pub fn current_worktree(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Worktree> {
  let lines = git_executor.execute_command_lines(&["rev-parse", "--path-format=absolute", "--git-dir", "--git-common-dir"], repo_path)?;
  let [git_dir, common_dir] = <[String; 2]>::try_from(lines).map_err(|lines| anyhow!("Unexpected rev-parse output: {lines:?}"))?;
  let id = if git_dir == common_dir {
    MAIN_WORKTREE.to_string()
  } else {
    git_dir.trim_end_matches('/').rsplit('/').next().unwrap_or(MAIN_WORKTREE).to_string()
  };
  Ok(Worktree { id, git_dir })
}

/// Whether `ancestor` is `descendant` or one of its ancestors. `merge-base --is-ancestor` exits with 1 for "no"; any
/// other failure (an object that is not there, a history git can't read) is an error with git's message, never a "no"
pub fn is_ancestor(git_executor: &GitCommandExecutor, repo_path: &str, ancestor: &str, descendant: &str) -> Result<bool> {
  Ok(merge_base_query(git_executor, repo_path, &["--is-ancestor"], &[ancestor, descendant])?.is_some())
}

/// The merge base of `a` and `b`, `None` when they have no common history: `merge-base` exits with 1 only then, and
/// any other failure is an error with git's message
pub fn merge_base(git_executor: &GitCommandExecutor, repo_path: &str, a: &str, b: &str) -> Result<Option<String>> {
  merge_base_query(git_executor, repo_path, &[], &[a, b])
}

/// The fork point of `commit` in the reflog of `tracking_ref` (`merge-base --fork-point`), `None` without a reflog or a
/// fork point (exit 1); a ref that is not there (exit 128) is an error with git's message
pub fn fork_point(git_executor: &GitCommandExecutor, repo_path: &str, tracking_ref: &str, commit: &str) -> Result<Option<String>> {
  merge_base_query(git_executor, repo_path, &["--fork-point"], &[tracking_ref, commit])
}

/// `merge-base <options> <revs>`: the output on exit 0, `None` on exit 1, an error with git's message on any other
fn merge_base_query(git_executor: &GitCommandExecutor, repo_path: &str, options: &[&str], revs: &[&str]) -> Result<Option<String>> {
  let mut args = vec!["merge-base"];
  args.extend_from_slice(options);
  args.push("--end-of-options");
  args.extend_from_slice(revs);
  match git_executor.execute_command_with_status(&args, repo_path)? {
    (output, 0) => Ok(Some(output)),
    (_, 1) => Ok(None),
    (stderr, exit_code) => Err(anyhow!("git {} exited with {exit_code}: {stderr}", args.join(" "))),
  }
}

#[cfg(test)]
#[path = "revs_test.rs"]
mod revs_test;
