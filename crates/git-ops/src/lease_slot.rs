//! The git of a lease slot (`bd lease`, spec/cli/lease.spec.md): a linked worktree of the repository that a pool owns.
//! A slot is pool property, not the user's checkout, so its working tree is written here, and only here, by name:
//! `checkout --force` moves a clean slot to the commit of a get, `reset --hard` and `clean -fd` discard the changes of a
//! return the agent confirmed. The worktree safety scan exempts exactly these argument lists in this file
//! (`worktree_safety_test.rs`, "Lease" in docs/design/git-operations.md).

use crate::ref_transaction::RefTransaction;
use crate::revs::{is_object_id, read_head};
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use std::path::{Path, PathBuf};

/// The git config key that moves the pool of a repository away from its default directory
pub const LEASE_POOL_KEY: &str = "branchdeck.leasePool";

/// A linked worktree, as its `.git` file and its admin directory name it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedWorktree {
  /// The worktree id: the name of its directory under `$GIT_COMMON_DIR/worktrees`
  pub id: String,
  /// `$GIT_COMMON_DIR`, canonical
  pub git_common_dir: PathBuf,
}

/// The linked worktree whose top level is `dir`, without a git process: `dir/.git` is a file `gitdir: <admin dir>`, the
/// admin dir names the common dir in `commondir` and points back at `dir/.git` in `gitdir`. `None` for anything else: a
/// missing directory, the main worktree, a submodule, a broken link
pub fn linked_worktree(dir: &Path) -> Option<LinkedWorktree> {
  let dot_git = dir.join(".git");
  let text = std::fs::read_to_string(&dot_git).ok()?;
  let admin = text.lines().next()?.strip_prefix("gitdir:")?.trim();
  let admin = std::fs::canonicalize(dir.join(admin)).ok()?;
  let common = std::fs::read_to_string(admin.join("commondir")).ok()?;
  let git_common_dir = std::fs::canonicalize(admin.join(common.trim())).ok()?;
  if admin.parent()? != git_common_dir.join("worktrees") {
    return None;
  }
  let back = std::fs::read_to_string(admin.join("gitdir")).ok()?;
  let back = std::fs::canonicalize(admin.join(back.trim())).ok()?;
  if back != std::fs::canonicalize(&dot_git).ok()? {
    return None;
  }
  Some(LinkedWorktree {
    id: admin.file_name()?.to_string_lossy().into_owned(),
    git_common_dir,
  })
}

/// The paths of the local changes of `slot`: staged, unstaged and untracked files, not the ignored ones. One
/// `status --porcelain=v1 -z --untracked-files=normal`; the repository's own `core.fsmonitor` and
/// `core.untrackedCache` apply, so a large slot answers in seconds
pub fn slot_changes(git_executor: &GitCommandExecutor, slot: &str) -> Result<Vec<String>> {
  let output = git_executor.cmd(&["status", "--porcelain=v1", "-z", "--untracked-files=normal"], slot).bytes()?;
  let mut paths = Vec::new();
  let mut entries = output.split(|&byte| byte == 0).filter(|entry| !entry.is_empty());
  while let Some(entry) = entries.next() {
    let Some(path) = entry.get(3..) else { continue };
    paths.push(String::from_utf8_lossy(path).into_owned());
    // a rename or a copy names its source in the next entry
    if matches!(entry.first(), Some(b'R' | b'C')) {
      entries.next();
    }
  }
  Ok(paths)
}

/// Move the clean `slot` to `commit`, detached, or on the new `branch` created there. Only the paths that differ between
/// the two commits are written. `--force` is safe after `slot_changes` found nothing: a case-insensitive file system
/// hides a case-only rename from `status`, and a checkout without it then refuses. A `branch` that exists fails
pub fn checkout_slot(git_executor: &GitCommandExecutor, slot: &str, commit: &str, branch: Option<&str>) -> Result<()> {
  match branch {
    Some(branch) => git_executor.execute_command(&["checkout", "--quiet", "--force", "-b", branch, commit], slot)?,
    None => git_executor.execute_command(&["checkout", "--quiet", "--force", "--detach", commit], slot)?,
  };
  Ok(())
}

/// A new slot at `path`, detached at `commit` or on the new `branch` created there: the only full checkout of the pool.
/// `worktree prune` first, best effort, so the admin directory of a slot deleted by hand frees its name
pub fn add_slot(git_executor: &GitCommandExecutor, repository: &str, path: &Path, commit: &str, branch: Option<&str>) -> Result<()> {
  let _ = git_executor.cmd(&["worktree", "prune"], repository).status();
  let path = path.to_str().ok_or_else(|| anyhow!("The slot path is not UTF-8: {}", path.display()))?;
  match branch {
    Some(branch) => git_executor.execute_command(&["worktree", "add", "--quiet", "-b", branch, path, commit], repository)?,
    None => git_executor.execute_command(&["worktree", "add", "--quiet", "--detach", path, commit], repository)?,
  };
  Ok(())
}

/// A commit of HEAD that no branch, tag or remote-tracking ref holds, with its subject
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlandedCommit {
  pub id: String,
  pub subject: String,
}

/// Up to `limit` commits of the HEAD of `slot` that no branch, tag or remote-tracking ref holds and that `base` (the
/// commit the slot was leased at) does not have, newest first: one `log HEAD --not --branches --tags --remotes <base>`.
/// Not `--all`, which holds the HEAD of every worktree, this one too. `base` keeps out the commits the slot started
/// with, which a rewrite of master may have left unreachable
pub fn unlanded_commits(git_executor: &GitCommandExecutor, slot: &str, base: Option<&str>, limit: usize) -> Result<Vec<UnlandedCommit>> {
  let max_count = format!("--max-count={limit}");
  let mut args = vec!["log", "-z", "--format=%H %s", &max_count, "HEAD", "--not", "--branches", "--tags", "--remotes"];
  // an id from the state file, never an option
  args.extend(base.filter(|base| is_object_id(base)));
  let output = git_executor.cmd(&args, slot).bytes()?;
  Ok(
    output
      .split(|&byte| byte == 0)
      .filter(|record| !record.is_empty())
      .map(|record| {
        let record = String::from_utf8_lossy(record);
        let (id, subject) = record.split_once(' ').unwrap_or((&record, ""));
        UnlandedCommit {
          id: id.to_string(),
          subject: subject.to_string(),
        }
      })
      .collect(),
  )
}

/// Discard the local changes of `slot` that its holder confirmed: `reset --hard HEAD`, then `clean -fd`, which removes
/// the untracked files and keeps the ignored ones (build outputs keep the next lease warm)
pub fn discard_slot_changes(git_executor: &GitCommandExecutor, slot: &str) -> Result<()> {
  git_executor.execute_command(&["reset", "--quiet", "--hard", "HEAD"], slot)?;
  git_executor.execute_command(&["clean", "-fd", "--quiet"], slot)?;
  Ok(())
}

/// Detach the HEAD of `slot` at its commit when it is on a branch, which survives; no file changes. One `rev-parse`,
/// and a `RefTransaction` with `option no-deref` when attached
pub fn detach_slot_head(git_executor: &GitCommandExecutor, slot: &str) -> Result<()> {
  let Some((head, branch)) = read_head(git_executor, slot)? else {
    bail!("The slot {slot} has no commit");
  };
  if branch.is_some() {
    RefTransaction::new()
      .update_no_deref("HEAD", &head, Some(&head))
      .commit(git_executor, slot, "lease return")?;
  }
  Ok(())
}

/// The paths of `paths` (relative to `repository`, `/`-separated) that git ignores there and does not track: one
/// `check-ignore -z --stdin`, which exits with 1 when it ignores none
pub fn ignored_paths(git_executor: &GitCommandExecutor, repository: &str, paths: &[String]) -> Result<Vec<String>> {
  if paths.is_empty() {
    return Ok(Vec::new());
  }
  let mut input = Vec::new();
  for path in paths {
    input.extend_from_slice(path.as_bytes());
    input.push(0);
  }
  let output = git_executor.cmd(&["check-ignore", "-z", "--stdin"], repository).stdin(&input).accept(&[1]).bytes()?;
  Ok(
    output
      .split(|&byte| byte == 0)
      .filter(|path| !path.is_empty())
      .map(|path| String::from_utf8_lossy(path).into_owned())
      .collect(),
  )
}

/// `branchdeck.leasePool` of the repository, `~` expanded; `None` when it is not set: one `config --get --path`, which
/// exits with 1 for a missing key
pub fn lease_pool_config(git_executor: &GitCommandExecutor, repository: &str) -> Result<Option<String>> {
  let output = git_executor.cmd(&["config", "--get", "--path", LEASE_POOL_KEY], repository).accept(&[1]).output()?;
  let value = output.into_text();
  Ok((!value.is_empty()).then_some(value))
}

/// Whether `name` is a valid new branch name: one `check-ref-format --branch`, which exits with 128 for an invalid one
pub fn is_valid_branch_name(git_executor: &GitCommandExecutor, repository: &str, name: &str) -> Result<bool> {
  if name.starts_with('-') {
    return Ok(false);
  }
  Ok(git_executor.cmd(&["check-ref-format", "--branch", name], repository).status()?.code == 0)
}
