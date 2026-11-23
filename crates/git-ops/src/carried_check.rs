//! The tree a green check saw, carried onto the base of a worktree now (`lease_pool::check_record`,
//! openspec/specs/cli/lease/spec.md, "Pick: A check carries over a clean update"): the change of the checked tree since
//! the base it was built on, merged onto the base of HEAD. When git merges it cleanly into `HEAD^{tree}`, HEAD differs
//! from the checked tree only by the commits of the target branch that came in between, so the check counts for HEAD.
//! The test is the tree, exact for binaries and whitespace; an update that needed a resolution writes another tree.

use crate::replay::merge_trees;
use crate::revs::{commit_count, merge_base};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::{debug, instrument};

/// A checked tree that carries onto HEAD
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedCheck {
  /// The merge base of the checked HEAD with the target: the base the checked tree was built on
  pub checked_base: String,
  /// The merge base of HEAD with the target
  pub base: String,
  /// The commits of the target in `checked_base..base`, which no check saw
  pub untested_commits: u64,
}

/// Whether the change of `checked_tree` (a tree a check saw on HEAD `checked_head`, uncommitted changes included) since
/// the merge base of `checked_head` with `target`, merged onto the merge base of `head` with `target`, is `head_tree`:
/// two `merge-base`, then one `merge-tree` and, when it is `head_tree`, one `rev-list --count`. `None` without a merge
/// base, when HEAD has the base the check had (its tree differs by its own edits), on a conflict, or for another tree
#[instrument(skip(git))]
pub fn carry_checked_tree(
  git: &GitCommandExecutor,
  repo_path: &str,
  checked_head: &str,
  checked_tree: &str,
  head: &str,
  head_tree: &str,
  target: &str,
) -> Result<Option<CarriedCheck>> {
  let Some(checked_base) = merge_base(git, repo_path, checked_head, target)? else {
    return Ok(None);
  };
  let Some(base) = merge_base(git, repo_path, head, target)? else {
    return Ok(None);
  };
  if base == checked_base {
    debug!("HEAD has the base of the check");
    return Ok(None);
  }
  match merge_trees(git, repo_path, &checked_base, checked_tree, &base)? {
    Some(tree) if tree == head_tree => {}
    merged => {
      debug!(conflict = merged.is_none(), "the checked tree does not carry onto HEAD");
      return Ok(None);
    }
  }
  let untested_commits = commit_count(git, repo_path, &base, std::slice::from_ref(&checked_base))?;
  Ok(Some(CarriedCheck {
    checked_base,
    base,
    untested_commits,
  }))
}
