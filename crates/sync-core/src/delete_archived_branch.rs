//! The delete of an archived branch (`<prefix>/archived/…`) or of an unapplied one (`<prefix>/unapplied/…`): one
//! `for-each-ref` for its tip and its worktree, then one ref transaction with compare-and-swap on that tip; a branch
//! checked out in any worktree is refused.

use anyhow::{Result, anyhow, bail};
use domain_types::branch_names::{BranchKind, strip_branch_namespace};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::ref_transaction::RefTransaction;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DeleteArchivedBranchParams {
  pub repository_path: String,
  pub branch_name: String,
}

/// Deletes an archived branch (`<prefix>/archived/…`): one `for-each-ref` reads its tip and the worktree that has it
/// checked out, and one ref transaction deletes it only from that tip (`branch-deck: delete <branch>`). A branch
/// checked out in any worktree is refused, as `git branch -D` does: `update-ref` would delete it under the worktree
#[instrument(skip(git_executor), fields(repo = %params.repository_path, branch = %params.branch_name))]
pub fn delete_archived_branch_core(git_executor: &GitCommandExecutor, params: DeleteArchivedBranchParams, branch_prefix: &str) -> Result<()> {
  delete_kept_branch(git_executor, params, branch_prefix, BranchKind::Archived)
}

/// `delete_archived_branch_core` of an unapplied branch (`<prefix>/unapplied/…`): its commits are gone for good unless
/// a remote branch or another ref keeps them
#[instrument(skip(git_executor), fields(repo = %params.repository_path, branch = %params.branch_name))]
pub fn delete_unapplied_branch_core(git_executor: &GitCommandExecutor, params: DeleteArchivedBranchParams, branch_prefix: &str) -> Result<()> {
  delete_kept_branch(git_executor, params, branch_prefix, BranchKind::Unapplied)
}

/// The delete of a branch under `<branch_prefix>/<kind>/`
fn delete_kept_branch(git_executor: &GitCommandExecutor, params: DeleteArchivedBranchParams, branch_prefix: &str, kind: BranchKind) -> Result<()> {
  let DeleteArchivedBranchParams { repository_path, branch_name } = params;

  // Safety checks: only allow deleting refs under <prefix>/<kind>/
  if strip_branch_namespace(&branch_name, branch_prefix, kind).is_none_or(str::is_empty) {
    bail!(
      "Can only delete {} branches under the configured branch prefix",
      if kind == BranchKind::Archived { "archived" } else { "unapplied" }
    );
  }

  if branch_name.starts_with('-') || branch_name.contains("..") || branch_name.contains('\n') || branch_name.contains('\r') {
    bail!("Invalid branch name");
  }

  let full_ref = format!("refs/heads/{branch_name}");
  // a pattern also matches the refs below it: the ref itself is the line with its name
  let output = git_executor
    .cmd(&["for-each-ref", "--format=%(refname)%00%(objectname)%00%(worktreepath)", &full_ref], &repository_path)
    .bytes()?;
  let output = String::from_utf8_lossy(&output);
  let found = output.lines().find_map(|line| {
    let mut fields = line.splitn(3, '\0');
    (fields.next() == Some(full_ref.as_str())).then(|| (fields.next().unwrap_or_default(), fields.next().unwrap_or_default()))
  });
  let Some((tip, worktree)) = found else {
    bail!("Branch does not exist");
  };
  if !worktree.is_empty() {
    bail!("The branch is checked out at {worktree}: switch that worktree to another branch first");
  }

  let mut transaction = RefTransaction::new();
  transaction.delete(&full_ref, Some(tip));
  transaction
    .commit(git_executor, &repository_path, &format!("delete {branch_name}"))
    .map_err(|e| anyhow!("Failed to delete branch: {e:#}"))
}

#[cfg(test)]
#[path = "delete_archived_branch_test.rs"]
mod delete_archived_branch_test;
