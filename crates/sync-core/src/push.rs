//! The push of virtual branches, forced and leased on the remote heads the user saw (`push_lease.rs`): the app's push of
//! one branch and of a stack (`push_branches`, `repository_model::push_guard`), and `bd push`, all through one leased push
//! (`push_leased`). A stack goes in one atomic `git push`, all of it or nothing; a remote that moved meanwhile is fetched
//! and reported, never overwritten blindly: the push goes again over what it has now, once, only when nothing of it is
//! lost (`domain_policy::push::over_moved_remote`; the app through `PushBranchesParams::over_moved`, `bd push` on its
//! own leases). The stack branch of a whole stack (`{prefix}/stack/<root>`, `whole_stack`) goes the same way.

use crate::config_snapshot::ConfigSnapshot;
use crate::push_lease::{PushOptions, PushRef, PushedRefStatus, fetch_remote_branches, prepare_push_expecting, push_refs};
use crate::push_size::{PushTooLarge, check_push_size};
use crate::remote_status::{AncestorCommits, StackParent, StatusBranch, compute_remote_statuses};
use crate::transfer_progress::TransferProgress;
use anyhow::{Context, Result, bail};
use domain_policy::push::over_moved_remote;
use domain_types::RemoteStatusUpdate;
use domain_types::branch_names::{BranchKind, qualified_branch_name, to_final_branch_name, to_stack_branch_name};
use domain_types::stack::BlockReason;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, instrument, warn};

/// The remote the pushes of the virtual branches go to: the remote-tracking refs `refs/remotes/origin/<full name>` they
/// move are the ones a session replaces in its sync inputs
pub const PUSH_REMOTE: &str = "origin";

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PushBranchParams {
  pub repository_path: String,
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
  /// The remote branch is at the pushed commit. `pushed_over`: the remote branch had moved since the status the user
  /// saw, without commits the branch lacks, so the push went again, once, over this status of it (fetched after the
  /// first push was refused); `None` when the first push went
  #[serde(rename_all = "camelCase")]
  Pushed {
    remote_status: RemoteStatusUpdate,
    pushed_over: Option<RemoteStatusUpdate>,
  },
  /// The remote branch was not where the user saw it (`expected_remote`): nothing was pushed. The branch was fetched,
  /// and `remote_status` shows what the remote has now; a push with its `remote_tip` replaces that
  #[serde(rename_all = "camelCase")]
  RemoteMoved { remote_status: RemoteStatusUpdate },
}

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

/// The refusal of a push before the sync whose branch table says which branches it built (`not_built_refusal`): the
/// page shows no branch to push before it
pub const NOT_SYNCED_YET: &str = "Nothing was pushed: the branches are not synced yet; sync first";

/// The refusal of a push of `branches` while one of their virtual branches is not the build of its commits: the last
/// sync did not build it (`not_built`, `SyncReport::not_built`), so its ref is a build of commits it may no longer have.
/// The stack branch of a whole stack and an unapplied branch are no build of that sync; `None` when nothing refuses
pub fn not_built_refusal(not_built: &HashMap<String, BlockReason>, branches: &[StackPushBranch]) -> Option<String> {
  let refusals: Vec<String> = branches
    .iter()
    .filter(|branch| !branch.whole_stack && !branch.unapplied)
    .filter_map(|branch| not_built.get(&branch.branch_name).map(|reason| not_synced_refusal(&branch.branch_name, *reason)))
    .collect();
  (!refusals.is_empty()).then(|| format!("Nothing was pushed: {}", refusals.join(", ")))
}

impl PushBranchParams {
  /// The stack push of this one branch (`push_branches`)
  pub fn as_stack(&self) -> PushBranchesParams {
    PushBranchesParams {
      repository_path: self.repository_path.clone(),
      my_email: self.my_email.clone(),
      baseline_branch: self.baseline_branch.clone(),
      branches: vec![StackPushBranch {
        branch_name: self.branch_name.clone(),
        parent_branch: self.parent_branch.clone(),
        expected_remote: self.expected_remote.clone(),
        tip: self.tip.clone(),
        whole_stack: false,
        unapplied: false,
      }],
    }
  }
}

impl PushOutcome {
  /// What the stack push of one branch (`PushBranchParams::as_stack`) did to it
  pub fn of_stack(outcome: PushBranchesOutcome) -> Result<Self> {
    let first = |statuses: Vec<RemoteStatusUpdate>| statuses.into_iter().next().context("No remote status of the pushed branch");
    Ok(match outcome {
      PushBranchesOutcome::Pushed { remote_statuses, pushed_over } => Self::Pushed {
        remote_status: first(remote_statuses)?,
        pushed_over: pushed_over.into_iter().next(),
      },
      PushBranchesOutcome::RemoteMoved { remote_statuses, .. } => Self::RemoteMoved {
        remote_status: first(remote_statuses)?,
      },
    })
  }
}

/// One branch of a stack push (`PushBranchesParams`)
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackPushBranch {
  pub branch_name: String,
  /// The branch it is built on (`BranchStack::effective_base`): its commits are not counted as unpushed commits of
  /// this branch
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub parent_branch: Option<String>,
  /// The remote head of the status the user saw (`RemoteStatusUpdate::remote_tip`): the push replaces the remote
  /// branch only if it is still there. `None`: the status showed no remote branch, and the branch must not exist
  pub expected_remote: Option<String>,
  /// Push Up to Here: the commit to push instead of the tip (`PushBranchParams::tip`)
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub tip: Option<String>,
  /// The stack branch of the whole stack whose root `branch_name` is (`{prefix}/stack/<root>`), not its virtual branch:
  /// the caller pushes it only while the sync built it (`StackBranchState::Built`); no parent, no tip
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub whole_stack: bool,
  /// The unapplied branch `{prefix}/unapplied/<branch_name>`, pushed under its own name so the remote keeps a copy: no
  /// parent, no tip; a pull request of its virtual branch stays as it was
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub unapplied: bool,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PushBranchesParams {
  pub repository_path: String,
  pub my_email: Option<String>,
  pub baseline_branch: String,
  /// Parents first: the order of the refspecs and of the remote statuses of the outcome
  pub branches: Vec<StackPushBranch>,
}

/// What a stack push did
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PushBranchesOutcome {
  /// Every branch is at its pushed commit on the remote; their remote statuses, in the order of the branches.
  /// `pushed_over`: the remote branch of one had moved since the status the user saw, without commits it lacks, so the
  /// push went again, once, over these statuses of every branch, fetched after the first push was refused, in the order
  /// of the branches (`PushBranchesParams::over_moved`); empty when the first push went
  #[serde(rename_all = "camelCase")]
  Pushed {
    remote_statuses: Vec<RemoteStatusUpdate>,
    pushed_over: Vec<RemoteStatusUpdate>,
  },
  /// The remote branch of `branch_name` (the first such, parents first) was not where the user saw it, so nothing was
  /// pushed: the push is atomic. Every branch was fetched, and `remote_statuses` show what the remote has now, in the
  /// order of the branches; a push with the `remote_tip` of `branch_name` replaces it. `whole_stack`: the one that moved
  /// is the stack branch of the whole stack of `branch_name` (`StackPushBranch::whole_stack`), not its virtual branch,
  /// which may be in the same push (Push with Children of the root)
  #[serde(rename_all = "camelCase")]
  RemoteMoved {
    branch_name: String,
    whole_stack: bool,
    remote_statuses: Vec<RemoteStatusUpdate>,
  },
}

impl PushBranchesParams {
  /// The push again over what the remote has now, after `refused` (the outcome of this push) found a remote branch moved
  /// since the status the user saw: every branch leased on the head fetched after the refusal, when the remote of each
  /// one that moved has no commits its branch lacks and the branch has work for it
  /// (`domain_policy::push::over_moved_remote`); `None` for a push that went, or when a moved remote has someone's
  /// commits or the branch as it is, which the outcome tells instead
  pub fn over_moved(&self, refused: &PushBranchesOutcome) -> Option<Self> {
    let PushBranchesOutcome::RemoteMoved { remote_statuses, .. } = refused else {
      return None;
    };
    if remote_statuses.len() != self.branches.len() {
      return None;
    }
    let moved: Vec<&RemoteStatusUpdate> = self
      .branches
      .iter()
      .zip(remote_statuses)
      .filter(|(branch, status)| status.remote_tip != branch.expected_remote)
      .map(|(_, status)| status)
      .collect();
    if moved.is_empty() || !moved.into_iter().all(over_moved_remote) {
      return None;
    }
    let branches = self
      .branches
      .iter()
      .zip(remote_statuses)
      .map(|(branch, status)| StackPushBranch {
        expected_remote: status.remote_tip.clone(),
        ..branch.clone()
      })
      .collect();
    Some(Self {
      repository_path: self.repository_path.clone(),
      my_email: self.my_email.clone(),
      baseline_branch: self.baseline_branch.clone(),
      branches,
    })
  }
}

impl PushBranchesOutcome {
  /// This outcome of the push again over the moved remote (`PushBranchesParams::over_moved`) after `refused`, the
  /// first push: one that went says what it went over
  #[must_use]
  pub fn over(self, refused: Self) -> Self {
    match (self, refused) {
      (Self::Pushed { remote_statuses, .. }, Self::RemoteMoved { remote_statuses: pushed_over, .. }) => Self::Pushed { remote_statuses, pushed_over },
      (outcome, _) => outcome,
    }
  }
}

/// Force-pushes the virtual branches to `origin` in one atomic `git push`, each leased on the remote head the user saw
/// (`push_lease`): the stack goes whole or not at all. The tips are read once (`prepare_push_branches`), a branch pushed
/// up to a commit is checked (`check_push_tip`), and a remote branch that moved meanwhile is fetched, not overwritten:
/// the outcome names it, with the remote statuses after the fetch (`PreparedPush::send`). A branch the last sync did
/// not build (`not_built`) refuses the whole push (`not_built_refusal`). The branches are named under `branch_prefix`;
/// `on_progress` gets how far the push is.
#[instrument(skip(git_executor, not_built, on_progress), fields(repository_path = %params.repository_path, branches = params.branches.len()))]
pub fn push_branches(
  git_executor: &GitCommandExecutor,
  branch_prefix: &str,
  params: &PushBranchesParams,
  not_built: &HashMap<String, BlockReason>,
  on_progress: &mut dyn FnMut(TransferProgress),
) -> Result<PushBranchesOutcome> {
  prepare_push_branches(git_executor, branch_prefix, params, not_built)?.send(git_executor, on_progress)
}

/// The full names of the refs a push of `branches` writes (`{prefix}/virtual/<branch>`, `{prefix}/stack/<root>` of a
/// whole stack, `{prefix}/unapplied/<branch>`, under `branch_prefix`), each with the virtual branch of its parent; no git
/// process
pub fn push_ref_names(branch_prefix: &str, params: &PushBranchesParams) -> Result<Vec<(String, Option<String>)>> {
  params
    .branches
    .iter()
    .map(|branch| {
      if branch.whole_stack {
        return Ok((to_stack_branch_name(branch_prefix, &branch.branch_name)?, None));
      }
      if branch.unapplied {
        return Ok((qualified_branch_name(branch_prefix, BranchKind::Unapplied, &branch.branch_name)?, None));
      }
      let final_name = to_final_branch_name(branch_prefix, &branch.branch_name)?;
      let parent_ref = branch.parent_branch.as_deref().map(|parent| to_final_branch_name(branch_prefix, parent)).transpose()?;
      Ok((final_name, parent_ref))
    })
    .collect()
}

/// A push whose commits and leases are fixed (`prepare_push_branches`): each ref goes as the commit it was at then
/// (`<commit>:refs/heads/<name>`), leased on the remote head the user saw, so a branch that moves before `send` changes
/// nothing of what goes
#[derive(Debug)]
pub struct PreparedPush {
  repository_path: String,
  baseline: String,
  my_email: Option<String>,
  /// `branchdeck.pushSizeLimit` as the push was prepared; `None` for no check
  size_limit: Option<u64>,
  branches: Vec<LeasedBranch>,
  /// `StackPushBranch::whole_stack` of each branch, in their order
  whole_stack: Vec<bool>,
}

/// The local part of `push_branches`, for a caller that checks what goes under the repository lock and pushes after
/// it: the refusal of a branch the last sync did not build (`not_built_refusal`), the tips read with one `for-each-ref`
/// (`prepare_push_expecting`), the commit of a push up to it checked (`check_push_tip`). No network
#[instrument(skip(git_executor, not_built), fields(repository_path = %params.repository_path, branches = params.branches.len()))]
pub fn prepare_push_branches(
  git_executor: &GitCommandExecutor,
  branch_prefix: &str,
  params: &PushBranchesParams,
  not_built: &HashMap<String, BlockReason>,
) -> Result<PreparedPush> {
  if let Some(refusal) = not_built_refusal(not_built, &params.branches) {
    bail!(refusal);
  }
  let repository_path = &params.repository_path;
  let names = push_ref_names(branch_prefix, params)?;
  let expected: Vec<(String, Option<String>)> = names
    .iter()
    .zip(&params.branches)
    .map(|((final_name, _), branch)| (final_name.clone(), branch.expected_remote.clone()))
    .collect();
  let mut refs = prepare_push_expecting(git_executor, repository_path, &expected)?;
  for ((push, branch), (_, parent_ref)) in refs.iter_mut().zip(&params.branches).zip(&names) {
    if let Some(tip) = &branch.tip {
      let base = parent_ref.as_deref().unwrap_or(&params.baseline_branch);
      push.tip = check_push_tip(git_executor, repository_path, &push.tip, base, tip, branch.expected_remote.as_deref())?;
    }
  }

  let branches: Vec<LeasedBranch> = refs
    .into_iter()
    .zip(&params.branches)
    .zip(names)
    .map(|((push, branch), (_, parent_ref))| LeasedBranch {
      name: branch.branch_name.clone(),
      push,
      parent_ref,
    })
    .collect();
  Ok(PreparedPush {
    repository_path: repository_path.clone(),
    baseline: params.baseline_branch.clone(),
    my_email: params.my_email.clone(),
    size_limit: ConfigSnapshot::read(git_executor, repository_path)?.push_size_limit()?,
    branches,
    whole_stack: params.branches.iter().map(|branch| branch.whole_stack).collect(),
  })
}

impl PreparedPush {
  /// The network part of `push_branches`: one `git push --porcelain --atomic --progress` of the prepared commits
  /// (`push_leased`), then the remote statuses of the pushed branches, or, refused, of the fetched ones
  #[instrument(skip_all, fields(repository_path = %self.repository_path, branches = self.branches.len()))]
  pub fn send(&self, git_executor: &GitCommandExecutor, on_progress: &mut dyn FnMut(TransferProgress)) -> Result<PushBranchesOutcome> {
    let target = PushTarget {
      repository_path: &self.repository_path,
      remote: PUSH_REMOTE,
      baseline: &self.baseline,
      my_email: self.my_email.as_deref(),
      size_limit: self.size_limit,
    };
    // one ref needs no `--atomic`, which a remote may not support
    let options = PushOptions {
      atomic: self.branches.len() > 1,
      dry_run: false,
    };
    match push_leased(git_executor, &target, &self.branches, options, Some(on_progress))? {
      LeasedPush::Pushed(_) => Ok(PushBranchesOutcome::Pushed {
        remote_statuses: leased_statuses(git_executor, &target, &self.branches).context("Failed to compute remote status")?,
        pushed_over: Vec::new(),
      }),
      LeasedPush::RemoteMoved { moved, remote_statuses } => Ok(PushBranchesOutcome::RemoteMoved {
        branch_name: self.branches[moved[0]].name.clone(),
        whole_stack: self.whole_stack[moved[0]],
        remote_statuses,
      }),
      LeasedPush::Refused { summary } => bail!("The remote rejected the push: {summary}"),
      LeasedPush::TooLarge(too_large) => Err(anyhow::Error::new(too_large)),
      LeasedPush::WouldReject => bail!("The remote would reject the push"),
    }
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
}

/// Where a leased push goes, and what the remote statuses after it compare with
#[derive(Debug, Clone, Copy)]
pub struct PushTarget<'a> {
  pub repository_path: &'a str,
  pub remote: &'a str,
  /// The baseline of the remote statuses ("origin/master")
  pub baseline: &'a str,
  /// The user's email: the statuses count the user's own commits; `None` counts none
  pub my_email: Option<&'a str>,
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
pub fn push_leased(
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
    })
    .collect();
  compute_remote_statuses(git_executor, target.repository_path, target.remote, &statuses, target.my_email, target.baseline)
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
