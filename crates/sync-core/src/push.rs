use crate::push_lease::{PushedRefStatus, fetch_remote_branches, parse_push_porcelain, prepare_push_expecting};
use crate::remote_command::execute_remote_command_with_progress;
use crate::remote_status::{AncestorCommits, StackParent, compute_remote_status_for_branch};
use crate::transfer_progress::TransferProgress;
use anyhow::{Context, Result, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::to_final_branch_name;
use serde::{Deserialize, Serialize};
use sync_types::RemoteStatusUpdate;
use tracing::{info, instrument};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PushBranchParams {
  pub repository_path: String,
  pub branch_prefix: String,
  pub branch_name: String,
  pub my_email: Option<String>,
  pub baseline_branch: String,
  /// The branch a stacked branch is built on (`BranchStack::effective_base`): its commits are not counted as unpushed
  /// commits of this branch
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub parent_branch: Option<String>,
  /// The remote head of the status the user saw (`RemoteStatusUpdate::remote_tip`): the push replaces the remote
  /// branch only if it is still there. `None`: the status showed no remote branch, and the branch must not exist
  pub expected_remote: Option<String>,
  /// Push Up to Here: the commit of the virtual branch to push instead of its tip, so the remote gets the branch up to
  /// it and the commits after it stay unpushed. It must be one of the branch's own commits (not of its parent or the
  /// baseline), newer than `expected_remote` on the same line (`check_push_tip`)
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub tip: Option<String>,
}

/// What a push did
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PushOutcome {
  /// The remote branch is at the pushed commit
  #[serde(rename_all = "camelCase")]
  Pushed { remote_status: RemoteStatusUpdate },
  /// The remote branch was not where the user saw it (`expected_remote`): nothing was pushed. The branch was fetched,
  /// and `remote_status` shows what the remote has now; a push with its `remote_tip` replaces that
  #[serde(rename_all = "camelCase")]
  RemoteMoved { remote_status: RemoteStatusUpdate },
}

/// Force-pushes the virtual branch to `origin`, leased on the remote head the user saw (`push_lease`). A remote that
/// moved meanwhile is fetched, not overwritten: the outcome then carries the new remote status, which a push can
/// replace once the user saw it. `on_progress` gets how far the push is.
#[instrument(skip(git_executor, on_progress), fields(repository_path = %params.repository_path, branch_name = %params.branch_name))]
pub fn push_branch(git_executor: &GitCommandExecutor, params: &PushBranchParams, on_progress: &mut dyn FnMut(TransferProgress)) -> Result<PushOutcome> {
  let repository_path = &params.repository_path;
  let branch_name = &params.branch_name;
  let final_branch_name = to_final_branch_name(&params.branch_prefix, branch_name)?;
  let parent_ref = params
    .parent_branch
    .as_deref()
    .map(|parent| to_final_branch_name(&params.branch_prefix, parent))
    .transpose()?;
  let remote_status = || {
    compute_remote_status_for_branch(
      git_executor,
      repository_path,
      "origin",
      &final_branch_name,
      branch_name,
      params.my_email.as_deref(),
      &params.baseline_branch,
      // outside the sync the parent's ref is its current head
      parent_ref.as_deref().map(|base| StackParent {
        base,
        ancestors: AncestorCommits::ReadWhenNeeded,
      }),
    )
    .context("Failed to compute remote status")
  };

  // The remote policy keeps the user's credential helpers but never prompts: the push must not block on input.
  // Exit code 1: the ref was rejected, and the porcelain report says so
  let mut refs = prepare_push_expecting(git_executor, repository_path, &[(final_branch_name.clone(), params.expected_remote.clone())])?;
  if let Some(tip) = &params.tip {
    let base = parent_ref.as_deref().unwrap_or(&params.baseline_branch);
    refs[0].tip = check_push_tip(git_executor, repository_path, &refs[0].tip, base, tip, params.expected_remote.as_deref())?;
  }
  let pushed = &refs[0];
  let output = execute_remote_command_with_progress(
    git_executor,
    &["push", "--porcelain", &pushed.lease_arg(), "origin", &pushed.refspec()],
    repository_path,
    &[1],
    on_progress,
  )?;
  let report = parse_push_porcelain(&output).remove(&format!("refs/heads/{final_branch_name}"));
  if report.as_ref().is_some_and(PushedRefStatus::accepted) {
    return Ok(PushOutcome::Pushed { remote_status: remote_status()? });
  }

  fetch_remote_branches(git_executor, repository_path, "origin", std::slice::from_ref(&final_branch_name))?;
  let status = remote_status()?;
  if status.remote_tip == params.expected_remote {
    // the lease held: the remote refused the push for another reason, e.g. a hook or a protected branch
    let summary = report.map_or_else(|| "no report".to_string(), |report| report.summary);
    bail!("The remote rejected the push of {final_branch_name}: {summary}");
  }
  info!(expected = ?params.expected_remote, remote_tip = ?status.remote_tip, "the remote branch moved, nothing pushed");
  Ok(PushOutcome::RemoteMoved { remote_status: status })
}

/// The commit `tip` resolved, after checking that a push of the branch at `local` up to it is a prefix of the branch:
/// `tip` is one of the commits on `local` after `base` (the parent's branch, or the baseline), and the remote head
/// `remote` (none: the remote has no branch) is an older one of them, so the push fast-forwards the remote. A virtual
/// branch is a line of cherry-picked commits, so the order of one `rev-list` says which commit is older
pub fn check_push_tip(git_executor: &GitCommandExecutor, repository_path: &str, local: &str, base: &str, tip: &str, remote: Option<&str>) -> Result<String> {
  let own = git_executor.execute_command_lines(&["rev-list", "--first-parent", "--end-of-options", local, &format!("^{base}"), "--"], repository_path)?;
  // newest first
  let position = |id: &str| own.iter().position(|it| it == id);
  let Some(at) = position(tip) else {
    bail!("{tip} is not a commit of the branch after {base}");
  };
  if let Some(remote) = remote {
    match position(remote) {
      Some(remote_at) if remote_at > at => {}
      Some(_) => bail!("the remote branch already has {tip}"),
      None => bail!("the branch was rewritten since its push: it pushes whole"),
    }
  }
  Ok(own[at].clone())
}
