use crate::cache::TreeIdCache;
use crate::commit_chain::{ChainMerge, CommitChain};
use crate::commit_files::is_object_id;
use crate::copy_commit::CopyCommitError;
use crate::merge_conflict::{ConflictContext, build_complete_conflict_info};
use crate::merge_memo::{CherryCommits, MemoizedMerge, MergeTrees};
use crate::merge_tree_output::parse_merge_tree_output;
use crate::model::{BranchError, BranchSyncStatus, MergeConflictInfo};
use anyhow::anyhow;
use git_executor::git_command_executor::GitCommandExecutor;
use std::cell::RefCell;
use std::path::PathBuf;
use tracing::{debug, instrument};

/// The tree of `cherry_commit_id` cherry-picked onto `target_commit_id` with `git merge-tree --write-tree`, without
/// touching the working directory. With `chain`, the merge runs in the chain's `merge-tree --stdin` process of the
/// branch; a conflict is merged again on its own, with the commits, for the conflict analysis
#[expect(clippy::too_many_arguments, reason = "the context of one cherry-pick of a sync")]
#[instrument(skip(git_executor, on_status, tree_id_cache, chain), fields(cherry_id = %cherry_commit_id, target_id = %target_commit_id))]
pub fn perform_fast_cherry_pick_with_context(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  cherry_commit_id: &str,
  cherry_parent_id: &str,
  target_commit_id: &str,
  on_status: &dyn Fn(BranchSyncStatus),
  tree_id_cache: &TreeIdCache,
  chain: Option<&RefCell<CommitChain>>,
) -> Result<String, CopyCommitError> {
  // Fast path: check if parent tree matches target tree
  let cherry_parent_tree_id = tree_id_cache.get_tree_id(git_executor, repo_path, cherry_parent_id)?;
  let target_tree_id = tree_id_cache.get_tree_id(git_executor, repo_path, target_commit_id)?;

  if cherry_parent_tree_id == target_tree_id {
    debug!("parent tree matches target tree, reusing commit tree");
    return tree_id_cache.get_tree_id(git_executor, repo_path, cherry_commit_id);
  }

  let memo = tree_id_cache.merge_memo();
  let trees = match (memo, chain) {
    (None, None) => None,
    _ => Some(MergeTrees {
      base: cherry_parent_tree_id.clone(),
      commit: tree_id_cache.get_tree_id(git_executor, repo_path, cherry_commit_id)?,
      target: target_tree_id.clone(),
    }),
  };
  let light_conflict = |files: Vec<String>| {
    CopyCommitError::BranchError(BranchError::MergeConflict(Box::new(MergeConflictInfo::light(
      cherry_commit_id,
      cherry_parent_id,
      target_commit_id,
      files,
    ))))
  };
  // the conflict view of these commits, remembered by the sync that analysed it: no merge and no analysis
  let commits = CherryCommits {
    commit: cherry_commit_id,
    parent: cherry_parent_id,
    target: target_commit_id,
  };
  let remembers_conflicts = memo.filter(|memo| !memo.light_conflicts() && [cherry_commit_id, cherry_parent_id, target_commit_id].iter().all(|id| is_object_id(id)));
  if let Some(info) = remembers_conflicts.and_then(|memo| memo.conflict(commits)) {
    debug!("the conflict of these commits is remembered");
    return Err(CopyCommitError::BranchError(BranchError::MergeConflict(Box::new(info))));
  }
  if let (Some(memo), Some(trees)) = (memo, &trees) {
    match memo.merge(trees) {
      Some(MemoizedMerge::Clean { tree }) => return Ok(tree),
      Some(MemoizedMerge::Conflict { files }) if memo.light_conflicts() => return Err(light_conflict(files)),
      _ => {}
    }
  }

  if let (Some(chain), Some(trees)) = (chain, &trees) {
    let merged = chain.borrow_mut().merge(&trees.base, &trees.target, &trees.commit)?;
    match merged {
      ChainMerge::Clean { tree } => {
        if let Some(memo) = memo {
          memo.remember_merge(trees, &MemoizedMerge::Clean { tree: tree.clone() });
        }
        return Ok(tree);
      }
      ChainMerge::Conflict { files } => {
        if let Some(memo) = memo {
          memo.remember_merge(trees, &MemoizedMerge::Conflict { files: files.clone() });
          if memo.light_conflicts() {
            return Err(light_conflict(files));
          }
        }
        // the analysis below reads the target commit, which may still be in the chain's writer
        chain.borrow_mut().flush()?;
      }
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
  let (output, _) = git_executor
    .execute_command_raw_accepting(&args, repo_path, &[1])
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to execute git merge-tree: {e}")))?;
  debug!(output_length = output.len(), "git merge-tree completed");
  let merge = parse_merge_tree_output(&output).map_err(CopyCommitError::Other)?;
  let tree_oid = merge.tree.as_str();

  if !merge.entries.is_empty() {
    // the unique conflicted paths
    let conflict_files: Vec<PathBuf> = merge
      .conflicted_paths()
      .into_iter()
      .map(|path| PathBuf::from(String::from_utf8_lossy(path).into_owned()))
      .collect();
    if let (Some(memo), Some(trees)) = (memo, &trees) {
      let files: Vec<String> = conflict_files.iter().map(|file| file.to_string_lossy().into_owned()).collect();
      memo.remember_merge(trees, &MemoizedMerge::Conflict { files: files.clone() });
      if memo.light_conflicts() {
        return Err(light_conflict(files));
      }
    }
    // Conflict analysis takes a while; let the UI show it
    on_status(BranchSyncStatus::AnalyzingConflict);

    let (conflict_info, complete) = build_complete_conflict_info(ConflictContext {
      git_executor,
      repo_path,
      conflict_files: &conflict_files,
      merge_tree_oid: tree_oid,
      parent_commit_id: cherry_parent_id,
      target_commit_id,
      cherry_commit_id,
    })?;
    if let Some(memo) = remembers_conflicts.filter(|_| complete) {
      memo.remember_conflict(commits, &conflict_info);
    }
    return Err(CopyCommitError::BranchError(BranchError::MergeConflict(Box::new(conflict_info))));
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
