//! `bd push`: sync, then push the virtual branches in one atomic `git push --porcelain` with an explicit lease per ref,
//! parents first, and print a ready `gh pr create` per branch, into the baseline: every pull request of a virtual
//! branch goes there, a stacked branch's with the commits of the branches it is built on. A stacked branch takes those
//! branches along when they have commits for the remote, as the app's push button does, so their remote branches stay
//! what it is built on; `--alone` pushes it without them. After the push, an open pull request of a pushed branch whose
//! base is a virtual branch is retargeted to the baseline (`pull_request_retarget`).
//!
//! The lease is the one the app's push button uses too (`sync_core::push_lease`): the remote head the sync in the same
//! process compared with (`remote.head`), or the one `--expect-remote` names. The push replaces exactly that remote
//! state; the refusal of a branch behind its remote without `--force` decides whether it may drop commits (an older
//! version of a commit amended or restacked here is replaced, not dropped: `remote.amended`, `remote.restacked`). A rejected
//! push fetches the branches and reports what the remote has now. With `--remote` other than `origin`, which the sync
//! compares with, the remote status of the pushed branches is computed the same way against that remote.

use crate::context::{Environment, RepoContext};
use crate::error::{CliError, ErrorBranch, ErrorCode};
use crate::output::{SchemaVersion, ShortIds};
use crate::pull_request_retarget::{RetargetedPullRequest, retarget_pull_requests_of};
use crate::push_hold::{PushHolds, read_push_holds};
use crate::safe_push_command::github_service;
use crate::sync_command::{RemoteSummary, SyncOutput, SyncState, SyncedBranch, lock, sync_locked};
use git_ops::branch_description::Description;
use git_ops::branch_names::to_final_branch_name;
use git_ops::revs::{ref_tips, resolve_commit, tips_under};
use schemars::JsonSchema;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use sync_core::pull_requests::{GitHubRepository, parse_github_repository, resolve_github_repository_with};
use sync_core::push::{LeasedBranch, LeasedPush, PushTarget, push_leased};
use sync_core::push_lease::{LeaseSource, PushOptions, PushRef, PushedRefStatus, leased_push_refs};
use sync_core::remote_status::{AncestorCommits, RemoteStatusMemo, StackParent};
use sync_core::stack_pull_request::{StackMemberText, stack_pull_request_text};
use sync_core::sync::SyncReport;
use sync_types::RemoteStatusUpdate;
use sync_types::patronus::PatronusRobot;
use sync_types::stack::{StackBranchState, StackBranchUpdate};

#[derive(Debug, Default, clap::Args)]
#[expect(clippy::struct_excessive_bools, reason = "command-line flags")]
pub struct PushArgs {
  /// The branches to push, with the branches they are built on that have commits for the remote
  pub branches: Vec<String>,
  /// Push every branch that syncs cleanly; needs --yes
  #[arg(long, conflicts_with = "branches")]
  pub all: bool,
  /// Confirm --all
  #[arg(long, requires = "all")]
  pub yes: bool,
  /// Push the branches stacked on the named ones too, directly or further down
  #[arg(long, requires = "branches", conflicts_with = "expect_remote")]
  pub with_children: bool,
  /// Push only the named branches, not the branches they are built on: a branch carries their commits, its pull request
  /// into the baseline includes them either way
  #[arg(long, requires = "branches")]
  pub alone: bool,
  /// Push branches the remote has commits for that they don't (behind > 0): the push drops the commits the sync
  /// counted, nothing pushed since. Older versions of commits amended or restacked here need no --force
  #[arg(long)]
  pub force: bool,
  /// Lease on this remote commit: the push replaces the remote branch only if it is still there. Pass the
  /// `remote.head` of the status the user saw, e.g. of a `lease_rejected` error. Needs exactly one branch
  #[arg(long, value_name = "COMMIT", conflicts_with = "all")]
  pub expect_remote: Option<String>,
  /// The remote; default: the remote of the baseline (`origin` for `origin/master`)
  #[arg(long)]
  pub remote: Option<String>,
  /// Ask the remote, change nothing (`git push --dry-run`)
  #[arg(long)]
  pub dry_run: bool,
  /// Push a branch whose Safe Push runs: cancel the Safe Push, push, and restart it on the pushed commit. Without it
  /// such a branch is refused (`robot_running`): a push won't land, and it fails the Safe Push at the end
  #[arg(long)]
  pub restart_running: bool,
}

/// `bd push`
#[derive(Debug, Serialize, JsonSchema)]
pub struct PushOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub remote: String,
  pub dry_run: bool,
  /// Parents first
  pub branches: Vec<PushedBranch>,
  /// The Safe Pushes `--restart-running` canceled before the push and restarted on the pushed commit
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub restarted: Vec<RestartedRobot>,
  /// The open pull requests of the pushed branches whose base was a virtual branch, retargeted to the baseline after the
  /// push (with `--dry-run`: the ones a push would retarget). Name them to the user, and the `approved_by` of each
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub retargeted: Vec<RetargetedPullRequest>,
  /// A retarget GitHub refused after the push, with the `gh pr edit` that does it, or why GitHub couldn't list the pull
  /// requests
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub retarget_warnings: Vec<String>,
  /// With `--with-children` of the root of a stack that forks: its stack branch (`{prefix}/stack/<root>`, all its commits
  /// in one branch, `name` the root), in the same atomic push, when the sync built it and the remote lacks it; its pull
  /// request is the one of the whole stack
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub stacks: Vec<PushedBranch>,
  /// A stack branch `--with-children` left out: not built by this sync (`stacks` of `bd sync` says why), or pushed to
  /// another remote than `origin`, which its remote status compares with
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub stack_warnings: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RestartedRobot {
  pub branch: String,
  /// The page of the restarted robot; open it for the user
  pub url: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PushedBranch {
  pub name: String,
  /// The branch on the remote, the same name as the local virtual branch
  pub remote_branch: String,
  pub commit: String,
  /// `new`, `updated`, `forced` or `up_to_date`
  pub status: String,
  /// The commit the remote branch had to be at; `null` when it had not to exist
  pub lease: Option<String>,
  /// `expected` (the remote head of the status: the sync's, or --expect-remote) or `absent` (no remote branch: it must
  /// not exist)
  pub lease_source: String,
  pub pull_request: PullRequestDraft,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PullRequestDraft {
  /// The remote branch the pull request merges into: the baseline without the remote (`master`), for every branch; a
  /// stacked branch's includes the commits of the branches it is built on
  pub base: String,
  pub head: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  /// Pipe it into the command (`--body-file -`)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub body: Option<String>,
  /// A ready `gh pr create`
  pub command: String,
}

/// A branch to push, with what decides its lease
struct Candidate<'a> {
  synced: &'a SyncedBranch,
  full_name: String,
  parent: Option<String>,
  /// The status against the remote pushed to; `None` without one
  remote: Option<RemoteSummary>,
}

pub fn push(context: &RepoContext, args: &PushArgs, environment: &Environment) -> Result<PushOutput, CliError> {
  if args.branches.is_empty() && !args.all {
    return Err(CliError::new(ErrorCode::InvalidArguments, "Name the branches to push, or pass --all --yes"));
  }
  if args.all && !args.yes {
    return Err(CliError::new(ErrorCode::InvalidArguments, "--all pushes every branch: confirm with --all --yes"));
  }
  if args.expect_remote.is_some() && args.branches.len() != 1 {
    return Err(CliError::new(
      ErrorCode::InvalidArguments,
      "--expect-remote is the remote head of one branch: name exactly one",
    ));
  }

  let repository_lock = lock(context)?;
  let synced = sync_locked(context)?;
  let holds = read_push_holds(context, environment, &synced);
  let mut output = push_locked(context, args, &synced, &holds)?;
  drop(repository_lock);

  // GitHub after the lock: the pull requests of the pushed branches go into the baseline
  if let Some(repository) = github_repository(context, &synced.report)?
    && !output.branches.is_empty()
  {
    let names: Vec<&str> = output.branches.iter().map(|branch| branch.name.as_str()).collect();
    let base = base_on_remote(&synced.baseline, &output.remote);
    let github = github_service(context, environment);
    let retargets = retarget_pull_requests_of(context, &github, context.runtime()?, &repository, (&names, base), args.dry_run);
    output.retargeted = retargets.retargeted;
    output.retarget_warnings = retargets.warnings;
  }
  Ok(output)
}

/// The baseline as `remote` names it: `master` of `origin/master`
fn base_on_remote<'a>(baseline: &'a str, remote: &str) -> &'a str {
  baseline.strip_prefix(remote).and_then(|rest| rest.strip_prefix('/')).unwrap_or(baseline)
}

/// `bd push` for a caller that holds the repository lock and synced under it (`bd safe-push`, which cancels the robot
/// it replaces itself and passes no `holds`). A branch a running Safe Push holds (`holds`) is refused, or, with
/// `--restart-running` and the robot on the branch itself, its robot is canceled before the push and restarted after it;
/// an ancestor taken along that a robot holds stays as it is, unless that robot is the one restarted.
pub fn push_locked(context: &RepoContext, args: &PushArgs, synced: &SyncOutput, holds: &PushHolds) -> Result<PushOutput, CliError> {
  let report = &synced.report;
  let remote = match &args.remote {
    Some(remote) => remote.clone(),
    None => synced
      .baseline
      .split_once('/')
      .map(|(remote, _)| remote.to_string())
      .ok_or_else(|| CliError::new(ErrorCode::NoRemote, format!("The baseline {} is local: pass --remote", synced.baseline)))?,
  };
  // a URL has no remote-tracking refs to compare with
  if !report.remotes.contains(&remote) {
    return Err(CliError::new(ErrorCode::NoRemote, format!("No remote named {remote:?}: `git remote` lists them")));
  }
  // the base of every pull request
  let base_branch = base_on_remote(&synced.baseline, &remote).to_string();

  // the stack as the sync placed it, parents first
  let placement = |name: &str| synced.placement(name);
  let parent_of = |name: &str| placement(name).and_then(|stack| stack.effective_base.clone());
  let order = synced.order();

  // the ancestors of the named branches that go along; one with nothing for the remote is left out
  let mut implicit: HashSet<String> = HashSet::new();
  let selected: Vec<&str> = if args.all {
    order.clone()
  } else {
    for name in &args.branches {
      if !synced.branches.iter().any(|branch| &branch.name == name) {
        return Err(CliError::new(ErrorCode::UnknownBranch, format!("No branch named {name:?}")));
      }
    }
    // the branches the named ones are built on go along, unless --alone; --expect-remote is the lease of one branch
    if !args.alone && args.expect_remote.is_none() {
      for name in &args.branches {
        let mut parent = parent_of(name);
        while let Some(ancestor) = parent {
          if !implicit.insert(ancestor.clone()) {
            break;
          }
          parent = parent_of(&ancestor);
        }
      }
      implicit.retain(|name| !args.branches.contains(name));
    }
    let mut selected: Vec<&str> = Vec::new();
    for name in &order {
      // a blocked child has no effective base: its declared parent places it, and the push refuses it as not synced
      let parent = placement(name).and_then(|stack| stack.effective_base.as_deref().or(stack.declared_parent.as_deref()));
      // the order is parents first: a child comes after its parent was selected
      if args.branches.iter().any(|named| named == name)
        || implicit.contains(*name)
        // a branch taken along brings no children: only the named ones do
        || (args.with_children && parent.is_some_and(|parent| selected.contains(&parent) && !implicit.contains(parent)))
      {
        selected.push(name);
      }
    }
    selected
  };

  // the robots the push restarts: of the named branches with --restart-running; the ancestors they land go along
  let restarted: Vec<&PatronusRobot> = if args.restart_running {
    selected
      .iter()
      .filter(|name| !implicit.contains(**name))
      .filter_map(|name| holds.get(name).filter(|hold| hold.is_own(name)).map(|hold| &hold.robot))
      .collect()
  } else {
    Vec::new()
  };
  let on_remote = remote_tips(context, report, &remote)?;
  // the statuses against another remote, computed here on the commits the sync left: `origin`'s are the sync's
  let statuses = (remote != "origin").then(|| {
    RemoteStatusMemo::new(
      context.git.clone(),
      &context.repository,
      context.cache(),
      &remote,
      report.baseline_commit.clone(),
      on_remote.as_ref().clone(),
    )
  });
  let mut candidates = Vec::with_capacity(selected.len());
  let mut refusals = Vec::new();
  let mut held = Vec::new();
  let mut behind = Vec::new();
  for name in &selected {
    let Some(branch) = synced.branches.iter().find(|branch| branch.name == *name) else {
      continue;
    };
    if let Some(hold) = holds.get(name)
      && !restarted.iter().any(|robot| robot.id == hold.robot.id)
    {
      // a branch taken along that a Safe Push lands is on the remote as the robot took it: the push of the stack starts
      // above it, as the app's does, and a child carries its commits
      if !implicit.contains(*name) {
        held.push(hold.reason(name));
      }
      continue;
    }
    if let Some(reason) = not_synced_reason(branch) {
      refusals.push(format!("{name} is not synced ({reason})"));
      continue;
    }
    let full_name = final_name(context, name)?;
    let Some(tip) = report.tips.get(&full_name) else {
      refusals.push(format!("{name} has no virtual branch"));
      continue;
    };
    let parent = parent_of(name);
    let parent_ref = parent.as_deref().map(|parent| final_name(context, parent)).transpose()?;
    let remote_state = remote_status(context, statuses.as_ref(), report, branch, (&full_name, tip), parent_ref.as_deref())?;
    let along = implicit.contains(*name);
    if along
      && remote_state
        .as_ref()
        .is_some_and(|remote| remote.exists && remote.unpushed == 0 && remote.behind == 0 && remote.amended == 0 && remote.restacked == 0)
    {
      // on the remote as it is: nothing to take along
      continue;
    }
    if let Some(remote_state) = &remote_state
      && remote_state.behind > 0
      && !args.force
    {
      let alone = if along { ", or --alone to push the named branches without it" } else { "" };
      refusals.push(format!(
        "{name}: the remote branch has {} commits the branch doesn't; pass --force to replace them{alone}",
        remote_state.behind
      ));
      behind.push(ErrorBranch {
        name: (*name).to_string(),
        remote: remote_state.clone(),
      });
    }
    candidates.push(Candidate {
      synced: branch,
      full_name,
      parent,
      remote: remote_state,
    });
  }
  // the whole stack of a named root goes along with its children: its stack branch, when this sync built it, never a
  // stale one
  let (stacks, stack_warnings) = if args.with_children {
    stack_branches_to_push(report, &args.branches, &remote)
  } else {
    (Vec::new(), Vec::new())
  };
  for stack in &stacks {
    if let Some(remote_state) = &stack.remote
      && remote_state.commits_behind > 0
      && !args.force
    {
      refusals.push(format!(
        "{}: the remote branch has {} commits the stack branch doesn't; pass --force to replace them",
        stack.branch_name, remote_state.commits_behind
      ));
      let short = context.short_ids(RemoteSummary::ids(remote_state));
      behind.push(ErrorBranch {
        name: stack.branch_name.clone(),
        remote: RemoteSummary::new(remote_state, usize::try_from(stack.commit_count).unwrap_or(usize::MAX), &short),
      });
    }
  }
  if !held.is_empty() {
    return Err(CliError::new(ErrorCode::RobotRunning, format!("Nothing was pushed: {}", held.join("; "))));
  }
  if !refusals.is_empty() {
    let mut error = CliError::new(ErrorCode::PushRefused, format!("Nothing was pushed: {}", refusals.join("; ")));
    error.branches = behind;
    return Err(error);
  }
  if !args.dry_run {
    for robot in &restarted {
      holds.cancel(context, robot)?;
    }
  }

  let expected: Vec<(String, Option<String>)> = match &args.expect_remote {
    Some(expected) => vec![(candidates[0].full_name.clone(), Some(resolve_expected(context, expected)?))],
    // the remote head the status compared with: a remote commit the branch has by content goes, and --force drops what
    // the status counted, but nothing pushed since
    None => candidates
      .iter()
      .map(|candidate| (candidate.full_name.clone(), candidate.remote.as_ref().and_then(|remote| remote.head_id.clone())))
      .collect(),
  };
  // the tips the sync left, under the lock: nothing moved them since
  let refs = leased_push_refs(&report.tips, &expected)?;
  let mut leased: Vec<LeasedBranch> = candidates
    .iter()
    .zip(refs)
    .map(|(candidate, push)| {
      Ok(LeasedBranch {
        name: candidate.synced.name.clone(),
        push,
        parent_ref: candidate.parent.as_deref().map(|parent| final_name(context, parent)).transpose()?,
      })
    })
    .collect::<Result<_, CliError>>()?;
  // after the branches: the stack branch of each whole stack, leased on the remote head its status compared with
  leased.extend(stacks.iter().filter_map(|stack| {
    let lease = stack.remote.as_ref().and_then(|remote| remote.remote_tip.clone());
    Some(LeasedBranch {
      name: stack.root.clone(),
      push: PushRef {
        full_name: stack.branch_name.clone(),
        tip: stack.tip.clone()?,
        source: if lease.is_some() { LeaseSource::Expected } else { LeaseSource::Absent },
        lease,
      },
      parent_ref: None,
    })
  }));
  // what a refused push names: the branches, then the stack branches
  let pushed_names: Vec<(String, usize)> = candidates
    .iter()
    .map(|candidate| (candidate.synced.name.clone(), candidate.synced.commit_count))
    .chain(
      stacks
        .iter()
        .map(|stack| (stack.branch_name.clone(), usize::try_from(stack.commit_count).unwrap_or(usize::MAX))),
    )
    .collect();
  // the one leased push of the app's push too: atomic, a stack goes whole or not at all
  let target = PushTarget {
    repository_path: &context.repository,
    remote: &remote,
    baseline: &synced.baseline,
    my_email: None,
  };
  let options = PushOptions {
    atomic: true,
    dry_run: args.dry_run,
  };
  let pushed = push_leased(&context.git, &target, &leased, options, None).map_err(|e| CliError::new(ErrorCode::NetworkError, format!("{e:#}")))?;
  let statuses = match pushed {
    LeasedPush::Pushed(statuses) => statuses,
    LeasedPush::RemoteMoved { moved, remote_statuses } => return Err(lease_rejected(context, &pushed_names, &moved, &remote_statuses)),
    LeasedPush::Refused { summary } => {
      return Err(CliError::new(
        ErrorCode::PushRefused,
        format!("Nothing was pushed: the remote refused the push ({summary})"),
      ));
    }
    LeasedPush::WouldReject => {
      return Err(CliError::new(
        ErrorCode::LeaseRejected,
        format!(
          "The remote would reject {}: a remote branch is not where its lease expects it",
          pushed_names.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>().join(", ")
        ),
      ));
    }
  };
  let refs: Vec<&PushRef> = leased.iter().map(|branch| &branch.push).collect();

  let descriptions = report.config.descriptions();
  let repository_option = github_repository_option(context, report)?;
  let short = context.short_ids(refs.iter().flat_map(|pushed| std::iter::once(pushed.tip.as_str()).chain(pushed.lease.as_deref())));
  let branches = candidates
    .iter()
    .zip(&refs)
    .map(|(candidate, &pushed)| {
      let draft = pull_request_draft(descriptions, &repository_option, base_branch.clone(), &candidate.full_name);
      pushed_branch(&candidate.synced.name, pushed, &statuses, draft, &short)
    })
    .collect();
  let stacks = stacks
    .iter()
    .zip(&refs[candidates.len()..])
    .map(|(stack, &pushed)| {
      let draft = stack_pull_request_draft(context, report, base_branch.clone(), stack)?;
      Ok(pushed_branch(&stack.root, pushed, &statuses, draft, &short))
    })
    .collect::<Result<_, CliError>>()?;

  // the restart takes the commit just pushed; a rejected push above left the robots canceled
  let mut restarted_robots = Vec::new();
  if !args.dry_run {
    for robot in &restarted {
      let robot = holds.restart(context, robot)?;
      restarted_robots.push(RestartedRobot {
        branch: robot.branch_name.clone().unwrap_or_default(),
        url: robot.url,
      });
    }
  }
  Ok(PushOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    remote,
    dry_run: args.dry_run,
    branches,
    restarted: restarted_robots,
    retargeted: Vec::new(),
    retarget_warnings: Vec::new(),
    stacks,
    stack_warnings,
  })
}

/// The stack branches of the stacks that fork whose root `--with-children` names, in the order of the sync: the
/// `built` ones the remote lacks (by content, as the sync's remote status compares), and a warning per one left out. A
/// stack branch in another state is not the stack's commits, and its remote status is of `origin` only
fn stack_branches_to_push<'a>(report: &'a SyncReport, named: &[String], remote: &str) -> (Vec<&'a StackBranchUpdate>, Vec<String>) {
  let mut stacks = Vec::new();
  let mut warnings = Vec::new();
  for stack in report.stack_branches.iter().filter(|stack| !stack.members.is_empty() && named.contains(&stack.root)) {
    if stack.state != StackBranchState::Built || stack.tip.is_none() {
      warnings.push(format!(
        "{} was not pushed: this sync didn't build it ({}; `stacks` of `bd sync` says why)",
        stack.branch_name,
        state_word(stack.state)
      ));
    } else if remote != "origin" {
      warnings.push(format!("{} was not pushed: its remote status is of origin, not of {remote}", stack.branch_name));
    } else if stack.remote.as_ref().is_none_or(has_work_for_remote) {
      stacks.push(stack);
    }
  }
  (stacks, warnings)
}

/// `blocked`, `conflict`, `stale`, as `stacks` names a state
const fn state_word(state: StackBranchState) -> &'static str {
  match state {
    StackBranchState::Built => "built",
    StackBranchState::Blocked => "blocked",
    StackBranchState::Conflict => "conflict",
    StackBranchState::Stale => "stale",
  }
}

/// A push of the branch would change its remote branch
pub(crate) const fn has_work_for_remote(remote: &RemoteStatusUpdate) -> bool {
  !remote.remote_exists || !remote.unpushed_commits.is_empty() || remote.commits_behind > 0 || remote.commits_amended > 0 || remote.commits_restacked > 0
}

/// The output of a pushed ref named `name`: its commit, what the remote did with it (`statuses` of the push, by remote
/// ref), its lease and its pull request
pub(crate) fn pushed_branch(name: &str, pushed: &PushRef, statuses: &HashMap<String, PushedRefStatus>, pull_request: PullRequestDraft, short: &ShortIds) -> PushedBranch {
  let flag = statuses.get(&format!("refs/heads/{}", pushed.full_name)).map_or(' ', |status| status.flag);
  PushedBranch {
    name: name.to_string(),
    remote_branch: pushed.full_name.clone(),
    commit: short.get(&pushed.tip).to_string(),
    status: match flag {
      '*' => "new",
      '+' => "forced",
      '=' => "up_to_date",
      _ => "updated",
    }
    .to_string(),
    lease: pushed.lease.as_deref().map(|lease| short.get(lease).to_string()),
    lease_source: pushed.source.as_str().to_string(),
    pull_request,
  }
}

/// The pull request of the whole stack: of its stack branch into `base` (the baseline without the remote), with the
/// text of the whole stack (`stack_pull_request_text`: the stack branch's description, else its members' descriptions,
/// else the subjects of their commits), as the printed `gh pr create` opens it. From what the sync read: no git process
pub(crate) fn stack_pull_request_draft(context: &RepoContext, report: &SyncReport, base: String, stack: &StackBranchUpdate) -> Result<PullRequestDraft, CliError> {
  let descriptions = report.config.descriptions();
  let described = |full_name: &str| descriptions.get(&format!("refs/heads/{full_name}"));
  let members: Vec<StackMemberText<'_>> = stack
    .members
    .iter()
    .map(|name| {
      let branch = report.branches.iter().find(|branch| &branch.name == name);
      let placement = branch.and_then(|branch| branch.stack.as_ref());
      StackMemberText {
        name,
        parent: placement.and_then(|stack| stack.effective_base.as_deref().or_else(|| stack.blocked_by.as_ref().map(|block| block.parent.as_str()))),
        description: final_name(context, name).ok().and_then(|full_name| described(&full_name)),
        // newest first in the grouping
        subjects: branch.map_or_default(|branch| branch.commits.iter().rev().map(|commit| commit.stripped_subject.as_str()).collect()),
      }
    })
    .collect();
  let (title, body) = stack_pull_request_text(described(&stack.branch_name), &members, &base);
  let repository_option = github_repository_option(context, report)?;
  let command = format!(
    "gh pr create{repository_option} --base {} --head {} --assignee @me --title {} --body-file -",
    shell_quote(&base),
    shell_quote(&stack.branch_name),
    shell_quote(&title)
  );
  Ok(PullRequestDraft {
    base,
    head: stack.branch_name.clone(),
    title: Some(title),
    body: Some(body).filter(|body| !body.is_empty()),
    command,
  })
}

/// The pull request of the remote branch `head` into `base` (the baseline without the remote), as the printed
/// `gh pr create` opens it: with the `bd describe` text, else filled from the commits (a stacked branch's with the
/// commits of the branches it is built on)
fn pull_request_draft(descriptions: &HashMap<String, Description>, repository_option: &str, base: String, head: &str) -> PullRequestDraft {
  let description = descriptions.get(&format!("refs/heads/{head}"));
  let command = match description {
    Some(description) => format!(
      "gh pr create{repository_option} --base {} --head {} --assignee @me --title {} --body-file -",
      shell_quote(&base),
      shell_quote(head),
      shell_quote(&description.title)
    ),
    None => format!(
      "gh pr create{repository_option} --base {} --head {} --assignee @me --fill",
      shell_quote(&base),
      shell_quote(head)
    ),
  };
  PullRequestDraft {
    base,
    head: head.to_string(),
    title: description.map(|description| description.title.clone()),
    body: description.map(|description| description.body.clone()).filter(|body| !body.is_empty()),
    command,
  }
}

/// The remote and the pull request draft of a branch whose remote branch is up to date by content, which `bd safe-push`
/// doesn't push: what `push_locked` returns for a pushed one
pub(crate) fn unpushed_pull_request_draft(context: &RepoContext, synced: &SyncOutput, name: &str) -> Result<(String, PullRequestDraft), CliError> {
  let (remote, base_branch) = synced
    .baseline
    .split_once('/')
    .ok_or_else(|| CliError::new(ErrorCode::NoRemote, format!("The baseline {} is local: pass --remote", synced.baseline)))?;
  let report = &synced.report;
  let draft = pull_request_draft(
    report.config.descriptions(),
    &github_repository_option(context, report)?,
    base_branch.to_string(),
    &final_name(context, name)?,
  );
  Ok((remote.to_string(), draft))
}

/// Why the sync didn't build the branch (its error, its conflict, or blocked); None when it did
pub fn not_synced_reason(branch: &SyncedBranch) -> Option<String> {
  if matches!(branch.status, SyncState::Created | SyncState::Updated | SyncState::Unchanged) {
    return None;
  }
  let reason = branch
    .error
    .clone()
    .or_else(|| branch.conflict.as_ref().map(|conflict| format!("conflict in {}", conflict.files.join(", "))));
  Some(reason.unwrap_or_else(|| format!("{:?}", branch.status).to_lowercase()))
}

/// `<prefix>/virtual/<name>`
fn final_name(context: &RepoContext, name: &str) -> Result<String, CliError> {
  to_final_branch_name(&context.prefix, name).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))
}

/// Full name -> tip of the remote-tracking refs of `remote` under the prefix: the ones the sync compared with for
/// `origin`, else one `for-each-ref`
fn remote_tips<'a>(context: &RepoContext, report: &'a SyncReport, remote: &str) -> Result<Cow<'a, HashMap<String, String>>, CliError> {
  if remote == "origin" {
    return Ok(Cow::Borrowed(&report.remote_tips));
  }
  let namespace = format!("refs/remotes/{remote}/");
  let pattern = format!("{namespace}{}/", context.prefix.trim_end_matches('/'));
  Ok(Cow::Owned(tips_under(&ref_tips(&context.git, &context.repository, &[&pattern])?, &namespace)))
}

/// The status of `branch` (`(full name, tip)`): the one of the sync for `origin` (`statuses` is `None`), which the sync
/// compares with; for another remote computed now by `statuses` the same way, on the commits the sync left
fn remote_status(
  context: &RepoContext,
  statuses: Option<&RemoteStatusMemo>,
  report: &SyncReport,
  branch: &SyncedBranch,
  (full_name, tip): (&str, &str),
  parent_ref: Option<&str>,
) -> Result<Option<RemoteSummary>, CliError> {
  let Some(statuses) = statuses else {
    return Ok(branch.remote.clone());
  };
  // the parent as the sync left it
  let parent_base = parent_ref.map(|parent_ref| report.tips.get(parent_ref).map_or(parent_ref, String::as_str));
  let status = statuses.status(full_name, &branch.name, tip, None, parent_base.map(stack_parent))?;
  let short = context.short_ids(RemoteSummary::ids(&status));
  Ok(Some(RemoteSummary::new(&status, branch.commit_count, &short)))
}

/// The parent of a stacked branch for a remote status computed outside the sync: its ancestors' commits are read only
/// when a remote commit matches none of the branch's
const fn stack_parent(base: &str) -> StackParent<'_> {
  StackParent {
    base,
    ancestors: AncestorCommits::ReadWhenNeeded,
  }
}

/// The full id of the commit `--expect-remote` names
fn resolve_expected(context: &RepoContext, rev: &str) -> Result<String, CliError> {
  resolve_commit(&context.git, &context.repository, rev)?.ok_or_else(|| {
    CliError::new(
      ErrorCode::InvalidArguments,
      format!("--expect-remote {rev} is not a commit: pass the `remote.head` bd printed"),
    )
  })
}

/// `lease_rejected` for the branches at `moved` (of `pushed`, each with its commit count), whose remote branch is not
/// where its lease expects it: nothing was pushed. `push_leased` fetched the branches, and the error lists what each
/// moved one has now (`remote_statuses`, in the order of `pushed`), so the user can see what a push with
/// `--force --expect-remote` would drop
fn lease_rejected(context: &RepoContext, pushed: &[(String, usize)], moved: &[usize], remote_statuses: &[RemoteStatusUpdate]) -> CliError {
  let names = moved.iter().map(|index| pushed[*index].0.as_str()).collect::<Vec<_>>().join(", ");
  let short = context.short_ids(moved.iter().flat_map(|index| RemoteSummary::ids(&remote_statuses[*index])));
  let mut error = CliError::new(
    ErrorCode::LeaseRejected,
    format!(
      "The remote rejected {names}: it is not where the lease expects it. Nothing was pushed; bd fetched it, and \
       `branches` says what the remote has now (`dropped_commits`: the commits a push drops). Show them to the user; to \
       replace them, push again with --force --expect-remote <remote.head>"
    ),
  );
  error.branches = moved
    .iter()
    .map(|index| ErrorBranch {
      name: pushed[*index].0.clone(),
      remote: RemoteSummary::new(&remote_statuses[*index], pushed[*index].1, &short),
    })
    .collect();
  error
}

/// The GitHub repository of the branches (`resolve_github_repository`) from what the sync read and the push URL of
/// `origin`: a git process only for the Space mirror, once per command (`RepoContext::github_repository`)
pub(crate) fn github_repository(context: &RepoContext, report: &SyncReport) -> Result<Option<GitHubRepository>, CliError> {
  context.github_repository(|| {
    Ok(resolve_github_repository_with(
      &context.git,
      &context.repository,
      report.config.github_repository(),
      context.origin_url()?,
    )?)
  })
}

/// ` --repo owner/name` when the GitHub repository of the branches is not the one `origin` pushes to (a mirror:
/// `branchdeck.githubRepository` or the Space settings): `gh` finds no GitHub repository among the remotes; empty
/// otherwise
fn github_repository_option(context: &RepoContext, report: &SyncReport) -> Result<String, CliError> {
  let Some(repository) = github_repository(context, report)? else {
    return Ok(String::new());
  };
  if context.origin_url()?.and_then(parse_github_repository).as_ref() == Some(&repository) {
    return Ok(String::new());
  }
  Ok(format!(" --repo {}", shell_quote(&format!("{}/{}", repository.owner, repository.name))))
}

fn shell_quote(word: &str) -> String {
  if !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':' | '@')) {
    word.to_string()
  } else {
    format!("'{}'", word.replace('\'', r"'\''"))
  }
}
