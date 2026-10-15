//! Update Project: fetch the baseline from its remote and bring the checked out branch onto it, keeping unrelated
//! local changes (docs/design/git-operations.md, "Update project"). The same check runs inside every sync against the
//! last fetched state, without the network, so the header can show whether an update would apply cleanly.

use crate::baseline::detect_baseline_branch;
use crate::commit_grouper::GroupingInputs;
use crate::issue_groups::read_grouping_inputs;
use crate::landed::{LandedQuery, find_landed, find_landing, landed_candidates, local_revs, record_update, remaining_revs, restore_before_update};
use crate::regroup::is_full_id;
use crate::remote_command::execute_remote_command_with_progress;
use crate::repository_lock::{DEFAULT_LOCK_TIMEOUT, RepositoryLocked, lock_repository};
use crate::transfer_progress::TransferProgress;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::rewrite_guard::get_current_branch;
use git_ops::update_branch::{
  AppliedUpdate, CheckPurpose, UndoCheck, UpdateCheck, UpdateState, UpstreamRef, apply_update, apply_update_undo, check_update, check_update_guards, check_update_undo,
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, UNIX_EPOCH};
use sync_types::landed::LandedBranch;
use sync_utils::timestamp::unix_now;
use tracing::{info, instrument, warn};

/// The remote branch the baseline tracks
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineUpstream {
  pub remote: String,
  /// The branch on the remote, e.g. `refs/heads/master`
  pub remote_ref: String,
  /// The local remote-tracking ref, e.g. `refs/remotes/origin/master`
  pub tracking_ref: String,
  /// The baseline as it is shown, e.g. `origin/master`
  pub name: String,
}

/// The remote branch behind `baseline_branch` (as `detect_baseline_branch` returns it); `None` for a local baseline.
/// The upstream configuration of the checked out branch gives the remote and the remote branch name when it tracks
/// the baseline; otherwise `<remote>/<branch>` is read with the default refspec.
#[instrument(skip(git_executor))]
pub fn resolve_baseline_upstream(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str) -> Result<Option<BaselineUpstream>> {
  let remotes = git_executor.execute_command_lines(&["remote"], repo_path)?;
  // the longest remote name that prefixes the baseline: remote names may contain slashes
  let Some(remote) = remotes
    .iter()
    .filter(|remote| baseline_branch.strip_prefix(remote.as_str()).is_some_and(|rest| rest.len() > 1 && rest.starts_with('/')))
    .max_by_key(|remote| remote.len())
  else {
    return Ok(None);
  };
  let tracking_ref = format!("refs/remotes/{baseline_branch}");

  if let Ok(branch) = get_current_branch(git_executor, repo_path) {
    let output = git_executor.execute_command(
      &[
        "for-each-ref",
        "--format=%(upstream)%00%(upstream:remotename)%00%(upstream:remoteref)",
        &format!("refs/heads/{branch}"),
      ],
      repo_path,
    )?;
    let mut fields = output.split('\0');
    if let (Some(upstream), Some(upstream_remote), Some(remote_ref)) = (fields.next(), fields.next(), fields.next())
      && upstream == tracking_ref
      && !upstream_remote.is_empty()
      && !remote_ref.is_empty()
    {
      return Ok(Some(BaselineUpstream {
        remote: upstream_remote.to_string(),
        remote_ref: remote_ref.to_string(),
        tracking_ref,
        name: baseline_branch.to_string(),
      }));
    }
  }

  Ok(Some(BaselineUpstream {
    remote: remote.clone(),
    remote_ref: format!("refs/heads/{}", &baseline_branch[remote.len() + 1..]),
    tracking_ref,
    name: baseline_branch.to_string(),
  }))
}

/// Fetch only the baseline's remote branch into its tracking ref. Other tracking refs stay where they are, so the
/// push lease of the virtual branches (`--force-with-lease`) still compares against what the last push saw.
/// Returns the tip before the fetch if the fetch was a forced update (the upstream was force-pushed).
/// `on_progress` gets how far the fetch is.
#[instrument(skip(git_executor, on_progress))]
pub fn fetch_baseline(git_executor: &GitCommandExecutor, repo_path: &str, upstream: &BaselineUpstream, on_progress: &mut dyn FnMut(TransferProgress)) -> Result<Option<String>> {
  let refspec = format!("+{}:{}", upstream.remote_ref, upstream.tracking_ref);
  let output = execute_remote_command_with_progress(
    git_executor,
    &[
      "fetch",
      "--porcelain",
      "--no-tags",
      "--no-write-fetch-head",
      "--no-recurse-submodules",
      "--no-auto-maintenance",
      &upstream.remote,
      &refspec,
    ],
    repo_path,
    &[],
    on_progress,
  )?;
  Ok(previous_tip_of_forced_update(&output, &upstream.tracking_ref))
}

/// The old id from the `fetch --porcelain` line of `tracking_ref` if that line is a forced update:
/// `<flag> <old> <new> <ref>`, flag `+` for forced, ` ` for a fast-forward, `*` for a new ref; nothing is printed for a
/// ref that is up to date
pub(crate) fn previous_tip_of_forced_update(output: &str, tracking_ref: &str) -> Option<String> {
  output.lines().find_map(|line| {
    let rest = line.strip_prefix("+ ")?;
    let mut fields = rest.split(' ');
    let (old, _new, local_ref) = (fields.next()?, fields.next()?, fields.next()?);
    (local_ref == tracking_ref && !old.bytes().all(|b| b == b'0')).then(|| old.to_string())
  })
}

/// When the repository was last fetched, from the time git last wrote `FETCH_HEAD` (Branch Deck's own fetch writes no
/// `FETCH_HEAD`, its time is returned with the update result)
fn last_fetch_time(git_executor: &GitCommandExecutor, repo_path: &str) -> Option<u32> {
  let path = git_executor
    .execute_command(&["rev-parse", "--path-format=absolute", "--git-path", "FETCH_HEAD"], repo_path)
    .ok()?;
  let modified = std::fs::metadata(path).and_then(|metadata| metadata.modified()).ok()?;
  u32::try_from(modified.duration_since(UNIX_EPOCH).ok()?.as_secs()).ok()
}

/// The check of the checked out branch against the last fetched baseline
#[derive(Debug)]
pub struct BaselineCheck {
  pub check: UpdateCheck,
  /// The local commits the upstream already has: the update drops them
  pub landing: Vec<LandedBranch>,
}

/// Check the checked out branch against the last fetched state of its baseline, without the network.
/// `None` for a local baseline: there is nothing to update from.
pub fn check_baseline_update(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str) -> Result<Option<BaselineCheck>> {
  check_baseline_update_with(git_executor, repo_path, baseline_branch, &read_grouping_inputs(git_executor, repo_path)?)
}

/// `check_baseline_update` with the stack setting and the issue groups already read
#[instrument(skip(git_executor, grouping))]
pub fn check_baseline_update_with(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str, grouping: &GroupingInputs) -> Result<Option<BaselineCheck>> {
  let Some(upstream) = resolve_baseline_upstream(git_executor, repo_path, baseline_branch)? else {
    return Ok(None);
  };
  let mut plan = check_update(
    git_executor,
    repo_path,
    UpstreamRef {
      tracking_ref: &upstream.tracking_ref,
      name: &upstream.name,
      previous: None,
    },
    CheckPurpose::Preview,
    &|scope| landed_candidates(git_executor, repo_path, scope, grouping),
  )?;
  plan.check.fetched_at = last_fetch_time(git_executor, repo_path);
  // the landing is a detail of the check: it must not fail it
  let landing = find_landing(git_executor, repo_path, &plan.check, plan.excluded(), plan.replayed_head(), grouping).unwrap_or_else(|e| {
    warn!(error = %format!("{e:#}"), "can't find the landed commits");
    Vec::new()
  });
  Ok(Some(BaselineCheck { check: plan.check, landing }))
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectParams {
  pub repository_path: String,
}

/// The step an update is at
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum UpdatePhase {
  Fetching,
  Checking,
  Updating,
}

/// What an update reports while it runs
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectResult {
  /// The check against the fetched upstream; on a conflict or a block nothing was changed
  pub check: UpdateCheck,
  /// What the update changed; `None` when up to date, conflicting or blocked
  pub applied: Option<AppliedUpdate>,
  /// The local commits this update dropped because the upstream has them, by branch
  pub landed: Vec<LandedBranch>,
}

/// How an update runs
#[derive(Debug, Clone, Copy)]
pub struct UpdateOptions {
  /// Fetch the baseline first; without, the update goes onto the last fetched state
  pub fetch: bool,
  /// How long the update waits for the repository lock, which it takes after the fetch
  pub lock_timeout: Duration,
}

/// The fetch of an update failed; nothing was changed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchFailed {
  /// The baseline, e.g. `origin/master`
  pub upstream: String,
  pub message: String,
}

impl std::fmt::Display for FetchFailed {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Failed to fetch {}: {}", self.upstream, self.message)
  }
}

impl std::error::Error for FetchFailed {}

/// The baseline is a local branch: there is no upstream to update from
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoUpstream {
  pub baseline: String,
}

impl std::fmt::Display for NoUpstream {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "The baseline {} is a local branch: there is no upstream to update from", self.baseline)
  }
}

impl std::error::Error for NoUpstream {}

/// The app's words for a held repository lock: the lock file is an internal detail
pub const LOCKED_MESSAGE: &str = "Branch Deck or bd is changing this repository. Try again when it is done.";

/// Fetch the baseline, check the update and apply it if it is clean. A conflict or a block is a result, not an error:
/// nothing is changed, and the check says why.
#[instrument(skip(git_executor, params, on_progress), fields(repository_path = %params.repository_path))]
pub fn update_project_core(git_executor: &GitCommandExecutor, params: &UpdateProjectParams, on_progress: &dyn Fn(UpdateProgress)) -> Result<UpdateProjectResult> {
  let options = UpdateOptions {
    fetch: true,
    lock_timeout: DEFAULT_LOCK_TIMEOUT,
  };
  update_project_with(git_executor, &params.repository_path, options, on_progress).map_err(app_error)
}

/// An error of `update_project_with` in the words of the app: a held lock is `LOCKED_MESSAGE`
pub fn app_error(error: anyhow::Error) -> anyhow::Error {
  if error.downcast_ref::<RepositoryLocked>().is_some() {
    anyhow!(LOCKED_MESSAGE)
  } else {
    error
  }
}

/// `update_project_core` with options (`bd update`). The fetch runs first, without the repository lock, so a slow
/// network never blocks another writer; the check and the update then run under the lock, so the app, `bd` and their
/// rewrites never interleave with an update. A held lock is a `RepositoryLocked` error, a failed fetch `FetchFailed`,
/// a local baseline `NoUpstream`, a refused guard `GuardRefusal`
#[instrument(skip(git_executor, on_progress))]
pub fn update_project_with(git_executor: &GitCommandExecutor, repo_path: &str, options: UpdateOptions, on_progress: &dyn Fn(UpdateProgress)) -> Result<UpdateProjectResult> {
  let baseline_branch = detect_baseline_branch(git_executor, repo_path, "master")?;
  let upstream = resolve_baseline_upstream(git_executor, repo_path, &baseline_branch)?.ok_or_else(|| {
    anyhow::Error::new(NoUpstream {
      baseline: baseline_branch.clone(),
    })
  })?;
  // refuse before the network: the check would refuse after it anyway
  check_update_guards(git_executor, repo_path)?;

  let (previous, fetched_at) = if options.fetch {
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

  on_progress(UpdatePhase::Checking.into());
  let _lock = lock_repository(git_executor, repo_path, options.lock_timeout)?;
  let grouping = read_grouping_inputs(git_executor, repo_path)?;
  let mut plan = check_update(
    git_executor,
    repo_path,
    UpstreamRef {
      tracking_ref: &upstream.tracking_ref,
      name: &upstream.name,
      previous: previous.as_deref(),
    },
    CheckPurpose::Apply,
    &|scope| landed_candidates(git_executor, repo_path, scope, &grouping),
  )?;
  plan.check.fetched_at = fetched_at;

  let mut landed = Vec::new();
  let applied = match &plan.check.state {
    UpdateState::Clean { .. } => {
      on_progress(UpdatePhase::Updating.into());
      let applied = apply_update(git_executor, repo_path, &plan)?;
      landed = record_landed_update(git_executor, repo_path, &plan.check, plan.excluded(), &applied.new_head, &grouping);
      Some(applied)
    }
    UpdateState::UpToDate => None,
    UpdateState::Conflict { .. } | UpdateState::Blocked { .. } => {
      info!(state = ?plan.check.state, "update not applied");
      None
    }
  };
  Ok(UpdateProjectResult {
    check: plan.check,
    applied,
    landed,
  })
}

/// What an applied update landed, added to the report. The update is done: a failure here is logged, not returned.
fn record_landed_update(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  check: &UpdateCheck,
  excluded: &[String],
  new_head: &str,
  grouping: &GroupingInputs,
) -> Vec<LandedBranch> {
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
      },
    )?;
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
    let valid = is_full_id(previous_head) && is_full_id(new_head) && previous_head.len() == new_head.len() && previous_head != new_head;
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

/// Local changes sit on paths the undo of an update would write; nothing was changed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoBlocked {
  pub paths: Vec<String>,
}

impl std::fmt::Display for UndoBlocked {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Uncommitted, staged or untracked files sit on paths the undo would write: {}", self.paths.join(", "))
  }
}

impl std::error::Error for UndoBlocked {}

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
  let _lock = lock_repository(git_executor, repo_path, lock_timeout)?;
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
