use crate::cache::TreeIdCache;
use crate::commit_list::Commit;
use crate::copy_commit::CopyCommitError;
use crate::merge_conflict::{ConflictDetailsParams, ConflictFileInfo, extract_conflict_details};
use crate::model::{BranchError, BranchSyncStatus, MergeConflictInfo};
use crate::progress::CherryPickProgress;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, instrument};

/// Cherry-pick implementation using Git CLI commands (git merge-tree)
/// This performs the cherry-pick without touching the working directory
/// This version uses git CLI exclusively for better performance
#[instrument(skip(git_executor, progress, tree_id_cache), fields(cherry_id = %cherry_commit_id, target_id = %target_commit_id))]
pub fn perform_fast_cherry_pick_with_context(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  cherry_commit_id: &str,
  target_commit_id: &str,
  progress: Option<&CherryPickProgress>,
  tree_id_cache: &TreeIdCache,
) -> Result<String, CopyCommitError> {
  // Get commit information using git CLI
  let cherry_parent_id = get_commit_parent(git_executor, repo_path, cherry_commit_id)?;

  // Fast path: check if parent tree matches target tree
  let cherry_parent_tree_id = tree_id_cache.get_tree_id(git_executor, repo_path, &cherry_parent_id)?;
  let target_tree_id = tree_id_cache.get_tree_id(git_executor, repo_path, target_commit_id)?;

  if cherry_parent_tree_id == target_tree_id {
    debug!("parent tree matches target tree, reusing commit tree");
    return tree_id_cache.get_tree_id(git_executor, repo_path, cherry_commit_id);
  }

  // Get the commit's tree ID for debugging
  let commit_tree_id = tree_id_cache.get_tree_id(git_executor, repo_path, cherry_commit_id)?;

  debug!(
    base_parent = %cherry_parent_tree_id,
    ours_target = %target_tree_id,
    theirs_commit = %commit_tree_id,
    "running git merge-tree"
  );

  // Use git merge-tree for 3-way merge
  let args = vec![
    "-c",
    "merge.conflictStyle=zdiff3", // Set conflict style to include base content
    "merge-tree",
    "--write-tree",
    "-z", // Use NUL character as separator for better parsing
    "--merge-base",
    &cherry_parent_id,
    target_commit_id,
    cherry_commit_id,
  ];

  let output = git_executor
    .execute_command(&args, repo_path)
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to execute git merge-tree: {}", e)))?;

  debug!(output_length = output.len(), "git merge-tree completed");

  // Check if command produced output
  if output.is_empty() {
    return Err(CopyCommitError::Other(anyhow!("git merge-tree did not produce output")));
  }

  // Parse the NUL-separated output
  let parts: Vec<&str> = output.trim_end_matches('\0').split('\0').collect();

  if parts.is_empty() || parts[0].is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No output from git merge-tree")));
  }

  let tree_oid = parts[0];

  // Check if there were conflicts by looking for file entries
  if parts.len() > 1 {
    let mut conflict_files: HashMap<PathBuf, ConflictFileInfo> = HashMap::new();

    // Parse file entries (mode object stage\tfilename)
    for part in parts.iter().skip(1).take_while(|p| !p.is_empty()) {
      // File entries have format: "<mode> <object> <stage>\t<filename>"
      if let Some(tab_pos) = part.find('\t') {
        let (prefix, filename) = part.split_at(tab_pos);
        let filename = &filename[1..]; // Skip the tab
        let path = PathBuf::from(filename);

        // Parse mode, object, stage
        let prefix_parts: Vec<&str> = prefix.split_whitespace().collect();
        if prefix_parts.len() == 3 {
          let object_id = prefix_parts[1].to_string();
          let stage = prefix_parts[2];

          let entry = conflict_files.entry(path.clone()).or_insert(ConflictFileInfo {
            path,
            base_oid: None,
            ours_oid: None,
            theirs_oid: None,
          });

          match stage {
            "1" => entry.base_oid = Some(object_id),
            "2" => entry.ours_oid = Some(object_id),
            "3" => entry.theirs_oid = Some(object_id),
            _ => {}
          }
        }
      }
    }

    if !conflict_files.is_empty() {
      // Send branch status event for conflict analysis if progress is available
      if let Some(progress) = &progress {
        let _ = progress.send_status(BranchSyncStatus::AnalyzingConflict, None);
      }

      // Get detailed conflict information with diffs
      let (detailed_conflicts, conflict_marker_commits) = extract_conflict_details(ConflictDetailsParams {
        git_executor,
        repo_path,
        conflict_files: &conflict_files,
        merge_tree_oid: parts[0], // merge_tree_oid
        parent_commit_id: &cherry_parent_id,
        target_commit_id,
        cherry_commit_id,
      })?;

      // Analyze the conflict to find missing commits
      let conflicting_paths: Vec<PathBuf> = conflict_files.keys().cloned().collect();
      let conflict_analysis = match crate::conflict_analysis::analyze_conflict(git_executor, repo_path, &cherry_parent_id, target_commit_id, &conflicting_paths) {
        Ok(analysis) => analysis,
        Err(e) => {
          // If conflict analysis fails, create a default analysis with empty data
          debug!(error = %e, "failed to analyze conflict");
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

      // Get commit information for error reporting
      let cherry_commit_info = get_commit_info(git_executor, repo_path, cherry_commit_id)?;
      let cherry_parent_info = get_commit_info(git_executor, repo_path, &cherry_parent_id)?;
      let target_commit_info = get_commit_info(git_executor, repo_path, target_commit_id)?;

      return Err(CopyCommitError::BranchError(BranchError::MergeConflict(Box::new(MergeConflictInfo {
        commit_message: cherry_commit_info.message,
        commit_hash: cherry_commit_id.to_string(),
        commit_author_time: cherry_commit_info.author_timestamp,
        commit_committer_time: cherry_commit_info.committer_timestamp,
        original_parent_message: cherry_parent_info.message,
        original_parent_hash: cherry_parent_id,
        original_parent_author_time: cherry_parent_info.author_timestamp,
        original_parent_committer_time: cherry_parent_info.committer_timestamp,
        target_branch_message: target_commit_info.message,
        target_branch_hash: target_commit_id.to_string(),
        target_branch_author_time: target_commit_info.author_timestamp,
        target_branch_committer_time: target_commit_info.committer_timestamp,
        conflicting_files: detailed_conflicts,
        conflict_analysis,
        conflict_marker_commits,
      }))));
    }
  }

  // No conflicts, return the merged tree ID
  Ok(tree_oid.to_string())
}

/// Get the parent commit ID using git CLI
#[instrument(skip(git_executor), fields(commit_id = %commit_id))]
fn get_commit_parent(git_executor: &GitCommandExecutor, repo_path: &str, commit_id: &str) -> Result<String, CopyCommitError> {
  let parent_ref = format!("{commit_id}^");
  let args = vec!["rev-parse", &parent_ref];
  let output = git_executor
    .execute_command(&args, repo_path)
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to get parent for {}: {}", commit_id, e)))?;
  Ok(output.trim().to_string())
}

/// Get basic commit information using git CLI
#[instrument(skip(git_executor), fields(commit_id = %commit_id))]
fn get_commit_info(git_executor: &GitCommandExecutor, repo_path: &str, commit_id: &str) -> Result<Commit, CopyCommitError> {
  // Get commit message
  let message_args = vec!["log", "-1", "--format=%s", commit_id];
  let message_output = git_executor
    .execute_command(&message_args, repo_path)
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to get commit message for {}: {}", commit_id, e)))?;
  let message = message_output.trim().to_string();

  // Get commit timestamp
  let timestamp_args = vec!["log", "-1", "--format=%ct", commit_id];
  let timestamp_output = git_executor
    .execute_command(&timestamp_args, repo_path)
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to get commit timestamp for {}: {}", commit_id, e)))?;
  let timestamp: u32 = timestamp_output
    .trim()
    .parse()
    .map_err(|e| CopyCommitError::Other(anyhow!("Invalid timestamp for commit {}: {}", commit_id, e)))?;

  Ok(Commit {
    id: commit_id.to_string(),
    subject: message.clone(),    // We only have the subject line from %s
    message: message.clone(),    // For error reporting, subject is sufficient
    author_name: String::new(),  // Not available from this query
    author_email: String::new(), // Not available from this query
    author_timestamp: timestamp,
    committer_timestamp: timestamp,
    parent_id: None,           // Not relevant for error reporting
    tree_id: String::new(),    // Not relevant for error reporting
    note: None,                // Not relevant for error reporting
    stripped_subject: message, // Same as subject for error reporting
    mapped_commit_id: None,    // Not relevant for error reporting
  })
}
