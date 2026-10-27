//! The push of virtual branches, forced and leased on the remote heads the user saw (`push_lease.rs`): the intent
//! `Push` of the engine and the push of a start (`operations::push`, `operations::land_execute`), and `bd push`, each
//! freezing its commits and leases (`PreparedPush::leased`) and sending them through one leased push (`PreparedPush::push_leased`).
//! A stack goes in one atomic `git push`, all of it or nothing; a remote that moved meanwhile is fetched and reported,
//! never overwritten blindly: the push goes again over what it has now, once, only when nothing of it is lost
//! (`domain_policy::push::over_moved_remote`, `PreparedPush::over_moved`, which `operations::push::send` runs for the
//! app and `bd push` alike). The stack branch of a whole stack (`{prefix}/stack/<root>`) goes the same way.

use crate::push_lease::{LeaseSource, PushOptions, PushRef, PushedRefStatus, fetch_remote_branches, push_refs};
use crate::push_size::{PushTooLarge, check_push_size};
use crate::remote_status::{AncestorCommits, StackParent, StatusBranch, compute_remote_statuses};
use crate::transfer_progress::TransferProgress;
use anyhow::{Context, Result};
use domain_policy::push::over_moved_remote;
use domain_types::RemoteStatusUpdate;
use domain_types::stack::BlockReason;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use tracing::{info, instrument, warn};

/// The remote the pushes of the virtual branches go to: the remote-tracking refs `refs/remotes/origin/<full name>` they
/// move are the ones a session replaces in its sync inputs
pub const PUSH_REMOTE: &str = "origin";

/// Why the last sync did not build a branch (`SyncReport::not_built`), in words
pub const fn not_built_reason(reason: BlockReason) -> &'static str {
  match reason {
    BlockReason::Problem => "a stack problem",
    BlockReason::Conflict => "a merge conflict",
    BlockReason::Error => "a sync error",
    BlockReason::Blocked => "its parent is not built",
  }
}

/// The refusal of the push of a branch the last sync did not build: `{name} is not synced (…)`, as `bd push` says it
pub fn not_synced_refusal(name: &str, reason: BlockReason) -> String {
  format!("{name} is not synced ({})", not_built_reason(reason))
}

/// A push whose commits and leases are fixed under the repository lock (`PreparedPush::leased`, by the engine on the
/// frozen tips, by `bd push` on the tips a sync under the same lock left): each ref goes as the commit it was at then
/// (`<commit>:refs/heads/<name>`), leased on the remote head the user saw, so a branch that moves before the push changes
/// nothing of what goes. Its network part is `push`, its push again over a moved remote `over_moved`
/// (`operations::push::send` runs both)
#[derive(Debug, Clone)]
pub struct PreparedPush {
  repository_path: String,
  baseline: String,
  /// `branchdeck.pushSizeLimit` as the push was prepared; `None` for no check
  size_limit: Option<u64>,
  remote: String,
  options: PushOptions,
  branches: Vec<LeasedBranch>,
}

impl PreparedPush {
  /// The push of `branches` (parents first, the stack branches of whole stacks after them) whose commits and leases the
  /// caller fixed itself under the repository lock (the engine, `bd push` on the tips of the sync it ran under it): to
  /// `target.remote` with `options`
  #[must_use]
  pub fn leased(target: &PushTarget<'_>, branches: Vec<LeasedBranch>, options: PushOptions) -> Self {
    Self {
      repository_path: target.repository_path.to_string(),
      baseline: target.baseline.to_string(),
      size_limit: target.size_limit,
      remote: target.remote.to_string(),
      options,
      branches,
    }
  }

  /// The refs of the push, parents first, as they go
  #[must_use]
  pub fn branches(&self) -> &[LeasedBranch] {
    &self.branches
  }

  fn target(&self) -> PushTarget<'_> {
    PushTarget {
      repository_path: &self.repository_path,
      remote: &self.remote,
      baseline: &self.baseline,
      size_limit: self.size_limit,
    }
  }

  /// The network part of every push: one `git push --porcelain --progress` of the prepared commits, `--atomic` for more
  /// than one ref
  ///
  /// # Errors
  /// What git failed with
  #[instrument(skip_all, fields(repository_path = %self.repository_path, branches = self.branches.len()))]
  pub fn push_leased(&self, git_executor: &GitCommandExecutor, on_progress: &mut dyn FnMut(TransferProgress)) -> Result<LeasedPush> {
    send_leased(git_executor, &self.target(), &self.branches, self.options, Some(on_progress))
  }

  /// The push again over what the remote has now, after `refused` (the answer of this push) found remote branches
  /// moved since the status the user saw: the same commits, every ref leased on the head fetched after the refusal,
  /// when the remote of each one that moved has no commits its branch lacks and the branch has work for it
  /// (`domain_policy::push::over_moved_remote`); `None` for a push that went, or when a moved remote has someone's
  /// commits or the branch as it is, which the answer tells instead
  #[must_use]
  pub fn over_moved(&self, refused: &LeasedPush) -> Option<Self> {
    let LeasedPush::RemoteMoved { moved, remote_statuses } = refused else {
      return None;
    };
    if remote_statuses.len() != self.branches.len() || !moved.iter().all(|index| remote_statuses.get(*index).is_some_and(over_moved_remote)) {
      return None;
    }
    let branches = self
      .branches
      .iter()
      .zip(remote_statuses)
      .map(|(branch, status)| LeasedBranch {
        push: PushRef {
          lease: status.remote_tip.clone(),
          source: if status.remote_tip.is_some() { LeaseSource::Expected } else { LeaseSource::Absent },
          ..branch.push.clone()
        },
        ..branch.clone()
      })
      .collect();
    Some(Self { branches, ..self.clone() })
  }

  /// The remote statuses of the pushed branches now, in their order
  ///
  /// # Errors
  /// What git failed with
  pub fn statuses(&self, git_executor: &GitCommandExecutor) -> Result<Vec<RemoteStatusUpdate>> {
    leased_statuses(git_executor, &self.target(), &self.branches).context("Failed to compute remote status")
  }
}

/// A branch of a leased push (`push_leased`): its push, and what its remote status is computed from
#[derive(Debug, Clone)]
pub struct LeasedBranch {
  /// The virtual branch name ("auth")
  pub name: String,
  pub push: PushRef,
  /// `{prefix}/virtual/<parent>` of a stacked branch: the commits up to it are the parent's, not the branch's
  pub parent_ref: Option<String>,
  /// The branch's author (`GroupedBranchInfo::my_email`): its remote status after the push counts that author's
  /// commits as the user's, as a sync does; `None` counts none
  pub my_email: Option<String>,
}

/// Where a leased push goes, and what the remote statuses after it compare with
#[derive(Debug, Clone, Copy)]
pub struct PushTarget<'a> {
  pub repository_path: &'a str,
  pub remote: &'a str,
  /// The baseline of the remote statuses ("origin/master")
  pub baseline: &'a str,
  /// The bytes the push may add (`branchdeck.pushSizeLimit`, `ConfigSnapshot::push_size_limit`); `None` for no check
  pub size_limit: Option<u64>,
}

/// What a leased push did
#[derive(Debug)]
pub enum LeasedPush {
  /// The remote took every branch (a dry run: would take them); remote ref -> its report
  Pushed(HashMap<String, PushedRefStatus>),
  /// The remote branches at `moved` (indexes into the branches, parents first) were not where their leases expected
  /// them, so nothing was pushed. Every branch was fetched; `remote_statuses` say what the remote has now, in the order
  /// of the branches
  RemoteMoved { moved: Vec<usize>, remote_statuses: Vec<RemoteStatusUpdate> },
  /// Every lease held, yet the remote refused the push (a hook, a protected branch, a pack too large): nothing was
  /// pushed. `summary` is what git said of each refused ref and what the remote said why, for an error message
  Refused { summary: String },
  /// A dry run the remote would refuse: nothing is fetched on a dry run, so which lease failed is not known
  WouldReject,
  /// The files the push adds exceed `PushTarget::size_limit`: it did not talk to the remote, a dry run neither
  TooLarge(PushTooLarge),
}

/// The one leased push of the app and of `bd push`: first the size check of the files the push adds
/// (`check_push_size`, under `target.size_limit`), then one `git push --porcelain` of every branch to `target.remote`,
/// each with its lease (`push_refs`), `--atomic` for a stack, so the remote takes all of them or none. A refused push
/// fetches every branch (`fetch_remote_branches`, not on a dry run) and compares the fetched heads with the leases: the
/// branches whose remote moved are named, with the remote statuses after the fetch; when none moved, the remote
/// refused for another reason.
#[instrument(skip(git_executor, branches, on_progress), fields(remote = %target.remote, branches = branches.len()))]
fn send_leased(
  git_executor: &GitCommandExecutor,
  target: &PushTarget<'_>,
  branches: &[LeasedBranch],
  options: PushOptions,
  on_progress: Option<&mut dyn FnMut(TransferProgress)>,
) -> Result<LeasedPush> {
  if let Some(limit) = target.size_limit {
    let tips: Vec<&str> = branches.iter().map(|branch| branch.push.tip.as_str()).collect();
    if let Some(too_large) = check_push_size(git_executor, target.repository_path, &tips, target.remote, limit)? {
      return Ok(LeasedPush::TooLarge(too_large));
    }
  }
  let refs: Vec<PushRef> = branches.iter().map(|branch| branch.push.clone()).collect();
  // the remote policy keeps the user's credential helpers but never prompts: the push must not block on input
  let report = push_refs(git_executor, target.repository_path, target.remote, &refs, options, on_progress)?;
  let report_of = |branch: &LeasedBranch| report.refs.get(&format!("refs/heads/{}", branch.push.full_name));
  let refused: Vec<&LeasedBranch> = branches.iter().filter(|branch| report_of(branch).is_none_or(|report| !report.accepted())).collect();
  if refused.is_empty() {
    return Ok(LeasedPush::Pushed(report.refs));
  }
  if options.dry_run {
    return Ok(LeasedPush::WouldReject);
  }

  let full_names: Vec<String> = branches.iter().map(|branch| branch.push.full_name.clone()).collect();
  fetch_remote_branches(git_executor, target.repository_path, target.remote, &full_names)
    .context("The remote rejected the push, and fetching the branches to show what it has failed. Nothing was pushed")?;
  let remote_statuses = leased_statuses(git_executor, target, branches).context("Failed to compute remote status")?;
  let moved: Vec<usize> = branches
    .iter()
    .zip(&remote_statuses)
    .enumerate()
    .filter(|(_, (branch, status))| status.remote_tip != branch.push.lease)
    .map(|(index, _)| index)
    .collect();
  if moved.is_empty() {
    // the leases held: the remote refused the push for another reason, e.g. a hook, a protected branch or a pack too
    // large for it; the report of a ref only names the kind, what the remote said names the reason
    let mut summary = refused
      .iter()
      .map(|branch| format!("{} {}", branch.push.full_name, report_of(branch).map_or("no report", |report| report.summary.as_str())))
      .collect::<Vec<_>>()
      .join("; ");
    if !report.messages.is_empty() {
      warn!(messages = ?report.messages, "the remote refused the push");
      summary = format!("{summary}. {}", report.messages.join(" · "));
    }
    return Ok(LeasedPush::Refused { summary });
  }
  info!(moved = ?moved.iter().map(|index| &branches[*index].name).collect::<Vec<_>>(), "a remote branch moved, nothing pushed");
  Ok(LeasedPush::RemoteMoved { moved, remote_statuses })
}

/// The remote statuses of the branches of a push, in their order (`compute_remote_statuses`); outside the sync the
/// parent's ref is its current head
fn leased_statuses(git_executor: &GitCommandExecutor, target: &PushTarget<'_>, branches: &[LeasedBranch]) -> Result<Vec<RemoteStatusUpdate>> {
  let statuses: Vec<StatusBranch> = branches
    .iter()
    .map(|branch| StatusBranch {
      local_ref: &branch.push.full_name,
      branch_name: &branch.name,
      parent: branch.parent_ref.as_deref().map(|base| StackParent {
        base,
        ancestors: AncestorCommits::ReadWhenNeeded,
      }),
      my_email: branch.my_email.as_deref(),
    })
    .collect();
  compute_remote_statuses(git_executor, target.repository_path, target.remote, &statuses, target.baseline)
}
