//! Unapply a virtual branch: its original commits dropped from HEAD (`git_ops::rewrite_plan`) and the virtual branch
//! moved to the unapplied namespace in one ref transaction, under the repository lock.

use crate::repository_lock::RewriteOptions;
use anyhow::{Result, anyhow};
use branch_integration::archive::{REF_RENAME_MUTEX, unique_branch_name};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_names::{BranchKind, qualified_branch_name, strip_branch_namespace};
use git_ops::ref_transaction::RefTransaction;
use git_ops::revs::{current_branch, ref_exists};
use git_ops::rewrite_plan::prepare_drop_commits;
use serde::{Deserialize, Serialize};
use std::sync::PoisonError;
use tracing::{debug, instrument};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UnapplyBranchParams {
  pub repository_path: String,
  pub branch_name: String,
  pub branch_prefix: String,
  pub original_commit_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UnapplyBranchResult {
  pub unapplied_branch_name: String,
  pub commits_removed: Vec<String>,
}

/// Name in the unapplied namespace for `from_branch`, with a numeric suffix if the plain name is taken
fn unapplied_branch_name(git_executor: &GitCommandExecutor, repo: &str, from_branch: &str, branch_prefix: &str) -> Result<String> {
  let simple_name = strip_branch_namespace(from_branch, branch_prefix, BranchKind::Virtual).ok_or_else(|| anyhow!("Could not extract simple name from branch: {from_branch}"))?;

  let base_unapplied_name = qualified_branch_name(branch_prefix, BranchKind::Unapplied, simple_name)?;
  unique_branch_name(git_executor, repo, &base_unapplied_name)
}

/// Core function to unapply a virtual branch: drops its original commits from HEAD (the checked out branch) and moves
/// the virtual branch to the unapplied namespace in one ref transaction, so either both happen or nothing does. Runs
/// under the repository lock.
#[instrument(skip(git_executor, options), fields(repo = %params.repository_path, branch = %params.branch_name))]
pub fn unapply_branch_core(git_executor: &GitCommandExecutor, params: UnapplyBranchParams, options: &RewriteOptions) -> Result<UnapplyBranchResult> {
  let UnapplyBranchParams {
    repository_path,
    branch_name,
    branch_prefix,
    original_commit_ids,
  } = params;

  // Safety check: only allow unapplying virtual branches
  let virtual_prefix = format!("{branch_prefix}/virtual/");
  if !branch_name.starts_with(&virtual_prefix) {
    return Err(anyhow!("Can only unapply virtual branches under the configured branch prefix"));
  }

  let _lock = options.lock(git_executor, &repository_path)?;
  // Verify branch exists
  if !ref_exists(git_executor, &repository_path, &format!("refs/heads/{branch_name}")) {
    return Err(anyhow!("Virtual branch does not exist: {branch_name}"));
  }

  // Check if we're currently on the virtual branch
  if current_branch(git_executor, &repository_path)?.as_deref() == Some(branch_name.as_str()) {
    return Err(anyhow!("Cannot unapply the currently checked out branch. Switch to another branch first."));
  }

  if original_commit_ids.is_empty() {
    return Err(anyhow!("No commits provided to unapply"));
  }
  // Fails on a commit that is not on the current branch, on conflicts and on staged changes that would be lost, before
  // anything changes
  let prepared = prepare_drop_commits(git_executor, &repository_path, &original_commit_ids).map_err(|e| anyhow!("Failed to drop commits from HEAD: {e}"))?;
  if let Err(moved) = options.ensure_head(git_executor, &repository_path, prepared.old_head()) {
    prepared.discard(git_executor, &repository_path);
    return Err(moved.into());
  }
  let virtual_ref = format!("refs/heads/{branch_name}");
  let virtual_tip = git_executor.execute_command(&["rev-parse", "--verify", &virtual_ref], &repository_path)?;

  let unapplied_branch_name = {
    let _guard = REF_RENAME_MUTEX.lock().unwrap_or_else(PoisonError::into_inner);
    let unapplied_branch_name = unapplied_branch_name(git_executor, &repository_path, &branch_name, &branch_prefix)?;
    let mut transaction = RefTransaction::new();
    transaction
      .create(&format!("refs/heads/{unapplied_branch_name}"), &virtual_tip)
      .delete(&virtual_ref, Some(&virtual_tip));
    prepared
      .apply(git_executor, &repository_path, transaction, &format!("unapply {branch_name}"))
      .map_err(|e| anyhow!("Failed to drop commits from HEAD: {e}"))?;
    unapplied_branch_name
  };
  debug!(unapplied_branch = %unapplied_branch_name, commits_dropped = original_commit_ids.len(), "Unapplied branch");

  Ok(UnapplyBranchResult {
    unapplied_branch_name,
    commits_removed: original_commit_ids,
  })
}
