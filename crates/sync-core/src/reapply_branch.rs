//! Reapply an unapplied branch (`{prefix}/unapplied/<name>`): its commits back on top of HEAD with their grouping
//! prefix, the files following, the unapplied branch deleted in the transaction that moves HEAD, under the repository
//! lock (`git_ops::reapply`, spec/branch/unapply.spec.md); and its undo (`ReapplyUndoToken`).

use crate::baseline::baseline_tracking_ref;
use crate::unapply_branch::{checked_undo, parse_move_token};
use anyhow::Result;
use branch_integration::archive::own_commits_start;
use domain_types::branch_names::{BranchKind, normalize_branch_prefix, qualified_branch_name};
use domain_types::error::ErrorCode;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::pick::{PickRequest, PickSource, PickState, check_pick};
use git_ops::reapply::{UnappliedRef, apply_reapply, apply_reapply_undo, check_reapply_undo, restore_chain_messages};
use git_ops::revs::with_dirs_of;
use git_ops::unapply::MoveBlocked;
use git_ops::update_branch::{REAPPLY, finish_pending_update};
use history_rewrite::repository_lock::{LockWait, RewriteOptions};
use history_rewrite::snapshot::read_grouping_inputs;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReapplyBranchParams {
  pub repository_path: String,
  /// The name of the unapplied branch, `<name>` of `{prefix}/unapplied/<name>`
  pub branch_name: String,
  /// Check only: nothing is written but git objects, and no lock is taken
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub dry_run: bool,
}

/// A commit of the unapplied branch that the reapply put on top of HEAD
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReappliedCommit {
  /// The commit of the unapplied branch (or of its restored chain)
  pub commit: String,
  /// Its copy on HEAD (with a dry run: the one the reapply writes)
  pub new_id: String,
  pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReapplyBranchResult {
  /// `{prefix}/unapplied/<name>`, deleted now (with a dry run: would be)
  pub unapplied_branch_name: String,
  /// The branch its commits group into again
  pub branch: String,
  /// Oldest first
  pub reapplied: Vec<ReappliedCommit>,
  /// The commits HEAD has already, by their patch or their content: left out
  pub present: Vec<String>,
  /// How many messages got the grouping prefix back (`(<name>) `): a virtual branch keeps none, so a commit whose
  /// subject groups elsewhere gets it
  pub prefix_restored: u32,
  pub previous_head: String,
  pub new_head: String,
  /// `ReapplyUndoToken` as it prints; `None` for a dry run
  pub undo_token: Option<String>,
}

/// A reapply refused before anything was written but git objects
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReapplyRefusal {
  #[error("No unapplied branch {0}")]
  UnknownBranch(String),
  #[error("{branch} is checked out in {worktree}: switch that worktree to another branch first")]
  CheckedOut { branch: String, worktree: String },
  #[error("{0} has no common history with the baseline: nothing tells its own commits")]
  NoCommonHistory(String),
  #[error("HEAD has every commit of {0} already: delete the unapplied branch")]
  AlreadyInHead(String),
  /// A commit does not apply onto HEAD; nothing was changed
  #[error("{subject} ({commit}) of {branch} conflicts with HEAD in {}: nothing was changed", .files.join(", "))]
  Conflict {
    branch: String,
    commit: String,
    subject: String,
    files: Vec<String>,
  },
}

impl ReapplyRefusal {
  /// The code of the refusal, the same in `bd` and in the app; a conflict names its commit and files besides
  pub const fn code(&self) -> ErrorCode {
    match self {
      Self::UnknownBranch(_) => ErrorCode::UnknownBranch,
      Self::CheckedOut { .. } => ErrorCode::BranchCheckedOut,
      Self::NoCommonHistory(_) | Self::AlreadyInHead(_) => ErrorCode::NothingToReapply,
      Self::Conflict { .. } => ErrorCode::ReapplyConflict,
    }
  }
}

/// Reapply the unapplied branch `params.branch_name` (named with `branch_prefix`, which the caller resolved): its own commits (from its stack note, else its merge base with the
/// baseline) get their grouping prefix back where their subject groups elsewhere, then they are picked onto HEAD
/// (`check_pick`: a commit HEAD has is left out, a conflict or local changes in the way refuse it), and HEAD moves with
/// the files while the unapplied branch is deleted, in one transaction. Under the repository lock unless a dry run
#[instrument(skip(git_executor, options), fields(repo = %params.repository_path, branch = %params.branch_name))]
pub fn reapply_branch_core(git_executor: &GitCommandExecutor, params: &ReapplyBranchParams, branch_prefix: &str, options: &RewriteOptions) -> Result<ReapplyBranchResult> {
  let repo = params.repository_path.as_str();
  let name = params.branch_name.as_str();
  let full_name = qualified_branch_name(normalize_branch_prefix(branch_prefix), BranchKind::Unapplied, name)?;
  // the guards and the lock take the directories read here
  let git_executor = &with_dirs_of(git_executor, repo)?;
  let _lock = if params.dry_run { None } else { options.lock(git_executor, repo)? };
  if !params.dry_run {
    finish_pending_update(git_executor, repo)?;
  }
  let full_ref = format!("refs/heads/{full_name}");
  let listed = git_executor.execute_command(&["for-each-ref", "--format=%(objectname)%00%(worktreepath)", &full_ref], repo)?;
  let Some((tip, worktree)) = listed.lines().next().and_then(|line| line.split_once('\0')) else {
    return Err(ReapplyRefusal::UnknownBranch(full_name).into());
  };
  if !worktree.is_empty() {
    return Err(
      ReapplyRefusal::CheckedOut {
        branch: full_name,
        worktree: worktree.to_string(),
      }
      .into(),
    );
  }
  let (baseline, upstream) = baseline_tracking_ref(git_executor, repo)?;
  let baseline = baseline.ok_or_else(|| anyhow::Error::new(history_rewrite::baseline::NoBaseline::NoCommits))?;
  let start = own_commits_start(git_executor, repo, &baseline, tip)?.ok_or_else(|| anyhow::Error::new(ReapplyRefusal::NoCommonHistory(full_name.clone())))?;

  // a virtual commit has no grouping prefix: the subject that groups elsewhere gets `(<name>) ` back
  let grouping = read_grouping_inputs(git_executor, repo)?;
  let chain = restore_chain_messages(git_executor, repo, &start, tip, |subject, message| {
    (grouping.branch_of(subject).as_deref() != Some(name)).then(|| [format!("({name}) ").as_bytes(), message].concat())
  })?;
  let range = format!("{start}..{}", chain.tip);
  let plan = check_pick(
    git_executor,
    repo,
    &PickRequest {
      source: PickSource::Revisions(&range),
      upstream: upstream.as_deref(),
      follow: false,
      expect_branch: None,
    },
  )
  .inspect_err(|_| chain.discard(git_executor, repo))?;
  let refused = match &plan.check.state {
    _ if let Err(moved) = options.ensure_head(git_executor, repo, &plan.check.head) => Some(anyhow::Error::new(moved)),
    PickState::Conflict(conflict) => Some(anyhow::Error::new(ReapplyRefusal::Conflict {
      branch: full_name.clone(),
      commit: conflict.commit.id.clone(),
      subject: conflict.commit.subject.clone(),
      files: conflict.files.clone(),
    })),
    PickState::Blocked { paths, .. } => Some(anyhow::Error::new(MoveBlocked::new(REAPPLY.done, paths.clone()))),
    PickState::Clean if plan.check.picked.is_empty() => Some(anyhow::Error::new(ReapplyRefusal::AlreadyInHead(full_name.clone()))),
    PickState::Clean => None,
  };
  if let Some(refused) = refused {
    chain.discard(git_executor, repo);
    return Err(refused);
  }
  let mut result = ReapplyBranchResult {
    unapplied_branch_name: full_name.clone(),
    branch: name.to_string(),
    reapplied: plan
      .check
      .picked
      .iter()
      .map(|commit| ReappliedCommit {
        commit: commit.commit.clone(),
        new_id: commit.new_id.clone(),
        subject: commit.subject.clone(),
      })
      .collect(),
    present: plan.check.present.iter().map(|commit| commit.commit.clone()).collect(),
    prefix_restored: u32::try_from(chain.restored).unwrap_or(u32::MAX),
    previous_head: plan.check.head.clone(),
    new_head: plan.check.new_head.clone(),
    undo_token: None,
  };
  if params.dry_run {
    chain.discard(git_executor, repo);
    return Ok(result);
  }
  let applied = apply_reapply(git_executor, repo, &plan, &UnappliedRef { full_ref: &full_ref, tip }, &chain)?;
  result.new_head.clone_from(&applied.new_head);
  result.undo_token = Some(
    ReapplyUndoToken {
      previous_head: result.previous_head.clone(),
      new_head: applied.new_head,
      tip: tip.to_string(),
      unapplied_branch: full_name,
    }
    .to_string(),
  );
  info!(reapplied = result.reapplied.len(), prefix_restored = result.prefix_restored, "reapplied the branch");
  Ok(result)
}

/// The undo token of a reapply: `reapply:<previous head>..<new head>:<tip>:<unapplied branch>`, full ids and the full
/// name of the unapplied branch it deleted (a ref name has no `:`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReapplyUndoToken {
  pub previous_head: String,
  pub new_head: String,
  /// The tip of the unapplied branch, which the undo creates again
  pub tip: String,
  pub unapplied_branch: String,
}

const REAPPLY_TOKEN: &str = "reapply:";

impl ReapplyUndoToken {
  /// A token as `Display` prints it; anything else, abbreviated ids included, is `None`
  pub fn parse(text: &str) -> Option<Self> {
    let (previous_head, new_head, tip, [unapplied_branch]) = parse_move_token(text, REAPPLY_TOKEN)?;
    Some(Self {
      previous_head,
      new_head,
      tip,
      unapplied_branch,
    })
  }

  /// Whether `text` claims to be a reapply token, valid or not
  pub fn is_reapply_token(text: &str) -> bool {
    text.starts_with(REAPPLY_TOKEN)
  }
}

impl std::fmt::Display for ReapplyUndoToken {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{REAPPLY_TOKEN}{}..{}:{}:{}", self.previous_head, self.new_head, self.tip, self.unapplied_branch)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UndoReapplyParams {
  pub repository_path: String,
  /// The `undoToken` of the reapply
  pub undo_token: String,
}

/// What the undo of a reapply did
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UndoneReapply {
  /// It was undone before: nothing changed now
  pub already_undone: bool,
}

/// Undo a reapply, under the repository lock: HEAD moves back with the files and the unapplied branch is created again
/// at its tip, in one transaction (`check_reapply_undo`). `HeadMoved` when HEAD moved on, `NotAnUpdate` when the last move
/// of the branch is not this reapply, `UndoBlocked` when local changes sit on a path the undo writes; an unapplied branch
/// of the name that exists again fails the transaction. Undoing again is no error: `already_undone`
#[instrument(skip(git_executor))]
pub fn undo_reapply_core(git_executor: &GitCommandExecutor, repo_path: &str, token: &ReapplyUndoToken, lock_wait: LockWait) -> Result<UndoneReapply> {
  let (_lock, undo) = checked_undo(git_executor, repo_path, lock_wait, || {
    check_reapply_undo(git_executor, repo_path, &token.previous_head, &token.new_head)
  })?;
  let Some(undo) = undo else {
    return Ok(UndoneReapply { already_undone: true });
  };
  let full_ref = format!("refs/heads/{}", token.unapplied_branch);
  apply_reapply_undo(
    git_executor,
    repo_path,
    &undo,
    &UnappliedRef {
      full_ref: &full_ref,
      tip: &token.tip,
    },
  )?;
  info!(branch = %token.unapplied_branch, "undid the reapply");
  Ok(UndoneReapply { already_undone: false })
}
