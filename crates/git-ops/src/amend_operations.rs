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
use crate::commit_utils::{create_commit_with_metadata, prefetch_commit_infos_map};
use crate::copy_commit::CopyCommitError;
use crate::merge_conflict::{ConflictDetailsParams, ConflictFileInfo, extract_conflict_details};
use crate::model::{BranchError, MergeConflictInfo};
use crate::reword_commits::{get_commit_info, update_branch_ref as update_ref_plumbing};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

/// Generic function to rewrite commit history with a transform function
/// This is the core rewriting logic used by both amend and drop operations
#[instrument(skip(git_executor, transform, cache))]
fn rewrite_commits<F>(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  start_commit: &str, // The commit to start rewriting from (exclusive)
  main_branch: &str,
  transform: F,
  cache: &TreeIdCache,
) -> Result<String, CopyCommitError>
where
  F: Fn(&str) -> Result<RewriteAction, CopyCommitError>,
{
  // Get all commits and their first parents from start to HEAD (excluding start)
  // Using --parents lets us avoid per-commit parent lookups.
  let range = format!("{start_commit}..HEAD");
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
        continue;
      }

      RewriteAction::Keep => {
        // Recreate commit with new parent
        let commit_info = match commit_info_map.get(commit).cloned() {
          Some(ci) => ci,
          None => get_commit_info(git_executor, repo_path, commit).map_err(CopyCommitError::Other)?,
        };

        let new_tree = if !has_changes {
          // Fast path: no commits were changed, just use the original tree
          // This is safe when we're just moving commits as-is with no modifications
          cache.get_tree_id(git_executor, repo_path, commit)?
        } else {
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
            let merge_base_arg = format!("--merge-base={}", base_tree);
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
          None => get_commit_info(git_executor, repo_path, commit).map_err(CopyCommitError::Other)?,
        };
        current_parent =
          create_commit_with_metadata(git_executor, repo_path, &new_tree, Some(&current_parent), &commit_info, &commit_info.message).map_err(CopyCommitError::Other)?;
      }
    }
  }

  // Update the main branch ref to the new tip
  // Capture previous HEAD tree before updating ref to determine if index needs refresh
  let prev_head_tree = git_executor.resolve_tree_id(repo_path, "HEAD").map_err(CopyCommitError::Other)?;

  update_ref_plumbing(git_executor, repo_path, main_branch, &current_parent).map_err(CopyCommitError::Other)?;

  // Fast path: only refresh index if the HEAD tree actually changed
  let new_head_tree = cache.get_tree_id(git_executor, repo_path, &current_parent)?;
  if prev_head_tree != new_head_tree {
    // If current branch equals main_branch, refresh the index to match HEAD without touching worktree
    let current_branch_result = git_executor.execute_command(&["symbolic-ref", "--short", "HEAD"], repo_path);
    if let Ok(current_branch) = current_branch_result {
      let current_branch = current_branch.trim();
      if current_branch == main_branch {
        // Reset index to match new HEAD, but preserve working directory
        let _ = git_executor.execute_command(&["reset", "--mixed", "-q", &current_parent], repo_path);
      }
    }
  }

  Ok(current_parent)
}

// prefetch_commit_infos_map is defined in commit_utils

/// Drop specified commits from HEAD while preserving working directory changes
/// Uses the generic rewrite_commits function
#[instrument(skip(git_executor))]
pub fn drop_commits_from_head(git_executor: &GitCommandExecutor, repo_path: &str, commit_ids_to_drop: &[String], main_branch: &str) -> Result<String, CopyCommitError> {
  if commit_ids_to_drop.is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No commits specified to drop")));
  }

  // Get all commits from HEAD to find the base (parent of oldest commit to drop)
  let all_commits = git_executor
    .execute_command_lines(&["rev-list", "--first-parent", "HEAD"], repo_path)
    .map_err(CopyCommitError::Other)?;

  // Build a fast lookup set for drop membership
  let drop_set: HashSet<&str> = commit_ids_to_drop.iter().map(|s| s.as_str()).collect();

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
  let base_commit =
    get_commit_parent(git_executor, repo_path, oldest_commit).map_err(|e| CopyCommitError::Other(anyhow!("Failed to get parent of oldest commit to drop: {}", e)))?;

  // Create cache for tree lookups
  let cache = TreeIdCache::new();

  // Use the generic rewrite_commits function with a filter that skips commits to drop
  rewrite_commits(
    git_executor,
    repo_path,
    &base_commit,
    main_branch,
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
  let fixup_arg = format!("--fixup={}", original_commit_id);
  let commit_args = vec![
    "commit",
    "-a", // Automatically stage modified and deleted files
    &fixup_arg,
    "--no-verify", // Skip hooks for fixup commit
  ];

  // Create fixup commit (will preserve original commit's author automatically)
  git_executor.execute_command(&commit_args, repo_path)?;

  debug!(commit_id = %original_commit_id, "created fixup commit");

  // Step 3: Find the base for rebase (parent of original commit or handle root commit)
  let base_ref = format!("{}^", original_commit_id);
  let base_result = git_executor.execute_command(&["rev-parse", &base_ref], repo_path);

  let rebase_base = match base_result {
    Ok(parent) => parent.trim().to_string(),
    Err(_) => {
      // Root commit - use --root flag for rebase
      debug!("original commit appears to be root commit");
      String::new() // Empty string indicates root commit
    }
  };

  // Step 4: Perform rebase with autosquash to apply the fixup
  let mut rebase_args = vec!["rebase", "--autosquash"];

  if rebase_base.is_empty() {
    // Root commit case - rebase everything
    rebase_args.push("--root");
  } else {
    rebase_args.push(&rebase_base);
  }

  let rebase_result = git_executor.execute_command(&rebase_args, repo_path);

  match rebase_result {
    Ok(output) => {
      debug!(output = %output, "autosquash rebase completed successfully");
    }
    Err(e) => {
      // Rebase failed - check if it's due to conflicts
      let error_msg = e.to_string();
      if error_msg.contains("conflict") || error_msg.contains("CONFLICT") {
        // Extract detailed conflict information before aborting rebase
        match extract_amend_conflict_info(git_executor, repo_path, &original_commit_id) {
          Ok(conflict_info) => {
            // Clean up rebase state after extracting conflict data
            let _ = git_executor.execute_command(&["rebase", "--abort"], repo_path);
            return Err(CopyCommitError::BranchError(BranchError::MergeConflict(Box::new(conflict_info))));
          }
          Err(extract_err) => {
            // Failed to extract conflict info - fallback to abort and generic error
            let _ = git_executor.execute_command(&["rebase", "--abort"], repo_path);
            debug!(error = %extract_err, "failed to extract conflict details");
            return Err(CopyCommitError::Other(anyhow!(
              "Rebase conflicts detected when amending commit {}. Conflict analysis failed: {}",
              original_commit_id,
              extract_err
            )));
          }
        }
      } else {
        // Some other rebase error - propagate it
        return Err(CopyCommitError::Other(anyhow!("Rebase failed: {}", error_msg)));
      }
    }
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

/// Check if amending to the given commit would create conflicts
/// Uses conflict analysis and git merge-tree to detect actual conflicts
#[instrument(skip(git_executor), fields(original_commit = %original_commit_id))]
pub fn check_amend_conflicts(git_executor: &GitCommandExecutor, repo_path: &str, main_branch: &str, original_commit_id: &str) -> Result<()> {
  // Get commits between original commit and main branch HEAD
  let range = format!("{}..{}", original_commit_id, main_branch);
  let commits_output = git_executor.execute_command(&["rev-list", "--first-parent", "--reverse", &range], repo_path)?;

  if commits_output.trim().is_empty() {
    // No commits between - safe to amend
    debug!("no commits between original and HEAD - safe to amend");
    return Ok(());
  }

  // Parse the commits that will be affected by rebase (hashes only)
  let affected_commits: Vec<&str> = commits_output.lines().map(|line| line.trim()).filter(|line| !line.is_empty()).collect();

  debug!(count = affected_commits.len(), "found commits that will be rebased");

  // Create a temporary tree from index to simulate the amended commit
  // Note: write-tree uses the index (staged + -a auto-stage will be reflected during amend)
  let working_tree = git_executor.execute_command(&["write-tree"], repo_path)?;
  let working_tree = working_tree.trim();

  // For each affected commit, check if it would conflict with the amended version
  for commit_hash in affected_commits.iter().copied() {
    debug!(commit = %commit_hash, "checking for conflicts");

    // Use git merge-tree to check if this commit would conflict
    // when rebased onto the amended version
    // Note: git merge-tree syntax is: git merge-tree --write-tree --merge-base=<base> <branch1> <branch2>
    let merge_base_arg = format!("--merge-base={}", original_commit_id);
    let merge_tree_args = vec![
      "merge-tree",
      "--write-tree",
      &merge_base_arg,
      &working_tree, // our amended version
      commit_hash,   // the commit being rebased
    ];

    // Use status-aware execution: exit code 1 from merge-tree means conflicts
    let (output_or_stderr, exit_code) = git_executor.execute_command_with_status(&merge_tree_args, repo_path)?;

    if exit_code == 1 {
      // Conflicts detected for this commit
      let commit_subject = git_executor
        .execute_command(&["log", "-1", "--format=%s", commit_hash], repo_path)
        .unwrap_or_default()
        .trim()
        .to_string();
      let short_hash = if commit_hash.len() >= 8 { &commit_hash[..8] } else { commit_hash };
      return Err(anyhow!(
        "Amending would create conflicts with commit {} ({}). {} other commit(s) would also be affected by the rebase.",
        short_hash,
        commit_subject,
        affected_commits.len() - 1
      ));
    } else if exit_code != 0 {
      // Some other failure: be conservative and report inability to guarantee safety
      let commit_subject = git_executor
        .execute_command(&["log", "-1", "--format=%s", commit_hash], repo_path)
        .unwrap_or_default()
        .trim()
        .to_string();
      let short_hash = if commit_hash.len() >= 8 { &commit_hash[..8] } else { commit_hash };
      debug!(exit_code, output = %output_or_stderr, "merge-tree returned unexpected status");
      return Err(anyhow!(
        "Cannot safely amend: commit {} ({}) may have conflicts. {} commit(s) would be rebased.",
        short_hash,
        commit_subject,
        affected_commits.len()
      ));
    }
  }

  debug!("all conflict checks passed - safe to proceed with amend");
  Ok(())
}

/// Extract detailed conflict information when amend rebase fails
/// Uses efficient git commands to get conflict data before aborting rebase
#[instrument(skip(git_executor), fields(original_commit = %original_commit_id))]
fn extract_amend_conflict_info(git_executor: &GitCommandExecutor, repo_path: &str, original_commit_id: &str) -> Result<MergeConflictInfo, CopyCommitError> {
  // Step 1: Use efficient git status to detect conflicted files
  let status_output = git_executor.execute_command(&["status", "--porcelain=v1"], repo_path)?;
  let mut conflict_files: HashMap<PathBuf, ConflictFileInfo> = HashMap::new();

  // Parse status output to find conflicted files (UU, AA, etc.)
  for line in status_output.lines() {
    let line = line.trim();
    if line.len() >= 3 {
      let status_chars = &line[..2];
      let file_path = &line[3..];

      // Check for conflict markers in status
      if status_chars.contains('U') || status_chars == "AA" || status_chars == "DD" {
        conflict_files.insert(
          PathBuf::from(file_path),
          ConflictFileInfo {
            path: PathBuf::from(file_path),
            base_oid: None,
            ours_oid: None,
            theirs_oid: None,
          },
        );
      }
    }
  }

  if conflict_files.is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No conflicts found in rebase state")));
  }

  // Step 2: Use efficient git ls-files to get staged file info with object IDs
  let ls_files_output = git_executor.execute_command(&["ls-files", "--stage"], repo_path)?;

  // Parse ls-files output to populate object IDs for conflict stages
  for line in ls_files_output.lines() {
    let line = line.trim();
    if let Some((prefix, file_path)) = line.split_once('\t') {
      let parts: Vec<&str> = prefix.split_whitespace().collect();
      if parts.len() >= 3 {
        let object_id = parts[1].to_string();
        let stage = parts[2];
        let path = PathBuf::from(file_path);

        if let Some(conflict_info) = conflict_files.get_mut(&path) {
          match stage {
            "1" => conflict_info.base_oid = Some(object_id),
            "2" => conflict_info.ours_oid = Some(object_id),
            "3" => conflict_info.theirs_oid = Some(object_id),
            _ => {}
          }
        }
      }
    }
  }

  // Step 3: Get current HEAD during rebase (this is the fixup commit)
  let current_head = git_executor.execute_command(&["rev-parse", "HEAD"], repo_path)?;
  let current_head = current_head.trim().to_string();

  // Step 4: Extract detailed conflict information using existing function
  let (detailed_conflicts, conflict_marker_commits) = extract_conflict_details(ConflictDetailsParams {
    git_executor,
    repo_path,
    conflict_files: &conflict_files,
    merge_tree_oid: &current_head, // Use current HEAD as merge tree reference
    parent_commit_id: original_commit_id,
    target_commit_id: &current_head,
    cherry_commit_id: &current_head, // In amend case, this is the same as target
  })?;

  // Step 5: Analyze conflicts to find missing commits (commits between original and current HEAD)
  let conflicting_paths: Vec<PathBuf> = conflict_files.keys().cloned().collect();
  let conflict_analysis = match crate::conflict_analysis::analyze_conflict(git_executor, repo_path, original_commit_id, &current_head, &conflicting_paths) {
    Ok(analysis) => analysis,
    Err(e) => {
      debug!(error = %e, "failed to analyze amend conflicts");
      // Create a default analysis if detailed analysis fails
      crate::conflict_analysis::ConflictAnalysis {
        missing_commits: vec![],
        merge_base_hash: String::new(),
        merge_base_subject: String::new(),
        merge_base_message: String::new(),
        merge_base_time: 0,
        merge_base_author: String::new(),
        divergence_summary: crate::conflict_analysis::DivergenceSummary {
          commits_ahead_in_source: 0,
          commits_ahead_in_target: 0,
          common_ancestor_distance: 0,
        },
      }
    }
  };

  // Step 6: Get commit information for all involved commits
  let original_commit_info = get_commit_info_for_conflict(git_executor, repo_path, original_commit_id)?;
  let current_head_info = get_commit_info_for_conflict(git_executor, repo_path, &current_head)?;

  // Step 7: Build complete MergeConflictInfo for the conflict viewer
  Ok(MergeConflictInfo {
    commit_message: current_head_info.message.clone(),
    commit_hash: current_head.clone(),
    commit_author_time: current_head_info.author_timestamp,
    commit_committer_time: current_head_info.committer_timestamp,
    original_parent_message: original_commit_info.message.clone(),
    original_parent_hash: original_commit_id.to_string(),
    original_parent_author_time: original_commit_info.author_timestamp,
    original_parent_committer_time: original_commit_info.committer_timestamp,
    target_branch_message: current_head_info.message.clone(),
    target_branch_hash: current_head.clone(),
    target_branch_author_time: current_head_info.author_timestamp,
    target_branch_committer_time: current_head_info.committer_timestamp,
    conflicting_files: detailed_conflicts,
    conflict_analysis,
    conflict_marker_commits,
  })
}

/// Get commit info for conflict reporting
#[instrument(skip(git_executor), fields(commit_id = %commit_id))]
fn get_commit_info_for_conflict(git_executor: &GitCommandExecutor, repo_path: &str, commit_id: &str) -> Result<crate::commit_list::Commit, CopyCommitError> {
  // Fetch subject and timestamps in a single git invocation for better performance
  // Format: %s<NULL>%at<NULL>%ct
  let format = "%s%x00%at%x00%ct";
  let output = git_executor.execute_command(&["log", "-1", &format!("--format={}", format), commit_id], repo_path)?;
  let mut parts = output.split('\0');
  let subject = parts.next().unwrap_or("").trim().to_string();
  let author_ts_str = parts.next().unwrap_or("").trim();
  let committer_ts_str = parts.next().unwrap_or("").trim();

  let author_timestamp: u32 = author_ts_str
    .parse()
    .map_err(|e| CopyCommitError::Other(anyhow!("Invalid author timestamp for commit {}: {}", commit_id, e)))?;
  let committer_timestamp: u32 = committer_ts_str
    .parse()
    .map_err(|e| CopyCommitError::Other(anyhow!("Invalid committer timestamp for commit {}: {}", commit_id, e)))?;

  Ok(crate::commit_list::Commit {
    id: commit_id.to_string(),
    subject: subject.clone(),
    message: subject.clone(),
    author_name: String::new(),
    author_email: String::new(),
    author_timestamp,
    committer_timestamp,
    parent_id: None,
    tree_id: String::new(),
    note: None,
    stripped_subject: subject,
    mapped_commit_id: None,
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

/// RAII guard for temporary index file cleanup
struct TempIndexGuard {
  path: PathBuf,
}

impl TempIndexGuard {
  fn new() -> Self {
    let tdir = std::env::temp_dir();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let path = tdir.join(format!("branchdeck_amend_{nanos}.idx"));
    Self { path }
  }

  fn path_str(&self) -> &str {
    // Safe because temp paths are valid UTF-8
    self.path.to_str().unwrap()
  }
}

impl Drop for TempIndexGuard {
  fn drop(&mut self) {
    let _ = fs::remove_file(&self.path);
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
  let tmp_idx = TempIndexGuard::new();

  // Start with the original commit's tree in the temporary index
  git_executor
    .execute_command_with_env(&["read-tree", &original_tree], repo_path, &[("GIT_INDEX_FILE", tmp_idx.path_str())])
    .map_err(CopyCommitError::Other)?;

  // Use git update-index with --stdin for optimal performance and correctness (handles deletions)
  if files.len() == 1 {
    // Single file - include --remove to handle deletions without extra checks
    git_executor
      .execute_command_with_env(&["update-index", "--add", "--remove", &files[0]], repo_path, &[("GIT_INDEX_FILE", tmp_idx.path_str())])
      .map_err(|e| CopyCommitError::Other(anyhow!("Failed to update index with working changes: {}", e)))?;
  } else {
    // Multiple files - use batch processing with NUL delimiters for safety and speed
    let mut input = String::new();
    for f in files {
      input.push_str(f);
      input.push('\0');
    }
    git_executor
      .execute_command_with_env_and_stdin(
        &["update-index", "--add", "--remove", "-z", "--stdin"],
        repo_path,
        &[("GIT_INDEX_FILE", tmp_idx.path_str())],
        &input,
      )
      .map_err(|e| CopyCommitError::Other(anyhow!("Failed to update index with working changes: {}", e)))?;
  }

  // Write the amended tree
  let amended_tree = git_executor
    .execute_command_with_env(&["write-tree"], repo_path, &[("GIT_INDEX_FILE", tmp_idx.path_str())])
    .map_err(CopyCommitError::Other)?
    .trim()
    .to_string();

  Ok(amended_tree)
}

/// Fast amend path for linear histories using object-level rewrite (no rebase, no checkout)
/// Now uses the generic rewrite_commits function
#[instrument(skip(git_executor, cache))]
fn fast_amend_linear(git_executor: &GitCommandExecutor, repo_path: &str, original_commit_id: &str, files: &[String], cache: &TreeIdCache) -> Result<AmendResult, CopyCommitError> {
  // Get the current branch to use for ref updates
  let current_branch = git_executor
    .execute_command(&["symbolic-ref", "--short", "HEAD"], repo_path)
    .map_err(CopyCommitError::Other)?
    .trim()
    .to_string();

  // Compute the amended tree for the original commit
  let amended_tree = compute_amended_tree(git_executor, repo_path, original_commit_id, files, cache)?;

  // Check if we're amending a root commit
  let is_root = get_commit_parent(git_executor, repo_path, original_commit_id).is_err();

  if is_root {
    // For root commits, we need to handle this specially since rewrite_commits expects a parent
    // First create the amended root commit
    let original_commit = get_commit_info(git_executor, repo_path, original_commit_id).map_err(CopyCommitError::Other)?;
    let amended_commit_id =
      create_commit_with_metadata(git_executor, repo_path, &amended_tree, None, &original_commit, &original_commit.message).map_err(CopyCommitError::Other)?;

    // Check if there are any descendants
    let range = format!("{}..HEAD", original_commit_id);
    let descendants = git_executor
      .execute_command_lines(&["rev-list", "--first-parent", "--reverse", &range], repo_path)
      .map_err(CopyCommitError::Other)?;

    if descendants.is_empty() {
      // No descendants, just update the branch
      let prev_head_tree = git_executor.resolve_tree_id(repo_path, "HEAD").map_err(CopyCommitError::Other)?;
      update_ref_plumbing(git_executor, repo_path, &current_branch, &amended_commit_id).map_err(CopyCommitError::Other)?;

      // Only refresh index when HEAD tree changed
      let new_head_tree = cache.get_tree_id(git_executor, repo_path, &amended_commit_id)?;
      if prev_head_tree != new_head_tree {
        let _ = git_executor.execute_command(&["reset", "--mixed", "-q", &amended_commit_id], repo_path);
      }

      return Ok(AmendResult {
        amended_commit_id: amended_commit_id.clone(),
        rebased_to_commit: amended_commit_id,
      });
    }

    // Has descendants - rewrite them on top of the amended root
    // Prefetch commit info for all descendants to avoid per-commit git calls
    let desc_range = format!("{}..HEAD", original_commit_id);
    let info_map = prefetch_commit_infos_map(git_executor, repo_path, &desc_range)?;
    let mut current_parent = amended_commit_id.clone();
    for commit in descendants {
      // Keep each descendant by rewriting it with the new parent
      let commit_info = match info_map.get(&commit).cloned() {
        Some(ci) => ci,
        None => get_commit_info(git_executor, repo_path, &commit).map_err(CopyCommitError::Other)?,
      };

      // Get the tree of this commit
      let tree = cache.get_tree_id(git_executor, repo_path, &commit)?;

      current_parent = create_commit_with_metadata(git_executor, repo_path, &tree, Some(&current_parent), &commit_info, &commit_info.message).map_err(CopyCommitError::Other)?;
    }

    // Update the branch ref to the new tip
    let prev_head_tree = git_executor.resolve_tree_id(repo_path, "HEAD").map_err(CopyCommitError::Other)?;
    update_ref_plumbing(git_executor, repo_path, &current_branch, &current_parent).map_err(CopyCommitError::Other)?;

    // Only refresh index when HEAD tree changed
    let new_head_tree = cache.get_tree_id(git_executor, repo_path, &current_parent)?;
    if prev_head_tree != new_head_tree {
      let _ = git_executor.execute_command(&["reset", "--mixed", "-q", &current_parent], repo_path);
    }

    return Ok(AmendResult {
      amended_commit_id,
      rebased_to_commit: current_parent,
    });
  }

  // Normal case: commit has a parent
  let parent = get_commit_parent(git_executor, repo_path, original_commit_id).unwrap();

  // Use the generic rewrite_commits function with a transform that modifies the target commit
  let original_commit_id_owned = original_commit_id.to_string();
  let amended_tree_clone = amended_tree.clone();
  let new_head = rewrite_commits(
    git_executor,
    repo_path,
    &parent,
    &current_branch,
    |commit| {
      if commit == original_commit_id_owned {
        Ok(RewriteAction::Modify(amended_tree_clone.clone()))
      } else {
        Ok(RewriteAction::Keep)
      }
    },
    cache,
  )?;

  // Find the amended commit ID in the rewritten history
  // The amended commit should be the first one after parent
  let range = format!("{}..{}", parent, new_head);
  let commits = git_executor
    .execute_command_lines(&["rev-list", "--first-parent", "--reverse", &range], repo_path)
    .map_err(CopyCommitError::Other)?;

  let amended_commit_id = commits
    .first()
    .ok_or_else(|| CopyCommitError::Other(anyhow!("No commits found after rewriting")))?
    .to_string();

  Ok(AmendResult {
    amended_commit_id,
    rebased_to_commit: new_head,
  })
}

// create_commit_with_tree replaced by commit_utils::create_commit_with_metadata
