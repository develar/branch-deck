//! `bd land`: the subject lands into master by the route of the repository (`branchdeck.landing`,
//! `domain_policy::landing_route::route`; spec/cli/agent-cli.spec.md, "bd land"; spec/branch/landing-routes.spec.md).
//!
//! - Patronus: the Safe Push `bd safe-push` starts, its plan and its output (`safe_push`).
//! - A push into the target: the sync under the repository lock, then the plan of the app's start over the one-shot
//!   session of the command (`operations::land::prepare_one_shot`: the pull requests with the permission of the token,
//!   the rules of the target, the landing gate of the subject), then its run through the engine
//!   (`operations::land_execute::execute`: the witness and the freeze under the lock, the push of the copy into the
//!   target by the push URL after it). What CI did not check and what the gate of the route asks are accepted only by
//!   flags (`--without-checks`, `--bypass-rules`, `domain_policy::land::unaccepted`), once the user said so. It never
//!   updates: the output names `bd update` as the next step.
//! - The merge of a pull request on GitHub (`land::merges_on_github`): the same frame as a push, the plan of the app's
//!   start merging the open pull request of the subject on GitHub, or enabling its auto-merge after the push of its
//!   branch, or while its checks run (`land_direct::decide_merge`); what refuses it is `not_mergeable` (a conflict, a
//!   head behind its base) or `merge_blocked` (approvals, checks, changes requested, a draft), a pull request to create
//!   first `no_pull_request`. It never updates: after a merge the output names `bd update`. On Space the pull-request
//!   route is the Safe Push, as the app has it while Patronus lands the repository; a merge on a mirror `origin` doesn't
//!   push to lands nothing (`landing_route_unsupported`).

use crate::landed::landed_branches;
use crate::push_command::{github_repository, unknown_or_unapplied};
use crate::robot_command::ClosedPullRequest;
use crate::safe_push_command::{
  CheckRead, Refusal, SafePushArgs, SafePushOutput, Start, check_next_step, compared_table, safe_push, subject_facts, table_error, whole_stack_of_branch,
};
use crate::safe_push_run::{BdStart, start_services};
use crate::session_facts::SessionFacts;
use bd_core::bd_command::shell_word;
use bd_core::check_summary::session_of_sync;
use bd_core::context::{Environment, RepoContext};
use bd_core::error::{CliError, regroup_error};
use bd_core::next_step_summary::NextStepSummary;
use bd_core::operation_record::{admit_planned, run_admitted};
use bd_core::output::SchemaVersion;
use bd_core::services::{BdServices, github_service, patronus_service, session_key};
use bd_core::stack_summary::{WholeStackSummary, stack_branch_of, whole_stack_summary};
use bd_core::sync_command::{SyncOutput, lock, sync_locked};
use domain_policy::land::{table_blocker, unaccepted};
use domain_policy::land_direct::update_after;
use domain_policy::landed::{Landing, Of, pull_requests_to_close};
use domain_policy::landing_route::{GitHubFacts, RouteFacts, TargetRules, route};
use domain_policy::stack::WholeStack;
use domain_types::error::ErrorCode;
use domain_types::landing::{Confirmation, Land, LandFact, LandingRefusal, PlannedEffect, ProfileChoice, Unchecked};
use domain_types::landing_route::{LandingRemote, LandingRoute, RouteGate};
use domain_types::operation::{EffectStatus, Plan};
use domain_types::patronus::{LandingAllowances, PatronusPushMode};
use domain_types::pull_request::PullRequestAvailability;
use domain_types::reason::Reason;
use domain_types::session::PullRequestsView;
use operations::engine::pushes_into_target;
use operations::land::LandHost;
use operations::land_execute::{Accepted, Steps, execute, refusal_code};
use operations::needs::FactNeed;
use repository_model::branch_table::BranchTable;
use repository_model::derive::{landed_subject, landing_route_of};
use repository_model::facts::Needs;
use repository_model::one_shot::OneShot;
use repository_model::refusal_words::start_refusal_text;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::Arc;
use sync_core::patronus::target_branch_of;
use trial_check::landing_gate::check_landing_gate;

#[derive(Debug, Default, clap::Args)]
#[expect(clippy::struct_excessive_bools, reason = "command-line flags")]
pub struct LandArgs {
  /// The branch that lands, with the branches it is built on
  pub branch: String,
  /// Land the whole stack of the branch: by a push, its stack branch `{prefix}/stack/<root>` of a stack that forks
  /// (only once the sync built it), the last branch of a straight chain
  #[arg(long)]
  pub whole_stack: bool,
  /// Sync, then print what lands, the next step and what stands in the way; nothing is pushed or started
  #[arg(long)]
  pub check: bool,
  /// Push into the target although CI did not pass on the commit it lands (`checks_not_passed`: no open pull request,
  /// one that shows another commit, failing or running checks, GitHub listing none) or changes are requested on its
  /// pull request: only once the user said so
  #[arg(long)]
  pub without_checks: bool,
  /// Push into the target past what its gate asks (a rule of the target the user may bypass, its required status
  /// checks, a refusal of the server before): only once the user said so
  #[arg(long)]
  pub bypass_rules: bool,
  /// Land although the branch removes a declaration that a commit of another branch still changes the use of
  #[arg(long)]
  pub allow_dangling: bool,
  /// Land although a commit of what lands uses a name that a commit of another branch adds
  #[arg(long)]
  pub allow_missing: bool,
}

/// `bd land`
#[derive(Debug, Serialize, JsonSchema)]
pub struct LandOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub branch: String,
  /// How the repository lands into master (`branchdeck.landing`): `patronus` (a Safe Push, `safe_push` tells it), `push`
  /// (the copy pushed into the target) or `pull-request` (GitHub merges its pull request)
  pub route: String,
  /// The branch of the baseline on its remote the push lands into, e.g. `master`
  pub target: String,
  /// What lands, root first: the branch with the branches it is built on that nothing landed whole yet; with
  /// `--whole-stack`, every branch of the stack. Name them to the user
  pub lands: Vec<String>,
  /// With `--whole-stack`: the stack it lands whole and the branch it lands from
  #[serde(skip_serializing_if = "Option::is_none")]
  pub whole_stack: Option<WholeStackSummary>,
  /// The commit the push lands: the copy the sync built of the branch (its virtual branch) or of the whole stack (its
  /// stack branch), on the baseline
  #[serde(skip_serializing_if = "Option::is_none")]
  pub commit: Option<String>,
  /// What the user accepts before the push, in words; each names the flag that accepts it, if any (`--without-checks`,
  /// `--bypass-rules`)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub confirmations: Vec<String>,
  /// With `--check`: the next step of the subject as the app's row shows it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub next_step: Option<NextStepSummary>,
  /// With `--check`: what refuses the push, each with the error code it would fail with
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub refusals: Vec<Refusal>,
  /// The push landed the commit into the target, or GitHub merged the pull request; `false` with `--check`
  pub landed: bool,
  /// The route `pull-request`: the pull request GitHub merges
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pull_request: Option<u32>,
  /// The route `pull-request`: GitHub merges the pull request once its checks pass (its auto-merge is enabled, after the
  /// push of the branch when its pull request showed another commit); nothing landed yet, and no update follows: tell the
  /// user it lands by itself
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub auto_merge: bool,
  /// The open pull requests of what the push landed that show an older version of a branch (a head the push left out),
  /// the pushed branch's own too: the app closes them with a comment, `bd` closes none
  /// (`domain_policy::landed::pull_requests_to_close`); one whose head the push put into the target is GitHub's to
  /// merge. Name them to the user
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub closes: Vec<ClosedPullRequest>,
  /// What to run next: `bd update` after a push that landed, which brings the branch onto the target and drops the
  /// commits that landed
  #[serde(skip_serializing_if = "Option::is_none")]
  pub next: Option<String>,
  /// The route `patronus`: what `bd safe-push` answered
  #[serde(skip_serializing_if = "Option::is_none")]
  pub safe_push: Option<Box<SafePushOutput>>,
  /// What the command decided, not printed: the scenario table compares it with the app's
  #[serde(skip)]
  #[schemars(skip)]
  pub decided: Decided,
}

/// What `bd land` decided, for the scenario table (`crates/bd-remote/src/scenario_test.rs`): the plan of the push
/// (`operations::land`), of the route Patronus the Safe Push's, and with `--check` the next step its words tell
#[derive(Debug, Default)]
pub struct Decided {
  pub plan: Option<Plan>,
  pub step: Option<domain_types::next_step::NextStep>,
}

/// What to do with an unapplied branch instead of its landing
const UNAPPLIED_LAND: &str = "nothing was pushed: reapply it to land it";

/// `bd land <branch>` by the route of the repository (the module's header)
///
/// # Errors
/// What refuses the landing, with its code: the route (`landing_route_unsupported`), the table, the gate of the route,
/// what CI did not check without `--without-checks`, a push the server refused (`baseline_refused`) or a target that
/// moved (`baseline_moved`)
pub fn land(context: &RepoContext, args: &LandArgs, environment: &Environment) -> Result<LandOutput, CliError> {
  // the config of the command, which its sync reads again from the same snapshot
  let setting = context
    .config()?
    .landing()
    .map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("{error:#}")))?;
  // the route by the rule of the app; its gate reads the facts of GitHub, which the push reads itself
  let remote = LandingRemote {
    setting,
    on_space: context.space_repository()?.is_some(),
    ..LandingRemote::default()
  };
  let routed = route(&RouteFacts {
    remote: &remote,
    baseline: "",
    github: GitHubFacts::Pending,
    rules: TargetRules::Pending,
    refused: None,
    update: None,
  });
  match routed {
    LandingRoute::Push { .. } => land_directly(context, args, environment, Direct::Push),
    // Patronus named off Space
    LandingRoute::Patronus {
      gate: RouteGate::Refused { reason },
    } => Err(CliError::new(
      refusal_code(&reason),
      format!("Nothing was pushed or started: {}", start_refusal_text(&reason, &args.branch, &[])),
    )),
    // Patronus lands a repository on Space, as the app has it
    LandingRoute::Patronus { .. } => land_by_robot(context, args, environment),
    LandingRoute::PullRequest { .. } if remote.on_space => land_by_robot(context, args, environment),
    LandingRoute::PullRequest { .. } => land_directly(context, args, environment, Direct::Merge),
  }
}

/// How a landing without a robot lands
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direct {
  /// A push of the copy into the target
  Push,
  /// GitHub merges the pull request
  Merge,
}

impl Direct {
  const fn route(self) -> &'static str {
    match self {
      Self::Push => "push",
      Self::Merge => "pull-request",
    }
  }

  /// What a refusal says was not done
  const fn nothing(self) -> &'static str {
    match self {
      Self::Push => "Nothing was pushed",
      Self::Merge => "Nothing was pushed or merged",
    }
  }
}

/// The route Patronus: the Safe Push of `bd safe-push`, its output in `safe_push`
fn land_by_robot(context: &RepoContext, args: &LandArgs, environment: &Environment) -> Result<LandOutput, CliError> {
  let safe_push_args = SafePushArgs {
    branch: args.branch.clone(),
    whole_stack: args.whole_stack,
    check: args.check,
    allow_dangling: args.allow_dangling,
    allow_missing: args.allow_missing,
    ..SafePushArgs::default()
  };
  let output = safe_push(context, &safe_push_args, environment)?;
  Ok(LandOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: args.branch.clone(),
    route: "patronus".to_owned(),
    target: output.target.clone(),
    lands: output.lands.clone(),
    whole_stack: None,
    commit: None,
    confirmations: Vec::new(),
    next_step: None,
    refusals: Vec::new(),
    landed: false,
    pull_request: None,
    auto_merge: false,
    closes: Vec::new(),
    next: None,
    decided: Decided {
      plan: output.plan.clone(),
      step: output.step.clone(),
    },
    safe_push: Some(Box::new(output)),
  })
}

/// The route of a push into the target, or of a merge of the pull request on GitHub (the module's header)
fn land_directly(context: &RepoContext, args: &LandArgs, environment: &Environment, direct: Direct) -> Result<LandOutput, CliError> {
  let github_service = Arc::new(github_service(context, environment));
  // the sync is the part under the lock (its merge-tree writes the virtual branches); the reads of GitHub and the trial
  // after it keep no other bd writer waiting, and the run takes the lock again only to witness the refs
  let synced = {
    let _lock = lock(context)?;
    sync_locked(context)?
  };
  let whole = whole_stack_of_branch(context, args.whole_stack.then_some(args.branch.as_str()), &synced)?;
  let start = start_of(args, &synced, whole.as_ref());
  let subject = start.subject();
  if let Start::Branch(name) = &start
    && !synced.branches.iter().any(|branch| branch.name == *name)
  {
    return Err(unknown_or_unapplied(context, name, UNAPPLIED_LAND));
  }
  let table = compared_table(context, &synced);
  // what the table alone refuses, before GitHub is asked: the sync did not build it, it stays local
  {
    let unasked = OneShot::default();
    let landing = unasked.landing(&table);
    if let Some(reason) = table_blocker(&subject_facts(context, &synced, &landing, &start, None)?) {
      return Err(table_error(context, &synced, &start, &reason));
    }
  }
  let target = target_branch_of(synced.report.upstream.as_ref()).unwrap_or_else(|| synced.baseline.clone());
  let github = github_repository(context, &synced.report)?;
  // on Space the robots refuse it as they refuse a Safe Push (one on its commit, one that lands it)
  let space = context.space_repository()?;
  let patronus = space.as_ref().and_then(|_| patronus_service(environment).ok()).map(Arc::new);
  let services = BdServices {
    patronus: patronus.clone(),
    space: space.clone(),
    github_repository: github.clone(),
    target: Some(target.clone()),
    ..BdServices::new(context, Arc::clone(&github_service))
  };
  let mut facts = SessionFacts::new(context, synced.table.clone(), table.clone(), services.sources());
  // the pull requests with the permission of the token: whether CI passed on the tip, the gate of the route; of a merge
  // the robots too, which answer off Space that Patronus lands nothing (`land::merges_on_github`)
  let robots = (patronus.is_some() || direct == Direct::Merge).then_some(FactNeed::Robots);
  let first: Vec<FactNeed> = robots.into_iter().chain([FactNeed::PullRequests]).collect();
  facts.read(context, &first)?;
  if direct == Direct::Merge {
    let session = &facts.session;
    let routed = landing_route_of(session.state().header(), &session.state().sections().landed, session.facts());
    if let LandingRoute::PullRequest {
      merge: RouteGate::Refused {
        reason: reason @ (Reason::GitHubMirror { .. } | Reason::NotOnGitHub),
      },
    } = &routed
    {
      return Err(CliError::new(
        ErrorCode::LandingRouteUnsupported,
        format!(
          "Nothing was pushed: the repository lands through the merge of its pull request ({}), and {}; push the branch (`{} push {}`) and merge its pull request where it lands, or ask the user to name another route",
          domain_types::landing_route::LANDING_CONFIG_KEY,
          start_refusal_text(reason, &args.branch, &[]),
          context.bd,
          shell_word(&args.branch)
        ),
      ));
    }
  }

  // what lands, in one trial: a removed declaration that another branch's commit still changes the use of, a name that a
  // branch outside it adds; the landing needs of a branch are its gate's
  let trial = session_of_sync(context, &synced.report)?;
  let landed_commits: HashSet<String> = landed_branches(&facts.session).values().flat_map(|landed| landed.landed.iter().cloned()).collect();
  let gate = check_landing_gate(&trial, &subject, &landed_commits).map_err(regroup_error)?;
  let gate_refusals = gate.refusals(&trial);
  if let Start::Branch(name) = &start {
    let needs = if gate_refusals.is_empty() {
      Needs::new()
    } else {
      Needs::from([(name.clone(), gate_refusals.clone())])
    };
    facts.session.landing_needs(|_| Ok(needs));
  }
  let host = LandHost {
    table: &table,
    baseline: &synced.landed_section,
    target: &target,
    github: github.is_some(),
    gate: &gate_refusals,
    // a push is its own check against the target: one that moved refuses it as no fast-forward
    target_conflict: None,
  };
  let land = Land {
    subject: subject.clone(),
    mode: PatronusPushMode::RebaseAutosquash,
    profile: ProfileChoice::Default,
    without_approvals: false,
    create_pull_request: false,
    allow: LandingAllowances {
      allow_dangling: args.allow_dangling,
      allow_missing: args.allow_missing,
      allow_without_checks: args.without_checks,
      bypass_rules: args.bypass_rules,
      ..LandingAllowances::default()
    },
  };
  // the plan of the app's start over what was read: the rules of the target it waits for are read next, then it is
  // decided again
  let planned = facts
    .prepare(context, &land, &host)?
    .ok_or_else(|| unknown_or_unapplied(context, subject.name(), UNAPPLIED_LAND))?;
  let plan = &planned.plan;
  let what = subject.words();
  let robots = facts.session.robots().to_vec();
  let mut output = LandOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: args.branch.clone(),
    route: direct.route().to_owned(),
    target: target.clone(),
    lands: plan.lands.clone(),
    whole_stack: whole.as_ref().map(|whole| whole_stack_summary(whole, &synced.report.stack_branches, &context.prefix)),
    commit: pushed_tip(plan).map(|tip| context.short_ids(std::iter::once(tip)).get(tip).to_string()),
    confirmations: plan.confirmations.iter().map(confirmation_words).collect(),
    next_step: None,
    refusals: Vec::new(),
    landed: false,
    pull_request: plan.pull_request.filter(|_| direct == Direct::Merge),
    auto_merge: false,
    closes: Vec::new(),
    next: None,
    safe_push: None,
    decided: Decided {
      plan: Some(plan.clone()),
      step: None,
    },
  };
  let refusals = refusals_of(context, plan, land.allow, &what, &robots, direct);
  if args.check {
    let read = CheckRead {
      table: &table,
      session: &facts.session,
      landed: &synced.landed_section,
      readiness: None,
      target: &target,
      route: &landing_route_of(facts.session.state().header(), &facts.session.state().sections().landed, facts.session.facts()),
    };
    (output.decided.step, output.next_step) = check_next_step(context, &read, &start, &what).unzip();
    output.refusals = refusals;
    return Ok(output);
  }
  if let Some(refusal) = refusals.into_iter().next() {
    return Err(refusal.next_steps.into_iter().fold(CliError::new(refusal.code, refusal.message), |error, step| {
      error.with_next_step(step.command, step.needs_approval)
    }));
  }
  // the auto-merge of the pull request is enabled already: GitHub merges it once its checks pass
  if direct == Direct::Merge && plan.effects.is_empty() {
    output.auto_merge = true;
    return Ok(output);
  }
  if !pushes_into_target(plan) {
    // another route since the sync read it: the plan starts a robot, which `bd land` of a push never does
    return Err(CliError::new(
      ErrorCode::PlanChanged,
      format!(
        "{}: the route of the repository changed since the sync; run `{} land {}` again",
        direct.nothing(),
        context.bd,
        shell_word(&args.branch)
      ),
    ));
  }

  let started = {
    let start = BdStart {
      session: &facts.session,
      land: Some(host),
      sources: facts.sources(),
      key: session_key(context),
      text: None,
    };
    let github = github.map(|repository| (Arc::clone(&github_service), repository));
    let space_name = space.map_or_default(|space| space.name);
    let services = start_services(context, patronus, space_name, github, target.clone(), &synced.baseline)?;
    // busy while another process runs what the push needs (a landing into the same target); the record leaves once the
    // run ended
    let entry = admit_planned(context, &services, &planned).map_err(CliError::from)?;
    let accepted = Accepted {
      fingerprint: plan.fingerprint.clone(),
      confirmations: plan.confirmations.clone(),
    };
    let steps = Steps::none().with_record(Some(entry.handle()));
    let execute = execute(&start, &services, &steps, entry.id().clone(), &land, &accepted, false);
    run_admitted(context, entry, execute).map_err(CliError::from)?
  };
  let update = format!("{} update", context.bd);
  for record in &started.outcome.effects {
    if let EffectStatus::Failed { error } = &record.status {
      let error = CliError::from(error.clone());
      return Err(match error.code {
        ErrorCode::BaselineMoved => error.with_next_step(update, true),
        _ => error,
      });
    }
  }
  output.landed = update_after(&started.outcome.effects).is_some();
  output.auto_merge = started
    .outcome
    .effects
    .iter()
    .any(|record| matches!(record.effect, PlannedEffect::EnableAutoMerge { .. }) && record.status == EffectStatus::Done);
  // the pull requests a merge closes the app's session tells by content once the listing shows it merged
  if output.landed && direct == Direct::Push {
    output.closes = landed_closes(&table, facts.session.facts().pull_requests.view(), &plan.lands, matches!(start, Start::Branch(_)));
  }
  // the update is the user's command: it moves the checked out branch and its files
  output.next = output.landed.then_some(update);
  Ok(output)
}

/// The open pull requests of `lands`, what a push landed, as the app's session tells them (`Landing::Push`): over
/// `table` and the listing as they were before the push; `chain` for a push of a virtual branch (a piece, the last branch
/// of a straight chain), whose history has the virtual branches it is built on. None while GitHub lists none
fn landed_closes(table: &BranchTable, pull_requests: &PullRequestsView, lands: &[String], chain: bool) -> Vec<ClosedPullRequest> {
  if !matches!(pull_requests.head.value, Some(PullRequestAvailability::Available { .. })) {
    return Vec::new();
  }
  pull_requests_to_close(&Landing::Push { branches: lands, chain }, |name| {
    table
      .branch(name)
      .map(|found| landed_subject(table, &table.remote, Of::Branch(&found.name), Some(pull_requests)))
  })
  .into_iter()
  .map(|close| ClosedPullRequest {
    branch: close.branch,
    number: close.number,
  })
  .collect()
}

/// What lands, and from which branch: the branch; of the whole stack, a stack that forks from its stack branch, a
/// straight chain from its last branch
fn start_of<'a>(args: &LandArgs, synced: &'a SyncOutput, whole: Option<&WholeStack<'_>>) -> Start<'a> {
  match whole {
    Some(WholeStack::Chain { leaf, .. }) => Start::Branch((*leaf).to_string()),
    Some(WholeStack::Forking { root }) => stack_branch_of(&synced.report.stack_branches, root).map_or_else(|| Start::Branch(args.branch.clone()), Start::WholeStack),
    _ => Start::Branch(args.branch.clone()),
  }
}

/// The commit the push of `plan` lands, or the head GitHub merges
fn pushed_tip(plan: &Plan) -> Option<&str> {
  plan.effects.iter().find_map(|effect| match effect {
    PlannedEffect::PushBaseline { tip, .. } => Some(tip.as_str()),
    PlannedEffect::MergePullRequest { head, .. } | PlannedEffect::EnableAutoMerge { head, .. } | PlannedEffect::MergeStack { head, .. } => Some(head.as_str()),
    _ => None,
  })
}

/// What refuses the push of `plan` of `what`, in order: its blockers, the facts it still waits for, then the
/// confirmations `allow` doesn't accept, each with its code and the flag that accepts it
fn refusals_of(context: &RepoContext, plan: &Plan, allow: LandingAllowances, what: &str, robots: &[domain_types::patronus::PatronusRobot], direct: Direct) -> Vec<Refusal> {
  let nothing = direct.nothing();
  let blockers = plan.blockers.iter().map(|reason| {
    let words = match reason {
      Reason::LandingRefused { refusal } => refusal.reason(),
      _ => start_refusal_text(reason, what, robots),
    };
    // a flag that lands anyway is the user's word, named in the message, never a step
    let words = match reason {
      Reason::LandingRefused { refusal } => match **refusal {
        LandingRefusal::DanglingReference { .. } => format!("{words}; --allow-dangling only when the user says it is a false alarm"),
        LandingRefusal::MissingDeclaration { .. } => format!("{words}; --allow-missing only when the user says it is a false alarm"),
      },
      _ => words,
    };
    let refusal = Refusal::new(refusal_code(reason), format!("{nothing}: {words}"));
    match reason {
      // a head behind its base: the update of the branch onto it, then the push of it
      Reason::LandedIn { .. } | Reason::BaselineAhead { .. } | Reason::PullRequestMerged { .. } | Reason::MergeBehind { .. } => {
        refusal.with_next_step(format!("{} update", context.bd), true)
      }
      Reason::MergeHeadBehind { .. } => refusal.with_next_step(format!("{} push {}", context.bd, shell_word(what)), true),
      _ => refusal,
    }
  });
  let waits = plan.waits.iter().map(|wait| {
    let fact = match wait.fact {
      LandFact::Permission => "what the token may do in the repository",
      LandFact::TargetRules => "the rules of the target",
      LandFact::PullRequests => "the pull requests",
      LandFact::Remote => "the copy the sync builds",
      LandFact::LandingNeeds => "what the build of what lands needs",
      LandFact::MergeState { .. } => "whether it can merge the pull request (it computes that once asked)",
      LandFact::Profiles | LandFact::Readiness | LandFact::Rules { .. } => "an answer of Patronus",
    };
    let (code, why) = wait
      .failed
      .as_ref()
      .map_or((ErrorCode::NetworkError, String::new()), |failed| (failed.code, format!(": {}", failed.message)));
    Refusal::new(code, format!("{nothing}: GitHub didn't answer {fact}{why}; run it again"))
  });
  let asked = unaccepted(&plan.confirmations, &allow).into_iter().map(|confirmation| {
    let (code, flag) = match confirmation {
      Confirmation::RouteRule { reason } => (refusal_code(reason), "--bypass-rules"),
      _ => (ErrorCode::ChecksNotPassed, "--without-checks"),
    };
    Refusal::new(
      code,
      format!("{nothing}: {}; ask the user, and only once they agree pass {flag}", confirmation_words(confirmation)),
    )
  });
  blockers.chain(waits).chain(asked).collect()
}

/// A confirmation of a push into the target in the words of `bd`, with the flag that accepts it
fn confirmation_words(confirmation: &Confirmation) -> String {
  match confirmation {
    Confirmation::LandsMoreThanRow { count } => format!("it lands {count} more branches it is built on with it"),
    Confirmation::Unchecked { why } => {
      let why = match why {
        Unchecked::NoPullRequest => "no open pull request ran CI on it".to_owned(),
        Unchecked::Unlisted => "GitHub lists no pull request of it, so what CI ran on it is not known".to_owned(),
        Unchecked::OtherHead { number } => format!("pull request #{number} shows another commit: its checks ran on that one"),
        Unchecked::Failing { number } => format!("the checks of pull request #{number} fail"),
        Unchecked::Running { number } => format!("the checks of pull request #{number} still run"),
      };
      format!("CI did not pass on the commit it pushes: {why} (--without-checks)")
    }
    Confirmation::ChangesRequested { number, by } if by.is_empty() => format!("changes are requested on pull request #{number} (--without-checks)"),
    Confirmation::ChangesRequested { number, by } => format!("{} requested changes on pull request #{number} (--without-checks)", by.join(", ")),
    Confirmation::RouteRule { reason } => format!("{} (--bypass-rules)", start_refusal_text(reason, "", &[])),
    Confirmation::ChecksNotRequired { number } => format!("checks of pull request #{number} that its base doesn't require fail or still run (--without-checks)"),
    Confirmation::SquashOnly { number } => {
      format!("the repository allows only squash merges: the commit GitHub writes of #{number} pairs with no original, so `bd update` may stop on a conflict")
    }
    // the confirmations of a Safe Push, never of a push into the target
    other => format!("{other:?}"),
  }
}
