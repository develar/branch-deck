use crate::git::copy_commit::CopyCommitError;
use crate::git::git_command::GitCommandExecutor;
use crate::git::merge_conflict::{ConflictDetailsParams, ConflictFileInfo, extract_conflict_details};
use crate::git::model::{BranchError, MergeConflictInfo};
use crate::progress::SyncEvent;
use anyhow::{Result, anyhow};
use git2::{Commit, Oid, Repository, Tree};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::ipc::Channel;
use tracing::{debug, instrument};

/// Cherry-pick implementation using Git plumbing commands (git merge-tree)
/// This performs the cherry-pick without touching the working directory
/// This version is for production use with GitCommandExecutor
#[instrument(skip_all)]
pub fn perform_fast_cherry_pick_with_context<'a>(
  repo: &'a Repository,
  cherry_commit: &'a Commit,
  target_commit: &'a Commit,
  git_executor: &GitCommandExecutor,
  progress: Option<(&Channel<SyncEvent>, &str, i16)>, // (channel, branch_name, task_index)
) -> Result<Tree<'a>, CopyCommitError> {
  // Fast path: if parent tree matches target tree, reuse commit tree
  if cherry_commit.parent_count() > 0 {
    let parent = cherry_commit.parent(0)?;
    if parent.tree_id() == target_commit.tree_id() {
      debug!("parent tree matches target tree, reusing commit tree");
      return Ok(cherry_commit.tree()?);
    }
  }

  // Get repository path
  let repo_path = repo.path().parent().ok_or_else(|| CopyCommitError::Other(anyhow!("Could not get repository path")))?;
  let repo_path_str = repo_path.to_str().ok_or_else(|| CopyCommitError::Other(anyhow!("Repository path is not valid UTF-8")))?;

  // Get the commit's tree and parent
  let commit_tree_id = cherry_commit.tree_id();
  let parent = cherry_commit
    .parent(0)
    .map_err(|e| CopyCommitError::Other(anyhow!("Cannot cherry-pick a root commit: {}", e)))?;
  let parent_tree_id = parent.tree_id();
  let target_tree_id = target_commit.tree_id();

  debug!(
    base_parent = %parent_tree_id,
    ours_target = %target_tree_id,
    theirs_commit = %commit_tree_id,
    "running git merge-tree"
  );

  // Use GitCommandExecutor to run git merge-tree
  let parent_id = parent.id().to_string();
  let target_id = target_commit.id().to_string();
  let cherry_id = cherry_commit.id().to_string();

  let args = vec![
    "-c",
    "merge.conflictStyle=zdiff3", // Set conflict style to include base content
    "merge-tree",
    "--write-tree",
    "-z", // Use NUL character as separator for better parsing
    "--merge-base",
    &parent_id,
    &target_id,
    &cherry_id,
  ];

  let output = git_executor
    .execute_command(&args, repo_path_str)
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

  let tree_oid = parts[0]
    .parse::<Oid>()
    .map_err(|e| CopyCommitError::Other(anyhow!("Invalid tree OID '{}' from merge-tree: {}", parts[0], e)))?;

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

    // Skip empty separator and subsequent sections
    // We only care about the actual conflicting file paths

    if !conflict_files.is_empty() {
      // Send branch status event for conflict analysis if progress channel is available
      if let Some((progress_channel, branch_name, _task_index)) = &progress {
        let _ = progress_channel.send(SyncEvent::BranchStatusUpdate {
          branch_name: branch_name.to_string(),
          status: crate::git::model::BranchSyncStatus::AnalyzingConflict,
        });
      }

      // Get detailed conflict information with diffs
      let original_parent = cherry_commit.parent(0)?;
      let (detailed_conflicts, conflict_marker_commits) = extract_conflict_details(ConflictDetailsParams {
        git_executor,
        repo_path: repo_path_str,
        conflict_files: &conflict_files,
        merge_tree_oid: parts[0], // merge_tree_oid
        parent_commit_id: &original_parent.id().to_string(),
        target_commit_id: &target_commit.id().to_string(),
        cherry_commit_id: &cherry_commit.id().to_string(),
      })?;

      // Analyze the conflict to find missing commits
      let conflicting_paths: Vec<PathBuf> = conflict_files.keys().cloned().collect();
      let conflict_analysis = match crate::git::conflict_analysis::analyze_conflict(
        git_executor,
        repo_path_str,
        &original_parent.id().to_string(),
        &target_commit.id().to_string(),
        &conflicting_paths,
      ) {
        Ok(analysis) => analysis,
        Err(e) => {
          // If conflict analysis fails, create a default analysis with empty data
          debug!(error = %e, "failed to analyze conflict");
          crate::git::conflict_analysis::ConflictAnalysis {
            missing_commits: vec![],
            merge_base_hash: String::new(),
            merge_base_message: String::new(),
            merge_base_time: 0,
            merge_base_author: String::new(),
            divergence_summary: crate::git::conflict_analysis::DivergenceSummary {
              commits_ahead_in_source: 0,
              commits_ahead_in_target: 0,
              common_ancestor_distance: 0,
            },
          }
        }
      };

      return Err(CopyCommitError::BranchError(BranchError::MergeConflict(Box::new(MergeConflictInfo {
        commit_message: cherry_commit.summary().unwrap_or_default().to_string(),
        commit_hash: cherry_commit.id().to_string(),
        commit_time: cherry_commit.time().seconds() as u32,
        original_parent_message: original_parent.summary().unwrap_or_default().to_string(),
        original_parent_hash: original_parent.id().to_string(),
        original_parent_time: original_parent.time().seconds() as u32,
        target_branch_message: target_commit.summary().unwrap_or_default().to_string(),
        target_branch_hash: target_commit.id().to_string(),
        target_branch_time: target_commit.time().seconds() as u32,
        conflicting_files: detailed_conflicts,
        conflict_analysis,
        conflict_marker_commits,
      }))));
    }
  }

  // No conflicts, return the merged tree
  let tree = repo.find_tree(tree_oid)?;
  Ok(tree)
}

/// Backward compatibility function that creates its own GitCommandExecutor
/// Used by existing tests and code that hasn't been updated yet
pub fn perform_fast_cherry_pick<'a>(repo: &'a Repository, cherry_commit: &'a Commit, target_commit: &'a Commit) -> Result<Tree<'a>, CopyCommitError> {
  let git_executor = GitCommandExecutor::new();
  perform_fast_cherry_pick_with_context(repo, cherry_commit, target_commit, &git_executor, None)
}
