//! Mark Reviewed (`bd mark-reviewed`): the base and the tip a local branch has now become its reviewed pair,
//! `refs/branch-deck/reviewed/<sha256 of the branch's full name>/{base,tip}` (`git_ops::reviewed_refs`), which the
//! next review of the branch compares it with. The base is the merge base of the tip and the base the caller names (the
//! head of the stack parent a review shows the branch on, `BranchReviewSegment::base`), else of the tip and the
//! baseline. One `for-each-ref` of the reviewed refs and the branches, one `merge-base`, one `update-ref --stdin` with
//! compare-and-swap on every ref that listing read, which also deletes the pairs of the branches that are gone; no
//! repository lock: the compare-and-swap of the transaction is the witness, as for a write of the stores alone. Without
//! a base or a baseline from the caller, `detect_baseline_branch` runs first; a base that names no branch is resolved
//! by one `rev-parse --verify` first. A base that names no commit (`UnknownReviewBase`) or shares no history with the
//! tip (`NoCommonHistory`) fails before the transaction, so nothing is written. Nothing else is written: no branch, no
//! index or working tree.

use anyhow::Result;
use domain_types::branch_names::to_final_branch_name;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::reviewed_refs::{MARK_REVIEWED, ReviewedPair, read_reviewed_refs};
use git_ops::revs::{merge_base, resolve_commit};
use history_rewrite::baseline::detect_baseline_branch;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, instrument};

const BRANCHES: &str = "refs/heads/";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MarkReviewedParams {
  pub repository_path: String,
  /// The branch, as `resolve_branch` takes it: a virtual branch by its name, a local branch by its name, or
  /// `refs/heads/<name>`
  pub branch: String,
  /// The commit the branch is reviewed on: for a branch stacked on another, the head of its parent, so that the
  /// reviewed commits are the branch's own; `None` takes the baseline
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub base: Option<String>,
  /// The baseline as the sync detected it (`origin/master`), used without a `base`; `None` detects that of the checked
  /// out branch
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub baseline_branch: Option<String>,
}

/// The reviewed pair a mark kept
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewedTip {
  /// The full name of the marked branch, without `refs/heads/` (`jane/virtual/auth`)
  pub branch: String,
  /// The merge base of the tip and the base of the params (the base itself when it is an ancestor of the tip), else
  /// of the tip and the baseline
  pub base: String,
  pub tip: String,
  /// The pair the mark replaced, `None` when the branch was never marked
  pub previous: Option<ReviewedPair>,
  /// Whether the mark moved or deleted a ref: `false` when the branch was marked at this base and tip already
  pub changed: bool,
  /// How many pairs of branches that are gone the mark deleted
  pub pruned: u32,
}

/// No local branch has the name a mark was asked for
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("No local branch {0:?}")]
pub struct UnknownReviewBranch(pub String);

/// The base a mark was asked for names neither a branch nor a commit
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("No branch or commit {0:?} to review the branch on")]
pub struct UnknownReviewBase(pub String);

/// The branch shares no history with the base it is reviewed on (the base of the params, else the baseline)
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{branch} has no history in common with {base}")]
pub struct NoCommonHistory {
  /// The full name of the branch
  pub branch: String,
  /// The base as the params or the baseline named it
  pub base: String,
}

/// The full name and the tip of the branch `name` names among `branches` (full name -> tip): `refs/heads/<name>`
/// exactly that branch; else the virtual branch `{branch_prefix}/virtual/<name>`, then the local branch `<name>`
pub fn resolve_branch(branches: &HashMap<String, String>, branch_prefix: &str, name: &str) -> Option<(String, String)> {
  let found = |full: &str| branches.get(full).map(|tip| (full.to_string(), tip.clone()));
  if let Some(full) = name.strip_prefix(BRANCHES) {
    return found(full);
  }
  to_final_branch_name(branch_prefix, name)
    .ok()
    .and_then(|virtual_branch| found(&virtual_branch))
    .or_else(|| found(name))
}

/// A branch with the base it is reviewed on
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchOnBase {
  /// The full name, without `refs/heads/`
  pub branch: String,
  pub tip: String,
  /// The merge base of the tip and the base its caller named, else of the tip and the baseline
  pub base: String,
}

/// The branch `name` names among `branches` (`resolve_branch`) with its base: the merge base of its tip and `base`
/// (a branch named as the branch is, at its tip from the listing; any other rev by one `rev-parse --verify`), else of its
/// tip and `baseline_branch`, else of its tip and the baseline of the checked out branch (`detect_baseline_branch`).
/// `UnknownReviewBranch`, `UnknownReviewBase` and `NoCommonHistory` before any write. What a mark and a plan both ask
pub fn branch_on_base(
  git: &GitCommandExecutor,
  repository_path: &str,
  branches: &HashMap<String, String>,
  branch_prefix: &str,
  name: &str,
  (base, baseline_branch): (Option<&str>, Option<&str>),
) -> Result<BranchOnBase> {
  let (branch, tip) = resolve_branch(branches, branch_prefix, name).ok_or_else(|| UnknownReviewBranch(name.to_string()))?;
  // a base that names a branch as the branch is named (the virtual branch of a stack parent) is that branch's tip,
  // any other is a rev of git, resolved first so that a mistyped one is the caller's error, not git's
  let (named, on) = match (base, baseline_branch) {
    (Some(base), _) => {
      let on = match resolve_branch(branches, branch_prefix, base) {
        Some((_, base_tip)) => base_tip,
        None => resolve_commit(git, repository_path, base)?.ok_or_else(|| UnknownReviewBase(base.to_string()))?,
      };
      (base.to_string(), on)
    }
    (None, Some(baseline)) => (baseline.to_string(), baseline.to_string()),
    (None, None) => {
      let baseline = detect_baseline_branch(git, repository_path)?;
      (baseline.clone(), baseline)
    }
  };
  let Some(base) = merge_base(git, repository_path, &tip, &on)?.map(|base| base.trim().to_string()) else {
    return Err(NoCommonHistory { branch, base: named }.into());
  };
  Ok(BranchOnBase { branch, tip, base })
}

/// Keep the base and the tip the branch of `params` has now as its reviewed pair, and delete the pairs of the
/// branches that are gone, in one transaction; `UnknownReviewBranch` when no branch has that name,
/// `UnknownReviewBase` when the base names no branch and no commit, `NoCommonHistory` when the branch shares no history
/// with its base, each before any write; an error when another process marked or pruned meanwhile
#[instrument(skip(git), fields(branch = %params.branch))]
pub fn mark_reviewed_core(git: &GitCommandExecutor, branch_prefix: &str, params: &MarkReviewedParams) -> Result<ReviewedTip> {
  let repository_path = params.repository_path.as_str();
  let refs = read_reviewed_refs(git, repository_path)?;
  let BranchOnBase { branch, tip, base } = branch_on_base(
    git,
    repository_path,
    &refs.branches,
    branch_prefix,
    &params.branch,
    (params.base.as_deref(), params.baseline_branch.as_deref()),
  )?;
  let pair = ReviewedPair { base, tip };
  let mark = refs.mark(&branch, &pair);
  mark.transaction.commit(git, repository_path, MARK_REVIEWED)?;
  debug!(branch, changed = mark.changed, pruned = mark.pruned, "marked reviewed");
  Ok(ReviewedTip {
    branch,
    base: pair.base,
    tip: pair.tip,
    previous: mark.previous,
    changed: mark.changed,
    pruned: u32::try_from(mark.pruned).unwrap_or(u32::MAX),
  })
}
