//! Unapply a virtual branch: its original commits leave HEAD with their files (`git_ops::unapply`) and the virtual
//! branch moves to the unapplied namespace in the transaction that moves HEAD (spec/branch/unapply.spec.md); and its
//! undo (`UnapplyUndoToken`), which moves both back. Each reads and checks before the repository lock, which holds only
//! the witness and the move.

use anyhow::Result;
use branch_integration::archive::{own_commits_start, unique_branch_name};
use domain_types::branch_names::{BranchKind, normalize_branch_prefix, qualified_branch_name};
use domain_types::commit::Commit;
use domain_types::error::ErrorCode;
use domain_types::rewrite::{UnapplyBranchResult, UndoneUnapply};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::{is_object_id, with_dirs_of};
use git_ops::unapply::{UnappliedMove, UnapplyPlan, apply_unapply, apply_unapply_undo, check_unapply, check_unapply_undo};
use git_ops::update_branch::UndoCheck;
use history_rewrite::issue_groups::ISSUE_GROUPS_REF;
use history_rewrite::repository_lock::{LockWait, Prepared, RewriteOptions, Witness, lock_checked_undo};
use history_rewrite::snapshot::{CommitsAhead, group_commits, read_grouping_inputs};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tracing::{debug, info, instrument};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UnapplyBranchParams {
  pub repository_path: String,
  /// The branch as the grouping names it (`auth`), not its full virtual branch name
  pub branch_name: String,
  /// The original commits the request was made for: when the branch groups others now, the unapply is refused. `None`
  /// takes the commits as they group now
  #[serde(default)]
  pub original_commit_ids: Option<Vec<String>>,
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

impl UnapplyRefusal {
  /// The code of the refusal, the same in `bd` and in the app
  pub const fn code(&self) -> ErrorCode {
    match self {
      Self::UnknownBranch(_) => ErrorCode::UnknownBranch,
      Self::BranchChanged(_) => ErrorCode::HeadMoved,
      Self::NotBuilt { .. } => ErrorCode::BranchNotBuilt,
      Self::CheckedOut { .. } => ErrorCode::BranchCheckedOut,
    }
  }
}

/// Unapply the virtual branch of `params.branch_name` (named with `branch_prefix`, which the caller resolved): its
/// original commits leave HEAD (the checked out branch), the files following, and the virtual branch moves to the
/// unapplied namespace, all in one ref transaction. Before the repository lock: the commits of the branch as they group
/// now (`original_commit_ids`, when given, must be them), the check that the virtual branch is their copies, the drop
/// replayed on objects, the dry run of the files (`check_unapply`) and a free name. Under it only the witness (HEAD,
/// the issue groups, both branches and the mark of an interrupted move, `RewriteOptions::lock_prepared_move`) and the
/// move
#[instrument(skip(git_executor, options), fields(repo = %params.repository_path, branch = %params.branch_name))]
pub fn unapply_branch_core(git_executor: &GitCommandExecutor, params: UnapplyBranchParams, branch_prefix: &str, options: &RewriteOptions) -> Result<UnapplyBranchResult> {
  let UnapplyBranchParams {
    repository_path,
    branch_name,
    original_commit_ids,
  } = params;
  let repo = repository_path.as_str();
  let prefix = normalize_branch_prefix(branch_prefix);
  let request = UnapplyRequest {
    branch_name: &branch_name,
    virtual_name: qualified_branch_name(prefix, BranchKind::Virtual, &branch_name)?,
    unapplied_base: qualified_branch_name(prefix, BranchKind::Unapplied, &branch_name)?,
    original_commit_ids: original_commit_ids.as_deref(),
  };

  // the guards, the lock and the mark take the directories read here
  let git_executor = &with_dirs_of(git_executor, repo)?;
  let prepare = || prepare_unapply(git_executor, repo, &request, options);
  let (prepared, _lock) = options.lock_prepared_move(git_executor, repo, prepare(), prepare)?;
  let PreparedUnapply {
    plan,
    ids,
    tip,
    unapplied_branch_name,
    ..
  } = prepared;
  let (virtual_ref, unapplied_ref) = (format!("refs/heads/{}", request.virtual_name), format!("refs/heads/{unapplied_branch_name}"));
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
  debug!(unapplied_branch = %unapplied_branch_name, commits_dropped = ids.len(), "Unapplied branch");

  let (previous_head, new_head) = (plan.head().to_string(), plan.new_head().to_string());
  let undo_token = UnapplyUndoToken {
    previous_head: previous_head.clone(),
    new_head: new_head.clone(),
    tip,
    virtual_branch: request.virtual_name,
    unapplied_branch: unapplied_branch_name.clone(),
  }
  .to_string();
  Ok(UnapplyBranchResult {
    unapplied_branch_name,
    commits_removed: ids,
    previous_head,
    new_head,
    undo_token,
  })
}

/// What an unapply is asked for, its branch names resolved
struct UnapplyRequest<'a> {
  branch_name: &'a str,
  /// `{prefix}/virtual/<name>`
  virtual_name: String,
  /// `{prefix}/unapplied/<name>`, the name the unapplied branch gets unless a branch has it
  unapplied_base: String,
  original_commit_ids: Option<&'a [String]>,
}

/// An unapply computed before the repository lock: the checked drop, the commits it drops, the tip of the virtual
/// branch and the free name of the unapplied one, on the HEAD and the issue groups read once
struct PreparedUnapply {
  plan: UnapplyPlan,
  ids: Vec<String>,
  tip: String,
  virtual_ref: String,
  unapplied_branch_name: String,
  /// The commit of the issue groups the grouping read: they decide which commits are the branch's
  issue_groups: Option<String>,
}

impl Prepared for PreparedUnapply {
  /// HEAD and its branch, the issue groups, the virtual branch at its tip and no branch of the free name
  fn witness(&self) -> Witness {
    Witness::of_head(self.plan.head(), self.plan.branch())
      .with_ref(ISSUE_GROUPS_REF, self.issue_groups.as_deref())
      .with_ref(&self.virtual_ref, Some(&self.tip))
      .with_ref(&format!("refs/heads/{}", self.unapplied_branch_name), None)
  }

  fn discard(&self, git_executor: &GitCommandExecutor, repository_path: &str) {
    self.plan.discard(git_executor, repository_path);
  }
}

/// Everything an unapply reads and computes: the commits of the branch as they group now, the check that the virtual
/// branch is their copies, the drop and the dry run of the files (`check_unapply`), and a free name for the unapplied
/// branch (`unique_branch_name`: the witness and the create of the transaction keep it free)
fn prepare_unapply(git_executor: &GitCommandExecutor, repo: &str, request: &UnapplyRequest<'_>, options: &RewriteOptions) -> Result<PreparedUnapply> {
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
    .filter(|commit| grouped.groups.get(&commit.id).and_then(Option::as_deref) == Some(request.branch_name))
    .collect();
  if commits.is_empty() {
    return Err(UnapplyRefusal::UnknownBranch(request.branch_name.to_string()).into());
  }
  let ids: Vec<String> = commits.iter().map(|commit| commit.id.clone()).collect();
  if let Some(expected) = request.original_commit_ids
    && expected.iter().collect::<BTreeSet<_>>() != ids.iter().collect::<BTreeSet<_>>()
  {
    return Err(UnapplyRefusal::BranchChanged(request.branch_name.to_string()).into());
  }
  let tip = built_tip(git_executor, repo, &request.virtual_name, &ahead.baseline, &commits)?;

  // fails on conflicts and on local changes on the paths the drop writes, before anything changes
  let plan = check_unapply(git_executor, repo, &ids)?;
  let named = options
    .ensure_head(git_executor, repo, plan.head())
    .map_err(anyhow::Error::new)
    .and_then(|()| unique_branch_name(git_executor, repo, &request.unapplied_base));
  let unapplied_branch_name = named.inspect_err(|_| plan.discard(git_executor, repo))?;
  Ok(PreparedUnapply {
    plan,
    ids,
    tip,
    virtual_ref: format!("refs/heads/{}", request.virtual_name),
    unapplied_branch_name,
    issue_groups: grouping.issue_groups.commit,
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

/// The undo token of an unapply: `unapply:<previous head>..<new head>:<tip>:<virtual branch>:<unapplied branch>`, full
/// ids and the full names of both branches (a ref name has no `:`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnapplyUndoToken {
  pub previous_head: String,
  pub new_head: String,
  /// The tip the virtual branch had, which the unapplied branch got
  pub tip: String,
  /// `{prefix}/virtual/<name>`
  pub virtual_branch: String,
  /// `{prefix}/unapplied/<name>`
  pub unapplied_branch: String,
}

const UNAPPLY_TOKEN: &str = "unapply:";

/// `<kind><previous head>..<new head>:<tip>:<branch>…` of an unapply or a reapply: three full ids of one length, the
/// heads different, and `N` full branch names (a ref name has no `:`, none starts with `-` or has `..`)
pub(crate) fn parse_move_token<const N: usize>(text: &str, kind: &str) -> Option<(String, String, String, [String; N])> {
  let mut parts = text.strip_prefix(kind)?.split(':');
  let (previous_head, new_head) = parts.next()?.split_once("..")?;
  let tip = parts.next()?;
  let ids = [previous_head, new_head, tip];
  if !ids.iter().all(|id| is_object_id(id) && id.len() == tip.len()) || previous_head == new_head {
    return None;
  }
  let names: Vec<String> = parts.map(str::to_string).collect();
  let names: [String; N] = names.try_into().ok()?;
  if names.iter().any(|name| name.is_empty() || name.starts_with('-') || name.contains("..")) {
    return None;
  }
  Some((previous_head.to_string(), new_head.to_string(), tip.to_string(), names))
}

impl UnapplyUndoToken {
  /// A token as `Display` prints it; anything else, abbreviated ids included, is `None`
  pub fn parse(text: &str) -> Option<Self> {
    let (previous_head, new_head, tip, [virtual_branch, unapplied_branch]) = parse_move_token(text, UNAPPLY_TOKEN)?;
    Some(Self {
      previous_head,
      new_head,
      tip,
      virtual_branch,
      unapplied_branch,
    })
  }

  /// Whether `text` claims to be an unapply token, valid or not
  pub fn is_unapply_token(text: &str) -> bool {
    text.starts_with(UNAPPLY_TOKEN)
  }
}

impl std::fmt::Display for UnapplyUndoToken {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{UNAPPLY_TOKEN}{}..{}:{}:{}:{}",
      self.previous_head, self.new_head, self.tip, self.virtual_branch, self.unapplied_branch
    )
  }
}

/// Undo an unapply: HEAD moves back from the token's new head to its previous head with the files, and the unapplied
/// branch back to the virtual one, in one transaction (`check_unapply_undo`, the trees swapped), checked before the
/// repository lock, which holds only the witness and the move (`lock_checked_undo`).
/// `HeadMoved` when HEAD moved on, `NotAnUpdate` when the last move of the branch is not this unapply, `UndoBlocked`
/// when local changes sit on a path the undo writes; a moved unapplied branch, or a virtual branch of the name that
/// exists again, fails the transaction and changes nothing. Undoing again is no error: `already_undone`
#[instrument(skip(git_executor))]
pub fn undo_unapply_core(git_executor: &GitCommandExecutor, repo_path: &str, token: &UnapplyUndoToken, lock_wait: LockWait) -> Result<UndoneUnapply> {
  // the guards, the lock, the mark and the move take the directories read here
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
  let (undo, _lock) = lock_checked_undo(lock_wait, git_executor, repo_path, |git| {
    check_unapply_undo(git, repo_path, &token.previous_head, &token.new_head)
  })?;
  let UndoCheck::Ready(undo) = undo else {
    return Ok(UndoneUnapply { already_undone: true });
  };
  let (virtual_ref, unapplied_ref) = (format!("refs/heads/{}", token.virtual_branch), format!("refs/heads/{}", token.unapplied_branch));
  apply_unapply_undo(
    git_executor,
    repo_path,
    &undo,
    &UnappliedMove {
      virtual_ref: &virtual_ref,
      unapplied_ref: &unapplied_ref,
      tip: &token.tip,
    },
  )?;
  info!(branch = %token.virtual_branch, "undid the unapply");
  Ok(UndoneUnapply { already_undone: false })
}
