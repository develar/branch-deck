//! `bd push`: sync, then push the virtual branches in one atomic `git push --porcelain` with an explicit lease per ref,
//! parents first, and print a ready `gh pr create` per branch, into the baseline: every pull request of a virtual
//! branch goes there, a stacked branch's with the commits of the branches it is built on. A stacked branch takes those
//! branches along when they have commits for the remote, as the app's push button does, so their remote branches stay
//! what it is built on; `--alone` pushes it without them. Before the push, an open pull request of a branch it pushes
//! whose base is a virtual branch is retargeted to the baseline (`pull_request_retarget`, D3a).
//!
//! The repository lock is held for the sync and the plan of the push (`plan_push`: which refs, their commits and leases
//! fixed on the tips the sync left), never across HTTP: the robots the holds are decided on and the pull requests of the
//! branches are asked before it, side by side (`read_remote`); the retargets, the cancel and the restart of
//! `--restart-running` and the network push run after it (`PlannedPush::send`, `operations::push`, the core the app's
//! push runs too).
//!
//! The lease is the one the app's push button uses too (`sync_core::push_lease`): the remote head the sync in the same
//! process compared with (`remote.head`), or the one `--expect-remote` names. The push replaces exactly that remote
//! state; the refusal of a branch behind its remote without `--force` decides whether it may drop commits (an older
//! version of a commit amended or restacked here is replaced, not dropped: `remote.amended`, `remote.restacked`). A rejected
//! push fetches the branches and reports what the remote has now. With `--remote` other than `origin`, which the sync
//! compares with, the remote status of the pushed branches is computed the same way against that remote.

use crate::landed::{LandedBranch, LandedBySummary};
use crate::pull_request_retarget::{RetargetedPullRequest, Retargets, retarget_listed};
use crate::push_hold::{PushHolds, push_holds, robot_command};
use crate::safe_push_command::youtrack_service;
use crate::session_facts::{SessionFacts, listed_robots};
use bd_core::bd_command::shell_word;
use bd_core::context::{Environment, RepoContext};
use bd_core::error::{CliError, ErrorBranch, NextStep};
use bd_core::output::{SchemaVersion, ShortIds};
use bd_core::services::{BdServices, github_service, patronus_service, session_key};
use bd_core::sync_command::{LandedRemoteCommitSummary, RemoteSummary, SyncOutput, SyncedBranch, landed_commits, lock, sync_locked};
use domain_policy::facts::Repository;
use domain_policy::parked::parked_state;
use domain_policy::push::{drops_remote_commits, stack_branch_along};
use domain_policy::stack::descendants;
use domain_types::branch_names::{BranchKind, qualified_branch_name, to_final_branch_name};
use domain_types::error::ErrorCode;
use domain_types::patronus::{PatronusRobot, PatronusRobotsResult};
use domain_types::pull_request::{PullRequestState, PullRequestsResult};
use domain_types::reason::Reason;
use domain_types::session::StackBranchAlong;
use domain_types::stack::{StackBranchState, StackBranchUpdate};
use domain_types::{GroupedBranchInfo, RemoteStatusUpdate};
use git_ops::revs::{ref_exists, ref_tips, resolve_commit, tips_under};
use github::service::GitHubService;
use history_rewrite::baseline::detect_baseline_branch;
use operations::push::{OverMoved, cancel_robots, restart_robots, send};
use patronus::service::PatronusService;
use remote_facts::fact_sources::FactSources;
use remote_facts::pull_request_text::{AncestorText, CommitText, IssueText, PullRequestTextInput, issue_texts, pull_request_text};
use remote_facts::pull_requests::list_pull_requests;
use remote_facts::stack_pull_request::{StackMemberText, stack_pull_request_text};
use repository_model::one_shot::OneShot;
use repository_model::push_guard::parked_words;
use schemars::JsonSchema;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use sync_core::config_snapshot::ConfigSnapshot;
use sync_core::github_repository::{GitHubRepository, parse_github_repository, resolve_github_repository_with};
use sync_core::issue_navigation::load_issue_navigation_config;
use sync_core::push::{LeasedBranch, LeasedPush, PreparedPush, PushTarget, not_built_reason};
use sync_core::push_lease::{LeaseSource, PushOptions, PushRef, PushedRefStatus, leased_push_refs};
use sync_core::remote_status::{AncestorCommits, RemoteStatusMemo, StackParent};
use sync_core::sync::SyncReport;
use youtrack::service::YouTrackService;

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
  /// Sync, then ask the remote what the push would do (`git push --dry-run`); push nothing
  #[arg(long)]
  pub dry_run: bool,
  /// Push a branch whose Safe Push runs: cancel the Safe Push, push, and restart it on the pushed commit. Without it
  /// such a branch is refused (`robot_running`): a push won't land, and it fails the Safe Push at the end
  #[arg(long)]
  pub restart_running: bool,
  /// Push the unapplied branch `{prefix}/unapplied/<name>` (one name, as `unapplied` of `bd status` names it) to
  /// `origin` under its own name, leased on the remote head the last fetch saw or --expect-remote: the remote keeps a
  /// copy. It lands nothing, and no pull request is drafted
  #[arg(long, requires = "branches", conflicts_with_all = ["all", "with_children", "alone", "force", "remote", "dry_run", "restart_running"])]
  pub unapplied: bool,
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
  /// The open pull requests of the pushed branches whose base was a virtual branch, retargeted to the baseline before the
  /// push (with `--dry-run`: the ones a push would retarget). Name them to the user, and the `approved_by` of each
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub retargeted: Vec<RetargetedPullRequest>,
  /// A retarget GitHub refused before the push, with the `gh pr edit` that does it, or why GitHub couldn't list the pull
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
  /// The branches asked for (with `--all`, or built on a named one with `--with-children`; a merged Safe Push landed: a
  /// named one too) that the push left out, the others pushed: a merged Safe Push landed all of each, nothing of it is
  /// left to push (`landed`; tell the user who landed it), or the user parked it or a branch it is built on (`parked`)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub skipped: Vec<SkippedBranch>,
}

/// A branch `bd push` left out (`skipped`)
#[derive(Debug, Serialize, JsonSchema)]
pub struct SkippedBranch {
  pub name: String,
  /// Why: `landed` or `parked`, the code a push with nothing else left is refused with
  pub code: ErrorCode,
  /// "auth landed in master with the Safe Push of login (<robot>): nothing of it is left to push", "auth is parked: …"
  pub message: String,
  /// The merged Safe Push that landed it, as `landed_by` of `bd status`; omitted for a parked branch
  #[serde(skip_serializing_if = "Option::is_none")]
  pub landed_by: Option<LandedBySummary>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RestartedRobot {
  pub branch: String,
  /// The page of the restarted robot; open it for the user
  pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PushState {
  New,
  Updated,
  Forced,
  UpToDate,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PushedBranch {
  pub name: String,
  /// The branch on the remote, the same name as the local virtual branch
  pub remote_branch: String,
  pub commit: String,
  /// What the remote did with it
  pub state: PushState,
  /// The commit the remote branch had to be at; `null` when it had not to exist
  pub lease: Option<String>,
  /// `expected` (the remote head of the status: the sync's, or --expect-remote) or `absent` (no remote branch: it must
  /// not exist)
  pub lease_source: String,
  /// The remote commits the push replaced (with `--dry-run`: replaces) that landed in the baseline (`remote.landed` of
  /// `bd sync`), each with the baseline commit that has it: no --force was needed, nothing of them is lost
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub replaced_landed: Vec<LandedRemoteCommitSummary>,
  /// The remote branch had moved since the sync, without commits the branch lacks: bd fetched it and pushed again, once,
  /// leased on the head it had then (`lease`; `null`: it was deleted), which the push replaced. Tell the user; omitted
  /// when the first push went
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub pushed_over: bool,
  /// Omitted for an unapplied branch (`--unapplied`), which lands nothing
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pull_request: Option<PullRequestDraft>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PullRequestDraft {
  /// The remote branch the pull request merges into: the baseline without the remote (`master`), for every branch; a
  /// stacked branch's includes the commits of the branches it is built on
  pub base: String,
  pub head: String,
  /// The `bd describe` title, else the title of its commits (`pull_request_text`)
  pub title: String,
  /// Pipe it into the command (`--body-file -`)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub body: Option<String>,
  /// A ready `gh pr create`
  pub command: String,
}

/// A branch to push, with what decides its lease
pub(crate) struct Candidate<'a> {
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

  if args.unapplied {
    return push_unapplied(context, args);
  }

  // what the push decides on of Patronus and GitHub is asked first: the lock holds the sync, the plan and the freeze,
  // never a request
  let github = Arc::new(github_service(context, environment));
  let remote = read_remote(context, environment, &github)?;
  let repository_lock = lock(context)?;
  let synced = sync_locked(context)?;
  let holds = holds_of(context, &synced, remote.robots);
  // the stack branch of a whole stack goes along once it is on the remote or its pull request is open
  let open_stack_pull_requests = match &remote.pull_requests {
    Some(listed) if args.with_children && stack_branches_off_the_remote(&synced.report, &args.branches) => open_stack_pull_requests(listed),
    _ => HashSet::new(),
  };
  let planned = plan_push(context, args, &synced, &holds, &open_stack_pull_requests)?;
  // the commits and the leases are fixed: GitHub, Patronus and the network push hold no lock
  drop(repository_lock);

  // the pull requests of the branches it pushes go into the baseline first (D3a), so GitHub never compares the pushed
  // commits with a parent's branch; one GitHub refuses is a warning, and a refused push leaves the retargets done
  let retargets = match (github_repository(context, &synced.report)?, &remote.pull_requests) {
    (Some(repository), Some(listed)) if !planned.candidates.is_empty() => {
      let names: Vec<&str> = planned.candidates.iter().map(|candidate| candidate.synced.name.as_str()).collect();
      let base = base_on_remote(&synced.baseline, &planned.remote);
      retarget_listed(context, &github, context.runtime()?, &repository, listed, (&names, base), args.dry_run)
    }
    _ => Retargets::default(),
  };
  let youtrack = youtrack_service(environment);
  let mut output = planned.send(context, args, &synced, holds.service(), Some(&youtrack))?;
  output.retargeted = retargets.retargeted;
  output.retarget_warnings = retargets.warnings;
  Ok(output)
}

/// What `bd push` asks the remote services before its lock, side by side: the user's robots, read as the app's session
/// reads them (one list, the needs of a push: `operations::needs`), and the pull requests of the branches (one search),
/// which the retargets read, and `--with-children` for the stack branch of a whole stack
struct RemoteReads {
  /// `None` off Space or without a Space token
  robots: Option<ListedRobots>,
  /// `None` without a GitHub repository of the branches
  pull_requests: Option<PullRequestsResult>,
}

/// The robots of the list, and the service and the reads they came through
struct ListedRobots {
  service: Arc<PatronusService>,
  sources: Arc<dyn FactSources>,
  result: PatronusRobotsResult,
}

/// The reads of `RemoteReads`, before the lock and the sync: the GitHub repository and the Space repository from the
/// config and the push URL of `origin`, which the sync reads too
fn read_remote(context: &RepoContext, environment: &Environment, github: &Arc<GitHubService>) -> Result<RemoteReads, CliError> {
  let repository = github_repository_of(context, &context.config()?)?;
  let space = context.space_repository().ok().flatten();
  let service = space.as_ref().and_then(|_| patronus_service(environment).ok()).map(Arc::new);
  let sources = BdServices {
    patronus: service.clone(),
    space,
    github_repository: repository.clone(),
    ..BdServices::new(context, Arc::clone(github))
  }
  .sources();
  let key = session_key(context);
  let robots = async {
    match &service {
      Some(_) => Some(sources.robots(&key, None, false).await),
      None => None,
    }
  };
  let pull_requests = async {
    match &repository {
      Some(repository) => Some(list_pull_requests(github, &context.repository, repository, &context.prefix).await),
      None => None,
    }
  };
  let (robots, pull_requests) = context.runtime()?.block_on(async { tokio::join!(robots, pull_requests) });
  let robots = service.zip(robots).map(|(service, result)| ListedRobots {
    service,
    sources: Arc::clone(&sources),
    result,
  });
  Ok(RemoteReads { robots, pull_requests })
}

/// What holds the push of the synced branches: the user's robots of `robots` that run on them, and what they land by
/// content (git, under the lock). Best effort: nothing holds a repository not on Space, without a Space token or without
/// an answer of Patronus, and nothing cancels or restarts its robots then
fn holds_of(context: &RepoContext, synced: &SyncOutput, robots: Option<ListedRobots>) -> PushHolds {
  let table = synced.branch_table(&context.prefix);
  let Some(robots) = robots else {
    return push_holds(context, table, OneShot::default(), None);
  };
  let mut facts = SessionFacts::new(context, synced.table.clone(), table.clone(), robots.sources);
  facts.know_listed(robots.result);
  match listed_robots(&facts.session) {
    Ok(_) => push_holds(context, table, facts.session, Some(robots.service)),
    Err(error) => {
      tracing::debug!(error = %error.message, "no push holds of robots: Patronus is not available");
      push_holds(context, table, OneShot::default(), None)
    }
  }
}

/// A named root of a stack that forks whose stack branch the remote lacks (`--with-children` takes it along only once it
/// is on the remote or its pull request is open, `stack_branches_to_push`)
fn stack_branches_off_the_remote(report: &SyncReport, named: &[String]) -> bool {
  report
    .stack_branches
    .iter()
    .any(|stack| !stack.members.is_empty() && named.contains(&stack.root) && !stack.remote.as_ref().is_some_and(|remote| remote.remote_exists))
}

/// The roots whose pull request of the whole stack is open
fn open_stack_pull_requests(listed: &PullRequestsResult) -> HashSet<String> {
  listed
    .stack_pull_requests
    .iter()
    .filter(|(_, pull_request)| pull_request.state == PullRequestState::Open)
    .map(|(root, _)| root.clone())
    .collect()
}

/// The baseline as `remote` names it: `master` of `origin/master`
fn base_on_remote<'a>(baseline: &'a str, remote: &str) -> &'a str {
  baseline.strip_prefix(remote).and_then(|rest| rest.strip_prefix('/')).unwrap_or(baseline)
}

/// The refs of `bd push` decided under the repository lock on the sync the caller ran under it (`bd push`, and `bd
/// safe-push`, which cancels the robot it replaces itself and passes no `holds`); `PlannedPush::send` pushes them after
/// the lock. A branch a merged Safe Push landed whole is left out, named or built on a named one with `--with-children`
/// (`skipped`), as the app's Push with Children leaves it, and the others push; the push is refused as `landed` only when
/// nothing else is left of it (the stack branch of a whole stack every branch of which landed included). An ancestor
/// taken along that landed stays as it is. A branch a running Safe Push holds (`holds`) is refused, or, with
/// `--restart-running` and the robot on the branch itself, its robot is canceled before the push and restarted after
/// it; an ancestor taken along that a robot holds stays as it is, unless that robot is the one restarted.
/// `open_stack_pull_requests`: the roots whose pull request of the whole stack is open, for `--with-children`. The
/// commits and the leases are fixed here, on the tips the sync left (`PreparedPush::leased`), so another writer moving a
/// branch after the lock changes nothing of what goes.
pub(crate) fn plan_push<'a>(
  context: &RepoContext,
  args: &PushArgs,
  synced: &'a SyncOutput,
  holds: &'a PushHolds,
  open_stack_pull_requests: &HashSet<String>,
) -> Result<PlannedPush<'a>, CliError> {
  let report = &synced.report;
  let repo = Repository::of_grouping(&report.branches, &report.grouping.parked.entries);
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
        return Err(unknown_or_unapplied(context, name, "push it with `bd push --unapplied`"));
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
    // the branches Push with Children of a named one covers, as the app's: those stacked on it by declared parent
    // (`push::push_plan`, `PushScope::WithChildren`); a blocked one is refused below as not synced. A branch taken along
    // brings no children: only the named ones do
    let children: HashSet<&str> = if args.with_children {
      args.branches.iter().flat_map(|name| descendants(&repo, name)).collect()
    } else {
      HashSet::new()
    };
    // parents first
    order
      .iter()
      .copied()
      .filter(|name| args.branches.iter().any(|named| named == name) || implicit.contains(*name) || children.contains(name))
      .collect()
  };

  // the robots the push restarts: of the named branches with --restart-running; the ancestors they land go along
  let restarted: Vec<PatronusRobot> = if args.restart_running {
    selected
      .iter()
      .filter(|name| !implicit.contains(**name))
      .filter_map(|name| holds.get(name).filter(|hold| hold.is_own(name)).map(|hold| hold.robot.clone()))
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
  // the commands that act on the refusals, in `next_steps`
  let mut held_steps = Vec::new();
  let mut refused_steps = Vec::new();
  // ` --remote <remote>` for another remote than the one the sync compares with
  let remote_option = if remote == "origin" {
    String::new()
  } else {
    format!(" --remote {}", shell_word(&remote))
  };
  // the branches asked for that a merged Safe Push landed whole: left out while the rest has work
  let mut landed: Vec<(&str, &LandedBranch, String)> = Vec::new();
  let mut behind = Vec::new();
  // the parked branches the push leaves out (`--all`, a child of `--with-children`), and the named ones it refuses
  let mut parked_skipped: Vec<(&str, String)> = Vec::new();
  let mut parked_named = Vec::new();
  for name in &selected {
    let Some(branch) = synced.branches.iter().find(|branch| branch.name == *name) else {
      continue;
    };
    // a parked branch stays local, and so does a branch built on one, which carries its commits
    if let Some(parked) = parked_state(&repo, name) {
      if args.branches.iter().any(|named| named == name) {
        parked_named.push(parked_words(name, &parked));
      } else if !implicit.contains(*name) {
        parked_skipped.push((name, parked_words(name, &parked)));
      }
      continue;
    }
    if let Some(hold) = holds.get(name)
      && !restarted.iter().any(|robot| robot.id == hold.robot.id)
    {
      // a branch taken along that a Safe Push lands is on the remote as the robot took it: the push of the stack starts
      // above it, as the app's does, and a child carries its commits
      if !implicit.contains(*name) {
        if let Some(landed_branch) = &hold.landed {
          // nothing of it is left to push: it stays as it is, the others push
          landed.push((name, landed_branch, hold.reason(name)));
        } else {
          held.push(hold.reason(name));
          held_steps.push(NextStep::new(robot_command(context, &hold.by), false));
          if hold.is_own(name) {
            held_steps.push(NextStep::new(format!("{} push {} --restart-running", context.bd, shell_word(name)), true));
          }
        }
      }
      continue;
    }
    if let Some(reason) = not_synced_reason(report, branch) {
      refusals.push(format!("{name} is not synced ({reason}): fix it first, `bd sync` says what holds it"));
      refused_steps.push(NextStep::new(format!("{} sync", context.bd), false));
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
    if along && remote_state.as_ref().is_some_and(|remote| !remote.needs_push()) {
      // on the remote as it is: nothing to take along
      continue;
    }
    if let Some(remote_state) = &remote_state
      && remote_state.drops_remote_commits()
      && !args.force
    {
      let alone = if along { ", or --alone to push the named branches without it" } else { "" };
      refusals.push(format!(
        "{name}: the remote branch has {} commits the branch doesn't (`dropped_commits`); show them to the user, and once the user approved, --force replaces them{alone}",
        remote_state.behind
      ));
      // the push the sync names for it, leased on the remote head it saw, when it compared with this remote
      let replace = branch
        .push
        .clone()
        .filter(|_| remote_option.is_empty())
        .unwrap_or_else(|| format!("{} push {} --force{remote_option}", context.bd, shell_word(name)));
      refused_steps.push(NextStep::new(replace, true));
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
    stack_branches_to_push(report, &args.branches, &remote, open_stack_pull_requests, holds)
  } else {
    (Vec::new(), Vec::new())
  };
  for stack in &stacks {
    if let Some(remote_state) = &stack.remote
      && drops_remote_commits(remote_state)
      && !args.force
    {
      refusals.push(format!(
        "{}: the remote branch has {} commits the stack branch doesn't (`dropped_commits`); show them to the user, and once the user approved, --force replaces them",
        stack.branch_name, remote_state.commits_behind
      ));
      refused_steps.push(NextStep::new(
        format!("{} push {} --with-children --force{remote_option}", context.bd, shell_word(&stack.root)),
        true,
      ));
      let short = context.short_ids(RemoteSummary::ids(remote_state));
      behind.push(ErrorBranch {
        name: stack.branch_name.clone(),
        remote: RemoteSummary::new(remote_state, usize::try_from(stack.commit_count).unwrap_or(usize::MAX), &short),
      });
    }
  }
  if !parked_named.is_empty() {
    return Err(CliError::new(
      ErrorCode::Parked,
      format!("Nothing was pushed: {}; it stays local until the user unparks it", parked_named.join("; ")),
    ));
  }
  // nothing asked for is left but parked branches
  if candidates.is_empty() && stacks.is_empty() && held.is_empty() && refusals.is_empty() && landed.is_empty() && !parked_skipped.is_empty() {
    let reasons: Vec<&str> = parked_skipped.iter().map(|(_, reason)| reason.as_str()).collect();
    return Err(CliError::new(
      ErrorCode::Parked,
      format!("Nothing was pushed: {}; tell the user, never unpark them yourself", reasons.join("; ")),
    ));
  }
  // nothing asked for is left but what merged Safe Pushes landed: the push sends nothing that lands. The named ones
  // say it (with --with-children their children landed too)
  if candidates.is_empty() && stacks.is_empty() && held.is_empty() && refusals.is_empty() && !landed.is_empty() {
    let named: Vec<&str> = landed
      .iter()
      .filter(|(name, ..)| args.branches.iter().any(|named| named == name))
      .map(|(.., reason)| reason.as_str())
      .collect();
    let reasons = if named.is_empty() {
      landed.iter().map(|(.., reason)| reason.as_str()).collect()
    } else {
      named
    };
    return Err(CliError::new(ErrorCode::Landed, format!("Nothing was pushed: {}; {LANDED_NEXT}", reasons.join("; "))).with_next_step(format!("{} update", context.bd), true));
  }
  if !held.is_empty() {
    let mut error = CliError::new(ErrorCode::RobotRunning, format!("Nothing was pushed: {}", held.join("; ")));
    for step in held_steps {
      error.push_next_step(step);
    }
    return Err(error);
  }
  if !refusals.is_empty() {
    let mut error = CliError::new(ErrorCode::PushRefused, format!("Nothing was pushed: {}", refusals.join("; ")));
    error.branches = behind;
    for step in refused_steps {
      error.push_next_step(step);
    }
    return Err(error);
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
        // `bd` counts no commits as the user's: its statuses say how many go, not whose
        my_email: None,
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
      my_email: None,
    })
  }));
  // what a refused push names: the branches, then the stack branches with their roots
  let pushed_names: Vec<PushedName> = candidates
    .iter()
    .map(|candidate| PushedName {
      name: candidate.synced.name.clone(),
      commit_count: candidate.synced.commit_count,
      stack_root: None,
    })
    .chain(stacks.iter().map(|stack| PushedName {
      name: stack.branch_name.clone(),
      commit_count: usize::try_from(stack.commit_count).unwrap_or(usize::MAX),
      stack_root: Some(stack.root.clone()),
    }))
    .collect();
  // the one leased push of the app's push too: atomic, a stack goes whole or not at all
  let target = PushTarget {
    repository_path: &context.repository,
    remote: &remote,
    baseline: &synced.baseline,
    size_limit: push_size_limit(context)?,
  };
  let options = PushOptions {
    atomic: true,
    dry_run: args.dry_run,
  };
  // the stack branches come after the branches
  let prepared = PreparedPush::leased(&target, leased, options);
  Ok(PlannedPush {
    remote,
    remote_option,
    base_branch,
    candidates,
    stacks,
    stack_warnings,
    restarted,
    landed,
    parked_skipped,
    pushed_names,
    prepared,
  })
}

/// What `plan_push` decided under the lock, which `send` pushes after it
pub(crate) struct PlannedPush<'a> {
  pub(crate) remote: String,
  /// ` --remote <remote>` for another remote than `origin`, which the sync compares with; empty for `origin`
  remote_option: String,
  /// The base of every pull request: the baseline without the remote
  base_branch: String,
  pub(crate) candidates: Vec<Candidate<'a>>,
  /// The stack branches that go after the branches
  stacks: Vec<&'a StackBranchUpdate>,
  stack_warnings: Vec<String>,
  /// The robots `--restart-running` cancels before the push and restarts on the pushed commits
  restarted: Vec<PatronusRobot>,
  /// The branches asked for that a merged Safe Push landed whole, left out
  landed: Vec<(&'a str, &'a LandedBranch, String)>,
  /// The parked branches the push leaves out
  parked_skipped: Vec<(&'a str, String)>,
  /// What a refused push names: the branches, then the stack branches with their roots
  pushed_names: Vec<PushedName>,
  /// The commits and the leases of every ref, fixed under the lock
  prepared: PreparedPush,
}

impl PlannedPush<'_> {
  /// The push of the planned refs after the lock (`operations::push`): the robots `--restart-running` restarts canceled
  /// (`service`, the Patronus of the holds), the frozen commits pushed in one atomic push (once more over a remote that
  /// moved since the sync without commits the branch lacks, D4, unless `--expect-remote` named the lease), the restarts on
  /// the pushed commits, and the ready `gh pr create` of each pushed branch, its issues' summaries from `youtrack`
  pub(crate) fn send(
    self,
    context: &RepoContext,
    args: &PushArgs,
    synced: &SyncOutput,
    service: Option<&PatronusService>,
    youtrack: Option<&YouTrackService>,
  ) -> Result<PushOutput, CliError> {
    let report = &synced.report;
    let Self {
      remote,
      remote_option,
      base_branch,
      candidates,
      stacks,
      stack_warnings,
      restarted,
      landed,
      parked_skipped,
      pushed_names,
      prepared,
    } = self;
    let restarts = (!args.dry_run && !restarted.is_empty())
      .then(|| service.ok_or_else(|| CliError::new(ErrorCode::PatronusError, "Patronus is not available")))
      .transpose()?;
    if let Some(service) = restarts {
      context
        .runtime()?
        .block_on(cancel_robots(service, &context.prefix, &restarted))
        .map_err(|message| CliError::new(ErrorCode::PatronusError, message))?;
    }
    // a remote branch moved since the sync without commits its branch lacks: nobody's work is lost, so the push goes again
    // over what the remote has now, once, as the app's does (D4); a lease the user named (`--expect-remote`) stays the
    // lease, and a remote with someone's commits, or that moves again, is told (`lease_rejected`)
    let over = if args.expect_remote.is_some() { OverMoved::Never } else { OverMoved::Once };
    let sent = send(&context.git, prepared, over, &mut |_| {}).map_err(|e| CliError::new(ErrorCode::NetworkError, format!("{e:#}")))?;
    let statuses = match &sent.answer {
      LeasedPush::Pushed(statuses) => statuses,
      LeasedPush::RemoteMoved { moved, remote_statuses } => return Err(lease_rejected(context, &pushed_names, moved, remote_statuses, &remote_option)),
      LeasedPush::Refused { summary } => {
        return Err(CliError::new(
          ErrorCode::PushRefused,
          format!("Nothing was pushed: the remote refused the push ({summary}). {REMOTE_REFUSED_NEXT}"),
        ));
      }
      LeasedPush::TooLarge(too_large) => return Err(CliError::from(too_large)),
      LeasedPush::WouldReject => {
        return Err(CliError::new(
          ErrorCode::LeaseRejected,
          format!(
            "The remote would reject {}: {WOULD_REJECT_NEXT}",
            pushed_names.iter().map(|pushed| pushed.name.as_str()).collect::<Vec<_>>().join(", ")
          ),
        ));
      }
    };
    let refs: Vec<&PushRef> = sent.prepared.branches().iter().map(|branch| &branch.push).collect();

    let repository_option = github_repository_option(context, report)?;
    let short = context.short_ids(refs.iter().flat_map(|pushed| std::iter::once(pushed.tip.as_str()).chain(pushed.lease.as_deref())));
    let names: Vec<&str> = candidates.iter().map(|candidate| candidate.synced.name.as_str()).collect();
    let issues = pull_request_issues(context, report, youtrack, &names)?;
    let branches = candidates
      .iter()
      .zip(&refs)
      .enumerate()
      .map(|(index, (candidate, &pushed))| {
        let draft = pull_request_draft(
          context,
          report,
          (&repository_option, base_branch.clone()),
          (&candidate.synced.name, &candidate.full_name),
          &issues,
        );
        let replaced_landed = match sent.moved_to(index) {
          Some(moved) => landed_commits(moved, &context.short_ids(RemoteSummary::ids(moved))),
          None => candidate.remote.as_ref().map_or_default(|remote| remote.landed_commits.clone()),
        };
        PushedBranch {
          pushed_over: sent.moved_to(index).is_some(),
          ..pushed_branch(&candidate.synced.name, pushed, statuses, draft, &short, replaced_landed)
        }
      })
      .collect();
    let stacks = stacks
      .iter()
      .zip(&refs[candidates.len()..])
      .enumerate()
      .map(|(at, (stack, &pushed))| {
        let draft = stack_pull_request_draft(context, report, base_branch.clone(), stack)?;
        let moved = sent.moved_to(candidates.len() + at);
        let replaced_landed = moved
          .or(stack.remote.as_ref())
          .map_or_default(|remote| landed_commits(remote, &context.short_ids(RemoteSummary::ids(remote))));
        Ok(PushedBranch {
          pushed_over: moved.is_some(),
          ..pushed_branch(&stack.root, pushed, statuses, draft, &short, replaced_landed)
        })
      })
      .collect::<Result<_, CliError>>()?;

    // the restart takes the commit just pushed; a rejected push above left the robots canceled
    let restarted = match restarts {
      Some(service) => context
        .runtime()?
        .block_on(restart_robots(service, &context.prefix, &restarted))
        .map_err(|message| CliError::new(ErrorCode::PatronusError, message))?,
      None => Vec::new(),
    };
    let restarted = restarted
      .into_iter()
      .map(|robot| RestartedRobot {
        branch: robot.branch_name.clone().unwrap_or_default(),
        url: robot.url,
      })
      .collect();
    let landed_ids = context.short_ids(landed.iter().flat_map(|(_, landed, _)| landed.landed.iter().chain(&landed.after).map(String::as_str)));
    let skipped = landed
      .into_iter()
      .map(|(name, landed, message)| SkippedBranch {
        name: name.to_string(),
        code: ErrorCode::Landed,
        message,
        landed_by: Some(LandedBySummary::new(context, landed, &landed_ids)),
      })
      .chain(parked_skipped.into_iter().map(|(name, message)| SkippedBranch {
        name: name.to_string(),
        code: ErrorCode::Parked,
        message,
        landed_by: None,
      }))
      .collect();
    Ok(PushOutput {
      schema: SchemaVersion,
      repository: context.repository.clone(),
      prefix: context.prefix.clone(),
      remote,
      dry_run: args.dry_run,
      branches,
      restarted,
      retargeted: Vec::new(),
      retarget_warnings: Vec::new(),
      stacks,
      stack_warnings,
      skipped,
    })
  }
}

/// The stack branches of the stacks that fork whose root `--with-children` names, in the order of the sync, by the rule
/// of the app's Push with Children (`push::stack_branch_along`): once it is on the remote (`origin`) or its pull request
/// is open (`open_stack_pull_requests`, by root), the `built` ones the remote lacks (by content, as the sync's remote
/// status compares), and a warning per one of them left out. One neither on the remote nor with an open pull request is no
/// branch to publish by itself: `bd safe-push --whole-stack` and its pull request put it there. A stack branch in
/// another state is not the stack's commits, and its remote status is of `origin` only. One whose every branch merged
/// Safe Pushes landed whole (`holds`) carries only commits that are in the target: a push of it lands nothing. One the
/// Safe Push of the whole stack runs on stays as it is: a push into it fails the robot at the end
fn stack_branches_to_push<'a>(
  report: &'a SyncReport,
  named: &[String],
  remote: &str,
  open_stack_pull_requests: &HashSet<String>,
  holds: &PushHolds,
) -> (Vec<&'a StackBranchUpdate>, Vec<String>) {
  let mut stacks = Vec::new();
  let mut warnings = Vec::new();
  for stack in report.stack_branches.iter().filter(|stack| named.contains(&stack.root)) {
    // a parked member, every member landed, its own robot, a build the sync did not make (`stack_hold`)
    let hold = holds.stack_hold(stack);
    match stack_branch_along(stack, open_stack_pull_requests.contains(&stack.root), hold.as_ref()) {
      // no stack that forks, or one neither on the remote nor in a pull request: no branch to publish by itself
      None | Some(StackBranchAlong::Unpublished) => {}
      Some(
        StackBranchAlong::Held {
          reason: Reason::StackNotBuilt { state, .. },
        }
        | StackBranchAlong::NotBuilt {
          reason: Reason::StackNotBuilt { state, .. },
        },
      ) => warnings.push(format!(
        "{} was not pushed: this sync didn't build it ({}; `stacks` of `bd sync` says why)",
        stack.branch_name,
        state_word(state)
      )),
      Some(StackBranchAlong::Held { reason } | StackBranchAlong::NotBuilt { reason }) => {
        warnings.push(format!("{} was not pushed: {}", stack.branch_name, holds.stack_hold_words(&stack.root, &reason)));
      }
      Some(_) if remote != "origin" => warnings.push(format!("{} was not pushed: its remote status is of origin, not of {remote}", stack.branch_name)),
      Some(StackBranchAlong::UpToDate) => {}
      // one that drops remote commits goes with --force, as a branch does
      Some(StackBranchAlong::Along | StackBranchAlong::DropsRemote) => stacks.push(stack),
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

/// `bd push --unapplied <name>`: the unapplied branch to `origin` under its own name, leased on the remote head of the
/// last fetch (or `--expect-remote`), its commit and lease fixed under the repository lock and pushed after it
/// (`operations::push::send`); no sync, since it is no group of HEAD, and no robot is read: no Safe Push runs on it
fn push_unapplied(context: &RepoContext, args: &PushArgs) -> Result<PushOutput, CliError> {
  let [name] = args.branches.as_slice() else {
    return Err(CliError::new(ErrorCode::InvalidArguments, "--unapplied pushes one unapplied branch: name exactly one"));
  };
  let full_name = qualified_branch_name(&context.prefix, BranchKind::Unapplied, name).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))?;
  let repository_lock = lock(context)?;
  let tip = resolve_commit(&context.git, &context.repository, &format!("refs/heads/{full_name}"))?
    .ok_or_else(|| CliError::new(ErrorCode::UnknownBranch, format!("No unapplied branch {full_name}: `unapplied` of `bd status` lists them")))?;
  let expected = match &args.expect_remote {
    Some(expected) => {
      Some(resolve_commit(&context.git, &context.repository, expected)?.ok_or_else(|| CliError::new(ErrorCode::InvalidArguments, format!("{expected:?} names no commit")))?)
    }
    None => resolve_commit(&context.git, &context.repository, &format!("refs/remotes/{REMOTE}/{full_name}"))?,
  };
  let baseline = detect_baseline_branch(&context.git, &context.repository)?;
  let target = PushTarget {
    repository_path: &context.repository,
    remote: REMOTE,
    baseline: &baseline,
    size_limit: push_size_limit(context)?,
  };
  // an unapplied branch is no group of HEAD: no sync built it; its commit and lease fixed here, under the lock
  let branch = LeasedBranch {
    name: name.clone(),
    push: PushRef {
      full_name: full_name.clone(),
      tip: tip.clone(),
      source: if expected.is_some() { LeaseSource::Expected } else { LeaseSource::Absent },
      lease: expected,
    },
    parent_ref: None,
    my_email: None,
  };
  let options = PushOptions { atomic: false, dry_run: false };
  let prepared = PreparedPush::leased(&target, vec![branch], options);
  drop(repository_lock);
  // the remote moved since the last fetch without commits the branch lacks: the push goes again over it, once, as the
  // app's does (D4); a lease the user named stays the lease
  let over = if args.expect_remote.is_some() { OverMoved::Never } else { OverMoved::Once };
  let sent = send(&context.git, prepared, over, &mut |_| {})?;
  let expected = sent.prepared.branches()[0].push.lease.clone();
  let pushed_over = sent.over.is_some();
  let short = context.short_ids(std::iter::once(tip.as_str()).chain(expected.as_deref()));
  match sent.answer {
    LeasedPush::Pushed(_) => Ok(PushOutput {
      schema: SchemaVersion,
      repository: context.repository.clone(),
      prefix: context.prefix.clone(),
      remote: REMOTE.to_string(),
      dry_run: false,
      branches: vec![PushedBranch {
        name: name.clone(),
        remote_branch: full_name,
        commit: short.get(&tip).to_string(),
        state: match &expected {
          None => PushState::New,
          Some(expected) if *expected == tip => PushState::UpToDate,
          Some(_) => PushState::Updated,
        },
        lease: expected.as_deref().map(|lease| short.get(lease).to_string()),
        lease_source: if expected.is_some() { "expected" } else { "absent" }.to_string(),
        replaced_landed: Vec::new(),
        pushed_over,
        pull_request: None,
      }],
      restarted: Vec::new(),
      retargeted: Vec::new(),
      retarget_warnings: Vec::new(),
      stacks: Vec::new(),
      stack_warnings: Vec::new(),
      skipped: Vec::new(),
    }),
    LeasedPush::RemoteMoved { remote_statuses, .. } => {
      let head = remote_statuses.first().and_then(|status| status.remote_tip.clone());
      let error = CliError::new(
        ErrorCode::LeaseRejected,
        format!(
          "The remote branch {full_name} is at {}, not where the lease expected it; nothing was pushed. Show it to the user; once the user approved, `--expect-remote` it to replace it",
          head.as_deref().map_or("nothing", |head| head)
        ),
      );
      Err(match head {
        Some(head) => error.with_next_step(format!("{} push {} --unapplied --expect-remote {head}", context.bd, shell_word(name)), true),
        None => error,
      })
    }
    LeasedPush::Refused { summary } => Err(CliError::new(
      ErrorCode::PushRefused,
      format!("Nothing was pushed: the remote refused the push ({summary}). {REMOTE_REFUSED_NEXT}"),
    )),
    LeasedPush::TooLarge(too_large) => Err(CliError::from(&too_large)),
    LeasedPush::WouldReject => Err(CliError::new(ErrorCode::LeaseRejected, format!("The remote would reject {full_name}: {WOULD_REJECT_NEXT}"))),
  }
}

/// The remote an unapplied branch goes to, the one the sync compares with
const REMOTE: &str = "origin";

/// The output of a pushed ref named `name`: its commit, what the remote did with it (`statuses` of the push, by remote
/// ref), its lease, the landed remote commits it replaced and its pull request
pub(crate) fn pushed_branch(
  name: &str,
  pushed: &PushRef,
  statuses: &HashMap<String, PushedRefStatus>,
  pull_request: PullRequestDraft,
  short: &ShortIds,
  replaced_landed: Vec<LandedRemoteCommitSummary>,
) -> PushedBranch {
  let flag = statuses.get(&format!("refs/heads/{}", pushed.full_name)).map_or(' ', |status| status.flag);
  PushedBranch {
    name: name.to_string(),
    remote_branch: pushed.full_name.clone(),
    commit: short.get(&pushed.tip).to_string(),
    state: match flag {
      '*' => PushState::New,
      '+' => PushState::Forced,
      '=' => PushState::UpToDate,
      _ => PushState::Updated,
    },
    lease: pushed.lease.as_deref().map(|lease| short.get(lease).to_string()),
    lease_source: pushed.source.as_str().to_string(),
    replaced_landed,
    pushed_over: false,
    pull_request: Some(pull_request),
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
  Ok(draft_of(&github_repository_option(context, report)?, base, &stack.branch_name, title, body))
}

/// The draft of the pull request of `head` into `base` with its text, and the `gh pr create` that opens it
fn draft_of(repository_option: &str, base: String, head: &str, title: String, body: String) -> PullRequestDraft {
  let command = format!(
    "gh pr create{repository_option} --base {} --head {} --assignee @me --title {} --body-file -",
    shell_quote(&base),
    shell_quote(head),
    shell_quote(&title)
  );
  PullRequestDraft {
    base,
    head: head.to_string(),
    title,
    body: Some(body).filter(|body| !body.is_empty()),
    command,
  }
}

/// The pull request of the grouped branch `name`, the remote branch `head`, into `base` (the baseline without the
/// remote), as the printed `gh pr create` opens it, with the text of `pull_request_text`: the `bd describe` text, else
/// the text of its commits, with the branches it is built on, whose commits it holds too. `issues` are the texts of
/// their issues (`pull_request_issues`). From what the sync read: no git process.
fn pull_request_draft(context: &RepoContext, report: &SyncReport, (repository_option, base): (&str, String), (name, head): (&str, &str), issues: &[IssueText]) -> PullRequestDraft {
  let (title, body) = pull_request_text(&pull_request_input(context, report, name), issues);
  draft_of(repository_option, base, head, title, body)
}

/// What the text of the pull request of the grouped branch `name` is written from: its description, its commits and
/// those of the branches it is built on, as the sync grouped them
fn pull_request_input<'a>(context: &RepoContext, report: &'a SyncReport, name: &'a str) -> PullRequestTextInput<'a> {
  let branch_of = |name: &str| report.branches.iter().find(|branch| branch.name == name);
  let built_on = |branch: &'a GroupedBranchInfo| branch.stack.as_ref().and_then(|stack| stack.effective_base.as_deref());
  // newest first in the grouping
  let subjects = |branch: Option<&GroupedBranchInfo>| branch.map_or_default(|branch| branch.commits.iter().rev().map(|commit| commit.stripped_subject.clone()).collect());
  let branch = branch_of(name);
  let mut ancestors: Vec<AncestorText> = Vec::new();
  let mut parent = branch.and_then(built_on);
  while let Some(parent_name) = parent
    && ancestors.len() < report.branches.len()
  {
    let ancestor = branch_of(parent_name);
    ancestors.push(AncestorText {
      name: parent_name.to_string(),
      subjects: subjects(ancestor),
    });
    parent = ancestor.and_then(built_on);
  }
  ancestors.reverse();
  PullRequestTextInput {
    branch_name: name,
    description: final_name(context, name)
      .ok()
      .and_then(|full_name| report.config.descriptions().get(&format!("refs/heads/{full_name}"))),
    commits: branch.map_or_default(|branch| {
      branch
        .commits
        .iter()
        .rev()
        .map(|commit| CommitText::of_message(&commit.stripped_subject, &commit.message))
        .collect()
    }),
    ancestors,
  }
}

/// The texts of the issues of the pull requests of the grouped branches `names`, asked once for all of them
/// (`issue_texts`: the links of `.idea/vcs.xml`, the summaries `youtrack` answers)
pub(crate) fn pull_request_issues(context: &RepoContext, report: &SyncReport, youtrack: Option<&YouTrackService>, names: &[&str]) -> Result<Vec<IssueText>, CliError> {
  let mut keys: Vec<String> = Vec::new();
  for name in names {
    let input = pull_request_input(context, report, name);
    if input.description.is_none() {
      for key in input.issue_keys() {
        if !keys.contains(&key) {
          keys.push(key);
        }
      }
    }
  }
  if keys.is_empty() {
    return Ok(Vec::new());
  }
  let links = load_issue_navigation_config(&context.repository);
  Ok(context.runtime()?.block_on(issue_texts(&keys, links.as_ref(), youtrack)))
}

/// The pull request draft of a branch whose remote branch is up to date by content, which `bd safe-push` doesn't push:
/// what `push_locked` returns for a pushed one
pub(crate) fn unpushed_pull_request_draft(context: &RepoContext, synced: &SyncOutput, name: &str, youtrack: Option<&YouTrackService>) -> Result<PullRequestDraft, CliError> {
  let mut drafts = pull_request_drafts(context, synced, &[name], youtrack)?;
  drafts.pop().ok_or_else(|| CliError::new(ErrorCode::Internal, format!("No pull request draft of {name}")))
}

/// The pull request of each of `names` into the baseline, as `gh pr create` opens it, their issues read in one request
pub(crate) fn pull_request_drafts(context: &RepoContext, synced: &SyncOutput, names: &[&str], youtrack: Option<&YouTrackService>) -> Result<Vec<PullRequestDraft>, CliError> {
  let (_, base_branch) = synced
    .baseline
    .split_once('/')
    .ok_or_else(|| CliError::new(ErrorCode::NoRemote, format!("The baseline {} is local: pass --remote", synced.baseline)))?;
  let report = &synced.report;
  let issues = pull_request_issues(context, report, youtrack, names)?;
  let repository_option = github_repository_option(context, report)?;
  names
    .iter()
    .map(|name| {
      let head = final_name(context, name)?;
      Ok(pull_request_draft(context, report, (&repository_option, base_branch.to_string()), (name, &head), &issues))
    })
    .collect()
}

/// Why the sync didn't build the branch (`SyncReport::not_built`, the rule of the app's push too): its error or its
/// conflict as the sync reported them, else the reason; None when it built it
pub fn not_synced_reason(report: &SyncReport, branch: &SyncedBranch) -> Option<String> {
  let reason = report.not_built.get(&branch.name)?;
  let detail = branch
    .error
    .clone()
    .or_else(|| branch.conflict.as_ref().map(|conflict| format!("conflict in {}", conflict.files.join(", "))));
  Some(detail.unwrap_or_else(|| not_built_reason(*reason).to_string()))
}

/// `<prefix>/virtual/<name>`
/// `branchdeck.pushSizeLimit` of the command's config (`ConfigSnapshot::push_size_limit`); a value that is neither a
/// size nor off refuses the push
pub(crate) fn push_size_limit(context: &RepoContext) -> Result<Option<u64>, CliError> {
  context
    .config()?
    .push_size_limit()
    .map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("{error:#}")))
}

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

/// A ref of a push, as a refused push names it
pub(crate) struct PushedName {
  /// The branch, or the stack branch `{prefix}/stack/<root>`
  pub(crate) name: String,
  pub(crate) commit_count: usize,
  /// The root of the stack branch, which `bd push --with-children` pushes
  pub(crate) stack_root: Option<String>,
}

/// What to do about a push the remote refused with every lease held (a hook, a protected branch)
pub(crate) const REMOTE_REFUSED_NEXT: &str = "Show the user what the remote answered: no flag of bd gets past it";

/// Why a dry run would be rejected, and how to see what the remote has
pub(crate) const WOULD_REJECT_NEXT: &str =
  "a remote branch is not where its lease expects it, or the remote refuses the push. Nothing was pushed, and a dry run fetches nothing: show it to the user";

/// What to do about a branch merged Safe Pushes landed whole
pub(crate) const LANDED_NEXT: &str = "once the user approved, `bd update` archives what landed";

/// `lease_rejected` for the refs at `moved` (of `pushed`), whose remote branch is not where its lease expects it:
/// nothing was pushed. `push_leased` fetched the branches, and the error lists what each moved one has now
/// (`remote_statuses`, in the order of `pushed`), so the user can see what a push with `--force --expect-remote` would
/// drop; its `next_steps` are those pushes, the user's to approve. `remote_option`: ` --remote <remote>`, or empty
pub(crate) fn lease_rejected(context: &RepoContext, pushed: &[PushedName], moved: &[usize], remote_statuses: &[RemoteStatusUpdate], remote_option: &str) -> CliError {
  let names = moved.iter().map(|index| pushed[*index].name.as_str()).collect::<Vec<_>>().join(", ");
  let short = context.short_ids(moved.iter().flat_map(|index| RemoteSummary::ids(&remote_statuses[*index])));
  let mut error = CliError::new(
    ErrorCode::LeaseRejected,
    format!(
      "The remote rejected {names}: it is not where the lease expects it. Nothing was pushed; bd fetched it, and \
       `branches` says what the remote has now (`dropped_commits`: the commits a push drops). Show them to the user; once \
       the user approved, push again with --force --expect-remote <remote.head> to replace them"
    ),
  );
  error.branches = moved
    .iter()
    .map(|index| ErrorBranch {
      name: pushed[*index].name.clone(),
      remote: RemoteSummary::new(&remote_statuses[*index], pushed[*index].commit_count, &short),
    })
    .collect();
  let bd = &context.bd;
  let steps: Vec<NextStep> = moved
    .iter()
    .zip(&error.branches)
    .map(|(index, rejected)| {
      let command = match (&pushed[*index].stack_root, &rejected.remote.head) {
        (Some(root), _) => format!("{bd} push {} --with-children --force{remote_option}", shell_word(root)),
        (None, Some(head)) => format!("{bd} push {} --force --expect-remote {head}{remote_option}", shell_word(&rejected.name)),
        (None, None) => format!("{bd} push {} --force{remote_option}", shell_word(&rejected.name)),
      };
      NextStep::new(command, true)
    })
    .collect();
  for step in steps {
    error.push_next_step(step);
  }
  error
}

/// The GitHub repository of the branches (`resolve_github_repository`) from what the sync read and the push URL of
/// `origin`: a git process only for the Space mirror, once per command (`RepoContext::github_repository`)
pub(crate) fn github_repository(context: &RepoContext, report: &SyncReport) -> Result<Option<GitHubRepository>, CliError> {
  github_repository_of(context, &report.config)
}

/// The GitHub repository of the branches by `config` (`branchdeck.githubRepository`, the Space settings), else `origin`
pub(crate) fn github_repository_of(context: &RepoContext, config: &ConfigSnapshot) -> Result<Option<GitHubRepository>, CliError> {
  context.github_repository(|| {
    Ok(resolve_github_repository_with(
      &context.git,
      &context.repository,
      config.github_repository(),
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

/// `unknown_branch` for a name no group of HEAD has; `unapplied` when it names an unapplied branch, whose commits are not
/// in HEAD: `what` says what to do instead
pub(crate) fn unknown_or_unapplied(context: &RepoContext, name: &str, what: &str) -> CliError {
  let unapplied = qualified_branch_name(&context.prefix, BranchKind::Unapplied, name).ok();
  if let Some(unapplied) = unapplied
    && ref_exists(&context.git, &context.repository, &format!("refs/heads/{unapplied}"))
  {
    return CliError::new(
      ErrorCode::Unapplied,
      format!("{name} is unapplied ({unapplied}): its commits are not in HEAD and land nothing; {what}"),
    );
  }
  CliError::new(ErrorCode::UnknownBranch, format!("No branch named {name:?}"))
}

/// `unapplied` when `name` has no virtual branch but an unapplied one: before a command asks anything of the network
/// (`bd safe-push` asks Patronus for its token first)
pub(crate) fn refuse_an_unapplied_name(context: &RepoContext, name: &str, what: &str) -> Result<(), CliError> {
  let virtual_exists = to_final_branch_name(&context.prefix, name).is_ok_and(|full| ref_exists(&context.git, &context.repository, &format!("refs/heads/{full}")));
  if virtual_exists {
    return Ok(());
  }
  match unknown_or_unapplied(context, name, what) {
    error if error.code == ErrorCode::Unapplied => Err(error),
    _ => Ok(()),
  }
}
