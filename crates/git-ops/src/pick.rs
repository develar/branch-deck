//! `bd pick`: land the commits of another worktree (or of a range) on the branch checked out here, without
//! `git cherry-pick` (docs/design/git-operations.md, "Pick"). The commits are replayed in memory with `git replay` onto
//! the target's HEAD, where a commit whose change the target has already becomes empty and drops; a dry run on a copy of
//! the index of each worktree proves its files can follow, and only then does the branch move, through the guarded
//! update of `update_branch`. With `follow`, the source worktree moves onto the new tip the same way, a detached HEAD
//! with `option no-deref`.

use crate::paths::{COMMIT_FORMAT, changed_paths, parse_commits};
use crate::replay::{first_conflict, replay_range_with_env};
use crate::revs::{RepositoryDirs, fork_point, is_ancestor, merge_base, repository_dirs, resolve_commit};
use crate::rewrite_guard::{HeadMoved, ensure_no_merges};
use crate::update_branch::{
  BranchSwitched, HeadAttached, HeadRef, MAX_BLOCKED_PATHS, PICK, UndoCheck, UpdateCommit, blocking_paths, check_move_undo, check_update_guards, move_branch_and_files,
};
use crate::update_resolve::pair_by_patch_id;
use anyhow::{Result, anyhow, bail};
use bstr::ByteSlice;
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

/// A commit of the range whose change the target has already: `git replay` dropped it as empty
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentCommit {
  pub commit: String,
  pub subject: String,
  /// The target's commit with the same patch id; `None` when the target has the change in another form
  pub on: Option<String>,
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
  pub present: Vec<PresentCommit>,
  pub follow: Option<FollowCheck>,
  pub state: PickState,
}

/// A checked pick; `apply_pick` applies a clean one. Only `check_pick` builds one, so no other crate can move HEAD to
/// another commit through the working-tree exemption
#[derive(Debug)]
pub struct PickPlan {
  pub check: PickCheck,
  files_change: bool,
  follow: Option<Follow>,
}

#[derive(Debug)]
struct Follow {
  head_ref: HeadRef,
  files_change: bool,
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
  let revs_to = |tip: &str| -> Vec<String> { std::iter::once(tip.to_string()).chain(negated.iter().cloned()).collect() };
  let range = range_of(git_executor, target, &revs_to(&source_head))?;

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
  if range.first().is_some_and(|oldest| oldest.parent == head) {
    check.picked = range
      .iter()
      .map(|commit| PickedCommit {
        commit: commit.commit.id.clone(),
        new_id: commit.commit.id.clone(),
        subject: commit.commit.subject.clone(),
      })
      .collect();
    check.new_head.clone_from(&source_head);
  } else if !range.is_empty() {
    let date = committer_date(head_date, &range);
    let env: Vec<(&str, &str)> = date.iter().map(|date| ("GIT_COMMITTER_DATE", date.as_str())).collect();
    let range: Vec<UpdateCommit> = range.into_iter().map(|commit| commit.commit).collect();
    let revs = revs_to(&source_head);
    let revs: Vec<&str> = revs.iter().map(String::as_str).collect();
    let Some(new_head) = replay_range_with_env(git_executor, target, &head, &revs, &env)? else {
      check.state = PickState::Conflict(conflict(git_executor, target, &head, &range, &revs_to, &env)?);
      return Ok(PickPlan {
        check,
        files_change: false,
        follow: None,
      });
    };
    let replayed = commits_of(git_executor, target, &[new_head.clone(), format!("^{head}")])?;
    let (picked, present) = match_replayed(&range, &replayed);
    check.present = present_on(git_executor, target, &head, &source_head, present)?;
    check.picked = picked;
    check.new_head = new_head;
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
      Some(Follow {
        head_ref,
        files_change: !changed.is_empty(),
      })
    }
    None => None,
  };
  if !blocked.is_empty() {
    check.state = blocked_state(target, blocked);
  }
  debug!(picked = check.picked.len(), present = check.present.len(), state = ?check.state, "checked the pick");
  Ok(PickPlan {
    check,
    files_change: !changed.is_empty(),
    follow,
  })
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
  let revs: Vec<String> = std::iter::once(source_head).chain(excluded.iter().map(|rev| format!("^{rev}"))).collect();
  let range = range_of(git_executor, repo_path, &revs)?;
  if range.is_empty() || range[0].parent == head {
    return Ok(range.iter().map(|commit| pick_commit(&commit.commit)).collect());
  }
  let date = committer_date(head_date, &range);
  let env: Vec<(&str, &str)> = date.iter().map(|date| ("GIT_COMMITTER_DATE", date.as_str())).collect();
  let commits: Vec<UpdateCommit> = range.into_iter().map(|commit| commit.commit).collect();
  let rev_args: Vec<&str> = revs.iter().map(String::as_str).collect();
  let Some(new_head) = replay_range_with_env(git_executor, repo_path, head, &rev_args, &env)? else {
    return Ok(commits.iter().map(pick_commit).collect());
  };
  let replayed = commits_of(git_executor, repo_path, &[new_head, format!("^{head}")])?;
  let (picked, _present) = match_replayed(&commits, &replayed);
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

/// A commit of the range: also its parent, which tells a fast-forward, and its committer date
#[derive(Debug)]
struct RangeCommit {
  commit: UpdateCommit,
  parent: String,
  /// Raw, `<seconds> <offset>`
  committer_date: String,
}

/// The commits of `revs`, oldest first, with their parents and committer dates (one `log`)
fn range_of(git_executor: &GitCommandExecutor, repo_path: &str, revs: &[String]) -> Result<Vec<RangeCommit>> {
  // `COMMIT_FORMAT`, then the parent and the raw committer date
  let format = format!("{COMMIT_FORMAT}%x00%P%x00%cd");
  let mut args = vec!["log", "--reverse", "-z", "--date=raw", &format];
  args.extend(revs.iter().map(String::as_str));
  args.push("--");
  let output = git_executor.execute_command(&args, repo_path)?;
  output
    .split('\x01')
    .filter(|record| !record.is_empty())
    .map(|record| {
      // `-z` ends every record but the last with a NUL
      let record = record.trim_end_matches('\0');
      let mut fields = record.rsplitn(3, '\0');
      let (Some(committer_date), Some(parent), Some(commit)) = (fields.next(), fields.next(), fields.next()) else {
        bail!("Unexpected log record {record:?}");
      };
      let commit = parse_commits(&format!("\x01{commit}")).pop().ok_or_else(|| anyhow!("Unexpected log record {record:?}"))?;
      Ok(RangeCommit {
        commit,
        parent: parent.to_string(),
        committer_date: committer_date.to_string(),
      })
    })
    .collect()
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

/// The commits the replay dropped, each with the target commit of the same patch id: one `rev-list --cherry-mark
/// --left-only` of the target's side, then `pair_by_patch_id`
fn present_on(git_executor: &GitCommandExecutor, target: &str, head: &str, source_head: &str, present: Vec<PickCommit>) -> Result<Vec<PresentCommit>> {
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
        PresentCommit {
          commit: commit.id,
          subject: commit.subject,
          on,
        }
      })
      .collect(),
  )
}

/// The first conflicting commit of the range: a binary search over its prefixes, then the files of its merge onto the
/// longest clean prefix
fn conflict(
  git_executor: &GitCommandExecutor,
  target: &str,
  head: &str,
  range: &[UpdateCommit],
  revs_to: &dyn Fn(&str) -> Vec<String>,
  env: &[(&str, &str)],
) -> Result<PickConflict> {
  let (index, clean_tip) = first_conflict(0, range.len() - 1, head.to_string(), |last| {
    let revs = revs_to(&range[last].id);
    let revs: Vec<&str> = revs.iter().map(String::as_str).collect();
    Ok(replay_range_with_env(git_executor, target, head, &revs, env)?)
  })?;
  let commit = &range[index];
  let merge_base = format!("--merge-base={}^", commit.id);
  // `<tree>\0<file>\0...`, exit code 1 on conflicts
  let output = git_executor
    .cmd(
      &["merge-tree", "--write-tree", "--name-only", "-z", "--no-messages", &merge_base, &clean_tip, &commit.id],
      target,
    )
    .accept(&[1])
    .bytes()?;
  let mut files: Vec<String> = output
    .split(|&b| b == 0)
    .skip(1)
    .filter(|file| !file.is_empty())
    .map(|file| file.to_str_lossy().into_owned())
    .collect();
  files.dedup();
  let file_count = u32::try_from(files.len()).unwrap_or(u32::MAX);
  files.truncate(kept(files.len(), MAX_BLOCKED_PATHS));
  debug!(commit = %commit.id, file_count, "the pick conflicts");
  Ok(PickConflict {
    commit: pick_commit(commit),
    clean: range[..index].iter().map(pick_commit).collect(),
    files,
    file_count,
  })
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
  let check = &plan.check;
  if check.state != PickState::Clean {
    bail!("Nothing to apply: the pick is not clean");
  }
  if check.new_head != check.head {
    move_branch_and_files(
      git_executor,
      target,
      &HeadRef::Branch(check.branch.clone()),
      &check.head,
      &check.new_head,
      plan.files_change,
      PICK.done,
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
      follow.files_change,
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
