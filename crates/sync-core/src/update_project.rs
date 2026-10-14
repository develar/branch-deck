//! Update Project: fetch the baseline from its remote and bring the checked out branch onto it, keeping unrelated
//! local changes (docs/design/git-operations.md, "Update project"). The same check runs inside every sync against the
//! last fetched state, without the network, so the header can show whether an update would apply cleanly.

use crate::baseline::detect_baseline_branch;
use crate::landed::{LandedQuery, find_landed, find_landing, landed_candidates, local_revs, record_update, remaining_revs};
use crate::remote_command::execute_remote_command_with_progress;
use crate::stack::stacks_enabled;
use crate::transfer_progress::TransferProgress;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::rewrite_guard::{ensure_no_operation_in_progress, get_current_branch};
use git_ops::update_branch::{AppliedUpdate, CheckPurpose, UpdateCheck, UpdateState, UpstreamRef, apply_update, check_update};
use serde::{Deserialize, Serialize};
use std::time::UNIX_EPOCH;
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
#[instrument(skip(git_executor))]
pub fn check_baseline_update(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str) -> Result<Option<BaselineCheck>> {
  let Some(upstream) = resolve_baseline_upstream(git_executor, repo_path, baseline_branch)? else {
    return Ok(None);
  };
  let stacks = stacks_enabled(git_executor, repo_path)?;
  let mut plan = check_update(
    git_executor,
    repo_path,
    UpstreamRef {
      tracking_ref: &upstream.tracking_ref,
      name: &upstream.name,
      previous: None,
    },
    CheckPurpose::Preview,
    &|scope| landed_candidates(git_executor, repo_path, scope, stacks),
  )?;
  plan.check.fetched_at = last_fetch_time(git_executor, repo_path);
  // the landing is a detail of the check: it must not fail it
  let landing = find_landing(git_executor, repo_path, &plan.check, plan.excluded(), plan.replayed_head(), stacks).unwrap_or_else(|e| {
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

/// Fetch the baseline, check the update and apply it if it is clean. A conflict or a block is a result, not an error:
/// nothing is changed, and the check says why.
#[instrument(skip(git_executor, params, on_progress), fields(repository_path = %params.repository_path))]
pub fn update_project_core(git_executor: &GitCommandExecutor, params: &UpdateProjectParams, on_progress: &dyn Fn(UpdateProgress)) -> Result<UpdateProjectResult> {
  let repo_path = &params.repository_path;
  let baseline_branch = detect_baseline_branch(git_executor, repo_path, "master")?;
  let upstream = resolve_baseline_upstream(git_executor, repo_path, &baseline_branch)?
    .ok_or_else(|| anyhow!("The baseline {baseline_branch} is a local branch: there is no upstream to update from"))?;
  // refuse before the network: the check would refuse after it anyway
  ensure_no_operation_in_progress(git_executor, repo_path)?;
  get_current_branch(git_executor, repo_path)?;

  on_progress(UpdatePhase::Fetching.into());
  let mut on_fetch_progress = |fetch| {
    on_progress(UpdateProgress {
      phase: UpdatePhase::Fetching,
      fetch: Some(fetch),
    });
  };
  let previous = fetch_baseline(git_executor, repo_path, &upstream, &mut on_fetch_progress).map_err(|e| anyhow!("Failed to fetch {}: {e:#}", upstream.name))?;
  let fetched_at = unix_now();

  on_progress(UpdatePhase::Checking.into());
  let stacks = stacks_enabled(git_executor, repo_path)?;
  let mut plan = check_update(
    git_executor,
    repo_path,
    UpstreamRef {
      tracking_ref: &upstream.tracking_ref,
      name: &upstream.name,
      previous: previous.as_deref(),
    },
    CheckPurpose::Apply,
    &|scope| landed_candidates(git_executor, repo_path, scope, stacks),
  )?;
  plan.check.fetched_at = Some(fetched_at);

  let mut landed = Vec::new();
  let applied = match &plan.check.state {
    UpdateState::Clean { .. } => {
      on_progress(UpdatePhase::Updating.into());
      let applied = apply_update(git_executor, repo_path, &plan)?;
      landed = record_landed_update(git_executor, repo_path, &plan.check, plan.excluded(), &applied.new_head, stacks);
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
fn record_landed_update(git_executor: &GitCommandExecutor, repo_path: &str, check: &UpdateCheck, excluded: &[String], new_head: &str, stacks: bool) -> Vec<LandedBranch> {
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
        stacks,
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

#[cfg(test)]
#[path = "update_project_test.rs"]
pub(crate) mod update_project_test;
