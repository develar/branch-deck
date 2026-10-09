use anyhow::{Result, anyhow};
use branch_integration::archive::rename_branch_unique;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::amend_operations::drop_commits_from_head;
use git_ops::model::{BranchKind, qualified_branch_name, strip_branch_namespace};
use serde::{Deserialize, Serialize};
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

/// Validate that the provided original commit IDs exist in HEAD
fn validate_commits_in_head(git_executor: &GitCommandExecutor, repository_path: &str, original_commit_ids: &[String]) -> Result<()> {
  if original_commit_ids.is_empty() {
    return Err(anyhow!("No commits provided to unapply"));
  }

  // Get all commits from HEAD
  let all_commits = git_executor.execute_command_lines(&["rev-list", "--first-parent", "HEAD"], repository_path)?;

  // Check that all provided commits exist in HEAD
  let mut missing_commits = Vec::new();
  for commit_id in original_commit_ids {
    if !all_commits.contains(commit_id) {
      missing_commits.push(commit_id);
    }
  }

  if !missing_commits.is_empty() {
    return Err(anyhow!("The following commits are not found in HEAD: {missing_commits:?}"));
  }

  Ok(())
}

/// Rename a branch to an unapplied namespace, ensuring no collisions by appending a numeric suffix if needed.
/// Returns the final unapplied branch name.
fn move_branch_to_unapplied(git_executor: &GitCommandExecutor, repo: &str, from_branch: &str, branch_prefix: &str) -> Result<String> {
  let simple_name = strip_branch_namespace(from_branch, branch_prefix, BranchKind::Virtual).ok_or_else(|| anyhow!("Could not extract simple name from branch: {from_branch}"))?;

  let base_unapplied_name = qualified_branch_name(branch_prefix, BranchKind::Unapplied, simple_name)?;
  rename_branch_unique(git_executor, repo, from_branch, &base_unapplied_name)
}

/// Core function to unapply a virtual branch
/// 1. Validates the provided commit IDs exist in HEAD
/// 2. Moves the virtual branch to unapplied namespace  
/// 3. Drops the specified commits from HEAD
#[instrument(skip(git_executor), fields(repo = %params.repository_path, branch = %params.branch_name))]
pub fn unapply_branch_core(git_executor: &GitCommandExecutor, params: UnapplyBranchParams, baseline_branch: &str) -> Result<UnapplyBranchResult> {
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

  // Verify branch exists
  if !git_executor.ref_exists(&repository_path, &format!("refs/heads/{branch_name}")) {
    return Err(anyhow!("Virtual branch does not exist: {branch_name}"));
  }

  // Check if we're currently on the virtual branch
  let current_branch = git_executor.execute_command(&["rev-parse", "--abbrev-ref", "HEAD"], &repository_path)?;
  let current_branch = current_branch.trim();
  if current_branch == branch_name {
    return Err(anyhow!("Cannot unapply the currently checked out branch. Switch to another branch first."));
  }

  // Validate that all provided commit IDs exist in HEAD
  validate_commits_in_head(git_executor, &repository_path, &original_commit_ids)?;
  debug!(commits_count = original_commit_ids.len(), "Validated that all commits exist in HEAD");

  // Move the virtual branch to unapplied namespace
  let unapplied_branch_name = move_branch_to_unapplied(git_executor, &repository_path, &branch_name, &branch_prefix)?;
  debug!(unapplied_branch = %unapplied_branch_name, "Moved virtual branch to unapplied");

  // Drop the specified commits from HEAD
  let _new_head = drop_commits_from_head(git_executor, &repository_path, &original_commit_ids, baseline_branch).map_err(|e| anyhow!("Failed to drop commits from HEAD: {e}"))?;

  debug!(commits_dropped = original_commit_ids.len(), "Successfully dropped commits from HEAD");

  Ok(UnapplyBranchResult {
    unapplied_branch_name,
    commits_removed: original_commit_ids,
  })
}
