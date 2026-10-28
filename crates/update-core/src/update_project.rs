//! Update Project: fetch the baseline from its remote and bring the checked out branch onto it, keeping unrelated
//! local changes (docs/design/git-operations.md, "Update project"), and its undo. The upstream, the fetch and the
//! check that every sync runs against the last fetched state are `baseline`.

use crate::resolve_copies::{Materialize, prepare_dir};
use crate::resolve_session::{DroppedCommit, SessionOutcome, ViewDetail, own_session, start_resolve, view_session, with_empty_drops, with_subjects};
use anyhow::{Result, anyhow, bail};
use domain_types::landed::LandedBranch;
use domain_types::update_project::{UpdateCheck, UpdateProjectResult, UpdateState};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::landed_pairing::LandedDrop;
use git_ops::revs::{Worktree, current_worktree, is_object_id, ref_exists, with_dirs_of};
use git_ops::update_branch::{
  CheckPurpose, LandedLookup, UndoBlocked, UndoCheck, UpdatePlan, UpstreamRef, apply_update, apply_update_undo, check_update, check_update_before_lock, check_update_guards_on,
  check_update_undo, finish_pending_update, pending_update, recheck_update,
};
use git_ops::word_resolve::ResolveMode;
use history_rewrite::commit_grouper::GroupingInputs;
use history_rewrite::repository_lock::{LockWait, lock_repository};
use std::path::Path;
use std::time::Duration;
use sync_core::baseline::{BaselineUpstream, fetch_baseline, last_fetch_time, read_check_inputs};
use sync_core::landed::{LandedQuery, find_landed, local_revs, record_update, remaining_revs, restore_before_update};
use sync_core::repo_facts::{Baseline, read_baseline};
use sync_core::transfer_progress::TransferProgress;
use sync_utils::timestamp::unix_now;
use tracing::{info, instrument, warn};

/// An update of the app's (`operations::update`)
#[derive(Debug, Clone)]
pub struct UpdateProjectParams {
  pub repository_path: String,
  /// Fetch the baseline first; false updates onto the tracking ref as the last fetch left it. The app fetches in the
  /// operation of the session (`Intent::FetchBaseline`) first, so the network never holds HEAD, then updates without
  /// a fetch
  pub fetch: bool,
}

/// The step an update is at
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdatePhase {
  Fetching,
  /// Another operation holds the repository lock (`bd`, a regroup); sent only when the update has to wait for it
  Waiting,
  Checking,
  Updating,
}

/// What an update reports while it runs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateProgress {
  pub phase: UpdatePhase,
  /// How far the fetch is; `None` until git reports a stage, and in the other phases
  pub fetch: Option<TransferProgress>,
}

impl From<UpdatePhase> for UpdateProgress {
  fn from(phase: UpdatePhase) -> Self {
    Self { phase, fetch: None }
  }
}

/// How an update runs
#[derive(Debug, Clone, Copy)]
pub struct UpdateOptions<'a> {
  /// Fetch the baseline first; without, the update goes onto the last fetched state
  pub fetch: bool,
  /// How the update waits for the repository lock, which it takes after the fetch: `bd` at most its timeout, the app
  /// as long as the holder lives
  pub lock_wait: LockWait<'a>,
  /// A local branch to update onto instead of the baseline, never fetched: the home of a lease (`bd lease pick`). The
  /// update may run on a detached HEAD, and writes no landed report: nothing reached the baseline by it
  pub onto: Option<&'a str>,
}

/// The fetch of an update failed; nothing was changed
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Failed to fetch {upstream}: {message}")]
pub struct FetchFailed {
  /// The baseline, e.g. `origin/master`
  pub upstream: String,
  pub message: String,
}

/// The baseline is a local branch: there is no upstream to update from
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("The baseline {baseline} is a local branch: there is no upstream to update from")]
pub struct NoUpstream {
  pub baseline: String,
}

/// Fetch the baseline, check the update and apply it if it is clean. A conflict or a block is a result, not an error:
/// nothing is changed, and the check says why. The lock is waited for as `lock_wait` says
#[instrument(skip(git_executor, params, on_progress), fields(repository_path = %params.repository_path))]
pub fn update_project_core(
  git_executor: &GitCommandExecutor,
  params: &UpdateProjectParams,
  on_progress: &dyn Fn(UpdateProgress),
  lock_wait: LockWait,
) -> Result<UpdateProjectResult> {
  let options = UpdateOptions {
    fetch: params.fetch,
    lock_wait,
    onto: None,
  };
  update_project_with(git_executor, &params.repository_path, options, on_progress)
}

/// `update_project_core` with options. The fetch and the check run first, without the repository lock, so a slow
/// network never blocks another writer; a check that moves nothing (up to date, blocked, a conflict) is the answer
/// without the lock; otherwise its witness and the update run under the lock, so the app, `bd` and their rewrites
/// never interleave with an update. A held lock is a `RepositoryLocked` error, a failed fetch `FetchFailed`, a local
/// baseline `NoUpstream`, a refused guard `GuardRefusal`
#[instrument(skip(git_executor, on_progress))]
pub fn update_project_with(git_executor: &GitCommandExecutor, repo_path: &str, options: UpdateOptions, on_progress: &dyn Fn(UpdateProgress)) -> Result<UpdateProjectResult> {
  // the guards, the lock, the pending moves and the index of the dry run take the directories read here
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
  match update_project_impl(git_executor, repo_path, options, on_progress, Resolve::No)? {
    BdUpdate::Update { result, .. } => Ok(*result),
    BdUpdate::Session(_) => Err(anyhow!("an update without a resolve session started one")),
  }
}

/// What `bd update` did: an update as the app's, with every local commit it dropped and its subject and proof (those the
/// replay left out because the upstream proves it has them, and those the replay dropped as empty, `landed`), or a
/// step of a resolve session
#[derive(Debug)]
pub enum BdUpdate {
  Update { result: Box<UpdateProjectResult>, dropped: Vec<DroppedCommit> },
  Session(Box<SessionOutcome>),
}

/// Whether a conflict of `update_project_impl` starts a resolve session
#[derive(Debug, Clone, Copy)]
enum Resolve<'a> {
  No,
  /// With the copies in this directory (`--dir`), else in the worktree's git dir
  Yes {
    dir: Option<&'a Path>,
    worktree: &'a Worktree,
  },
}

/// `bd update`: `update_project_with`, but a conflict starts a resolve session under the same lock, with its copies in
/// `dir` (`--dir`) or in the worktree's git dir. With a session open already, nothing is fetched or changed: the open
/// round is shown again, and its missing copies written
#[instrument(skip(git_executor))]
pub fn update_and_resolve(git_executor: &GitCommandExecutor, repo_path: &str, options: UpdateOptions, dir: Option<&Path>) -> Result<BdUpdate> {
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
  let worktree = current_worktree(git_executor, repo_path)?;
  if let Some(session) = own_session(git_executor, repo_path, &worktree)? {
    if dir.is_some() {
      bail!("A resolve session is open already; --dir applies when a session starts");
    }
    let view = view_session(git_executor, repo_path, &session, Some(Materialize::Missing), ViewDetail::Full)?;
    return Ok(BdUpdate::Session(Box::new(SessionOutcome::Round(view))));
  }
  update_project_impl(git_executor, repo_path, options, &|_| {}, Resolve::Yes { dir, worktree: &worktree })
}

/// `update_project_with`; with `Resolve::Yes`, a conflict starts a resolve session
fn update_project_impl(git_executor: &GitCommandExecutor, repo_path: &str, options: UpdateOptions, on_progress: &dyn Fn(UpdateProgress), resolve: Resolve<'_>) -> Result<BdUpdate> {
  // onto a local branch the guards run in the check, which reads HEAD
  let upstream = if let Some(branch) = options.onto {
    local_upstream(git_executor, repo_path, branch)?
  } else {
    let Baseline { name, upstream, branch } = read_baseline(git_executor, repo_path)?;
    let upstream = upstream.ok_or_else(|| anyhow::Error::new(NoUpstream { baseline: name }))?;
    // refuse before the network: the check would refuse after it anyway; the branch is the one the baseline read found
    check_update_guards_on(git_executor, repo_path, branch.as_deref())?;
    upstream
  };
  let local = options.onto.is_some();

  let (previous, fetched_at) = if options.fetch && !local {
    on_progress(UpdatePhase::Fetching.into());
    let mut on_fetch_progress = |fetch| {
      on_progress(UpdateProgress {
        phase: UpdatePhase::Fetching,
        fetch: Some(fetch),
      });
    };
    let previous = fetch_baseline(git_executor, repo_path, &upstream, &mut on_fetch_progress).map_err(|e| {
      anyhow::Error::new(FetchFailed {
        upstream: upstream.name.clone(),
        message: format!("{e:#}"),
      })
    })?;
    (previous, Some(unix_now()))
  } else {
    (None, last_fetch_time(git_executor, repo_path))
  };

  let upstream_ref = UpstreamRef {
    tracking_ref: &upstream.tracking_ref,
    name: &upstream.name,
    previous: previous.as_deref(),
    local,
  };
  // the check (the replay, the resolution by words, the landed candidates) on the HEAD and the upstream it reads once,
  // before the lock: the lock is held only for the witness, the local changes and the move
  on_progress(UpdatePhase::Checking.into());
  let mut prepared = prepare_update(git_executor, repo_path, upstream_ref, Lock::NotHeld)?;
  // nothing to move (up to date, blocked, or a conflict that starts no session): the check is the answer, and no lock
  // is waited for, unless a crash left files to follow
  if moves_nothing(&prepared.plan.check.state, resolve) && pending_update(git_executor, repo_path)?.is_none() {
    let mut check = prepared.plan.check;
    check.fetched_at = fetched_at;
    info!(state = ?check.state, "update not applied, checked without the lock");
    return Ok(BdUpdate::Update {
      result: Box::new(UpdateProjectResult {
        check,
        applied: None,
        landed: Vec::new(),
      }),
      dropped: Vec::new(),
    });
  }
  let lock = options.lock_wait.lock_with(git_executor, repo_path, &|| on_progress(UpdatePhase::Waiting.into()))?;
  // an update or undo that a crash stopped before its files followed: they follow first
  let recovered = finish_pending_update(git_executor, repo_path)?.is_some();
  if recovered || !recheck_update(git_executor, repo_path, &mut prepared.plan, upstream_ref)? {
    // HEAD or the upstream moved during the check: once more, under the lock now, so it can't move again. The step is
    // the check again, no longer the wait for another operation
    info!(recovered, "the update is checked again under the lock");
    on_progress(UpdatePhase::Checking.into());
    prepared = prepare_update(git_executor, repo_path, upstream_ref, Lock::Held)?;
  }
  let Prepared { grouping, resolve_mode, mut plan } = prepared;
  plan.check.fetched_at = fetched_at;

  if let (UpdateState::Conflict { .. }, Resolve::Yes { dir, worktree }) = (&plan.check.state, resolve) {
    let dir = dir.map(|dir| prepare_dir(git_executor, repo_path, dir, &worktree.git_dir)).transpose()?;
    let outcome = start_resolve(git_executor, repo_path, worktree, &plan, &grouping, dir, resolve_mode, lock)?;
    return Ok(BdUpdate::Session(Box::new(outcome)));
  }
  let moved = match &plan.check.state {
    UpdateState::Clean { .. } => {
      on_progress(UpdatePhase::Updating.into());
      Some(apply_update(git_executor, repo_path, &plan)?)
    }
    UpdateState::UpToDate => None,
    UpdateState::Conflict { .. } | UpdateState::Blocked { .. } => {
      info!(state = ?plan.check.state, "update not applied");
      None
    }
  };
  // the update is done: what it did and what it landed are a report, read without the lock on the ids the move froze
  drop(lock);
  let applied = moved.map(|moved| moved.report(git_executor, repo_path)).transpose()?;
  let landed = applied.as_ref().filter(|_| !local).map_or_default(|applied| {
    let report = LandedReport {
      check: &plan.check,
      excluded: plan.excluded(),
      new_head: &applied.new_head,
      proven: plan.landed_drops(),
      not_landed: &[],
    };
    record_landed_update(git_executor, repo_path, &report, &grouping)
  });
  // what `bd` lists of the drops: the app shows `landed`
  let dropped = if applied.is_some() && matches!(resolve, Resolve::Yes { .. }) {
    with_empty_drops(with_subjects(git_executor, repo_path, plan.landed_drops()), &landed)
  } else {
    Vec::new()
  };
  Ok(BdUpdate::Update {
    result: Box::new(UpdateProjectResult {
      check: plan.check,
      applied,
      landed,
    }),
    dropped,
  })
}

/// The local branch `branch` as the upstream of an update onto it (`UpdateOptions::onto`); no remote, never fetched
fn local_upstream(git_executor: &GitCommandExecutor, repo_path: &str, branch: &str) -> Result<BaselineUpstream> {
  let tracking_ref = format!("refs/heads/{branch}");
  if !ref_exists(git_executor, repo_path, &tracking_ref) {
    bail!("There is no branch {branch} to update onto");
  }
  Ok(BaselineUpstream {
    remote: String::new(),
    remote_ref: tracking_ref.clone(),
    tracking_ref,
    name: branch.to_string(),
  })
}

/// Whether an update in `state` moves nothing: only a clean one moves the branch, and a conflict only when it starts a
/// resolve session
const fn moves_nothing(state: &UpdateState, resolve: Resolve<'_>) -> bool {
  match state {
    UpdateState::UpToDate | UpdateState::Blocked { .. } => true,
    UpdateState::Conflict { .. } => matches!(resolve, Resolve::No),
    UpdateState::Clean { .. } => false,
  }
}

/// Whether the caller of `prepare_update` holds the repository lock
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lock {
  /// The check runs before it: `recheck_update` under the lock reads the local changes and that nothing moved
  NotHeld,
  /// Under it: the check is the exact one
  Held,
}

/// The check of an update and what it read
struct Prepared {
  grouping: GroupingInputs,
  resolve_mode: ResolveMode,
  plan: UpdatePlan,
}

/// The inputs of the check, then the check of `upstream` for an apply
fn prepare_update(git_executor: &GitCommandExecutor, repo_path: &str, upstream: UpstreamRef<'_>, lock: Lock) -> Result<Prepared> {
  let (grouping, resolve_mode) = read_check_inputs(git_executor, repo_path)?;
  let landed = LandedLookup::ByKey { stacks: grouping.stacks };
  let plan = match lock {
    Lock::NotHeld => check_update_before_lock(git_executor, repo_path, upstream, landed, resolve_mode)?,
    Lock::Held => check_update(git_executor, repo_path, upstream, CheckPurpose::Apply, landed, resolve_mode)?,
  };
  Ok(Prepared { grouping, resolve_mode, plan })
}

/// What an applied update moved, for its landed report
#[derive(Debug, Clone, Copy)]
pub(crate) struct LandedReport<'a> {
  pub check: &'a UpdateCheck,
  pub excluded: &'a [String],
  pub new_head: &'a str,
  /// The local commits the replay left out because the upstream proves it has them: landed as their upstream commit
  pub proven: &'a [LandedDrop],
  /// The local commits that are gone for another reason: a resolution dropped them, and no upstream commit has their
  /// identity
  pub not_landed: &'a [String],
}

/// What an applied update landed, added to the report. The update is done: a failure here is logged, not returned.
pub(crate) fn record_landed_update(git_executor: &GitCommandExecutor, repo_path: &str, report: &LandedReport<'_>, grouping: &GroupingInputs) -> Vec<LandedBranch> {
  let LandedReport {
    check,
    excluded,
    new_head,
    proven,
    not_landed,
  } = *report;
  let result = (|| {
    let local = local_revs(check, excluded);
    let remaining = remaining_revs(new_head, &check.upstream_head);
    let landed = find_landed(
      git_executor,
      repo_path,
      LandedQuery {
        local: &local,
        remaining: Some(&remaining),
        base: &check.base,
        upstream_head: &check.upstream_head,
        gone_is_landed: true,
        grouping,
        proven,
      },
    )?;
    let mut landed = landed;
    if !not_landed.is_empty() {
      for branch in &mut landed {
        branch.commits.retain(|commit| !not_landed.contains(&commit.id));
      }
      landed.retain(|branch| !branch.commits.is_empty());
    }
    record_update(git_executor, repo_path, check, new_head, landed.clone())?;
    anyhow::Ok(landed)
  })();
  result.unwrap_or_else(|e| {
    warn!(error = %format!("{e:#}"), "can't record the landed commits");
    Vec::new()
  })
}

/// The undo token of an applied update: `update:<previous head>..<new head>`, with full ids
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateUndoToken {
  pub previous_head: String,
  pub new_head: String,
}

const UPDATE_TOKEN: &str = "update:";

impl UpdateUndoToken {
  /// A token as `Display` prints it; anything else, abbreviated ids included, is `None`
  pub fn parse(text: &str) -> Option<Self> {
    let (previous_head, new_head) = text.strip_prefix(UPDATE_TOKEN)?.split_once("..")?;
    let valid = is_object_id(previous_head) && is_object_id(new_head) && previous_head.len() == new_head.len() && previous_head != new_head;
    valid.then(|| Self {
      previous_head: previous_head.to_string(),
      new_head: new_head.to_string(),
    })
  }

  /// Whether `text` claims to be an update token, valid or not
  pub fn is_update_token(text: &str) -> bool {
    text.starts_with(UPDATE_TOKEN)
  }
}

impl std::fmt::Display for UpdateUndoToken {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{UPDATE_TOKEN}{}..{}", self.previous_head, self.new_head)
  }
}

/// What an undone update left
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoneUpdate {
  /// The branch that moved back
  pub branch: String,
  /// The landed report is the one from before the update again
  pub landed_restored: bool,
  /// The undo was done before: nothing changed now
  pub already_undone: bool,
}

/// Undo an applied update, under the repository lock: the branch moves back from the token's new head to its previous
/// head, and the files follow through the same guarded update, with the trees swapped (`check_update_undo`). Refused
/// with `HeadMoved` when the branch moved (`BranchSwitched` when HEAD switched), `NotAnUpdate` when the last move of
/// the branch is not this update, and `UndoBlocked` when local changes sit on a path the undo writes. The landed report
/// goes back to the one before the update. Undoing again is no error: `already_undone`, nothing changes
#[instrument(skip(git_executor))]
pub fn undo_update_core(git_executor: &GitCommandExecutor, repo_path: &str, token: &UpdateUndoToken, lock_timeout: Duration) -> Result<UndoneUpdate> {
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
  let _lock = lock_repository(git_executor, repo_path, lock_timeout)?;
  finish_pending_update(git_executor, repo_path)?;
  let undo = match check_update_undo(git_executor, repo_path, &token.previous_head, &token.new_head)? {
    UndoCheck::Ready(undo) => undo,
    UndoCheck::AlreadyUndone { branch } => {
      return Ok(UndoneUpdate {
        branch,
        landed_restored: false,
        already_undone: true,
      });
    }
  };
  if !undo.blocked().is_empty() {
    return Err(anyhow::Error::new(UndoBlocked { paths: undo.blocked().to_vec() }));
  }
  apply_update_undo(git_executor, repo_path, &undo)?;
  // the undo is done: a failure here is logged, not returned
  let landed_restored = restore_before_update(git_executor, repo_path, undo.branch(), &token.new_head).unwrap_or_else(|e| {
    warn!(error = %format!("{e:#}"), "can't restore the landed report");
    false
  });
  info!(branch = undo.branch(), landed_restored, "undid the update");
  Ok(UndoneUpdate {
    branch: undo.branch().to_string(),
    landed_restored,
    already_undone: false,
  })
}

#[cfg(test)]
#[path = "update_project_test.rs"]
pub(crate) mod update_project_test;
