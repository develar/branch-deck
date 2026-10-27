//! The push of the tests of the core push: the branches of a stack, each leased on the remote head the user saw,
//! frozen at their tips now (`prepare_push_expecting`) as the engine freezes the refs of a plan
//! (`operations::land_execute`), sent through `PreparedPush::push_leased`, and what the remote has after it. The app and `bd`
//! push through the same `PreparedPush`; this is only the shape the tests name a push in

use crate::config_snapshot::ConfigSnapshot;
use crate::push::{LeasedBranch, LeasedPush, PUSH_REMOTE, PreparedPush, PushTarget};
use crate::push_lease::{PushOptions, prepare_push_expecting};
use crate::transfer_progress::TransferProgress;
use anyhow::{Result, bail};
use domain_types::RemoteStatusUpdate;
use domain_types::branch_names::{BranchKind, qualified_branch_name, to_final_branch_name, to_stack_branch_name};
use git_executor::git_command_executor::GitCommandExecutor;

/// One branch of a push of the tests
#[derive(Debug, Clone, Default)]
pub(crate) struct StackPushBranch {
  pub(crate) branch_name: String,
  /// The branch it is built on: its commits are not counted as unpushed commits of this branch
  pub(crate) parent_branch: Option<String>,
  /// The remote head the push replaces; `None`: the remote branch must not exist
  pub(crate) expected_remote: Option<String>,
  /// Push Up to Here: the commit pushed instead of the tip
  pub(crate) tip: Option<String>,
  /// The stack branch of the whole stack of `branch_name` (`{prefix}/stack/<root>`)
  pub(crate) whole_stack: bool,
  /// The unapplied branch `{prefix}/unapplied/<branch_name>`
  pub(crate) unapplied: bool,
}

/// A push of the tests: the branches parents first, the baseline the statuses compare with
pub(crate) struct PushBranchesParams {
  pub(crate) repository_path: String,
  pub(crate) my_email: Option<String>,
  pub(crate) baseline_branch: String,
  pub(crate) branches: Vec<StackPushBranch>,
}

/// What a push of the tests did
#[derive(Debug)]
pub(crate) enum PushBranchesOutcome {
  /// The remote statuses after it, in the order of the branches; those a push again over a moved remote went over
  Pushed {
    remote_statuses: Vec<RemoteStatusUpdate>,
    pushed_over: Vec<RemoteStatusUpdate>,
  },
  /// The remote of `branch_name` (the first that moved; `whole_stack`: its stack branch) was not where its lease
  /// expected it: nothing was pushed, the statuses of what the remote has now
  RemoteMoved {
    branch_name: String,
    whole_stack: bool,
    remote_statuses: Vec<RemoteStatusUpdate>,
  },
}

/// The push of `params` frozen at the tips of its branches now, under `branch_prefix`
pub(crate) fn prepare_push_branches(git: &GitCommandExecutor, branch_prefix: &str, params: &PushBranchesParams) -> Result<PreparedPush> {
  let full = |name: &str| to_final_branch_name(branch_prefix, name);
  let mut names = Vec::with_capacity(params.branches.len());
  for branch in &params.branches {
    let full_name = if branch.whole_stack {
      to_stack_branch_name(branch_prefix, &branch.branch_name)?
    } else if branch.unapplied {
      qualified_branch_name(branch_prefix, BranchKind::Unapplied, &branch.branch_name)?
    } else {
      full(&branch.branch_name)?
    };
    names.push((full_name, branch.expected_remote.clone()));
  }
  let refs = prepare_push_expecting(git, &params.repository_path, &names)?;
  let mut leased = Vec::with_capacity(refs.len());
  for (mut push, branch) in refs.into_iter().zip(&params.branches) {
    if let Some(tip) = &branch.tip {
      push.tip.clone_from(tip);
    }
    leased.push(LeasedBranch {
      name: branch.branch_name.clone(),
      push,
      parent_ref: branch.parent_branch.as_deref().map(full).transpose()?,
      my_email: params.my_email.clone(),
    });
  }
  let target = PushTarget {
    repository_path: &params.repository_path,
    remote: PUSH_REMOTE,
    baseline: &params.baseline_branch,
    size_limit: ConfigSnapshot::read(git, &params.repository_path)?.push_size_limit()?,
  };
  let options = PushOptions {
    atomic: leased.len() > 1,
    dry_run: false,
  };
  Ok(PreparedPush::leased(&target, leased, options))
}

/// What the prepared push of `params` did once sent
pub(crate) fn send_prepared(
  git: &GitCommandExecutor,
  prepared: &PreparedPush,
  params: &PushBranchesParams,
  on_progress: &mut dyn FnMut(TransferProgress),
) -> Result<PushBranchesOutcome> {
  match prepared.push_leased(git, on_progress)? {
    LeasedPush::Pushed(_) => Ok(PushBranchesOutcome::Pushed {
      remote_statuses: prepared.statuses(git)?,
      pushed_over: Vec::new(),
    }),
    LeasedPush::RemoteMoved { moved, remote_statuses } => {
      let first = &params.branches[moved[0]];
      Ok(PushBranchesOutcome::RemoteMoved {
        branch_name: first.branch_name.clone(),
        whole_stack: first.whole_stack,
        remote_statuses,
      })
    }
    LeasedPush::Refused { summary } => bail!("The remote rejected the push: {summary}"),
    LeasedPush::TooLarge(too_large) => Err(anyhow::Error::new(too_large)),
    LeasedPush::WouldReject => bail!("The remote would reject the push"),
  }
}

/// The push of `params` frozen now and sent at once
pub(crate) fn push_branches(
  git: &GitCommandExecutor,
  branch_prefix: &str,
  params: &PushBranchesParams,
  on_progress: &mut dyn FnMut(TransferProgress),
) -> Result<PushBranchesOutcome> {
  let prepared = prepare_push_branches(git, branch_prefix, params)?;
  send_prepared(git, &prepared, params, on_progress)
}
