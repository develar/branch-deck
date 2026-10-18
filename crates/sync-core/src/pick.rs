//! `bd pick`: the commits of another worktree landed on the checked out branch under the repository lock, the source
//! worktree following with `follow` (`git_ops::pick`, spec/cli/pick.spec.md), and the undo of both.

use crate::baseline::{NoBaseline, detect_baseline_branch};
use crate::commit_grouper::{GroupingInputs, issue_group_of, parse_subject};
use crate::repository_lock::{RewriteOptions, lock_repository};
use crate::snapshot::read_grouping_inputs;
use crate::update_project::{UndoBlocked, resolve_baseline_upstream};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::pick::{AppliedPick, PickCheck, PickRequest, PickSource, PickState, apply_pick, check_pick, check_pick_undo};
use git_ops::revs::is_object_id;
use git_ops::update_branch::{UndoCheck, UpdateUndo, apply_update_undo, finish_pending_update};
use std::path::Path;
use std::time::Duration;
use tracing::{info, instrument};

/// Where `bd pick` takes the commits from
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickFrom {
  /// The HEAD of the worktree that holds this directory
  Worktree(String),
  /// `<rev>` or `<a>..<b>`
  Revisions(String),
}

#[derive(Debug, Clone)]
pub struct PickParams {
  /// The target: a directory of the worktree whose checked out branch gets the commits
  pub repository_path: String,
  pub from: PickFrom,
  /// Move the source worktree onto the new tip too
  pub follow: bool,
  /// Check only: nothing is written but git objects, and no lock is taken
  pub dry_run: bool,
}

/// What groups a picked commit on the target, by its subject
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupedBy {
  /// Its `(name)` prefix
  Prefix,
  /// The issue group of its issue key (`refs/branch-deck/issue-groups`)
  IssueGroup,
  /// Its issue key
  Issue,
  /// Nothing: an unassigned commit, written without a prefix
  None,
}

/// The branch a picked commit lands in, and why
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedGroup {
  pub group: Option<String>,
  pub grouped_by: GroupedBy,
}

#[derive(Debug)]
pub struct PickResult {
  pub check: PickCheck,
  /// Per commit of `check.picked`, in its order
  pub groups: Vec<PickedGroup>,
  /// `None` for a dry run and for a pick that is not clean
  pub applied: Option<AppliedPick>,
}

impl PickResult {
  /// The undo token of an applied pick that moved anything; the source worktree is part of it when it moved
  pub fn undo_token(&self) -> Option<PickUndoToken> {
    let applied = self.applied.as_ref()?;
    let follow = self
      .check
      .follow
      .as_ref()
      .filter(|follow| follow.head != applied.new_head && applied.followed == Some(git_ops::pick::FollowState::Moved));
    let token = PickUndoToken {
      head: (applied.new_head != self.check.head).then(|| (self.check.head.clone(), applied.new_head.clone())),
      follow: follow.map(|follow| FollowToken {
        worktree_id: follow.worktree_id.clone(),
        previous_head: follow.head.clone(),
        new_head: applied.new_head.clone(),
      }),
    };
    (token.head.is_some() || token.follow.is_some()).then_some(token)
  }
}

/// Pick the commits of `params.from` onto the branch checked out at `params.repository_path`. Under the repository lock
/// (shared by every worktree), after an interrupted move of the target (and of the source with `follow`) is finished:
/// the check, then, when it is clean, the move of the target and of the source. A conflict or local changes in the way
/// change nothing (`check.state`)
#[instrument(skip(git_executor, options))]
pub fn pick_core(git_executor: &GitCommandExecutor, params: &PickParams, options: &RewriteOptions<'_>) -> Result<PickResult> {
  let target = params.repository_path.as_str();
  let _lock = if params.dry_run { None } else { options.lock(git_executor, target)? };
  if !params.dry_run {
    finish_pending_update(git_executor, target)?;
    if let (true, PickFrom::Worktree(source)) = (params.follow, &params.from) {
      finish_pending_update(git_executor, source)?;
    }
  }
  let upstream = baseline_tracking_ref(git_executor, target)?;
  let source = match &params.from {
    PickFrom::Worktree(dir) => PickSource::Worktree(dir),
    PickFrom::Revisions(text) => PickSource::Revisions(text),
  };
  let plan = check_pick(
    git_executor,
    target,
    &PickRequest {
      source,
      upstream: upstream.as_deref(),
      follow: params.follow,
    },
  )?;
  options.ensure_head(git_executor, target, &plan.check.head)?;
  let applied = if !params.dry_run && plan.check.state == PickState::Clean {
    Some(apply_pick(git_executor, target, &plan)?)
  } else {
    None
  };
  let groups = if plan.check.picked.is_empty() {
    Vec::new()
  } else {
    let grouping = read_grouping_inputs(git_executor, target)?;
    plan.check.picked.iter().map(|commit| picked_group(&grouping, &commit.subject)).collect()
  };
  Ok(PickResult {
    check: plan.check,
    groups,
    applied,
  })
}

/// The remote-tracking ref of the target's baseline, `None` for a local baseline or none
fn baseline_tracking_ref(git_executor: &GitCommandExecutor, target: &str) -> Result<Option<String>> {
  let baseline = match detect_baseline_branch(git_executor, target) {
    Ok(baseline) => baseline,
    Err(e) if e.downcast_ref::<NoBaseline>().is_some() => return Ok(None),
    Err(e) => return Err(e),
  };
  Ok(resolve_baseline_upstream(git_executor, target, &baseline)?.map(|upstream| upstream.tracking_ref))
}

/// The branch a commit with this subject lands in on the target, and what decides it
pub fn picked_group(grouping: &GroupingInputs, subject: &str) -> PickedGroup {
  let group = grouping.branch_of(subject);
  let grouped_by = if group.is_none() {
    GroupedBy::None
  } else if parse_subject(subject).prefix.is_some() {
    GroupedBy::Prefix
  } else if issue_group_of(subject, &grouping.issue_groups.entries).is_some() {
    GroupedBy::IssueGroup
  } else {
    GroupedBy::Issue
  };
  PickedGroup { group, grouped_by }
}

/// The undo token of an applied pick: `pick:<previous head>..<new head>` of the target, with
/// `+follow:<worktree id>:<previous head>..<new head>` when the source worktree moved; full ids
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickUndoToken {
  /// The target's move; `None` when only the source moved (a pick again after a follow that failed)
  pub head: Option<(String, String)>,
  pub follow: Option<FollowToken>,
}

/// The move of the source worktree in a `PickUndoToken`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowToken {
  pub worktree_id: String,
  pub previous_head: String,
  pub new_head: String,
}

const PICK_TOKEN: &str = "pick:";
const FOLLOW_PART: &str = "+follow:";

/// `<a>..<b>` of two different full ids of one length
fn parse_move(text: &str) -> Option<(String, String)> {
  let (previous, new) = text.split_once("..")?;
  (is_object_id(previous) && is_object_id(new) && previous.len() == new.len() && previous != new).then(|| (previous.to_string(), new.to_string()))
}

impl PickUndoToken {
  /// A token as `Display` prints it; anything else, abbreviated ids included, is `None`
  pub fn parse(text: &str) -> Option<Self> {
    let rest = text.strip_prefix(PICK_TOKEN)?;
    let (head, follow) = match rest.split_once(FOLLOW_PART) {
      Some((head, follow)) => (head, Some(follow)),
      None => (rest, None),
    };
    let head = if head.is_empty() { None } else { Some(parse_move(head)?) };
    let follow = match follow {
      Some(follow) => {
        let (worktree_id, range) = follow.rsplit_once(':')?;
        let (previous_head, new_head) = parse_move(range)?;
        // a worktree id is a name under `$GIT_COMMON_DIR/worktrees`, never a path
        if worktree_id.is_empty() || worktree_id.contains(['/', '\\']) || worktree_id == "." || worktree_id == ".." {
          return None;
        }
        Some(FollowToken {
          worktree_id: worktree_id.to_string(),
          previous_head,
          new_head,
        })
      }
      None => None,
    };
    (head.is_some() || follow.is_some()).then_some(Self { head, follow })
  }

  /// Whether `text` claims to be a pick token, valid or not
  pub fn is_pick_token(text: &str) -> bool {
    text.starts_with(PICK_TOKEN)
  }
}

impl std::fmt::Display for PickUndoToken {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(PICK_TOKEN)?;
    if let Some((previous, new)) = &self.head {
      write!(f, "{previous}..{new}")?;
    }
    if let Some(follow) = &self.follow {
      write!(f, "{FOLLOW_PART}{}:{}..{}", follow.worktree_id, follow.previous_head, follow.new_head)?;
    }
    Ok(())
  }
}

/// What the undo of a pick did
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndonePick {
  /// The target moved back; `false` when the token has no move of it, or it was undone before
  pub target_undone: bool,
  /// The source worktree moved back; `false` when the token has no move of it, or it was undone before
  pub follow_undone: bool,
  /// Every move of the token was undone before: nothing changed now
  pub already_undone: bool,
}

/// Undo an applied pick, under the repository lock: the target branch moves back from the token's new head to its
/// previous head, and the source worktree that followed moves back too, each with its files, through the same guarded
/// update with the trees swapped (`check_pick_undo`). Both are checked before either moves: `HeadMoved` when one moved
/// on, `NotAnUpdate` when its last move is not this pick, `UndoBlocked` when local changes sit on a path the undo
/// writes. Undoing again is no error: `already_undone`
#[instrument(skip(git_executor))]
pub fn undo_pick_core(git_executor: &GitCommandExecutor, repo_path: &str, token: &PickUndoToken, lock_timeout: Duration) -> Result<UndonePick> {
  let _lock = lock_repository(git_executor, repo_path, lock_timeout)?;
  let source = token
    .follow
    .as_ref()
    .map(|follow| worktree_path(git_executor, repo_path, &follow.worktree_id))
    .transpose()?;
  // both checked before either moves; `None` for a move undone before
  let check = |path: &str, previous: &str, new: &str| -> Result<Option<UpdateUndo>> {
    finish_pending_update(git_executor, path)?;
    match check_pick_undo(git_executor, path, previous, new)? {
      UndoCheck::Ready(undo) if !undo.blocked().is_empty() => Err(anyhow::Error::new(UndoBlocked { paths: undo.blocked().to_vec() })),
      UndoCheck::Ready(undo) => Ok(Some(undo)),
      UndoCheck::AlreadyUndone { .. } => Ok(None),
    }
  };
  let target_undo = token.head.as_ref().map(|(previous, new)| check(repo_path, previous, new)).transpose()?;
  let source_undo = match (&token.follow, &source) {
    (Some(follow), Some(source)) => Some(check(source, &follow.previous_head, &follow.new_head)?),
    _ => None,
  };
  let already_undone = !matches!(target_undo, Some(Some(_))) && !matches!(source_undo, Some(Some(_)));
  if let Some(Some(undo)) = &target_undo {
    apply_update_undo(git_executor, repo_path, undo)?;
  }
  if let (Some(Some(undo)), Some(source)) = (&source_undo, &source) {
    apply_update_undo(git_executor, source, undo)?;
  }
  let undone = UndonePick {
    target_undone: matches!(target_undo, Some(Some(_))),
    follow_undone: matches!(source_undo, Some(Some(_))),
    already_undone,
  };
  info!(?undone, "undid the pick");
  Ok(undone)
}

/// The top level of the worktree `id` of the repository of `repo_path`: `main`, the first of `git worktree list`, or a
/// linked one by the `gitdir` file git keeps for it in `$GIT_COMMON_DIR/worktrees/<id>`
fn worktree_path(git_executor: &GitCommandExecutor, repo_path: &str, id: &str) -> Result<String> {
  if id == "main" {
    let output = git_executor.cmd(&["worktree", "list", "--porcelain", "-z"], repo_path).bytes()?;
    let output = String::from_utf8_lossy(&output);
    return output
      .split('\0')
      .find_map(|field| field.strip_prefix("worktree "))
      .map(str::to_string)
      .ok_or_else(|| anyhow!("git worktree list names no main worktree"));
  }
  let common_dir = git_executor.execute_command(&["rev-parse", "--path-format=absolute", "--git-common-dir"], repo_path)?;
  let gitdir_file = Path::new(&common_dir).join("worktrees").join(id).join("gitdir");
  let gitdir = std::fs::read_to_string(&gitdir_file).map_err(|e| anyhow!("The worktree {id} of the undo token is gone ({}: {e})", gitdir_file.display()))?;
  let gitdir = Path::new(gitdir.trim());
  gitdir
    .parent()
    .and_then(Path::to_str)
    .map(str::to_string)
    .ok_or_else(|| anyhow!("Unexpected gitdir of the worktree {id}: {}", gitdir.display()))
}

#[cfg(test)]
#[path = "pick_test.rs"]
mod pick_test;
