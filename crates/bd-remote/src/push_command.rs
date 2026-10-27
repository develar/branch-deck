//! `bd push`: sync, then push the virtual branches as the app's push does, through the same engine
//! (docs/design/git-operations.md, "Push and land", "Agent push"): an intent `Push` per named branch over the one-shot
//! session of the command's sync (`operations::push::push_planned`: Push Stack of it by default, the branch alone with
//! `--alone` and `--expect-remote`, Push with Children beside Push Stack with `--with-children`, the unapplied branch
//! with `--unapplied`), the plans of all of them one atomic push (`push_together`), run as the app's run
//! (`land_execute::run_planned`): the witness, the repository lock only for the full witness and the freeze, the
//! retarget of each open pull request into a virtual branch to the baseline first (D3a), the cancel of the Safe Push
//! `--restart-running` restarts, the leased push of the frozen commits, once more over a remote that moved without
//! commits a branch lacks (D4), the restart. It prints a ready `gh pr create` per branch, into the baseline: every pull
//! request of a virtual branch goes there, a stacked branch's with the commits of the branches it is built on.
//!
//! The repository lock is held for the sync only; the facts the plans read (the user's robots, the pull requests of the
//! branches: `operations::needs`, one request each, side by side) and the texts of the pull requests are asked after
//! it, so no other writer waits for Patronus, GitHub or the network. A rewrite of another session between the sync and
//! the witness changes the plan: `plan_changed` (exit 3), nothing done.
//!
//! The lease is the remote head the sync compared with (`remote.head`, the session's), or the one `--expect-remote`
//! names. Where the app's push leaves a branch its scope covers out to its row, `bd push` refuses the push (`push_plan`
//! of `domain_policy::push` decides what each branch does, `push_refusals` words it): a branch the sync didn't build, a
//! branch a running Safe Push holds, a branch whose push drops remote commits it lacks without `--force`; the branches
//! merged Safe Pushes landed and the parked ones it leaves out (`skipped`), refused only when nothing else is left. With
//! `--remote` other than `origin`, the remote status of the branches is computed against that remote, so the plans,
//! the refusals and the leases are about it.

use crate::landed::LandedBySummary;
use crate::push_refusals::{Covered, decide};
use crate::request_review_command::availability_error;
use crate::safe_push_command::{STACK_REMOTE, youtrack_service};
use crate::safe_push_run::{BdStart, admit, start_services};
use crate::session_facts::SessionFacts;
use bd_core::bd_command;
use bd_core::context::{Environment, RepoContext};
use bd_core::error::{CliError, ErrorBranch, NextStep};
use bd_core::output::{SchemaVersion, ShortIds};
use bd_core::services::{BdServices, github_service, patronus_service, session_key};
use bd_core::sync_command::{LandedRemoteCommitSummary, RemoteSummary, SyncOutput, SyncedBranch, landed_commits, lock, sync_locked};
use domain_types::branch_names::{BranchKind, qualified_branch_name, to_final_branch_name};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{PlannedEffect, Subject};
use domain_types::operation::{EffectStatus, Intent, OperationId, Push, PushScope};
use domain_types::patronus::PatronusRobotStatus;
use domain_types::pull_request::{PullRequest, PullRequestAvailability, ReviewerState};
use domain_types::repository_view::HoldKind;
use domain_types::stack::StackBranchUpdate;
use domain_types::{GroupedBranchInfo, RemoteStatusUpdate, SyncEvent};
use git_ops::revs::{ref_exists, ref_tips, resolve_commit, tips_under};
use operations::land_execute::{Accepted, Planned, PushedRef, StartServices, Started, Steps, dry_run, run_planned};
use operations::needs::needs;
use operations::push::Sent;
use remote_facts::pull_request_text::{AncestorText, CommitText, IssueText, PullRequestTextInput, issue_texts, pull_request_text};
use remote_facts::pull_requests::{GitHubWriteError, failure_message};
use remote_facts::stack_pull_request::{StackMemberText, stack_pull_request_text};
use repository_model::one_shot::OneShot;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use sync_core::config_snapshot::ConfigSnapshot;
use sync_core::github_repository::{GitHubRepository, parse_github_repository, resolve_github_repository_with};
use sync_core::issue_navigation::load_issue_navigation_config;
use sync_core::push::{LeasedPush, not_built_reason};
use sync_core::push_lease::{PushRef, PushedRefStatus};
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
  /// Lease on this remote commit: the push replaces the remote branch while it is there, or once more over what it moved
  /// to when that has no commits the branch lacks. Pass the `remote.head` of the status the user saw, e.g. of a
  /// `lease_rejected` error. Needs exactly one branch
  #[arg(long, value_name = "COMMIT", conflicts_with = "all")]
  pub expect_remote: Option<String>,
  /// The remote; default: the remote of the baseline (`origin` for `origin/master`)
  #[arg(long)]
  pub remote: Option<String>,
  /// Sync, then ask the remote what the push would do (`git push --dry-run`); push nothing
  #[arg(long)]
  pub dry_run: bool,
  /// Push a branch whose Safe Push runs: cancel the Safe Push, push the branch, and restart it on the pushed commit.
  /// Without it such a branch is refused (`robot_running`): a push won't land, and it fails the Safe Push at the end
  #[arg(long)]
  pub restart_running: bool,
  /// Push the unapplied branch `{prefix}/unapplied/<name>` (one name, as `unapplied` of `bd status` names it) to
  /// `origin` under its own name, leased on the remote head the last fetch saw or --expect-remote: the remote keeps a
  /// copy. It lands nothing, and no pull request is drafted
  #[arg(long, requires = "branches", conflicts_with_all = ["all", "with_children", "alone", "remote", "dry_run", "restart_running"])]
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

/// An open pull request whose base was a virtual branch, retargeted to the baseline (D3a: every pull request of a
/// virtual branch goes into the baseline, with the commits of the branches it is built on; `bd push` and `bd safe-push`)
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RetargetedPullRequest {
  pub branch: String,
  pub number: u32,
  pub url: String,
  /// The virtual branch it went into, e.g. `jane/virtual/auth`
  pub old_base: String,
  /// The baseline as the remote names it, e.g. `master`
  pub new_base: String,
  /// Who had approved it: GitHub may dismiss the approvals of a pull request whose base changes; tell the user
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub approved_by: Vec<String>,
}

impl RetargetedPullRequest {
  /// The retarget of `pull_request` of the virtual branch `branch` from its base to `new_base`, with who approved it
  pub(crate) fn of(branch: &str, pull_request: &PullRequest, new_base: &str) -> Self {
    Self {
      branch: branch.to_string(),
      number: pull_request.number,
      url: pull_request.url.clone(),
      old_base: pull_request.base_ref_name.clone(),
      new_base: new_base.to_string(),
      approved_by: pull_request
        .reviewers
        .iter()
        .filter(|reviewer| reviewer.state == ReviewerState::Approved)
        .map(|reviewer| reviewer.login.clone())
        .collect(),
    }
  }
}

/// "cannot retarget pull request #8 of login to master: <what GitHub said>; `gh pr edit 8 --repo acme/monorepo --base
/// master` retargets it": a retarget GitHub refused, which stops no push
pub(crate) fn refusal_warning(retarget: &RetargetedPullRequest, repository: &GitHubRepository, error: GitHubWriteError) -> String {
  let reason = match error {
    GitHubWriteError::GitHub(failure) => failure_message(&failure),
    GitHubWriteError::Refused(message) => message,
  };
  format!(
    "cannot retarget pull request #{} of {} to {}: {reason}; `gh pr edit {} --repo {} --base {}` retargets it",
    retarget.number,
    retarget.branch,
    retarget.new_base,
    retarget.number,
    repository.full_name(),
    retarget.new_base
  )
}

pub fn push(context: &RepoContext, args: &PushArgs, environment: &Environment) -> Result<PushOutput, CliError> {
  check_arguments(args)?;
  // the sync is the part under the lock (its merge-tree writes the virtual branches); the reads of the remote and the
  // plans keep no other writer waiting, and the run takes the lock again only to witness the refs and freeze the push
  let synced = {
    let _lock = lock(context)?;
    sync_locked(context)?
  };
  let remote = push_remote(args, &synced)?;
  let expect_remote = args.expect_remote.as_deref().map(|rev| resolve_expected(context, rev)).transpose()?;
  if !args.unapplied {
    for name in &args.branches {
      if !synced.branches.iter().any(|branch| &branch.name == name) {
        return Err(unknown_or_unapplied(context, name, "push it with `bd push --unapplied`"));
      }
    }
  }
  let github = Arc::new(github_service(context, environment));
  let repository = github_repository(context, &synced.report)?;
  // no Safe Push runs on an unapplied branch, and its pull request stays as it was: nothing is asked of the remote
  let space = if args.unapplied { None } else { context.space_repository().ok().flatten() };
  let patronus = space.as_ref().and_then(|_| patronus_service(environment).ok()).map(Arc::new);
  let sources = BdServices {
    patronus: patronus.clone(),
    space: space.clone(),
    github_repository: repository.clone(),
    ..BdServices::new(context, Arc::clone(&github))
  }
  .sources();
  let (statuses, remotes) = remote_statuses(context, &synced, &remote)?;
  let mut state = synced.table.clone();
  for status in statuses {
    state.apply(SyncEvent::RemoteStatusUpdate(status));
  }
  let mut facts = SessionFacts::new(context, state, synced.branch_table(&context.prefix), Arc::clone(&sources));
  let run = PushRun {
    context,
    environment,
    args,
    synced: &synced,
    remote,
    remotes,
  };
  let mut intents = run.intents(expect_remote.as_deref());
  // the facts the plans read (the robots that hold the push, the pull requests it retargets), each once, side by side
  let mut wanted = Vec::new();
  for intent in &intents {
    for need in needs(&Intent::Push(intent.clone())) {
      if !wanted.contains(&need) {
        wanted.push(need);
      }
    }
  }
  if !wanted.is_empty() {
    facts.read(context, &wanted)?;
    facts.derive();
  }
  if args.restart_running {
    intents = PushRun::restarts(&facts.session, intents);
  }
  let decided = decide(&run, &facts.session, &intents)?;
  let Some(planned) = operations::push::push_together(decided.plans, &synced.order()) else {
    return run.output(&facts, None, Told::default(), decided.covered);
  };
  let target = base_on_remote(&synced.baseline, STACK_REMOTE).to_string();
  let services = {
    let github = repository.map(|repository| (Arc::clone(&github), repository));
    let mut services = start_services(context, patronus, space.map_or_default(|space| space.name), github, target, &synced.baseline)?;
    services.push.remote.clone_from(&run.remote);
    services
  };
  let runtime = context.runtime()?;
  if args.dry_run {
    let sent = runtime.block_on(dry_run(&services, &planned, facts.session.robots())).map_err(CliError::from)?;
    let told = Told {
      retargeted: planned.plan.effects.iter().filter_map(|effect| retarget_of(&facts, effect)).collect(),
      ..Told::default()
    };
    return run.output(&facts, Some((&planned, sent.as_ref())), told, decided.covered);
  }
  let start = BdStart {
    session: &facts.session,
    land: None,
    sources,
    key: session_key(context),
    text: None,
  };
  let op = OperationId(sync_utils::operation_id::OperationId::new().to_string());
  // busy while another process runs what the push needs; the record leaves once the run ended
  let _entry = admit(context, &op, &planned)?;
  let accepted = Accepted {
    fingerprint: planned.plan.fingerprint.clone(),
    confirmations: planned.plan.confirmations.clone(),
  };
  let started = runtime
    .block_on(run_planned(&start, &services, &Steps::none(), op, planned.clone(), &accepted, std::time::Instant::now()))
    .map_err(CliError::from)?;
  let told = run.tell(&facts, &services, &planned, &started)?;
  run.output(&facts, Some((&planned, started.pushed.as_ref())), told, decided.covered)
}

/// The arguments `bd push` refuses before it syncs
fn check_arguments(args: &PushArgs) -> Result<(), CliError> {
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
  if args.unapplied && args.branches.len() != 1 {
    return Err(CliError::new(ErrorCode::InvalidArguments, "--unapplied pushes one unapplied branch: name exactly one"));
  }
  Ok(())
}

/// The remote the push goes to: `--remote`, else the remote of the baseline (an unapplied branch: `origin`, which the
/// sync compares with); a remote the sync didn't list is `no_remote`: a URL has no remote-tracking refs to compare with
fn push_remote(args: &PushArgs, synced: &SyncOutput) -> Result<String, CliError> {
  let remote = match &args.remote {
    Some(remote) => remote.clone(),
    None if args.unapplied => STACK_REMOTE.to_string(),
    None => synced
      .baseline
      .split_once('/')
      .map(|(remote, _)| remote.to_string())
      .ok_or_else(|| CliError::new(ErrorCode::NoRemote, format!("The baseline {} is local: pass --remote", synced.baseline)))?,
  };
  if !synced.report.remotes.contains(&remote) {
    return Err(CliError::new(ErrorCode::NoRemote, format!("No remote named {remote:?}: `git remote` lists them")));
  }
  Ok(remote)
}

/// The baseline as `remote` names it: `master` of `origin/master`
fn base_on_remote<'a>(baseline: &'a str, remote: &str) -> &'a str {
  baseline.strip_prefix(remote).and_then(|rest| rest.strip_prefix('/')).unwrap_or(baseline)
}

/// The remote status of each branch the sync built against `remote`, by name, as `bd sync` summarizes it: the sync's for
/// `origin`, which it compares with; for another remote computed now the same way on the commits the sync left (one
/// `for-each-ref` of its remote-tracking refs, then `RemoteStatusMemo`), with the statuses the session applies, so the
/// plans, the refusals and the leases are about that remote
fn remote_statuses(context: &RepoContext, synced: &SyncOutput, remote: &str) -> Result<(Vec<RemoteStatusUpdate>, HashMap<String, RemoteSummary>), CliError> {
  let report = &synced.report;
  if remote == STACK_REMOTE {
    let summaries = synced.branches.iter().filter_map(|branch| Some((branch.name.clone(), branch.remote.clone()?))).collect();
    return Ok((Vec::new(), summaries));
  }
  let namespace = format!("refs/remotes/{remote}/");
  let pattern = format!("{namespace}{}/", context.prefix.trim_end_matches('/'));
  let on_remote = tips_under(&ref_tips(&context.git, &context.repository, &[&pattern])?, &namespace);
  let memo = RemoteStatusMemo::new(context.git.clone(), &context.repository, context.cache(), remote, report.baseline_commit.clone(), on_remote);
  let mut statuses = Vec::new();
  let mut summaries = HashMap::new();
  for branch in &synced.branches {
    let full_name = final_name(context, &branch.name)?;
    let Some(tip) = report.tips.get(&full_name).filter(|_| !report.not_built.contains_key(&branch.name)) else {
      continue;
    };
    // the parent as the sync left it
    let parent = synced
      .placement(&branch.name)
      .and_then(|stack| stack.effective_base.as_deref())
      .map(|parent| final_name(context, parent))
      .transpose()?;
    let parent_base = parent.as_deref().map(|parent| report.tips.get(parent).map_or(parent, String::as_str));
    let status = memo.status(&full_name, &branch.name, tip, None, parent_base.map(stack_parent))?;
    let short = context.short_ids(RemoteSummary::ids(&status));
    summaries.insert(branch.name.clone(), RemoteSummary::new(&status, branch.commit_count, &short));
    statuses.push(status);
  }
  Ok((statuses, summaries))
}

/// What a run of `bd push` decides and tells on: its arguments, its sync, the remote it pushes to and the remote status
/// of each branch of the sync against it
pub(crate) struct PushRun<'a> {
  pub(crate) context: &'a RepoContext,
  environment: &'a Environment,
  pub(crate) args: &'a PushArgs,
  pub(crate) synced: &'a SyncOutput,
  pub(crate) remote: String,
  /// The remote status of each branch the sync built against `remote` (the sync's for `origin`), by name
  pub(crate) remotes: HashMap<String, RemoteSummary>,
}

/// What the effects of a run told besides its push: the retargets GitHub made and a warning of each it refused, the
/// robots it restarted
#[derive(Default)]
struct Told {
  retargeted: Vec<RetargetedPullRequest>,
  retarget_warnings: Vec<String>,
  restarted: Vec<RestartedRobot>,
}

impl PushRun<'_> {
  /// What the commands the run names add to `bd push <branch>`: ` --unapplied` of an unapplied branch, ` --remote
  /// <remote>` for another remote than `origin`, which the sync compares with; empty otherwise
  pub(crate) fn option(&self) -> String {
    if self.args.unapplied {
      " --unapplied".to_string()
    } else if self.remote == STACK_REMOTE {
      String::new()
    } else {
      format!(" --remote {}", bd_command::shell_word(&self.remote))
    }
  }

  /// The intents of the push, the named branches (every branch with `--all`) in the order of the sync, parents first:
  /// Push Stack of each by default, the branch alone with `--alone`, `--all` and `--expect-remote` (leased on that
  /// commit), Push with Children beside Push Stack with `--with-children` (alone with `--alone`), the unapplied branch
  /// with `--unapplied`
  fn intents(&self, expect_remote: Option<&str>) -> Vec<Push> {
    let order = self.synced.order();
    let names: Vec<&str> = if self.args.unapplied {
      self.args.branches.iter().map(String::as_str).collect()
    } else if self.args.all {
      order
    } else {
      order.into_iter().filter(|name| self.args.branches.iter().any(|named| named == name)).collect()
    };
    let scopes: &[PushScope] = if self.args.unapplied {
      &[PushScope::Unapplied]
    } else if self.args.all || self.args.expect_remote.is_some() || (self.args.alone && !self.args.with_children) {
      &[PushScope::Alone]
    } else if self.args.with_children && self.args.alone {
      &[PushScope::WithChildren]
    } else if self.args.with_children {
      &[PushScope::WithAncestors, PushScope::WithChildren]
    } else {
      &[PushScope::WithAncestors]
    };
    names
      .iter()
      .flat_map(|name| {
        scopes.iter().map(|scope| Push {
          subject: Subject::branch(name),
          scope: scope.clone(),
          expect_remote: expect_remote.map(str::to_string),
          force: self.args.force,
          restart_running: false,
        })
      })
      .collect()
  }

  /// `intents` with `--restart-running`: a named branch whose own Safe Push runs is pushed alone, its robot canceled
  /// first and restarted on the pushed commit, as the app's restart that pushes first; its children still go with
  /// `--with-children`
  fn restarts(session: &OneShot, intents: Vec<Push>) -> Vec<Push> {
    let mut restarted: Vec<Push> = Vec::with_capacity(intents.len());
    for intent in intents {
      let name = intent.subject.name();
      let own = session
        .branch(name)
        .and_then(|branch| branch.landing.hold.as_ref())
        .is_some_and(|hold| hold.kind == HoldKind::Own);
      let runs = own && session.branch_robot(name).is_some_and(|robot| robot.status == PatronusRobotStatus::Running);
      if !runs || intent.scope == PushScope::WithChildren {
        restarted.push(intent);
      } else if !restarted.iter().any(|known| known.subject == intent.subject && known.restart_running) {
        restarted.push(Push {
          scope: PushScope::Alone,
          restart_running: true,
          ..intent
        });
      }
    }
    restarted
  }

  /// What the effects of the run did, in their order: each retarget (one GitHub refused is a warning, the push went
  /// on), the robots restarted; an effect that failed fails the command in the words of `bd`, after what the effects
  /// before it did
  fn tell(&self, facts: &SessionFacts, services: &StartServices, planned: &Planned, started: &Started) -> Result<Told, CliError> {
    let mut told = Told::default();
    let mut answers = started.retargets.iter();
    for record in &started.outcome.effects {
      match (&record.effect, &record.status) {
        (PlannedEffect::Retarget { .. }, EffectStatus::Done | EffectStatus::Failed { .. }) => {
          let (Some(retarget), Some(answer)) = (retarget_of(facts, &record.effect), answers.next()) else {
            continue;
          };
          match (answer, &services.github) {
            (Ok(()), _) => told.retargeted.push(retarget),
            (Err(error), Some((_, repository))) => told.retarget_warnings.push(refusal_warning(&retarget, repository, error.clone())),
            (Err(_), None) => {}
          }
        }
        (_, EffectStatus::Done | EffectStatus::Unknown | EffectStatus::Skipped { reason: None }) => {}
        (PlannedEffect::Push { .. }, EffectStatus::Failed { error }) => return Err(self.push_error(planned, started.pushed.as_ref(), error)),
        (_, EffectStatus::Failed { error }) => return Err(CliError::from(error.clone())),
        (_, EffectStatus::Skipped { reason: Some(reason) }) => {
          let mut error = CliError::from(reason.clone());
          let pushed: Vec<&str> = started.pushed.iter().flat_map(|sent| sent.prepared.branches()).map(|branch| branch.name.as_str()).collect();
          if !pushed.is_empty() {
            error.message = format!("{}; pushed before it: {}", error.message, pushed.join(", "));
          }
          return Err(error);
        }
      }
    }
    told.restarted = started
      .restarted
      .iter()
      .map(|robot| RestartedRobot {
        branch: robot.branch_name.clone().unwrap_or_default(),
        url: robot.url.clone(),
      })
      .collect();
    Ok(told)
  }

  /// The push of `planned` failed (`error`), as `sent` says why: a remote that moved is `lease_rejected` with what each
  /// moved one has now, a remote that refused it `push_refused`, a push too large `push_too_large`; a push that failed
  /// before the remote answered (`sent` is `None`) is a network error
  fn push_error(&self, planned: &Planned, sent: Option<&Sent>, error: &ApiError) -> CliError {
    let Some(sent) = sent else {
      return CliError::new(ErrorCode::NetworkError, error.message.clone());
    };
    let names: Vec<PushedName> = planned
      .pushes
      .iter()
      .map(|pushed| PushedName {
        name: if pushed.whole_stack { pushed.full_name.clone() } else { pushed.name.clone() },
        commit_count: self.commit_count(pushed),
        stack_root: pushed.whole_stack.then(|| pushed.name.clone()),
      })
      .collect();
    match &sent.answer {
      LeasedPush::RemoteMoved { moved, remote_statuses } => lease_rejected(self.context, &names, moved, remote_statuses, &self.option()),
      LeasedPush::Refused { summary } => CliError::new(
        ErrorCode::PushRefused,
        format!("Nothing was pushed: the remote refused the push ({summary}). {REMOTE_REFUSED_NEXT}"),
      ),
      LeasedPush::TooLarge(too_large) => CliError::from(too_large),
      LeasedPush::WouldReject => CliError::new(
        ErrorCode::LeaseRejected,
        format!(
          "The remote would reject {}: {WOULD_REJECT_NEXT}",
          names.iter().map(|pushed| pushed.name.as_str()).collect::<Vec<_>>().join(", ")
        ),
      ),
      LeasedPush::Pushed(_) => CliError::from(error.clone()),
    }
  }

  /// How many commits the ref `pushed` sends of its branch, or of the stack branch of a whole stack
  fn commit_count(&self, pushed: &PushedRef) -> usize {
    if pushed.whole_stack {
      let stack = self.synced.report.stack_branches.iter().find(|stack| stack.root == pushed.name);
      return stack.map_or(0, |stack| usize::try_from(stack.commit_count).unwrap_or(usize::MAX));
    }
    self
      .synced
      .branches
      .iter()
      .find(|branch| branch.name == pushed.name)
      .map_or(0, |branch| branch.commit_count)
  }
}

/// The retarget of `effect` (a `Retarget` of the plan) with the pull request of the session it is of
fn retarget_of(facts: &SessionFacts, effect: &PlannedEffect) -> Option<RetargetedPullRequest> {
  let PlannedEffect::Retarget { branch, to, .. } = effect else {
    return None;
  };
  let pull_request = facts.session.facts().pull_requests.view().pull_requests.get(branch)?;
  Some(RetargetedPullRequest::of(branch, pull_request, to))
}

impl PushRun<'_> {
  /// The output of the run: each branch it pushed (with `--dry-run`: would push) and each named or covered branch the
  /// remote has as it is, parents first, with its pull request into the baseline; the stack branches; the retargets; the
  /// robots it restarted; what it left out and why
  fn output(&self, facts: &SessionFacts, pushed: Option<(&Planned, Option<&Sent>)>, told: Told, covered: Covered) -> Result<PushOutput, CliError> {
    let (context, report) = (self.context, &self.synced.report);
    let none = HashMap::new();
    let (sent, statuses) = match pushed {
      Some((planned, Some(sent))) => match &sent.answer {
        LeasedPush::Pushed(statuses) => (Some((planned, sent)), statuses),
        _ => return Err(self.push_error(planned, Some(sent), &ApiError::internal("The remote refused the push"))),
      },
      _ => (None, &none),
    };
    let refs: Vec<(&PushedRef, &PushRef, Option<&RemoteStatusUpdate>)> = sent.map_or_default(|(planned, sent)| {
      planned
        .pushes
        .iter()
        .zip(sent.prepared.branches())
        .enumerate()
        .map(|(index, (pushed, leased))| (pushed, &leased.push, sent.moved_to(index)))
        .collect()
    });
    let up_to_date: Vec<(&str, String, String, Option<String>)> = covered
      .up_to_date
      .iter()
      .filter_map(|name| self.on_the_remote(facts, name).map(|(full, tip, head)| (name.as_str(), full, tip, head)))
      .collect();
    let ids = refs
      .iter()
      .flat_map(|(_, pushed, _)| std::iter::once(pushed.tip.as_str()).chain(pushed.lease.as_deref()))
      .chain(up_to_date.iter().flat_map(|(_, _, tip, head)| std::iter::once(tip.as_str()).chain(head.as_deref())));
    let short = context.short_ids(ids);
    // every pull request goes into the baseline; the issues of all of them are asked once
    let base = base_on_remote(&self.synced.baseline, &self.remote).to_string();
    let drafted: Vec<&str> = if self.args.unapplied {
      Vec::new()
    } else {
      let pushed = refs.iter().filter(|(pushed, ..)| !pushed.whole_stack).map(|(pushed, ..)| pushed.name.as_str());
      pushed.chain(up_to_date.iter().map(|(name, ..)| *name)).collect()
    };
    let youtrack = youtrack_service(self.environment);
    let issues = pull_request_issues(context, report, Some(&youtrack), &drafted)?;
    let repository_option = github_repository_option(context, report)?;
    let draft = |name: &str, head: &str| (!self.args.unapplied).then(|| pull_request_draft(context, report, (&repository_option, base.clone()), (name, head), &issues));
    let (mut branches, mut stacks) = (Vec::new(), Vec::new());
    for (pushed, leased, moved) in refs {
      let replaced = |remote: &RemoteStatusUpdate| landed_commits(remote, &context.short_ids(RemoteSummary::ids(remote)));
      if pushed.whole_stack {
        let Some(stack) = report.stack_branches.iter().find(|stack| stack.root == pushed.name) else {
          continue;
        };
        let draft = stack_pull_request_draft(context, report, base.clone(), stack)?;
        let replaced_landed = moved.or(stack.remote.as_ref()).map_or_default(replaced);
        stacks.push(PushedBranch {
          pushed_over: moved.is_some(),
          ..pushed_branch(&pushed.name, leased, statuses, Some(draft), &short, replaced_landed)
        });
        continue;
      }
      let replaced_landed = match moved {
        Some(moved) => replaced(moved),
        None => self.remotes.get(&pushed.name).map_or_default(|remote| remote.landed_commits.clone()),
      };
      branches.push(PushedBranch {
        pushed_over: moved.is_some(),
        ..pushed_branch(&pushed.name, leased, statuses, draft(&pushed.name, &pushed.full_name), &short, replaced_landed)
      });
    }
    for (name, full_name, tip, head) in up_to_date {
      branches.push(PushedBranch {
        name: name.to_string(),
        commit: short.get(&tip).to_string(),
        state: PushState::UpToDate,
        lease: head.as_deref().map(|head| short.get(head).to_string()),
        lease_source: if head.is_some() { "expected" } else { "absent" }.to_string(),
        replaced_landed: Vec::new(),
        pushed_over: false,
        pull_request: draft(name, &full_name),
        remote_branch: full_name,
      });
    }
    let order = self.synced.order();
    branches.sort_by_key(|branch| order.iter().position(|name| *name == branch.name).unwrap_or(usize::MAX));
    let mut retarget_warnings = told.retarget_warnings;
    retarget_warnings.extend(self.listing_warning(facts, !branches.is_empty() || !stacks.is_empty())?);
    Ok(PushOutput {
      schema: SchemaVersion,
      repository: context.repository.clone(),
      prefix: context.prefix.clone(),
      remote: self.remote.clone(),
      dry_run: self.args.dry_run,
      branches,
      restarted: told.restarted,
      retargeted: told.retargeted,
      retarget_warnings,
      stacks,
      stack_warnings: covered.stack_warnings,
      skipped: covered.skipped,
    })
  }

  /// The branch `name` the remote has as it is: its name on the remote, its tip and the remote head (`None`: no remote
  /// branch); of an unapplied branch as the sync listed it
  fn on_the_remote(&self, facts: &SessionFacts, name: &str) -> Option<(String, String, Option<String>)> {
    if self.args.unapplied {
      let branch = facts.session.state().sections().unapplied.iter().find(|branch| branch.branch == name)?;
      let head = branch.remote.as_ref().and_then(|remote| remote.remote_tip.clone());
      return Some((branch.name.clone(), branch.tip.clone(), head));
    }
    let full_name = to_final_branch_name(&self.context.prefix, name).ok()?;
    let tip = self.synced.report.tips.get(&full_name)?.clone();
    let head = self.remotes.get(name).and_then(|remote| remote.head_id.clone());
    Some((full_name, tip, head))
  }

  /// Why the pull requests weren't checked for a base to retarget, when GitHub couldn't list them for another reason
  /// than a missing token or GitHub repository; nothing when the push pushed and listed nothing
  fn listing_warning(&self, facts: &SessionFacts, listed: bool) -> Result<Option<String>, CliError> {
    if !listed || self.args.unapplied || github_repository(self.context, &self.synced.report)?.is_none() {
      return Ok(None);
    }
    Ok(match &facts.session.facts().pull_requests.view().head.value {
      None | Some(PullRequestAvailability::Available { .. } | PullRequestAvailability::SignedOut | PullRequestAvailability::NotGitHub) => None,
      Some(other) => Some(format!("the pull requests weren't checked for a base to retarget: {}", availability_error(other).message)),
    })
  }
}

/// The output of a pushed ref named `name`: its commit, what the remote did with it (`statuses` of the push, by remote
/// ref), its lease, the landed remote commits it replaced and its pull request
pub(crate) fn pushed_branch(
  name: &str,
  pushed: &PushRef,
  statuses: &HashMap<String, PushedRefStatus>,
  pull_request: Option<PullRequestDraft>,
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

/// `branchdeck.pushSizeLimit` of the command's config (`ConfigSnapshot::push_size_limit`); a value that is neither a
/// size nor off refuses the push
pub(crate) fn push_size_limit(context: &RepoContext) -> Result<Option<u64>, CliError> {
  context
    .config()?
    .push_size_limit()
    .map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("{error:#}")))
}

/// `<prefix>/virtual/<name>`
fn final_name(context: &RepoContext, name: &str) -> Result<String, CliError> {
  to_final_branch_name(&context.prefix, name).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))
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
        (Some(root), _) => format!("{bd} push {} --with-children --force{remote_option}", bd_command::shell_word(root)),
        (None, Some(head)) => format!("{bd} push {} --force --expect-remote {head}{remote_option}", bd_command::shell_word(&rejected.name)),
        (None, None) => format!("{bd} push {} --force{remote_option}", bd_command::shell_word(&rejected.name)),
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
