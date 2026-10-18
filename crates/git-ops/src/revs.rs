//! Small rev and worktree facts shared by operations: the current worktree, HEAD and its branch, commits by name, the tips of a ref
//! namespace, ancestry and merge bases, the short form of a commit id and the check of a full one.

use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;

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

/// The top level of the working tree that holds a directory, its git directory and the one its worktrees share
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryDirs {
  /// Absolute
  pub top_level: String,
  /// `$GIT_DIR` of the worktree, absolute: its `config.worktree` lives there
  pub git_dir: String,
  /// `$GIT_COMMON_DIR`, absolute: the `config`, the object cache and the repository lock live there
  pub git_common_dir: String,
}

/// The working tree `directory` is in, `None` outside one (a bare repository, a `.git` directory, no repository): one
/// `rev-parse`
pub fn repository_dirs(git_executor: &GitCommandExecutor, directory: &str) -> Result<Option<RepositoryDirs>> {
  let (output, exit_code) = git_executor.execute_command_with_status(&["rev-parse", "--show-toplevel", "--path-format=absolute", "--git-dir", "--git-common-dir"], directory)?;
  let mut lines = output.lines().map(str::trim);
  let (0, Some(top_level), Some(git_dir), Some(git_common_dir)) = (exit_code, lines.next().filter(|line| !line.is_empty()), lines.next(), lines.next()) else {
    return Ok(None);
  };
  Ok(Some(RepositoryDirs {
    top_level: top_level.to_string(),
    git_dir: git_dir.to_string(),
    git_common_dir: git_common_dir.to_string(),
  }))
}

/// The branch HEAD is on, by its name under `refs/heads/` (never git's `heads/<name>` beside a tag of the same name),
/// also when it has no commits yet; `None` for a detached HEAD: one `symbolic-ref --quiet HEAD`, whose exit 1 means
/// detached, while any other failure is an error with git's message
pub fn current_branch(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Option<String>> {
  match git_executor.execute_command_with_status(&["symbolic-ref", "--quiet", "HEAD"], repo_path)? {
    (reference, 0) => Ok(reference.strip_prefix("refs/heads/").map(str::to_string)),
    (_, 1) => Ok(None),
    (stderr, exit_code) => Err(anyhow!("git symbolic-ref HEAD exited with {exit_code}: {stderr}")),
  }
}

/// HEAD is not on a branch where an operation needs one
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("Not on any branch (detached HEAD state)")]
pub struct DetachedHead;

/// The branch HEAD is on (`current_branch`); a detached HEAD is the error `DetachedHead`
pub fn checked_out_branch(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<String> {
  current_branch(git_executor, repo_path)?.ok_or_else(|| DetachedHead.into())
}

/// The commit HEAD is at and the branch it names (`None` when detached), from one `rev-parse`. `None` when HEAD names a
/// branch without commits (`git init`), which a caller reports as such (`NoBaseline::NoCommits`): telling it from an
/// error costs one `symbolic-ref`, only when `rev-parse` fails
pub fn read_head(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Option<(String, Option<String>)>> {
  let (output, exit_code) = git_executor.execute_command_with_status(&["rev-parse", "HEAD", "--symbolic-full-name", "HEAD"], repo_path)?;
  let mut lines = output.lines();
  if let (Some(id), 0) = (lines.next(), exit_code) {
    let branch = lines.next().and_then(|reference| reference.strip_prefix("refs/heads/")).map(str::to_string);
    return Ok(Some((id.to_string(), branch)));
  }
  // HEAD names a branch, yet no commit
  if matches!(current_branch(git_executor, repo_path), Ok(Some(_))) {
    return Ok(None);
  }
  Err(anyhow!("Cannot read HEAD: {output}"))
}

/// The abbreviated commit id used in messages and in the output of the agent CLI
pub fn short_id(id: &str) -> &str {
  id.get(..8).unwrap_or(id)
}

/// A full object id as git prints it: 40 (SHA-1) or 64 (SHA-256) lowercase hex digits, so an id never reads as an
/// option or a revision expression, and equals the id git prints for the same object
pub fn is_object_id(id: &str) -> bool {
  matches!(id.len(), 40 | 64) && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The full id of the commit `rev` names (a ref, a short id, `HEAD~1`, a tag of a commit), `None` when it names none
pub fn resolve_commit(git_executor: &GitCommandExecutor, repo_path: &str, rev: &str) -> Result<Option<String>> {
  peel(git_executor, repo_path, rev, "commit")
}

/// The full id of the tree `rev` names (a commit, a ref, `HEAD~1`, a tree), an error when it names none. No cache: a
/// caller caches by its own rules (`TreeIdCache`)
pub fn resolve_tree_id(git_executor: &GitCommandExecutor, repo_path: &str, rev: &str) -> Result<String> {
  peel(git_executor, repo_path, rev, "tree")?.ok_or_else(|| anyhow!("{rev} names no tree"))
}

/// The full id of the object of `kind` (`commit`, `tree`) that `rev` names, `None` when it names none: one
/// `rev-parse --verify --quiet`, `--end-of-options` so a `rev` never reads as an option
fn peel(git_executor: &GitCommandExecutor, repo_path: &str, rev: &str, kind: &str) -> Result<Option<String>> {
  let (id, exit_code) = git_executor.execute_command_with_status(&["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{rev}^{{{kind}}}")], repo_path)?;
  Ok((exit_code == 0).then_some(id))
}

/// Whether a fully qualified ref (`refs/heads/main`) exists: one `show-ref --verify --quiet`, a failure never logged
pub fn ref_exists(git_executor: &GitCommandExecutor, repo_path: &str, full_ref: &str) -> bool {
  git_executor
    .cmd(&["show-ref", "--verify", "--quiet", full_ref], repo_path)
    .status()
    .is_ok_and(|output| output.code == 0)
}

/// Full ref -> tip of the refs under each of `namespaces` (`refs/heads/<prefix>/`, `refs/remotes/origin/<prefix>/`),
/// from one `for-each-ref`; none without a namespace
pub fn ref_tips(git_executor: &GitCommandExecutor, repo_path: &str, namespaces: &[&str]) -> Result<HashMap<String, String>> {
  if namespaces.is_empty() {
    return Ok(HashMap::new());
  }
  let mut args = vec!["for-each-ref", "--format=%(refname)%00%(objectname)"];
  args.extend_from_slice(namespaces);
  let lines = git_executor.execute_command_lines(&args, repo_path)?;
  Ok(
    lines
      .iter()
      .filter_map(|line| line.split_once('\0'))
      .map(|(reference, tip)| (reference.to_string(), tip.to_string()))
      .collect(),
  )
}

/// The tips of `tips` (of `ref_tips`) under `namespace`, by the name after it
pub fn tips_under(tips: &HashMap<String, String>, namespace: &str) -> HashMap<String, String> {
  tips
    .iter()
    .filter_map(|(reference, tip)| Some((reference.strip_prefix(namespace)?.to_string(), tip.clone())))
    .collect()
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
