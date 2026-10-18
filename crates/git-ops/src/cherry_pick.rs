//! The cherry-pick of a commit onto a virtual branch as a tree, by `merge-tree --write-tree` in the branch's
//! `CommitChain`, one process for the whole branch; a conflict is merged again on its own, with the commits, for its
//! report.

use crate::commit_chain::CommitChain;
use crate::copy_commit::CopyCommitError;
use crate::merge_conflict::{ConflictContext, build_complete_conflict_info, merge_with_markers};
use crate::merge_memo::{CherryCommits, MergeMemo, MergeOutcome, MergeTrees};
use crate::model::{BranchError, BranchSyncStatus, MergeConflictInfo};
use crate::revs::is_object_id;
use crate::tree_id_cache::TreeIdCache;
use git_executor::git_command_executor::GitCommandExecutor;
use std::cell::RefCell;
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
      Some(MergeOutcome::Clean { tree }) => return Ok(tree),
      Some(MergeOutcome::Conflict { files }) if memo.light_conflicts() => return Err(light_conflict(files)),
      _ => {}
    }
  }

  if let (Some(chain), Some(trees)) = (chain, &trees) {
    let merged = chain.borrow_mut().merge(&trees.base, &trees.target, &trees.commit)?;
    if let Some(memo) = memo {
      memo.remember_merge(trees, &merged);
    }
    match merged {
      MergeOutcome::Clean { tree } => return Ok(tree),
      MergeOutcome::Conflict { files } if memo.is_some_and(MergeMemo::light_conflicts) => return Err(light_conflict(files)),
      // the analysis below reads the target commit, which may still be in the chain's writer
      MergeOutcome::Conflict { .. } => chain.borrow_mut().flush()?,
    }
  }

  debug!(
    base_parent = %cherry_parent_tree_id,
    ours_target = %target_tree_id,
    cherry_commit = %cherry_commit_id,
    "running git merge-tree"
  );
  let (tree_oid, conflict_files) = merge_with_markers(git_executor, repo_path, commits)?;

  if !conflict_files.is_empty() {
    if let (Some(memo), Some(trees)) = (memo, &trees) {
      let files: Vec<String> = conflict_files.iter().map(|file| file.to_string_lossy().into_owned()).collect();
      memo.remember_merge(trees, &MergeOutcome::Conflict { files: files.clone() });
      if memo.light_conflicts() {
        return Err(light_conflict(files));
      }
    }
    // Conflict analysis takes a while; let the UI show it
    on_status(BranchSyncStatus::AnalyzingConflict);

    let (conflict_info, contents, complete) = build_complete_conflict_info(ConflictContext {
      git_executor,
      repo_path,
      conflict_files: &conflict_files,
      merge_tree_oid: &tree_oid,
      parent_commit_id: cherry_parent_id,
      target_commit_id,
      cherry_commit_id,
    })?;
    if let Some(memo) = remembers_conflicts.filter(|_| complete) {
      memo.remember_conflict(commits, &conflict_info, contents);
    }
    return Err(CopyCommitError::BranchError(BranchError::MergeConflict(Box::new(conflict_info))));
  }

  // No conflicts, return the merged tree ID
  if let (Some(memo), Some(trees)) = (memo, &trees) {
    memo.remember_merge(trees, &MergeOutcome::Clean { tree: tree_oid.clone() });
  }
  Ok(tree_oid)
}
