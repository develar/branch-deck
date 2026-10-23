//! `bd pick`: land the commits of another worktree (or of a range) on the branch checked out here, without
//! `git cherry-pick` (docs/design/git-operations.md, "Pick"). A commit with the patch id of a target commit is left out
//! before the replay; the others are replayed in memory with `git replay` onto the target's HEAD, in segments between
//! the left out ones, where a commit whose change the target has already becomes empty and drops; a dry run on a copy of
//! the index of each worktree proves its files can follow, and only then does the branch move, through the guarded
//! update of `update_branch`. With `follow`, the source worktree moves onto the new tip the same way, a detached HEAD
//! with `option no-deref`. The check needs no lock: its caller checks before the repository lock and, under it, only
//! the witness (`recheck_pick`) before the moves.

use crate::paths::{COMMIT_FORMAT, ChangedPath, changed_paths, parse_commits};
use crate::ref_transaction::RefTransaction;
use crate::replay::{conflicting_files, first_conflict, replay_range_with_env};
use crate::revs::{RepositoryDirs, fork_point, is_ancestor, merge_base, repository_dirs, resolve_commit};
use crate::rewrite_guard::{HeadMoved, ensure_no_merges};
use crate::update_branch::{
  BranchSwitched, FileMove, HeadAttached, HeadRef, MAX_BLOCKED_PATHS, PICK, UndoCheck, blocking_paths, check_move_undo, check_update_guards, move_branch_and_files,
  move_branch_and_files_carrying, recheck_blocking_paths,
};
use crate::update_resolve::pair_by_patch_id;
use anyhow::{Result, anyhow, bail};
use domain_types::update_project::UpdateCommit;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_utils::list_cut::kept;
use tracing::{debug, info, instrument};

/// Where the commits of a pick come from
#[derive(Debug, Clone, Copy)]
pub enum PickSource<'a> {
  /// The HEAD of the worktree that holds this directory: the commits it has that the target lacks
  Worktree(&'a str),
  /// One commit, `<rev>`, or a range, `<a>..<b>`
  Revisions(&'a str),
}

/// What to pick, and where from
#[derive(Debug, Clone, Copy)]
pub struct PickRequest<'a> {
  pub source: PickSource<'a>,
  /// The remote-tracking ref of the target's baseline, e.g. `refs/remotes/origin/master`, if it has one: the commits
  /// it has are never picked, and a source on a newer baseline is refused
  pub upstream: Option<&'a str>,
  /// Move the source worktree onto the new tip too; only with `PickSource::Worktree`
  pub follow: bool,
}

/// A commit of the range, oldest first
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickCommit {
  pub id: String,
  pub subject: String,
}

/// A commit the pick replays onto the target
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedCommit {
  pub commit: String,
  /// Its copy on the target
  pub new_id: String,
  pub subject: String,
}

/// How the target has the change of a commit of the range
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentBy {
  /// A target commit has its patch id (`log --cherry-mark` of the range): left out before the replay
  PatchId,
  /// `git replay` dropped it as empty: the target has the change in another form
  Empty,
}

/// A commit of the range whose change the target has already
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentCommit {
  pub commit: String,
  pub subject: String,
  /// The target's commit with the same patch id; `None` when the target has the change in another form
  pub on: Option<String>,
  pub by: PresentBy,
}

/// The first commit of the range that does not apply onto the target
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickConflict {
  pub commit: PickCommit,
  /// The commits of the range before it: they apply cleanly
  pub clean: Vec<PickCommit>,
  /// The files the commit conflicts on, at most the first `MAX_BLOCKED_PATHS` (all of them when only one more)
  pub files: Vec<String>,
  pub file_count: u32,
}

/// The source worktree that follows the pick
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowCheck {
  /// Its top level
  pub worktree: String,
  /// `main`, or the name of a linked worktree
  pub worktree_id: String,
  /// Its HEAD before the pick
  pub head: String,
  pub detached: bool,
}

/// Whether the pick can be applied
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickState {
  /// It applies; also when nothing moves (every commit is on the target already)
  Clean,
  /// A commit of the range conflicts with the target; nothing was changed
  Conflict(PickConflict),
  /// Uncommitted, staged or untracked files of `worktree` sit on paths the pick (or the follow) writes; nothing was
  /// changed
  Blocked { worktree: String, paths: Vec<String>, path_count: u32 },
}

/// The result of checking a pick
#[derive(Debug, Clone)]
pub struct PickCheck {
  /// The checked out branch of the target
  pub branch: String,
  pub head: String,
  /// The newest commit of the range: the source worktree's HEAD, or `<b>` of `<a>..<b>`
  pub source_head: String,
  /// The commit the range starts after: the fork point of the source from the target, or `<a>`
  pub base: String,
  /// The commits of the range, oldest first
  pub commits: Vec<PickCommit>,
  /// HEAD after the pick; `head` on a conflict and when every commit is on the target already
  pub new_head: String,
  pub picked: Vec<PickedCommit>,
  /// In the order of the range; on a conflict only those left out by patch id
  pub present: Vec<PresentCommit>,
  pub follow: Option<FollowCheck>,
  pub state: PickState,
}

/// A checked pick; `apply_pick` applies a clean one. Only `check_pick` builds one, so no other crate can move HEAD to
/// another commit through the working-tree exemption
#[derive(Debug)]
pub struct PickPlan {
  pub check: PickCheck,
  /// The upstream the range excludes, and its commit as the check read it: part of the witness (`recheck_pick`)
  upstream: Option<String>,
  upstream_head: Option<String>,
  /// The paths the move of the target changes; empty when its files stay
  changed: Vec<ChangedPath>,
  follow: Option<Follow>,
}

#[derive(Debug)]
struct Follow {
  head_ref: HeadRef,
  /// The paths the move of the source changes
  changed: Vec<ChangedPath>,
}

impl PickPlan {
  /// Whether applying the plan moves a HEAD: the target's, or the source's that follows onto another commit. One that
  /// moves nothing applies without git, so it needs no lock
  pub fn moves_anything(&self) -> bool {
    self.check.new_head != self.check.head || (self.follow.is_some() && self.check.follow.as_ref().is_some_and(|follow| follow.head != self.check.new_head))
  }
}

/// A pick refused before anything was written but git objects
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PickRefusal {
  /// The arguments name no source: a directory outside the repository, the target worktree itself, revisions that are
  /// no commit range, `follow` without a worktree
  #[error("{0}")]
  InvalidSource(String),
  /// The source has no fork point in the target's reflog, and its history with the target is local: commits the target
  /// rewrote since would come back. `<a>..<b>` names the range
  #[error("{0}")]
  RangeUnknown(String),
  /// The source stands on a newer baseline than the target: the target is updated first
  #[error("{0}")]
  SourceOnNewerBaseline(String),
}

/// Check, writing nothing but git objects, whether the commits of `request` apply onto the branch checked out at
/// `target`. Refuses (an error) while a rebase, merge, cherry-pick or revert is in progress in the target (or, with
/// `follow`, in the source), on a detached target HEAD, and when the range contains a merge.
///
/// The range of a worktree is `<source HEAD> ^<target HEAD> ^<upstream> ^<base>`, the base being the fork point of the
/// source in the reflog of the target branch: it sees through a fold, a regroup or new dates that the target got since
/// the worktree was cut from it, so the target's old commits are never picked back.
///
/// A commit of the range with the patch id of a target commit above the base (and the upstream) is left out, and the
/// rest is replayed in segments between the left out ones, each onto the tip of the one before: `git replay
/// --cherry-pick` would replay the child of a left out commit onto `--onto`, a fork of the chain. One more replay per
/// left out commit, none when nothing is left out.
///
/// A range that starts at the target's HEAD is a fast-forward: the commits land as they are. Otherwise the copies keep
/// their authors, the committer is the current user, and the committer date is the newest of the range's and HEAD's
/// (`committer_date`), so the same pick writes the same commits, the dry run's and the real one's.
#[instrument(skip(git_executor))]
pub fn check_pick(git_executor: &GitCommandExecutor, target: &str, request: &PickRequest<'_>) -> Result<PickPlan> {
  let branch = check_update_guards(git_executor, target)?;
  let head_line = git_executor.execute_command(&["log", "-1", "--date=raw", "--format=%H%x00%cd", "HEAD", "--"], target)?;
  let (head, head_date) = head_line.split_once('\0').ok_or_else(|| anyhow!("Unexpected HEAD {head_line:?}"))?;
  let head = head.to_string();
  let upstream_head = match request.upstream {
    Some(upstream) => resolve_commit(git_executor, target, upstream)?,
    None => None,
  };
  let (source_head, base, follow) = match request.source {
    PickSource::Worktree(dir) => {
      let source = source_worktree(git_executor, target, dir)?;
      let head_ref = if request.follow { Some(HeadRef::read(git_executor, &source.top_level)?) } else { None };
      let source_head = git_executor.execute_command(&["rev-parse", "HEAD^{commit}"], &source.top_level)?;
      let base = worktree_base(git_executor, target, &branch, &head, &source_head, upstream_head.as_deref(), dir)?;
      let follow = head_ref.map(|head_ref| {
        (
          FollowCheck {
            worktree: source.top_level.clone(),
            worktree_id: source.worktree_id(),
            head: source_head.clone(),
            detached: head_ref == HeadRef::Detached,
          },
          head_ref,
        )
      });
      (source_head, base, follow)
    }
    PickSource::Revisions(text) => {
      if request.follow {
        return Err(anyhow::Error::new(PickRefusal::InvalidSource(
          "Only the worktree of --from can follow the pick".to_string(),
        )));
      }
      let (from, to) = resolve_revisions(git_executor, target, text)?;
      (to, from, None)
    }
  };

  let mut excluded = vec![head.as_str(), base.as_str()];
  excluded.extend(upstream_head.as_deref());
  ensure_no_merges(git_executor, target, &source_head, &excluded)?;
  let negated: Vec<String> = excluded.iter().map(|rev| format!("^{rev}")).collect();
  let range = range_of(git_executor, target, &head, &source_head, &negated[1..])?;

  let mut check = PickCheck {
    branch,
    head: head.clone(),
    source_head: source_head.clone(),
    base,
    commits: range.iter().map(|commit| pick_commit(&commit.commit)).collect(),
    new_head: head.clone(),
    picked: Vec::new(),
    present: Vec::new(),
    follow: follow.as_ref().map(|(check, _)| check.clone()),
    state: PickState::Clean,
  };
  let by_patch_id: Vec<PickCommit> = range.iter().filter(|commit| commit.same_patch).map(|commit| pick_commit(&commit.commit)).collect();
  if range.first().is_some_and(|oldest| oldest.parent == head) {
    // the target is an ancestor of the source: it has no commit of its own to share a patch id with one of the range
    check.picked = range
      .iter()
      .map(|commit| PickedCommit {
        commit: commit.commit.id.clone(),
        new_id: commit.commit.id.clone(),
        subject: commit.commit.subject.clone(),
      })
      .collect();
    check.new_head.clone_from(&source_head);
  } else if range.iter().any(|commit| !commit.same_patch) {
    let date = committer_date(head_date, &range);
    let env: Vec<(&str, &str)> = date.iter().map(|date| ("GIT_COMMITTER_DATE", date.as_str())).collect();
    let replay = Replay {
      git_executor,
      target,
      range: &range,
      negated: &negated,
      env: &env,
    };
    let mut tip = head.clone();
    for (first, last) in segments(&range) {
      let Some(new_tip) = replay.segment(&tip, first, last)? else {
        check.state = PickState::Conflict(replay.conflict(&tip, first, last)?);
        check.present = present_on(git_executor, target, &head, &source_head, by_patch_id, &[])?;
        return Ok(PickPlan {
          check,
          upstream: request.upstream.map(str::to_string),
          upstream_head,
          changed: Vec::new(),
          follow: None,
        });
      };
      tip = new_tip;
    }
    let kept: Vec<UpdateCommit> = range.iter().filter(|commit| !commit.same_patch).map(|commit| commit.commit.clone()).collect();
    let replayed = commits_of(git_executor, target, &[tip.clone(), format!("^{head}")])?;
    let (picked, dropped) = match_replayed(&kept, &replayed);
    let present: Vec<PickCommit> = range
      .iter()
      .filter(|commit| commit.same_patch || dropped.iter().any(|dropped| dropped.id == commit.commit.id))
      .map(|commit| pick_commit(&commit.commit))
      .collect();
    let dropped: Vec<&str> = dropped.iter().map(|commit| commit.id.as_str()).collect();
    check.present = present_on(git_executor, target, &head, &source_head, present, &dropped)?;
    check.picked = picked;
    check.new_head = tip;
  } else {
    check.present = present_on(git_executor, target, &head, &source_head, by_patch_id, &[])?;
  }

  let changed = changed_paths(git_executor, target, &head, &check.new_head)?;
  let blocked = if changed.is_empty() {
    Vec::new()
  } else {
    blocking_paths(git_executor, target, &head, &check.new_head, &changed)?
  };
  let follow = match follow {
    Some((follow_check, head_ref)) => {
      let changed = changed_paths(git_executor, &follow_check.worktree, &follow_check.head, &check.new_head)?;
      let follow_blocked = if changed.is_empty() {
        Vec::new()
      } else {
        blocking_paths(git_executor, &follow_check.worktree, &follow_check.head, &check.new_head, &changed)?
      };
      if blocked.is_empty() && !follow_blocked.is_empty() {
        check.state = blocked_state(&follow_check.worktree, follow_blocked);
      }
      Some(Follow { head_ref, changed })
    }
    None => None,
  };
  if !blocked.is_empty() {
    check.state = blocked_state(target, blocked);
  }
  debug!(picked = check.picked.len(), present = check.present.len(), state = ?check.state, "checked the pick");
  Ok(PickPlan {
    check,
    upstream: request.upstream.map(str::to_string),
    upstream_head,
    changed,
    follow,
  })
}

/// The witness of a clean pick that `check_pick` checked before the repository lock, for its caller under the lock
/// before `apply_pick`: `false` when the target's branch, its HEAD or the upstream moved since, or, with `follow`, the
/// source's HEAD or what it is on, so the caller checks again. Else the local changes on the paths each move writes,
/// read again now that nobody else moves a HEAD (`recheck_blocking_paths`: by the content of those paths, the exact
/// dry run only where that finds one), which can make the plan `Blocked`. The guards and HEAD of each worktree, the
/// upstream, then the local changes only when files change
#[instrument(skip(git_executor, plan), fields(branch = %plan.check.branch))]
pub fn recheck_pick(git_executor: &GitCommandExecutor, target: &str, plan: &mut PickPlan) -> Result<bool> {
  let branch = check_update_guards(git_executor, target)?;
  let head = resolve_commit(git_executor, target, "HEAD")?;
  let upstream_head = match &plan.upstream {
    Some(upstream) => resolve_commit(git_executor, target, upstream)?,
    None => None,
  };
  if branch != plan.check.branch || head.as_deref() != Some(plan.check.head.as_str()) || upstream_head != plan.upstream_head {
    debug!(%branch, ?head, ?upstream_head, "the target moved during the check");
    return Ok(false);
  }
  if let (Some(follow), Some(follow_check)) = (&plan.follow, &plan.check.follow) {
    let head_ref = HeadRef::read(git_executor, &follow_check.worktree)?;
    let head = resolve_commit(git_executor, &follow_check.worktree, "HEAD")?;
    if head_ref != follow.head_ref || head.as_deref() != Some(follow_check.head.as_str()) {
      debug!(worktree = %follow_check.worktree, ?head, "the source moved during the check");
      return Ok(false);
    }
  }
  let check = &plan.check;
  let blocked = recheck_blocking_paths(git_executor, target, &check.head, &check.new_head, &plan.changed)?;
  if !blocked.is_empty() {
    plan.check.state = blocked_state(target, blocked);
    return Ok(true);
  }
  if let (Some(follow), Some(follow_check)) = (&plan.follow, &check.follow) {
    let blocked = recheck_blocking_paths(git_executor, &follow_check.worktree, &follow_check.head, &check.new_head, &follow.changed)?;
    if !blocked.is_empty() {
      plan.check.state = blocked_state(&follow_check.worktree, blocked);
    }
  }
  Ok(true)
}

/// The source worktree that holds `dir`: another worktree of the target's repository
fn source_worktree(git_executor: &GitCommandExecutor, target: &str, dir: &str) -> Result<RepositoryDirs> {
  let invalid = |message: String| anyhow::Error::new(PickRefusal::InvalidSource(message));
  let source = repository_dirs(git_executor, dir)?.ok_or_else(|| invalid(format!("{dir} is not inside a git working tree")))?;
  let target_dirs = repository_dirs(git_executor, target)?.ok_or_else(|| anyhow!("{target} is not inside a git working tree"))?;
  if source.git_common_dir != target_dirs.git_common_dir {
    return Err(invalid(format!("{dir} is a worktree of another repository")));
  }
  if source.git_dir == target_dirs.git_dir {
    return Err(invalid(format!(
      "{dir} is the worktree of {target}: --from names the other worktree, the one whose commits land here"
    )));
  }
  Ok(source)
}

/// The commit the range of the source worktree starts after: the fork point of `source_head` in the reflog of the
/// target branch, else the merge base when the shared history is the upstream's
fn worktree_base(git_executor: &GitCommandExecutor, target: &str, branch: &str, head: &str, source_head: &str, upstream_head: Option<&str>, dir: &str) -> Result<String> {
  if let Some(upstream_head) = upstream_head
    && let Some(source_base) = merge_base(git_executor, target, source_head, upstream_head)?
    && !is_ancestor(git_executor, target, &source_base, head)?
  {
    return Err(anyhow::Error::new(PickRefusal::SourceOnNewerBaseline(format!(
      "The worktree {dir} stands on a newer baseline than {branch}: its commits may need what {branch} lacks. Update {branch} first \
       (`bd update`, by the session that owns it), then pick again"
    ))));
  }
  if let Some(fork_point) = fork_point(git_executor, target, &format!("refs/heads/{branch}"), source_head)? {
    return Ok(fork_point);
  }
  let Some(base) = merge_base(git_executor, target, source_head, head)? else {
    bail!("{dir} and {branch} have no common history");
  };
  if let Some(upstream_head) = upstream_head
    && !is_ancestor(git_executor, target, &base, upstream_head)?
  {
    return Err(anyhow::Error::new(PickRefusal::RangeUnknown(format!(
      "The reflog of {branch} has no fork point of {dir}, and their shared history is local: commits {branch} rewrote since would come \
       back. Name the commits: `bd pick <first>^..<last>`"
    ))));
  }
  Ok(base)
}

/// `(<a>, <b>)` of `<a>..<b>`, or `(<c>^, <c>)` of one commit, as full ids
fn resolve_revisions(git_executor: &GitCommandExecutor, target: &str, text: &str) -> Result<(String, String)> {
  let invalid = |message: String| anyhow::Error::new(PickRefusal::InvalidSource(message));
  let commit = |rev: &str| -> Result<String> { resolve_commit(git_executor, target, rev)?.ok_or_else(|| invalid(format!("{rev:?} names no commit of this repository"))) };
  match text.split_once("..") {
    Some((_, to)) if to.starts_with('.') => Err(invalid(format!("{text:?} is a symmetric range; name <first>^..<last>"))),
    Some((from, to)) if !from.is_empty() && !to.is_empty() => Ok((commit(from)?, commit(to)?)),
    Some(_) => Err(invalid(format!("{text:?} needs both ends: <first>^..<last>"))),
    None => {
      let to = commit(text)?;
      let from = resolve_commit(git_executor, target, &format!("{to}^"))?.ok_or_else(|| invalid(format!("{text:?} is a root commit: nothing to pick it onto")))?;
      Ok((from, to))
    }
  }
}

/// The commits of the worktree of `source_dir` that a pick onto `branch` would pick, checked on git objects only, from
/// any worktree of the repository: the range of `check_pick` (the fork point of the source in the reflog of `branch`),
/// replayed onto the tip of `branch`. A commit whose change the branch has, by its patch or by its content, drops.
/// Unlike `check_pick` it needs no worktree on `branch`, no index and no lock, and it writes no worktree: the return of a
/// lease asks it (`bd lease return`). A range that conflicts is picked whole, as nothing of it is proven present
pub fn commits_to_pick(git_executor: &GitCommandExecutor, repo_path: &str, branch: &str, source_dir: &str, upstream: Option<&str>) -> Result<Vec<PickCommit>> {
  let tip = format!("refs/heads/{branch}");
  let head_line = git_executor.execute_command(&["log", "-1", "--date=raw", "--format=%H%x00%cd", &tip, "--"], repo_path)?;
  let (head, head_date) = head_line.split_once('\0').ok_or_else(|| anyhow!("Unexpected tip of {branch}: {head_line:?}"))?;
  let upstream_head = match upstream {
    Some(upstream) => resolve_commit(git_executor, repo_path, upstream)?,
    None => None,
  };
  let source_head = git_executor.execute_command(&["rev-parse", "HEAD^{commit}"], source_dir)?;
  let base = worktree_base(git_executor, repo_path, branch, head, &source_head, upstream_head.as_deref(), source_dir)?;
  let mut excluded = vec![head, base.as_str()];
  excluded.extend(upstream_head.as_deref());
  ensure_no_merges(git_executor, repo_path, &source_head, &excluded)?;
  let negated: Vec<String> = excluded.iter().map(|rev| format!("^{rev}")).collect();
  let range = range_of(git_executor, repo_path, head, &source_head, &negated[1..])?;
  // a commit with the patch id of a commit of the branch is there already
  let kept: Vec<UpdateCommit> = range.iter().filter(|commit| !commit.same_patch).map(|commit| commit.commit.clone()).collect();
  if range.is_empty() || range[0].parent == head || kept.is_empty() {
    return Ok(kept.iter().map(pick_commit).collect());
  }
  let date = committer_date(head_date, &range);
  let env: Vec<(&str, &str)> = date.iter().map(|date| ("GIT_COMMITTER_DATE", date.as_str())).collect();
  let replay = Replay {
    git_executor,
    target: repo_path,
    range: &range,
    negated: &negated,
    env: &env,
  };
  let mut tip = head.to_string();
  for (first, last) in segments(&range) {
    let Some(new_tip) = replay.segment(&tip, first, last)? else {
      return Ok(kept.iter().map(pick_commit).collect());
    };
    tip = new_tip;
  }
  let replayed = commits_of(git_executor, repo_path, &[tip, format!("^{head}")])?;
  let (picked, _present) = match_replayed(&kept, &replayed);
  Ok(
    picked
      .into_iter()
      .map(|commit| PickCommit {
        id: commit.commit,
        subject: commit.subject,
      })
      .collect(),
  )
}

/// The commits of `revs`, oldest first, with their authors (one `log`)
fn commits_of(git_executor: &GitCommandExecutor, repo_path: &str, revs: &[String]) -> Result<Vec<UpdateCommit>> {
  let mut args = vec!["log", "--reverse", "-z", COMMIT_FORMAT];
  args.extend(revs.iter().map(String::as_str));
  args.push("--");
  let output = git_executor.execute_command(&args, repo_path)?;
  Ok(parse_commits(&output))
}

/// A commit of the range: also its parent, which tells a fast-forward, its committer date, and whether a target commit
/// has its patch id
#[derive(Debug)]
struct RangeCommit {
  commit: UpdateCommit,
  parent: String,
  /// Raw, `<seconds> <offset>`
  committer_date: String,
  /// `=` of `--cherry-mark`: a commit of the target's side has its patch id
  same_patch: bool,
}

/// The commits that `source_head` has and `head` lacks, without `excluded` (`^<base>`, `^<upstream>`), oldest first,
/// with their parents, committer dates and patch id marks: one `log --cherry-mark --right-only <head>...<source_head>`,
/// whose target side is the target's commits above the base and the upstream
fn range_of(git_executor: &GitCommandExecutor, repo_path: &str, head: &str, source_head: &str, excluded: &[String]) -> Result<Vec<RangeCommit>> {
  // `COMMIT_FORMAT`, then the parent, the raw committer date and the mark, `=` or `>`
  let format = format!("{COMMIT_FORMAT}%x00%P%x00%cd%x00%m");
  let symmetric = format!("{head}...{source_head}");
  let mut args = vec!["log", "--reverse", "-z", "--date=raw", "--cherry-mark", "--right-only", &format, &symmetric];
  args.extend(excluded.iter().map(String::as_str));
  args.push("--");
  let output = git_executor.execute_command(&args, repo_path)?;
  output
    .split('\x01')
    .filter(|record| !record.is_empty())
    .map(|record| {
      // `-z` ends every record but the last with a NUL
      let record = record.trim_end_matches('\0');
      let mut fields = record.rsplitn(4, '\0');
      let (Some(mark), Some(committer_date), Some(parent), Some(commit)) = (fields.next(), fields.next(), fields.next(), fields.next()) else {
        bail!("Unexpected log record {record:?}");
      };
      let commit = parse_commits(&format!("\x01{commit}")).pop().ok_or_else(|| anyhow!("Unexpected log record {record:?}"))?;
      Ok(RangeCommit {
        commit,
        parent: parent.to_string(),
        committer_date: committer_date.to_string(),
        same_patch: mark == "=",
      })
    })
    .collect()
}

/// The runs of the range between the commits left out by patch id, as `(first, last)` indices, oldest first
fn segments(range: &[RangeCommit]) -> Vec<(usize, usize)> {
  let mut segments: Vec<(usize, usize)> = Vec::new();
  for (index, commit) in range.iter().enumerate() {
    if commit.same_patch {
      continue;
    }
    match segments.last_mut() {
      Some((_, last)) if *last + 1 == index => *last = index,
      _ => segments.push((index, index)),
    }
  }
  segments
}

/// The replay of segments of the range
struct Replay<'a> {
  git_executor: &'a GitCommandExecutor,
  target: &'a str,
  range: &'a [RangeCommit],
  /// `^<target HEAD> ^<base> [^<upstream>]`
  negated: &'a [String],
  env: &'a [(&'a str, &'a str)],
}

impl Replay<'_> {
  /// Replay the commits `first..=last` of the range onto `onto`; `None` on a conflict
  fn segment(&self, onto: &str, first: usize, last: usize) -> Result<Option<String>> {
    let mut revs: Vec<&str> = vec![&self.range[last].commit.id];
    revs.extend(self.negated.iter().map(String::as_str));
    let before = first.checked_sub(1).map(|before| format!("^{}", self.range[before].commit.id));
    revs.extend(before.as_deref());
    Ok(replay_range_with_env(self.git_executor, self.target, onto, &revs, self.env)?)
  }

  /// The first conflicting commit of the segment `first..=last`, which conflicts onto `onto`: a binary search over its
  /// prefixes, then the files of its merge onto the longest clean prefix
  fn conflict(&self, onto: &str, first: usize, last: usize) -> Result<PickConflict> {
    let (index, clean_tip) = first_conflict(first, last, onto.to_string(), |prefix_last| self.segment(onto, first, prefix_last))?;
    let commit = &self.range[index].commit;
    let mut files = conflicting_files(self.git_executor, self.target, &clean_tip, &commit.id)?;
    let file_count = u32::try_from(files.len()).unwrap_or(u32::MAX);
    files.truncate(kept(files.len(), MAX_BLOCKED_PATHS));
    debug!(commit = %commit.id, file_count, "the pick conflicts");
    Ok(PickConflict {
      commit: pick_commit(commit),
      clean: self.range[..index]
        .iter()
        .filter(|commit| !commit.same_patch)
        .map(|commit| pick_commit(&commit.commit))
        .collect(),
      files,
      file_count,
    })
  }
}

/// The committer date of the copies, as `GIT_COMMITTER_DATE`: the newest committer date of the range and of `head`,
/// the commit they land on, with its offset. The same commits on the same HEAD give the same copies (the dry run reports
/// the commits the pick writes, a pick in another second too), and a copy is never older than its parent. `None` when
/// the user sets `GIT_COMMITTER_DATE`: theirs wins
fn committer_date(head_date: &str, range: &[RangeCommit]) -> Option<String> {
  if std::env::var_os("GIT_COMMITTER_DATE").is_some() {
    return None;
  }
  let seconds = |date: &str| date.split_once(' ').map_or(date, |(seconds, _)| seconds).parse::<i64>().unwrap_or_default();
  let newest = range
    .iter()
    .map(|commit| commit.committer_date.as_str())
    .fold(head_date.trim_end(), |newest, date| if seconds(date) > seconds(newest) { date } else { newest });
  Some(format!("@{newest}"))
}

fn pick_commit(commit: &UpdateCommit) -> PickCommit {
  PickCommit {
    id: commit.id.clone(),
    subject: commit.subject.clone(),
  }
}

/// The replayed commits paired with the commits of the range they copy, in order: `git replay` keeps the author, its
/// time and the message, and drops a commit that becomes empty, so the copies are the range without the dropped ones
fn match_replayed(range: &[UpdateCommit], replayed: &[UpdateCommit]) -> (Vec<PickedCommit>, Vec<PickCommit>) {
  let same = |a: &UpdateCommit, b: &UpdateCommit| a.author == b.author && a.time == b.time && a.subject == b.subject;
  let mut copies = replayed.iter().peekable();
  let mut picked = Vec::new();
  let mut present = Vec::new();
  for commit in range {
    match copies.next_if(|copy| same(commit, copy)) {
      Some(copy) => picked.push(PickedCommit {
        commit: commit.id.clone(),
        new_id: copy.id.clone(),
        subject: commit.subject.clone(),
      }),
      None => present.push(pick_commit(commit)),
    }
  }
  (picked, present)
}

/// The commits the target has already, each with the target commit of the same patch id: one `rev-list --cherry-mark
/// --left-only` of the target's side, then `pair_by_patch_id`. `dropped` are those the replay dropped as empty, the
/// others were left out by patch id
fn present_on(git_executor: &GitCommandExecutor, target: &str, head: &str, source_head: &str, present: Vec<PickCommit>, dropped: &[&str]) -> Result<Vec<PresentCommit>> {
  if present.is_empty() {
    return Ok(Vec::new());
  }
  let marked = git_executor.execute_command_lines(
    &["rev-list", "--cherry-mark", "--left-only", "--no-merges", &format!("{head}...{source_head}"), "--"],
    target,
  )?;
  let equivalent: Vec<&str> = marked.iter().filter_map(|line| line.strip_prefix('=')).collect();
  let local: Vec<&str> = present.iter().map(|commit| commit.id.as_str()).collect();
  let pairs = pair_by_patch_id(git_executor, target, &local, &equivalent)?;
  Ok(
    present
      .into_iter()
      .map(|commit| {
        let on = pairs.iter().find(|pair| pair.commit == commit.id).map(|pair| pair.upstream.clone());
        let by = if dropped.contains(&commit.id.as_str()) { PresentBy::Empty } else { PresentBy::PatchId };
        PresentCommit {
          commit: commit.id,
          subject: commit.subject,
          on,
          by,
        }
      })
      .collect(),
  )
}

fn blocked_state(worktree: &str, mut paths: Vec<String>) -> PickState {
  let path_count = u32::try_from(paths.len()).unwrap_or(u32::MAX);
  paths.truncate(kept(paths.len(), MAX_BLOCKED_PATHS));
  PickState::Blocked {
    worktree: worktree.to_string(),
    paths,
    path_count,
  }
}

/// How the source worktree followed an applied pick
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FollowState {
  /// Its HEAD and files are at the new tip (also when they were already)
  Moved,
  /// Its HEAD moved or switched since the check: it stays where it is; the target moved
  HeadMoved(String),
  /// Its files could not follow: it stays where it is; the target moved
  Blocked(String),
}

/// What an applied pick did
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedPick {
  pub new_head: String,
  /// With `follow`: how the source worktree followed. When it did not, the same pick again moves only the source: every
  /// commit is on the target then
  pub followed: Option<FollowState>,
}

/// Move the target branch to the checked pick, its files following, then, with `follow`, the source worktree. If the
/// target's files can't follow (a file changed since the check), the branch moves back and nothing is changed; if the
/// source can't follow after the target moved, the target stays and `followed` says why
#[instrument(skip(git_executor, plan), fields(branch = %plan.check.branch))]
pub fn apply_pick(git_executor: &GitCommandExecutor, target: &str, plan: &PickPlan) -> Result<AppliedPick> {
  apply_pick_carrying(git_executor, target, plan, PICK.done, &RefTransaction::new(), &RefTransaction::new())
}

/// `apply_pick` by `operation` with more ref changes in the move of the target and in its rollback: the unapplied
/// branch that a reapply deletes (`reapply`)
pub(crate) fn apply_pick_carrying(
  git_executor: &GitCommandExecutor,
  target: &str,
  plan: &PickPlan,
  operation: &str,
  with: &RefTransaction,
  rollback_with: &RefTransaction,
) -> Result<AppliedPick> {
  let check = &plan.check;
  if check.state != PickState::Clean {
    bail!("Nothing to apply: the pick is not clean");
  }
  if check.new_head != check.head {
    move_branch_and_files_carrying(
      git_executor,
      target,
      &FileMove {
        head_ref: &HeadRef::Branch(check.branch.clone()),
        head: &check.head,
        new_head: &check.new_head,
        files_change: !plan.changed.is_empty(),
        operation,
        with,
        rollback_with,
      },
    )?;
  }
  let followed = plan.follow.as_ref().zip(check.follow.as_ref()).map(|(follow, follow_check)| {
    if follow_check.head == check.new_head {
      return FollowState::Moved;
    }
    let moved = move_branch_and_files(
      git_executor,
      &follow_check.worktree,
      &follow.head_ref,
      &follow_check.head,
      &check.new_head,
      !follow.changed.is_empty(),
      PICK.done,
    );
    match moved {
      Ok(()) => FollowState::Moved,
      Err(e) if e.downcast_ref::<HeadMoved>().is_some() || e.downcast_ref::<BranchSwitched>().is_some() || e.downcast_ref::<HeadAttached>().is_some() => {
        FollowState::HeadMoved(format!("{e:#}"))
      }
      Err(e) => FollowState::Blocked(format!("{e:#}")),
    }
  });
  info!(picked = check.picked.len(), present = check.present.len(), new_head = %check.new_head, ?followed, "picked");
  Ok(AppliedPick {
    new_head: check.new_head.clone(),
    followed,
  })
}

/// Check the undo of a pick that moved HEAD of the worktree of `repo_path` (the target branch, or the source worktree
/// that followed, also a detached HEAD) from `previous_head` to `new_head`: the last entry of its reflog must be that
/// pick, as for the undo of an update (`update_branch::check_update_undo`); `apply_update_undo` applies it
#[instrument(skip(git_executor))]
pub fn check_pick_undo(git_executor: &GitCommandExecutor, repo_path: &str, previous_head: &str, new_head: &str) -> Result<UndoCheck> {
  let head_ref = HeadRef::read(git_executor, repo_path)?;
  check_move_undo(git_executor, repo_path, PICK, &head_ref, previous_head, new_head)
}

#[cfg(test)]
#[path = "pick_test.rs"]
mod pick_test;
