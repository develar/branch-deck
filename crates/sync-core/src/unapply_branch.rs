use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::amend_operations::drop_commits_from_head;
use git_ops::model::{extract_branch_name_from_final, to_unapplied_branch_name};
use serde::{Deserialize, Serialize};
use std::sync::{LazyLock, Mutex};
use tracing::{debug, instrument, warn};

// Global mutex to prevent race conditions when creating unapplied directories
pub static UNAPPLY_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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
    return Err(anyhow!("The following commits are not found in HEAD: {:?}", missing_commits));
  }

  Ok(())
}

/// Rename a branch to an unapplied namespace, ensuring no collisions by appending a numeric suffix if needed.
/// Returns the final unapplied branch name.
fn move_branch_to_unapplied(git_executor: &GitCommandExecutor, repo: &str, from_branch: &str, branch_prefix: &str) -> Result<String> {
  let simple_name = extract_branch_name_from_final(from_branch, branch_prefix).ok_or_else(|| anyhow!("Could not extract simple name from branch: {}", from_branch))?;

  let base_unapplied_name = to_unapplied_branch_name(branch_prefix, &simple_name)?;
  let mut target = base_unapplied_name.clone();
  let mut suffix: u32 = 1;

  // Find an available name
  loop {
    let full_ref = format!("refs/heads/{target}");
    let exists = git_executor.execute_command(&["show-ref", "--verify", &full_ref], repo).is_ok();
    if !exists {
      break;
    }
    target = format!("{base_unapplied_name}-{suffix}");
    suffix += 1;
    if suffix > 1000 {
      warn!("move_branch_to_unapplied: excessive collisions for {}", base_unapplied_name);
      break;
    }
  }

  // Lock to prevent race conditions when multiple tasks try to create branches in same directory
  let _guard = UNAPPLY_MUTEX.lock().map_err(|e| anyhow!("Failed to acquire unapply mutex: {}", e))?;

  // Create directory structure for unapplied branches if needed
  // The unapplied path (e.g., user/unapplied) requires directory structure in .git/refs.
  if target.matches('/').count() > 0
    && let Some((parent_path, _)) = target.rsplit_once('/')
  {
    // Create temporary branch to establish the directory structure
    let temp_branch = format!("{parent_path}/.unapply-temp");

    // Try to create the temp branch (ignore errors if it already exists)
    let _ = git_executor.execute_command(&["branch", &temp_branch, "HEAD"], repo);

    // Immediately delete it (the directory structure remains in git)
    let _ = git_executor.execute_command(&["branch", "-D", &temp_branch], repo);
  }

  // Now rename the branch to its unapplied location
  git_executor.execute_command(&["branch", "-m", from_branch, &target], repo)?;
  Ok(target)
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
  let virtual_prefix = format!("{}/virtual/", branch_prefix);
  if !branch_name.starts_with(&virtual_prefix) {
    return Err(anyhow!("Can only unapply virtual branches under the configured branch prefix"));
  }

  // Verify branch exists
  let branch_ref = format!("refs/heads/{}", branch_name);
  let exists = git_executor.execute_command(&["show-ref", "--verify", &branch_ref], &repository_path).is_ok();
  if !exists {
    return Err(anyhow!("Virtual branch does not exist: {}", branch_name));
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
  let _new_head = drop_commits_from_head(git_executor, &repository_path, &original_commit_ids, baseline_branch).map_err(|e| anyhow!("Failed to drop commits from HEAD: {}", e))?;

  debug!(commits_dropped = original_commit_ids.len(), "Successfully dropped commits from HEAD");

  Ok(UnapplyBranchResult {
    unapplied_branch_name,
    commits_removed: original_commit_ids,
  })
}
