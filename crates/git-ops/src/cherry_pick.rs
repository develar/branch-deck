use crate::cache::TreeIdCache;
use crate::copy_commit::CopyCommitError;
use crate::merge_conflict::{ConflictContext, build_merge_conflict_info};
use crate::merge_memo::{MemoizedMerge, MergeTrees};
use crate::model::{BranchError, BranchSyncStatus, MergeConflictInfo};
use anyhow::anyhow;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::BTreeSet;
use std::path::PathBuf;
use tracing::{debug, instrument};

/// Cherry-pick implementation using Git CLI commands (git merge-tree)
/// This performs the cherry-pick without touching the working directory
/// This version uses git CLI exclusively for better performance
#[instrument(skip(git_executor, on_status, tree_id_cache), fields(cherry_id = %cherry_commit_id, target_id = %target_commit_id))]
pub fn perform_fast_cherry_pick_with_context(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  cherry_commit_id: &str,
  cherry_parent_id: &str,
  target_commit_id: &str,
  on_status: &dyn Fn(BranchSyncStatus),
  tree_id_cache: &TreeIdCache,
) -> Result<String, CopyCommitError> {
  // Fast path: check if parent tree matches target tree
  let cherry_parent_tree_id = tree_id_cache.get_tree_id(git_executor, repo_path, cherry_parent_id)?;
  let target_tree_id = tree_id_cache.get_tree_id(git_executor, repo_path, target_commit_id)?;

  if cherry_parent_tree_id == target_tree_id {
    debug!("parent tree matches target tree, reusing commit tree");
    return tree_id_cache.get_tree_id(git_executor, repo_path, cherry_commit_id);
  }

  let memo = tree_id_cache.merge_memo();
  let trees = match memo {
    Some(_) => Some(MergeTrees {
      base: cherry_parent_tree_id.clone(),
      commit: tree_id_cache.get_tree_id(git_executor, repo_path, cherry_commit_id)?,
      target: target_tree_id.clone(),
    }),
    None => None,
  };
  let light_conflict = |files: Vec<String>| {
    CopyCommitError::BranchError(BranchError::MergeConflict(Box::new(MergeConflictInfo::light(
      cherry_commit_id,
      cherry_parent_id,
      target_commit_id,
      files,
    ))))
  };
  if let (Some(memo), Some(trees)) = (memo, &trees) {
    match memo.merge(trees) {
      Some(MemoizedMerge::Clean { tree }) => return Ok(tree),
      Some(MemoizedMerge::Conflict { files }) if memo.light_conflicts() => return Err(light_conflict(files)),
      _ => {}
    }
  }

  debug!(
    base_parent = %cherry_parent_tree_id,
    ours_target = %target_tree_id,
    cherry_commit = %cherry_commit_id,
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
    cherry_parent_id,
    target_commit_id,
    cherry_commit_id,
  ];

  // Exit code 1 means the merge has conflicts; the output still holds the tree and the conflicted entries
  let output = git_executor
    .execute_command_accepting(&args, repo_path, &[1])
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to execute git merge-tree: {e}")))?;

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
    // Parse file entries (one per conflict stage) and collect the unique conflicted paths
    let mut conflict_paths: BTreeSet<PathBuf> = BTreeSet::new();
    for part in parts.iter().skip(1).take_while(|p| !p.is_empty()) {
      // File entries have format: "<mode> <object> <stage>\t<filename>"
      if let Some((prefix, filename)) = part.split_once('\t')
        && prefix.split_whitespace().count() == 3
      {
        conflict_paths.insert(PathBuf::from(filename));
      }
    }
    let conflict_files: Vec<PathBuf> = conflict_paths.into_iter().collect();

    if !conflict_files.is_empty() {
      if let (Some(memo), Some(trees)) = (memo, &trees) {
        let files: Vec<String> = conflict_files.iter().map(|file| file.to_string_lossy().into_owned()).collect();
        memo.remember_merge(trees, &MemoizedMerge::Conflict { files: files.clone() });
        if memo.light_conflicts() {
          return Err(light_conflict(files));
        }
      }
      // Conflict analysis takes a while; let the UI show it
      on_status(BranchSyncStatus::AnalyzingConflict);

      let conflict_info = build_merge_conflict_info(ConflictContext {
        git_executor,
        repo_path,
        conflict_files: &conflict_files,
        merge_tree_oid: tree_oid,
        parent_commit_id: cherry_parent_id,
        target_commit_id,
        cherry_commit_id,
      })?;
      return Err(CopyCommitError::BranchError(BranchError::MergeConflict(Box::new(conflict_info))));
    }
  }

  // No conflicts, return the merged tree ID
  if let (Some(memo), Some(trees)) = (memo, &trees) {
    memo.remember_merge(trees, &MemoizedMerge::Clean { tree: tree_oid.to_string() });
  }
  Ok(tree_oid.to_string())
}

/// First parent of a commit, `None` for a root commit. An unknown commit is an error.
#[instrument(skip(git_executor), fields(commit_id = %commit_id))]
pub fn get_commit_parent(git_executor: &GitCommandExecutor, repo_path: &str, commit_id: &str) -> Result<Option<String>, CopyCommitError> {
  // Prints "<commit> <parent>..." and only "<commit>" for a root commit, so a root commit is not a failure
  let output = git_executor
    .execute_command(&["rev-list", "--no-walk", "--parents", "--end-of-options", commit_id], repo_path)
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to get parent for {commit_id}: {e}")))?;
  Ok(output.split_whitespace().nth(1).map(str::to_string))
}
