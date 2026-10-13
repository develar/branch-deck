use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

/// Action to perform on a commit during rewriting
#[derive(Debug, Clone)]
pub enum RewriteAction {
  /// Keep the commit unchanged
  Keep,
  /// Skip (drop) this commit  
  Skip,
  /// Replace the commit's tree with the provided tree ID
  Modify(String),
}

use crate::cache::TreeIdCache;
use crate::cherry_pick::get_commit_parent;
use crate::commit_utils::{create_commit_with_metadata, get_commit_info, prefetch_commit_infos_map};
use crate::copy_commit::CopyCommitError;
use crate::merge_conflict::{ConflictContext, build_merge_conflict_info};
use crate::model::{BranchError, MergeConflictInfo};
use crate::reword_commits::{get_current_branch, update_branch_ref as update_ref_plumbing};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

/// Generic function to rewrite commit history with a transform function
/// This is the core rewriting logic used by both amend and drop operations.
/// Rewrites the checked out branch; fails if it moved meanwhile (e.g. a commit made in a terminal).
#[instrument(skip(git_executor, transform, cache))]
fn rewrite_commits<F>(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  start_commit: &str, // The commit to start rewriting from (exclusive)
  transform: F,
  cache: &TreeIdCache,
) -> Result<String, CopyCommitError>
where
  F: Fn(&str) -> Result<RewriteAction, CopyCommitError>,
{
  let current_branch = get_current_branch(git_executor, repo_path).map_err(CopyCommitError::Other)?;
  // the tip to rewrite, read once: the branch is moved only if it still points at it
  let original_head = git_executor.execute_command(&["rev-parse", "HEAD"], repo_path)?.trim().to_string();

  // Get all commits and their first parents from start to HEAD (excluding start)
  // Using --parents lets us avoid per-commit parent lookups.
  let range = format!("{start_commit}..{original_head}");
  let commits_with_parents_lines = git_executor
    .execute_command_lines(&["rev-list", "--first-parent", "--reverse", "--parents", &range], repo_path)
    .map_err(CopyCommitError::Other)?;

  // Parse lines like: "<commit> <parent> [other-parents...]" and keep only first parent
  let mut commits_to_process: Vec<(String, String)> = Vec::with_capacity(commits_with_parents_lines.len());
  for line in commits_with_parents_lines {
    let mut parts = line.split_whitespace();
    if let Some(commit) = parts.next() {
      // First parent (if any). In first-parent traversal, there should be at least one.
      if let Some(parent) = parts.next() {
        commits_to_process.push((commit.to_string(), parent.to_string()));
      } else {
        // Fallback: no parent listed (shouldn't happen for a range excluding the start). Skip safely.
        commits_to_process.push((commit.to_string(), String::new()));
      }
    }
  }

  // Prefetch commit info for all selected commits in one go to avoid per-commit git calls
  let commit_info_map = prefetch_commit_infos_map(git_executor, repo_path, &range).map_err(CopyCommitError::Other)?;

  if commits_to_process.is_empty() {
    // Nothing to rewrite
    return Ok(start_commit.to_string());
  }

  // Start rewriting from the start commit
  let mut current_parent = start_commit.to_string();

  // Track if any commits were changed to determine when conflict detection is needed
  let mut has_changes = false;

  // Process each commit
  for (commit, parent_of_commit) in &commits_to_process {
    let action = transform(commit)?;

    match action {
      RewriteAction::Skip => {
        // Skip this commit entirely
        has_changes = true;
      }

      RewriteAction::Keep => {
        // Recreate commit with new parent
        let commit_info = match commit_info_map.get(commit).cloned() {
          Some(ci) => ci,
          None => get_commit_info(git_executor, repo_path, commit)?,
        };

        let new_tree = if has_changes {
          // Need conflict detection: commits were skipped or modified upstream
          let base_tree = cache.get_tree_id(git_executor, repo_path, parent_of_commit)?;
          let ours_tree = cache.get_tree_id(git_executor, repo_path, &current_parent)?;
          let theirs_tree = cache.get_tree_id(git_executor, repo_path, commit)?;

          // Optimization: check if trees already match to avoid merge-tree
          if base_tree == ours_tree {
            // Parent tree matches current tree, just reuse commit tree
            theirs_tree
          } else if ours_tree == theirs_tree {
            // Reapplying a commit that results in the same tree; no-op merge
            ours_tree
          } else if theirs_tree == base_tree {
            // The commit being replayed changed nothing relative to base; keep our tree
            ours_tree
          } else {
            // Use merge-tree to compute the new tree
            let merge_base_arg = format!("--merge-base={base_tree}");
            let (merged_out, status) = git_executor
              .execute_command_with_status(&["merge-tree", "--write-tree", &merge_base_arg, &ours_tree, &theirs_tree], repo_path)
              .map_err(CopyCommitError::Other)?;

            if status == 1 {
              // Conflict detected
              return Err(CopyCommitError::BranchError(BranchError::Generic(format!(
                "Rewriting would create conflicts when replaying commit {}: {}",
                &commit[..commit.len().min(8)],
                commit_info.subject.trim()
              ))));
            } else if status != 0 {
              return Err(CopyCommitError::Other(anyhow!("git merge-tree failed while rewriting: {}", merged_out.trim())));
            }

            merged_out.trim().to_string()
          }
        } else {
          // Fast path: no commits were changed, just use the original tree
          // This is safe when we're just moving commits as-is with no modifications
          cache.get_tree_id(git_executor, repo_path, commit)?
        };

        current_parent =
          create_commit_with_metadata(git_executor, repo_path, &new_tree, Some(&current_parent), &commit_info, &commit_info.message).map_err(CopyCommitError::Other)?;
      }

      RewriteAction::Modify(new_tree) => {
        // Use the provided tree for this commit
        // Mark that we have changes since this commit was modified
        has_changes = true;

        let commit_info = match commit_info_map.get(commit).cloned() {
          Some(ci) => ci,
          None => get_commit_info(git_executor, repo_path, commit)?,
        };
        current_parent =
          create_commit_with_metadata(git_executor, repo_path, &new_tree, Some(&current_parent), &commit_info, &commit_info.message).map_err(CopyCommitError::Other)?;
      }
    }
  }

  // Update the main branch ref to the new tip
  // Capture previous HEAD tree before updating ref to determine if index needs refresh
  let prev_head_tree = git_executor.resolve_tree_id(repo_path, "HEAD").map_err(CopyCommitError::Other)?;

  update_ref_plumbing(git_executor, repo_path, &current_branch, &current_parent, Some(&original_head))
    .map_err(|e| CopyCommitError::Other(anyhow!("Branch {current_branch} changed while rewriting, sync and try again: {e:#}")))?;

  // Fast path: only refresh index if the HEAD tree actually changed
  let new_head_tree = cache.get_tree_id(git_executor, repo_path, &current_parent)?;
  if prev_head_tree != new_head_tree {
    // Reset index to match new HEAD, but preserve working directory
    let _ = git_executor.execute_command(&["reset", "--mixed", "-q", &current_parent], repo_path);
  }

  Ok(current_parent)
}

/// Drop specified commits from HEAD (the checked out branch) while preserving working directory changes
/// Uses the generic `rewrite_commits` function
#[instrument(skip(git_executor))]
pub fn drop_commits_from_head(git_executor: &GitCommandExecutor, repo_path: &str, commit_ids_to_drop: &[String]) -> Result<String, CopyCommitError> {
  if commit_ids_to_drop.is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No commits specified to drop")));
  }

  // Get all commits from HEAD to find the base (parent of oldest commit to drop)
  let all_commits = git_executor
    .execute_command_lines(&["rev-list", "--first-parent", "HEAD"], repo_path)
    .map_err(CopyCommitError::Other)?;

  // Build a fast lookup set for drop membership
  let drop_set: HashSet<&str> = commit_ids_to_drop.iter().map(std::string::String::as_str).collect();

  // Find the oldest commit to drop to determine where to start rewriting from
  let mut oldest_pos = None;
  for (pos, commit) in all_commits.iter().enumerate() {
    if drop_set.contains(commit.as_str()) {
      oldest_pos = Some(pos);
    }
  }

  let oldest_position = oldest_pos.ok_or_else(|| CopyCommitError::Other(anyhow!("None of the specified commits found in HEAD")))?;

  // Get the base commit (parent of the oldest commit to drop)
  let oldest_commit = &all_commits[oldest_position];
  let base_commit = get_commit_parent(git_executor, repo_path, oldest_commit)?
    .ok_or_else(|| CopyCommitError::Other(anyhow!("Cannot drop root commit {oldest_commit}: it has no parent to rewrite from")))?;

  // Create cache for tree lookups
  let cache = TreeIdCache::new();

  // Use the generic rewrite_commits function with a filter that skips commits to drop
  rewrite_commits(
    git_executor,
    repo_path,
    &base_commit,
    |commit| if drop_set.contains(commit) { Ok(RewriteAction::Skip) } else { Ok(RewriteAction::Keep) },
    &cache,
  )
}

/// Parameters for amending uncommitted changes to a specific commit in main branch
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AmendToCommitParams {
  pub original_commit_id: String,
  pub files: Vec<String>,
}

/// Result of amending operation
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AmendResult {
  pub amended_commit_id: String,
  pub rebased_to_commit: String,
}

/// Amend uncommitted changes to a specific commit in current branch history
/// Uses Git's built-in fixup and autosquash functionality with fail-fast conflict handling:
/// 1. git commit --fixup=<commit> (create fixup commit)
/// 2. git rebase --autosquash (automatically apply fixup, handle conflicts if they occur)
#[instrument(skip(git_executor), fields(original_commit = %params.original_commit_id))]
pub fn amend_to_commit_in_main(git_executor: &GitCommandExecutor, repo_path: &str, params: AmendToCommitParams) -> Result<AmendResult, CopyCommitError> {
  let AmendToCommitParams { original_commit_id, files } = params;

  // Step 1: Check if there are uncommitted changes
  let status_output = git_executor.execute_command(&["status", "--porcelain"], repo_path)?;

  if status_output.trim().is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No uncommitted changes to amend")));
  }

  // Step 1.5: Check if we're amending to HEAD - use direct amend for performance
  let current_head = git_executor.execute_command(&["rev-parse", "HEAD"], repo_path)?.trim().to_string();
  if current_head == original_commit_id {
    // Direct amend for HEAD - no fixup/rebase needed, much faster
    git_executor.execute_command(
      &[
        "commit",
        "-a", // Automatically stage modified and deleted files
        "--amend",
        "--no-edit",   // Keep the existing commit message
        "--no-verify", // Skip hooks for consistency with fixup approach
      ],
      repo_path,
    )?;

    debug!(commit_id = %original_commit_id, "amended HEAD commit directly");

    let final_commit = git_executor.execute_command(&["rev-parse", "HEAD"], repo_path)?;
    let final_commit = final_commit.trim().to_string();
    return Ok(AmendResult {
      amended_commit_id: final_commit.clone(),
      rebased_to_commit: final_commit,
    });
  }

  // Create cache for tree lookups
  let cache = TreeIdCache::new();

  // Prefer a fast object-only rewrite for linear histories; fall back to fixup+autosquash otherwise
  let is_linear = is_linear_range(git_executor, repo_path, &original_commit_id, "HEAD")?;
  if is_linear {
    return fast_amend_linear(git_executor, repo_path, &original_commit_id, &files, &cache);
  }

  // Fall back: fixup + autosquash rebase
  // Step 2: Create a fixup commit with the staged changes
  // This will automatically amend the changes to the target commit
  let fixup_arg = format!("--fixup={original_commit_id}");
  let commit_args = vec![
    "commit",
    "-a", // Automatically stage modified and deleted files
    &fixup_arg,
    "--no-verify", // Skip hooks for fixup commit
  ];

  // Create fixup commit (will preserve original commit's author automatically)
  git_executor.execute_command(&commit_args, repo_path)?;

  debug!(commit_id = %original_commit_id, "created fixup commit");

  // Steps 3-4: squash the fixup into the original commit; on any failure undo the fixup commit
  if let Err(e) = autosquash_fixup(git_executor, repo_path, &original_commit_id) {
    restore_after_failed_autosquash(git_executor, repo_path, &current_head);
    return Err(e);
  }

  // Step 5: Get the final commit ID that HEAD points to
  let final_commit = git_executor.execute_command(&["rev-parse", "HEAD"], repo_path)?;
  let final_commit = final_commit.trim().to_string();

  // Step 6: Return the HEAD commit as the amended commit
  // since autosquash has properly integrated our changes
  let amended_commit_id = final_commit.clone();

  Ok(AmendResult {
    amended_commit_id,
    rebased_to_commit: final_commit,
  })
}

/// Rebase with `--autosquash` so the fixup commit on HEAD is squashed into `original_commit_id`.
/// On a conflict, the conflict details are extracted while the rebase is still in progress; the caller cleans up.
#[instrument(skip(git_executor))]
fn autosquash_fixup(git_executor: &GitCommandExecutor, repo_path: &str, original_commit_id: &str) -> Result<(), CopyCommitError> {
  // Rebase from the parent of the original commit; a root commit rebases everything with --root
  let rebase_base = get_commit_parent(git_executor, repo_path, original_commit_id)?;
  let rebase_args = ["rebase", "--autosquash", rebase_base.as_deref().unwrap_or("--root")];

  let error_msg = match git_executor.execute_command(&rebase_args, repo_path) {
    Ok(output) => {
      debug!(output = %output, "autosquash rebase completed successfully");
      return Ok(());
    }
    Err(e) => e.to_string(),
  };

  if !(error_msg.contains("conflict") || error_msg.contains("CONFLICT")) {
    return Err(CopyCommitError::Other(anyhow!("Rebase failed: {error_msg}")));
  }

  match extract_amend_conflict_info(git_executor, repo_path, original_commit_id) {
    Ok(conflict_info) => Err(CopyCommitError::BranchError(BranchError::MergeConflict(Box::new(conflict_info)))),
    Err(extract_err) => {
      debug!(error = %extract_err, "failed to extract conflict details");
      Err(CopyCommitError::Other(anyhow!(
        "Rebase conflicts detected when amending commit {original_commit_id}. Conflict analysis failed: {extract_err}"
      )))
    }
  }
}

/// Undo a failed fixup + autosquash: abort the rebase if one is in progress and drop the fixup commit,
/// so the branch is back at `pre_amend_head` and the user's changes are in the working tree, unstaged
#[instrument(skip(git_executor))]
fn restore_after_failed_autosquash(git_executor: &GitCommandExecutor, repo_path: &str, pre_amend_head: &str) {
  // Exits non-zero when no rebase is in progress (e.g. the rebase failed to start), which is fine here
  if let Ok((output, exit_code)) = git_executor.execute_command_with_status(&["rebase", "--abort"], repo_path)
    && exit_code != 0
  {
    debug!(exit_code, output = %output, "rebase --abort did not run");
  }
  if let Err(e) = git_executor.execute_command(&["reset", "--mixed", "-q", pre_amend_head], repo_path) {
    tracing::warn!(error = %e, pre_amend_head, "failed to restore HEAD after a failed amend");
  }
}

/// Extract detailed conflict information when amend rebase fails
/// Uses efficient git commands to get conflict data before aborting rebase
#[instrument(skip(git_executor), fields(original_commit = %original_commit_id))]
fn extract_amend_conflict_info(git_executor: &GitCommandExecutor, repo_path: &str, original_commit_id: &str) -> Result<MergeConflictInfo, CopyCommitError> {
  // Step 1: Detect conflicted files (UU, AA, DD, ...) from NUL-separated status entries `XY path`
  let status_output = git_executor.execute_command_raw(&["status", "--porcelain=v1", "-z"], repo_path)?;
  let mut conflict_paths: BTreeSet<PathBuf> = BTreeSet::new();

  let mut entries = status_output.split('\0');
  while let Some(entry) = entries.next() {
    let (Some(status), Some(path)) = (entry.get(..2), entry.get(3..)) else {
      continue;
    };
    // Rename and copy entries are followed by the original path as a separate field
    if status.contains(['R', 'C']) {
      entries.next();
    }
    if status.contains('U') || status == "AA" || status == "DD" {
      conflict_paths.insert(PathBuf::from(path));
    }
  }

  if conflict_paths.is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No conflicts found in rebase state")));
  }
  let conflict_files: Vec<PathBuf> = conflict_paths.into_iter().collect();

  // Step 2: Current HEAD during the rebase (the commit being replayed onto)
  let current_head = git_executor.execute_command(&["rev-parse", "HEAD"], repo_path)?;

  // Step 3: Build the conflict report; in the amend case HEAD is the merge tree, the target and the applied commit
  build_merge_conflict_info(ConflictContext {
    git_executor,
    repo_path,
    conflict_files: &conflict_files,
    merge_tree_oid: &current_head,
    parent_commit_id: original_commit_id,
    target_commit_id: &current_head,
    cherry_commit_id: &current_head,
  })
}

/// Determine if the range from `from` (exclusive) to `to` (inclusive) is linear on first-parent (no merges)
#[instrument(skip(git_executor))]
fn is_linear_range(git_executor: &GitCommandExecutor, repo_path: &str, from: &str, to: &str) -> Result<bool, CopyCommitError> {
  let range = format!("{from}..{to}");
  // If there is any merge commit on first-parent in the range, it's not linear.
  let out = git_executor
    .execute_command(&["rev-list", "--first-parent", "--merges", &range, "-n", "1"], repo_path)
    .map_err(CopyCommitError::Other)?;
  Ok(out.trim().is_empty())
}

/// Temporary index file in its own temporary directory, removed with the directory on drop.
/// The file itself is not created up front: git rejects an empty index file, and `read-tree` creates it.
struct TempIndex {
  _dir: tempfile::TempDir,
  path: String,
}

impl TempIndex {
  fn new() -> Result<Self, CopyCommitError> {
    let dir = tempfile::Builder::new()
      .prefix("branchdeck-amend-")
      .tempdir()
      .map_err(|e| CopyCommitError::Other(anyhow!("Failed to create temporary index directory: {e}")))?;
    let path = dir.path().join("index");
    let path = path
      .to_str()
      .ok_or_else(|| CopyCommitError::Other(anyhow!("Temporary index path is not valid UTF-8: {}", path.display())))?
      .to_string();
    Ok(Self { _dir: dir, path })
  }

  /// Environment that points git at this index
  fn env(&self) -> [(&str, &str); 1] {
    [("GIT_INDEX_FILE", self.path.as_str())]
  }
}

/// Compute the amended tree for a commit by applying working changes to it
#[instrument(skip(git_executor, cache))]
fn compute_amended_tree(git_executor: &GitCommandExecutor, repo_path: &str, original_commit_id: &str, files: &[String], cache: &TreeIdCache) -> Result<String, CopyCommitError> {
  // Validate files list early
  if files.is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No files specified to amend")));
  }

  // Get the original commit's tree
  let original_tree = cache.get_tree_id(git_executor, repo_path, original_commit_id)?;

  // Create temporary index file with RAII cleanup
  let tmp_idx = TempIndex::new()?;

  // Start with the original commit's tree in the temporary index
  git_executor
    .execute_command_with_env(&["read-tree", &original_tree], repo_path, &tmp_idx.env())
    .map_err(CopyCommitError::Other)?;

  // Use git update-index with --stdin for optimal performance and correctness (handles deletions)
  if files.len() == 1 {
    // Single file - include --remove to handle deletions without extra checks
    git_executor
      .execute_command_with_env(&["update-index", "--add", "--remove", &files[0]], repo_path, &tmp_idx.env())
      .map_err(|e| CopyCommitError::Other(anyhow!("Failed to update index with working changes: {e}")))?;
  } else {
    // Multiple files - use batch processing with NUL delimiters for safety and speed
    let mut input = String::new();
    for f in files {
      input.push_str(f);
      input.push('\0');
    }
    git_executor
      .execute_command_with_env_and_stdin(&["update-index", "--add", "--remove", "-z", "--stdin"], repo_path, &tmp_idx.env(), &input)
      .map_err(|e| CopyCommitError::Other(anyhow!("Failed to update index with working changes: {e}")))?;
  }

  // Write the amended tree
  let amended_tree = git_executor
    .execute_command_with_env(&["write-tree"], repo_path, &tmp_idx.env())
    .map_err(CopyCommitError::Other)?
    .trim()
    .to_string();

  Ok(amended_tree)
}

/// Fast amend path for linear histories using object-level rewrite (no rebase, no checkout)
#[instrument(skip(git_executor, cache))]
fn fast_amend_linear(git_executor: &GitCommandExecutor, repo_path: &str, original_commit_id: &str, files: &[String], cache: &TreeIdCache) -> Result<AmendResult, CopyCommitError> {
  // Get the current branch to use for ref updates
  let current_branch = get_current_branch(git_executor, repo_path).map_err(CopyCommitError::Other)?;

  // Compute the amended tree for the original commit
  let amended_tree = compute_amended_tree(git_executor, repo_path, original_commit_id, files, cache)?;

  let Some(parent) = get_commit_parent(git_executor, repo_path, original_commit_id)? else {
    return amend_root_commit(git_executor, repo_path, original_commit_id, &amended_tree, &current_branch, cache);
  };

  // Rewrite history from the parent, replacing the tree of the target commit
  let new_head = rewrite_commits(
    git_executor,
    repo_path,
    &parent,
    |commit| {
      if commit == original_commit_id {
        Ok(RewriteAction::Modify(amended_tree.clone()))
      } else {
        Ok(RewriteAction::Keep)
      }
    },
    cache,
  )?;

  // Find the amended commit ID in the rewritten history
  // The amended commit should be the first one after parent
  let range = format!("{parent}..{new_head}");
  let commits = git_executor
    .execute_command_lines(&["rev-list", "--first-parent", "--reverse", &range], repo_path)
    .map_err(CopyCommitError::Other)?;

  let amended_commit_id = commits.first().ok_or_else(|| CopyCommitError::Other(anyhow!("No commits found after rewriting")))?.clone();

  Ok(AmendResult {
    amended_commit_id,
    rebased_to_commit: new_head,
  })
}

/// Amend a root commit: `rewrite_commits` needs a parent to start from, so recreate the root and replay its descendants here
#[instrument(skip(git_executor, cache))]
fn amend_root_commit(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  original_commit_id: &str,
  amended_tree: &str,
  current_branch: &str,
  cache: &TreeIdCache,
) -> Result<AmendResult, CopyCommitError> {
  let original_commit = get_commit_info(git_executor, repo_path, original_commit_id)?;
  let amended_commit_id = create_commit_with_metadata(git_executor, repo_path, amended_tree, None, &original_commit, &original_commit.message)?;
  // the tip to rewrite, read once: the branch is moved only if it still points at it
  let original_head = git_executor.execute_command(&["rev-parse", "HEAD"], repo_path)?.trim().to_string();

  // Replay the descendants on top of the amended root, using prefetched commit info to avoid per-commit git calls
  let range = format!("{original_commit_id}..{original_head}");
  let descendants = git_executor
    .execute_command_lines(&["rev-list", "--first-parent", "--reverse", &range], repo_path)
    .map_err(CopyCommitError::Other)?;
  let info_map = if descendants.is_empty() {
    HashMap::new()
  } else {
    prefetch_commit_infos_map(git_executor, repo_path, &range)?
  };
  let mut current_parent = amended_commit_id.clone();
  for commit in descendants {
    let commit_info = match info_map.get(&commit) {
      Some(ci) => ci.clone(),
      None => get_commit_info(git_executor, repo_path, &commit)?,
    };
    let tree = cache.get_tree_id(git_executor, repo_path, &commit)?;
    current_parent = create_commit_with_metadata(git_executor, repo_path, &tree, Some(&current_parent), &commit_info, &commit_info.message)?;
  }

  // Update the branch ref to the new tip, refreshing the index only when the HEAD tree changed
  let prev_head_tree = git_executor.resolve_tree_id(repo_path, "HEAD").map_err(CopyCommitError::Other)?;
  update_ref_plumbing(git_executor, repo_path, current_branch, &current_parent, Some(&original_head))
    .map_err(|e| CopyCommitError::Other(anyhow!("Branch {current_branch} changed while amending, sync and try again: {e:#}")))?;
  let new_head_tree = cache.get_tree_id(git_executor, repo_path, &current_parent)?;
  if prev_head_tree != new_head_tree {
    let _ = git_executor.execute_command(&["reset", "--mixed", "-q", &current_parent], repo_path);
  }

  Ok(AmendResult {
    amended_commit_id,
    rebased_to_commit: current_parent,
  })
}
