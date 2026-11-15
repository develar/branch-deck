//! The files of Git LFS of a new lease slot (docs/design/git-operations.md, "Lease"): the checkout of the slot under the
//! clones lock of its pool writes their pointers (`git_ops::lease_slot::LfsFiles::Pointers`), since the smudge filter
//! would download their content while every get that adds a slot waits; `pull_lfs_files` downloads and writes it once
//! the lock is free.

use crate::remote_command::execute_remote_command;
use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;

/// `git lfs pull` in the worktree `worktree`, under the policy of every git that talks to a remote: the objects that the
/// store of the repository lacks downloaded, and every pointer of HEAD's files replaced by its content, the index
/// refreshed
pub fn pull_lfs_files(git_executor: &GitCommandExecutor, worktree: &str) -> Result<()> {
  execute_remote_command(git_executor, &["lfs", "pull"], worktree).with_context(|| format!("Cannot pull the files of Git LFS into {worktree}"))?;
  Ok(())
}
