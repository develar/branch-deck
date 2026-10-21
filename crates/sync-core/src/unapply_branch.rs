//! Unapply a virtual branch: its original commits leave HEAD with their files (`git_ops::unapply`) and the virtual
//! branch moves to the unapplied namespace in the transaction that moves HEAD, under the repository lock
//! (spec/branch/unapply.spec.md).

use anyhow::Result;
use branch_integration::archive::{REF_RENAME_MUTEX, own_commits_start, unique_branch_name};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_names::{BranchKind, normalize_branch_prefix, qualified_branch_name};
use git_ops::commit_list::Commit;
use git_ops::unapply::{UnappliedMove, apply_unapply, check_unapply};
use git_ops::update_branch::finish_pending_update;
use history_rewrite::repository_lock::RewriteOptions;
use history_rewrite::snapshot::{CommitsAhead, group_commits, read_grouping_inputs};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::PoisonError;
use tracing::{debug, instrument};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UnapplyBranchParams {
  pub repository_path: String,
  /// The branch as the grouping names it (`auth`), not its full virtual branch name
  pub branch_name: String,
  pub branch_prefix: String,
  /// The original commits the request was made for: when the branch groups others now, the unapply is refused. `None`
  /// takes the commits as they group now
  #[serde(default)]
  pub original_commit_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UnapplyBranchResult {
  /// The full name of the unapplied branch: `{prefix}/unapplied/<name>`, with a `-N` suffix when the name is taken
  pub unapplied_branch_name: String,
  /// The original commits that left HEAD, oldest first
  pub commits_removed: Vec<String>,
}

/// An unapply refused before anything was written but git objects
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UnapplyRefusal {
  /// No commit ahead of the baseline groups into the branch
  #[error("No branch named {0:?} among the commits ahead of the baseline")]
  UnknownBranch(String),
  /// The branch groups other commits than the request was made for
  #[error("{0} changed since the request: sync and try again")]
  BranchChanged(String),
  /// The virtual branch does not exist or is not built from the branch's commits now (a conflict, a sync that did not
  /// run): its commits would leave HEAD without a copy
  #[error("{branch} is not built from its commits now ({reason}): sync it, fix a conflict first, then unapply it")]
  NotBuilt { branch: String, reason: String },
  /// A worktree has the virtual branch checked out
  #[error("{branch} is checked out in {worktree}: switch that worktree to another branch first")]
  CheckedOut { branch: String, worktree: String },
}

/// Unapply the virtual branch of `params.branch_name`: its original commits leave HEAD (the checked out branch), the
/// files following, and the virtual branch moves to the unapplied namespace, all in one ref transaction. Under the
/// repository lock, after an interrupted move of the files is finished: the commits of the branch as they group now
/// (`original_commit_ids`, when given, must be them), the check that the virtual branch is their copies, the drop
/// replayed on objects and the dry run of the files (`check_unapply`), then the move
#[instrument(skip(git_executor, options), fields(repo = %params.repository_path, branch = %params.branch_name))]
pub fn unapply_branch_core(git_executor: &GitCommandExecutor, params: UnapplyBranchParams, options: &RewriteOptions) -> Result<UnapplyBranchResult> {
  let UnapplyBranchParams {
    repository_path,
    branch_name,
    branch_prefix,
    original_commit_ids,
  } = params;
  let repo = repository_path.as_str();
  let prefix = normalize_branch_prefix(&branch_prefix);
  let virtual_name = qualified_branch_name(prefix, BranchKind::Virtual, &branch_name)?;

  let _lock = options.lock(git_executor, repo)?;
  finish_pending_update(git_executor, repo)?;

  let grouping = read_grouping_inputs(git_executor, repo)?;
  let ahead = CommitsAhead::read(git_executor, repo)?;
  // the newest commit ahead is HEAD
  if let Some(head) = ahead.commits.last() {
    options.ensure_head(git_executor, repo, &head.id)?;
  }
  let grouped = group_commits(&ahead.commits, &grouping);
  let commits: Vec<&Commit> = ahead
    .commits
    .iter()
    .filter(|commit| grouped.groups.get(&commit.id).and_then(Option::as_deref) == Some(branch_name.as_str()))
    .collect();
  if commits.is_empty() {
    return Err(UnapplyRefusal::UnknownBranch(branch_name).into());
  }
  let ids: Vec<String> = commits.iter().map(|commit| commit.id.clone()).collect();
  if let Some(expected) = &original_commit_ids
    && expected.iter().collect::<BTreeSet<_>>() != ids.iter().collect::<BTreeSet<_>>()
  {
    return Err(UnapplyRefusal::BranchChanged(branch_name).into());
  }

  let virtual_ref = format!("refs/heads/{virtual_name}");
  let tip = built_tip(git_executor, repo, &virtual_name, &ahead.baseline, &commits)?;

  // fails on conflicts and on local changes on the paths the drop writes, before anything changes
  let plan = check_unapply(git_executor, repo, &ids)?;
  if let Err(moved) = options.ensure_head(git_executor, repo, plan.head()) {
    plan.discard(git_executor, repo);
    return Err(moved.into());
  }

  let unapplied_branch_name = {
    let _guard = REF_RENAME_MUTEX.lock().unwrap_or_else(PoisonError::into_inner);
    let unapplied_branch_name = qualified_branch_name(prefix, BranchKind::Unapplied, &branch_name)
      .and_then(|name| unique_branch_name(git_executor, repo, &name))
      .inspect_err(|_| plan.discard(git_executor, repo))?;
    let unapplied_ref = format!("refs/heads/{unapplied_branch_name}");
    apply_unapply(
      git_executor,
      repo,
      &plan,
      &UnappliedMove {
        virtual_ref: &virtual_ref,
        unapplied_ref: &unapplied_ref,
        tip: &tip,
      },
    )?;
    unapplied_branch_name
  };
  debug!(unapplied_branch = %unapplied_branch_name, commits_dropped = ids.len(), "Unapplied branch");

  Ok(UnapplyBranchResult {
    unapplied_branch_name,
    commits_removed: ids,
  })
}

/// The tip of the virtual branch `virtual_name` when its own commits are the copies of `commits`, in their order: one
/// `for-each-ref` (the tip, the worktree that has it), the start of its own commits (`own_commits_start`) and one
/// `rev-list`. Else `NotBuilt` or `CheckedOut`
fn built_tip(git_executor: &GitCommandExecutor, repo: &str, virtual_name: &str, baseline: &str, commits: &[&Commit]) -> Result<String> {
  let not_built = |reason: &str| -> anyhow::Error {
    UnapplyRefusal::NotBuilt {
      branch: virtual_name.to_string(),
      reason: reason.to_string(),
    }
    .into()
  };
  let virtual_ref = format!("refs/heads/{virtual_name}");
  let listed = git_executor.execute_command(&["for-each-ref", "--format=%(objectname)%00%(worktreepath)", &virtual_ref], repo)?;
  let Some((tip, worktree)) = listed.lines().next().and_then(|line| line.split_once('\0')) else {
    return Err(not_built("it does not exist"));
  };
  if !worktree.is_empty() {
    return Err(
      UnapplyRefusal::CheckedOut {
        branch: virtual_name.to_string(),
        worktree: worktree.to_string(),
      }
      .into(),
    );
  }
  let start = own_commits_start(git_executor, repo, baseline, tip)?;
  let excluded = start.map(|start| format!("^{start}"));
  let mut args = vec!["rev-list", "--reverse", "--first-parent", tip];
  args.extend(excluded.as_deref());
  args.push("--");
  let own = git_executor.execute_command_lines(&args, repo)?;
  let copies: Vec<Option<&str>> = commits.iter().map(|commit| commit.mapped_commit_id.as_deref()).collect();
  if own.iter().map(|id| Some(id.as_str())).ne(copies.iter().copied()) {
    debug!(own = own.len(), copies = copies.len(), "the virtual branch is not the copies of the commits");
    return Err(not_built(if copies.contains(&None) {
      "a commit has no copy"
    } else {
      "its commits are other copies"
    }));
  }
  Ok(tip.to_string())
}
