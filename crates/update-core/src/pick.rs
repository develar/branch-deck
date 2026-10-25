//! `bd pick`: the commits of another worktree landed on the checked out branch, the source worktree following with
//! `follow` (`git_ops::pick`, spec/cli/pick.spec.md), and the undo of both. The check runs before the repository lock,
//! the witness and the moves under it. After the moves, one read of how the branch groups tells the group of each
//! picked commit, the branches whose stack parent a `(parent/leaf)` prefix changes, and the picked commits whose branch
//! is not built on the branches they need.

use anyhow::{Result, anyhow};
use domain_types::commit::Commit;
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::get_commit_list_at;
use git_ops::pick::{AppliedPick, PickCheck, PickPlan, PickRequest, PickSource, PickState, PickedCommit, apply_pick, check_pick, check_pick_undo, recheck_pick};
use git_ops::revs::{git_common_dir, is_object_id, with_dirs_of};
use git_ops::update_branch::UndoBlocked;
use git_ops::update_branch::{UndoCheck, UpdateUndo, apply_update_undo, finish_pending_update, pending_update};
use history_rewrite::commit_grouper::{GroupingInputs, issue_group_of};
use history_rewrite::repository_lock::{RewriteOptions, lock_repository};
use history_rewrite::snapshot::{Grouped, group_commits, read_grouping_inputs};
use history_rewrite::stack::{StackForest, stack_top};
use object_cache::cache::ObjectCache;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;
use sync_core::baseline::baseline_tracking_ref;
use sync_utils::commit_subject::parse_subject;
use tracing::{debug, info, instrument, warn};
use trial_check::dependencies::compute_dependencies;
use trial_check::diff_scan::read_commit_diffs_cached;

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

/// A picked commit whose prefix puts it into a branch that is not built on every branch it needs: its hunks change
/// lines (or a file) that commits of those branches wrote, so the branch can't sync
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickDependency {
  /// The copy on the branch (with a dry run: the one the pick writes)
  pub commit: String,
  pub subject: String,
  /// The branch its prefix names
  pub prefix_branch: String,
  /// The branches it needs that `prefix_branch` is not built on, sorted
  pub depends_on: Vec<String>,
  /// The regroup that builds it on what it needs; `None` when no branch is built on all of them
  pub fix: Option<DependencyFix>,
}

/// How a picked commit gets the branches it needs; offered, never applied by the pick
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyFix {
  /// Move the commit into this branch, which is built on all of them (a `bd group` plan)
  Move(String),
  /// The commit needs its own branch too: stack `prefix_branch` on this branch (`bd restack <prefix_branch> <parent>`),
  /// which is built on all of them and on the current parent
  Restack(String),
}

#[derive(Debug)]
pub struct PickResult {
  pub check: PickCheck,
  /// Per commit of `check.picked`, in its order
  pub groups: Vec<PickedGroup>,
  /// The picked commits whose branch is not built on what they need; only for a prefix that names a branch the target
  /// has already
  pub dependencies: Vec<PickDependency>,
  /// Branch -> the parent that a `(parent/leaf)` prefix of a picked commit declares for a branch the target has
  /// already, with no or another parent before the pick
  pub reparented: BTreeMap<String, String>,
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

/// How many times a pick checks without the repository lock: a writer that moves the target or the source between the
/// check and the lock sends it back to check again, and after these the check runs under the lock, where neither moves
const CHECKS_WITHOUT_THE_LOCK: usize = 2;

/// Pick the commits of `params.from` onto the branch checked out at `params.repository_path`. The check (the range, the
/// replay, the dry runs of both worktrees on copies of their index) runs before the repository lock, which every
/// worktree shares, and a check that moves nothing (a conflict, local changes in the way, every commit present) is the
/// answer without it. Under the lock only the witness (`recheck_pick`: the branch, both HEADs, the upstream, the local
/// changes on the paths the moves write), then the move of the target and of the source (`land`). After the lock, how
/// the branch groups with the picked commits (`landing`), from the head and the commits the check froze: a report that
/// decides nothing, so its `log` and the diffs of its dependency check never make another writer wait. `cache` and
/// `file_diffs` give those diffs
#[instrument(skip(git_executor, options, cache, file_diffs))]
pub fn pick_core(git_executor: &GitCommandExecutor, params: &PickParams, options: &RewriteOptions<'_>, cache: &ObjectCache, file_diffs: &FileDiffs) -> Result<PickResult> {
  let target = params.repository_path.as_str();
  // the steps on the target take its directories from here; those on the source worktree read its own
  let git_executor = &with_dirs_of(git_executor, target)?;
  let (baseline, upstream) = baseline_tracking_ref(git_executor, target)?;
  let source = match &params.from {
    PickFrom::Worktree(dir) => PickSource::Worktree(dir),
    PickFrom::Revisions(text) => PickSource::Revisions(text),
  };
  let request = PickRequest {
    source,
    upstream: upstream.as_deref(),
    follow: params.follow,
  };
  let (plan, applied) = if params.dry_run {
    let plan = check_pick(git_executor, target, &request)?;
    options.ensure_head(git_executor, target, &plan.check.head)?;
    (plan, None)
  } else {
    let mut worktrees = vec![target];
    if let (true, PickFrom::Worktree(source)) = (params.follow, &params.from) {
      worktrees.push(source);
    }
    land(git_executor, &worktrees, &request, options)?
  };
  let landing = if plan.check.picked.is_empty() {
    Landing::default()
  } else {
    let diffs = Diffs { cache, file_diffs };
    match landing(git_executor, target, baseline.as_deref(), &plan.check.head, &plan.check.picked, &diffs) {
      Ok(landing) => landing,
      // the pick is done: its result and undo token stand without the report
      Err(e) if applied.is_some() => {
        warn!(error = %format!("{e:#}"), "cannot read how the picked commits land");
        Landing::default()
      }
      Err(e) => return Err(e),
    }
  };
  Ok(PickResult {
    check: plan.check,
    groups: landing.groups,
    dependencies: landing.dependencies,
    reparented: landing.reparented,
    applied,
  })
}

/// The check and the moves of a pick into `worktrees[0]`, the source following when it is `worktrees[1]`. The check runs
/// without the lock, then the wait for the index locks of other processes (`PickPlan::wait_for_index_locks`, so nobody
/// waits for the repository lock while this pick waits for git's), and under it the witness: a target or a source that moved in between (another session's pick, a
/// regroup, a fetch) sends the pick back to check again, `CHECKS_WITHOUT_THE_LOCK` times, then once under the lock. An
/// interrupted move of either worktree (its mark, `pending_update`) has its files follow under the lock before a check
/// reads them. `apply_pick` of a check that moves nothing runs no git, so it needs no lock
fn land(git_executor: &GitCommandExecutor, worktrees: &[&str], request: &PickRequest<'_>, options: &RewriteOptions<'_>) -> Result<(PickPlan, Option<AppliedPick>)> {
  let target = worktrees[0];
  let apply = |plan: &PickPlan| (plan.check.state == PickState::Clean).then(|| apply_pick(git_executor, target, plan)).transpose();
  for _ in 0..CHECKS_WITHOUT_THE_LOCK {
    if options.held_lock.is_some() || any_pending(git_executor, worktrees)? {
      break;
    }
    let mut plan = check_pick(git_executor, target, request)?;
    options.ensure_head(git_executor, target, &plan.check.head)?;
    if plan.check.state != PickState::Clean || !plan.moves_anything() {
      let applied = apply(&plan)?;
      return Ok((plan, applied));
    }
    plan.wait_for_index_locks(git_executor, target)?;
    let lock = options.lock(git_executor, target)?;
    if finish_pending(git_executor, worktrees)? || !recheck_pick(git_executor, target, &mut plan)? {
      info!("the target or the source moved during the check of the pick: it checks again");
      drop(lock);
      continue;
    }
    let applied = apply(&plan)?;
    return Ok((plan, applied));
  }
  let _lock = options.lock(git_executor, target)?;
  finish_pending(git_executor, worktrees)?;
  let plan = check_pick(git_executor, target, request)?;
  options.ensure_head(git_executor, target, &plan.check.head)?;
  let applied = apply(&plan)?;
  Ok((plan, applied))
}

/// Whether a move of one of `worktrees` was interrupted before its files followed: reads only
fn any_pending(git_executor: &GitCommandExecutor, worktrees: &[&str]) -> Result<bool> {
  for worktree in worktrees {
    if pending_update(git_executor, worktree)?.is_some() {
      return Ok(true);
    }
  }
  Ok(false)
}

/// Let the files of an interrupted move of each of `worktrees` follow, under the lock; whether one was
fn finish_pending(git_executor: &GitCommandExecutor, worktrees: &[&str]) -> Result<bool> {
  let mut finished = false;
  for worktree in worktrees {
    finished |= finish_pending_update(git_executor, worktree)?.is_some();
  }
  Ok(finished)
}

/// Where the diffs of the dependency check come from
struct Diffs<'a> {
  cache: &'a ObjectCache,
  file_diffs: &'a FileDiffs,
}

/// How the picked commits land in the groups of the target
#[derive(Default)]
struct Landing {
  groups: Vec<PickedGroup>,
  dependencies: Vec<PickDependency>,
  reparented: BTreeMap<String, String>,
}

/// How the picked commits group on the target, from one read before the move: the grouping inputs (`git config`, one
/// `cat-file --batch`) and the commits ahead of the baseline (one `log`), grouped without and with the copies on top.
/// Only when a prefix of a picked commit names a branch the target has, the diffs of the commits ahead and of the copies
/// (`read_commit_diffs_cached`) give the edges by lines (`compute_dependencies`, no symbols)
fn landing(git_executor: &GitCommandExecutor, target: &str, baseline: Option<&str>, head: &str, picked: &[PickedCommit], diffs: &Diffs) -> Result<Landing> {
  let grouping = read_grouping_inputs(git_executor, target)?;
  let groups: Vec<PickedGroup> = picked.iter().map(|commit| picked_group(&grouping, &commit.subject)).collect();
  let Some(baseline) = baseline else {
    return Ok(Landing { groups, ..Landing::default() });
  };
  // the target as the check saw it: after the move, HEAD has the picked commits already
  let mut commits = get_commit_list_at(git_executor, target, baseline, head)?;
  let before = group_commits(&commits, &grouping);
  let existing: BTreeSet<&str> = before.groups.values().flatten().map(String::as_str).collect();
  let picked_at = commits.len();
  commits.extend(picked.iter().map(copy_commit));
  let after = group_commits(&commits, &grouping);

  let mut reparented = BTreeMap::new();
  if let (Some(before_forest), Some(after_forest)) = (&before.forest, &after.forest) {
    for group in groups.iter().filter_map(|group| group.group.as_deref()).filter(|group| existing.contains(group)) {
      if let Some(parent) = after_forest.declared_parent(group)
        && before_forest.declared_parent(group) != Some(parent)
      {
        reparented.insert(group.to_string(), parent.to_string());
      }
    }
  }

  // the picked commits whose prefix names a branch the target has
  let checked: Vec<(usize, &str)> = groups
    .iter()
    .enumerate()
    .filter(|(_, group)| group.grouped_by == GroupedBy::Prefix)
    .filter_map(|(index, group)| group.group.as_deref().filter(|group| existing.contains(group)).map(|group| (index, group)))
    .collect();
  let dependencies = if checked.is_empty() {
    Vec::new()
  } else {
    let ids: Vec<String> = commits.iter().map(|commit| commit.id.clone()).collect();
    let edges = compute_dependencies(&read_commit_diffs_cached(git_executor, target, diffs.cache, diffs.file_diffs, &ids)?);
    checked
      .into_iter()
      .filter_map(|(index, group)| {
        let position = picked_at + index;
        let needs: BTreeSet<&str> = edges
          .iter()
          .filter(|edge| edge.from == position)
          .filter_map(|edge| after.groups.get(&ids[edge.to]).and_then(Option::as_deref))
          .collect();
        dependency(&picked[index], group, &needs, &after, grouping.stacks)
      })
      .collect()
  };
  debug!(dependencies = dependencies.len(), reparented = reparented.len(), "the landing of the pick");
  Ok(Landing { groups, dependencies, reparented })
}

/// The branch and the branches it is built on, nearest first; only the branch with stacked branches off
fn chain_of<'a>(forest: &'a StackForest, branch: &'a str, stacks: bool) -> Vec<&'a str> {
  if stacks { forest.chain(branch) } else { vec![branch] }
}

/// The copy of a picked commit as the grouping sees it: its id and subject
fn copy_commit(commit: &PickedCommit) -> Commit {
  Commit {
    id: commit.new_id.clone(),
    stripped_subject: commit.subject.clone(),
    message: commit.subject.clone(),
    author_name: String::new(),
    author_email: String::new(),
    author_timestamp: 0,
    committer_timestamp: 0,
    author_timezone: String::new(),
    committer_timezone: String::new(),
    subject: commit.subject.clone(),
    parent_id: None,
    tree_id: String::new(),
    note: None,
    mapped_commit_id: None,
    mapped_source: None,
    mapped_resolution: None,
  }
}

/// The dependency of a picked commit of `group` that needs the commits of the branches `needs`: `None` when `group` is
/// built on each of them (or is one). The fix: the commit moves into the branch of the stack that holds all of them; a
/// commit that needs its own branch too stacks the branch instead, on a branch that holds them and its parent
fn dependency(commit: &PickedCommit, group: &str, needs: &BTreeSet<&str>, after: &Grouped, stacks: bool) -> Option<PickDependency> {
  let empty = StackForest::default();
  let forest = after.forest.as_ref().unwrap_or(&empty);
  let built_on = chain_of(forest, group, stacks);
  let depends_on: Vec<String> = needs.iter().filter(|need| !built_on.contains(need)).map(|need| (*need).to_string()).collect();
  if depends_on.is_empty() {
    return None;
  }
  let fix = if needs.contains(group) {
    let mut owners: BTreeSet<&str> = needs.iter().copied().filter(|need| *need != group).collect();
    owners.extend(forest.declared_parent(group));
    stack_top(&owners, stacks, |branch| chain_of(forest, branch, stacks))
      .filter(|top| stacks && !chain_of(forest, top, stacks).contains(&group))
      .map(|top| DependencyFix::Restack(top.to_string()))
  } else {
    stack_top(needs, stacks, |branch| chain_of(forest, branch, stacks)).map(|top| DependencyFix::Move(top.to_string()))
  };
  Some(PickDependency {
    commit: commit.new_id.clone(),
    subject: commit.subject.clone(),
    prefix_branch: group.to_string(),
    depends_on,
    fix,
  })
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
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
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
  let common_dir = git_common_dir(git_executor, repo_path)?;
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
