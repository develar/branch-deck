//! The push of virtual branches, forced and leased on the remote heads the user saw (`push_lease.rs`): the app's push of
//! one branch and of a stack (`push_branches`, `repository_view::push_guard`), and `bd push`, all through one leased push
//! (`push_leased`). A stack goes in one atomic `git push`, all of it or nothing; a remote that moved meanwhile is fetched
//! and reported, never overwritten. The stack branch of a whole stack (`{prefix}/stack/<root>`, `whole_stack`) goes the
//! same way.

use crate::push_lease::{PushOptions, PushRef, PushedRefStatus, fetch_remote_branches, prepare_push_expecting, push_refs};
use crate::remote_status::{AncestorCommits, StackParent, StatusBranch, compute_remote_statuses};
use crate::transfer_progress::TransferProgress;
use anyhow::{Context, Result, bail};
use domain_types::RemoteStatusUpdate;
use domain_types::branch_names::{BranchKind, qualified_branch_name, to_final_branch_name, to_stack_branch_name};
use domain_types::stack::BlockReason;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
      branch_prefix: self.branch_prefix.clone(),
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
    let (remote_statuses, moved) = match outcome {
      PushBranchesOutcome::Pushed { remote_statuses } => (remote_statuses, false),
      PushBranchesOutcome::RemoteMoved { remote_statuses, .. } => (remote_statuses, true),
    };
    let remote_status = remote_statuses.into_iter().next().context("No remote status of the pushed branch")?;
    Ok(if moved { Self::RemoteMoved { remote_status } } else { Self::Pushed { remote_status } })
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
  pub branch_prefix: String,
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
  /// Every branch is at its pushed commit on the remote; their remote statuses, in the order of the branches
  #[serde(rename_all = "camelCase")]
  Pushed { remote_statuses: Vec<RemoteStatusUpdate> },
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

/// Force-pushes the virtual branches to `origin` in one atomic `git push`, each leased on the remote head the user saw
/// (`push_lease`): the stack goes whole or not at all. The tips are read once (`prepare_push_expecting`), a branch pushed
/// up to a commit is checked (`check_push_tip`), and a remote branch that moved meanwhile is fetched, not overwritten:
/// the outcome names it, with the remote statuses after the fetch. A branch the last sync did not build (`not_built`)
/// refuses the whole push (`not_built_refusal`). `on_progress` gets how far the push is.
#[instrument(skip(git_executor, not_built, on_progress), fields(repository_path = %params.repository_path, branches = params.branches.len()))]
pub fn push_branches(
  git_executor: &GitCommandExecutor,
  params: &PushBranchesParams,
  not_built: &HashMap<String, BlockReason>,
  on_progress: &mut dyn FnMut(TransferProgress),
) -> Result<PushBranchesOutcome> {
  if let Some(refusal) = not_built_refusal(not_built, &params.branches) {
    bail!(refusal);
  }
  let repository_path = &params.repository_path;
  let names = params
    .branches
    .iter()
    .map(|branch| {
      if branch.whole_stack {
        return Ok((to_stack_branch_name(&params.branch_prefix, &branch.branch_name)?, None));
      }
      if branch.unapplied {
        return Ok((qualified_branch_name(&params.branch_prefix, BranchKind::Unapplied, &branch.branch_name)?, None));
      }
      let final_name = to_final_branch_name(&params.branch_prefix, &branch.branch_name)?;
      let parent_ref = branch
        .parent_branch
        .as_deref()
        .map(|parent| to_final_branch_name(&params.branch_prefix, parent))
        .transpose()?;
      Ok((final_name, parent_ref))
    })
    .collect::<Result<Vec<_>>>()?;
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
  let target = PushTarget {
    repository_path,
    remote: "origin",
    baseline: &params.baseline_branch,
    my_email: params.my_email.as_deref(),
  };
  // one ref needs no `--atomic`, which a remote may not support
  let options = PushOptions {
    atomic: branches.len() > 1,
    dry_run: false,
  };
  match push_leased(git_executor, &target, &branches, options, Some(on_progress))? {
    LeasedPush::Pushed(_) => Ok(PushBranchesOutcome::Pushed {
      remote_statuses: leased_statuses(git_executor, &target, &branches).context("Failed to compute remote status")?,
    }),
    LeasedPush::RemoteMoved { moved, remote_statuses } => Ok(PushBranchesOutcome::RemoteMoved {
      branch_name: branches[moved[0]].name.clone(),
      whole_stack: params.branches[moved[0]].whole_stack,
      remote_statuses,
    }),
    LeasedPush::Refused { summary } => bail!("The remote rejected the push: {summary}"),
    LeasedPush::WouldReject => bail!("The remote would reject the push"),
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
  /// Every lease held, yet the remote refused the push (a hook, a protected branch): nothing was pushed. `summary` is
  /// what git said of each refused ref, for an error message
  Refused { summary: String },
  /// A dry run the remote would refuse: nothing is fetched on a dry run, so which lease failed is not known
  WouldReject,
}

/// The one leased push of the app and of `bd push`: one `git push --porcelain` of every branch to `target.remote`, each
/// with its lease (`push_refs`), `--atomic` for a stack, so the remote takes all of them or none. A refused push
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
  let refs: Vec<PushRef> = branches.iter().map(|branch| branch.push.clone()).collect();
  // the remote policy keeps the user's credential helpers but never prompts: the push must not block on input
  let reports = push_refs(git_executor, target.repository_path, target.remote, &refs, options, on_progress)?;
  let report_of = |branch: &LeasedBranch| reports.get(&format!("refs/heads/{}", branch.push.full_name));
  let refused: Vec<&LeasedBranch> = branches.iter().filter(|branch| report_of(branch).is_none_or(|report| !report.accepted())).collect();
  if refused.is_empty() {
    return Ok(LeasedPush::Pushed(reports));
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
    // the leases held: the remote refused the push for another reason, e.g. a hook or a protected branch
    let summary = refused
      .iter()
      .map(|branch| format!("{} {}", branch.push.full_name, report_of(branch).map_or("no report", |report| report.summary.as_str())))
      .collect::<Vec<_>>()
      .join("; ");
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
