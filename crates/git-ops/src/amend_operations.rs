use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::copy_commit::CopyCommitError;
use crate::merge_conflict::{ConflictDetailsParams, ConflictFileInfo, extract_conflict_details};
use crate::model::{BranchError, MergeConflictInfo};
use std::collections::HashMap;
use std::path::PathBuf;

/// Parameters for amending uncommitted changes to a specific commit in main branch
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AmendToCommitParams {
  pub original_commit_id: String,
}

/// Result of amending operation
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AmendResult {
  pub amended_commit_id: String,
  pub rebased_to_commit: String,
}

/// Amend uncommitted changes to a specific commit in main branch history
/// Uses Git's built-in fixup and autosquash functionality with fail-fast conflict handling:
/// 1. git commit --fixup=<commit> (create fixup commit)
/// 2. git rebase --autosquash (automatically apply fixup, handle conflicts if they occur)
#[instrument(skip(git_executor), fields(original_commit = %params.original_commit_id))]
pub fn amend_to_commit_in_main(git_executor: &GitCommandExecutor, repo_path: &str, _main_branch: &str, params: AmendToCommitParams) -> Result<AmendResult, CopyCommitError> {
  let AmendToCommitParams { original_commit_id } = params;

  // Step 1: Check if there are uncommitted changes
  let status_output = git_executor.execute_command(&["status", "--porcelain"], repo_path)?;

  if status_output.trim().is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No uncommitted changes to amend")));
  }

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
  let final_commit = git_executor.execute_command(&["rev-parse", "HEAD"], repo_path)?.trim().to_string();

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
  let commits_output = git_executor.execute_command(&["rev-list", "--oneline", &range], repo_path)?;

  if commits_output.trim().is_empty() {
    // No commits between - safe to amend
    debug!("no commits between original and HEAD - safe to amend");
    return Ok(());
  }

  // Parse the commits that will be affected by rebase
  let affected_commits: Vec<&str> = commits_output.lines().map(|line| line.trim()).filter(|line| !line.is_empty()).collect();

  debug!(count = affected_commits.len(), "found commits that will be rebased");

  // Create a temporary tree from working directory to simulate the amended commit
  let working_tree = git_executor.execute_command(&["write-tree"], repo_path)?.trim().to_string();

  // For each affected commit, check if it would conflict with the amended version
  for commit_line in affected_commits.iter() {
    let commit_hash = commit_line.split_whitespace().next().ok_or_else(|| anyhow!("Invalid commit line: {}", commit_line))?;

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

    let merge_result = git_executor.execute_command(&merge_tree_args, repo_path);

    match merge_result {
      Ok(output) => {
        // git merge-tree --write-tree outputs:
        // - Just the tree hash if no conflicts
        // - Tree hash followed by conflict info if there are conflicts
        // Check if the output contains conflict markers (lines starting with +<<< or +>>>)
        if output.contains("\n+<<<") || output.contains("\n+>>>") || output.contains("\n+===") {
          // There are conflict markers in the output
          let commit_subject = commit_line.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
          let short_hash = if commit_hash.len() >= 8 { &commit_hash[..8] } else { commit_hash };
          return Err(anyhow!(
            "Amending would create conflicts with commit {} ({}). {} other commit(s) would also be affected by the rebase.",
            short_hash,
            commit_subject,
            affected_commits.len() - 1
          ));
        }
        // Also check if the output has multiple lines (tree hash + conflict info)
        let lines: Vec<&str> = output.lines().collect();
        if lines.len() > 1 {
          // Multiple lines typically indicate conflicts or issues
          let commit_subject = commit_line.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
          let short_hash = if commit_hash.len() >= 8 { &commit_hash[..8] } else { commit_hash };
          return Err(anyhow!(
            "Amending would create conflicts with commit {} ({}). {} other commit(s) would also be affected by the rebase.",
            short_hash,
            commit_subject,
            affected_commits.len() - 1
          ));
        }
      }
      Err(_) => {
        // merge-tree failed, which could indicate conflicts or other issues
        let commit_subject = commit_line.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
        let short_hash = if commit_hash.len() >= 8 { &commit_hash[..8] } else { commit_hash };
        return Err(anyhow!(
          "Cannot safely amend: commit {} ({}) may have conflicts. {} commit(s) would be rebased.",
          short_hash,
          commit_subject,
          affected_commits.len()
        ));
      }
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
  let current_head = git_executor.execute_command(&["rev-parse", "HEAD"], repo_path)?.trim().to_string();

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
  // Get commit message
  let message_output = git_executor.execute_command(&["log", "-1", "--format=%s", commit_id], repo_path)?;
  let message = message_output.trim().to_string();

  // Get commit timestamp
  let timestamp_output = git_executor.execute_command(&["log", "-1", "--format=%ct", commit_id], repo_path)?;
  let timestamp: u32 = timestamp_output
    .trim()
    .parse()
    .map_err(|e| CopyCommitError::Other(anyhow!("Invalid timestamp for commit {}: {}", commit_id, e)))?;

  Ok(crate::commit_list::Commit {
    id: commit_id.to_string(),
    subject: message.clone(),
    message: message.clone(),
    author_name: String::new(),
    author_email: String::new(),
    author_timestamp: timestamp,
    committer_timestamp: timestamp,
    parent_id: None,
    tree_id: String::new(),
    note: None,
    stripped_subject: message,
    mapped_commit_id: None,
  })
}
