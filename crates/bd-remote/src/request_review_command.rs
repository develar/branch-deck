//! `brd request-review`: ask GitHub users to review the open pull request of a virtual branch, as the app's Request
//! Review… does (`openspec/specs/branch/pull-requests/spec.md#request-review`): the same intent of the engine
//! (`Intent::RequestReview`, `operations::pull_request`), planned over the one-shot session of the command and run as
//! the app runs it. Without `--reviewer` it asks the code owners Patronus suggests that the app's popover preselects
//! (`domain_policy::review::candidates`, over the review readiness the session reads: the pre-check with the pull
//! request, asked again). Users only: a team is not asked. A branch whose review is a shown one (a review after landing,
//! `shown_command`) has its shown review opened or the fixes that landed since added to it instead, with or without
//! `--after-landing`, also once it left the table.
//!
//! The sync runs first, without the repository lock, so the command knows whether the branch has work its remote lacks: such work
//! refuses the request (`needs_push`), and `--push` pushes it first, the push of the app's request (the ancestors with
//! work first, the pull requests of what it pushes into a virtual branch retargeted before), so the reviewers see the
//! commits (D5). The writes run after the lock (the engine takes it again only to witness and freeze the push): the push,
//! then `POST pulls/<n>/requested_reviewers`.

use crate::push_command::{unknown_or_unapplied, unpushed_pull_request_draft};
use crate::safe_push_command::{UNKNOWN_GITHUB_REPOSITORY, compared_table, readiness_names};
use crate::safe_push_run::{BdStart, start_services};
use crate::session_facts::SessionFacts;
use crate::show_reviewers::show_reviewers;
use crate::shown_command::{ShownRead, read_shown, read_shown_of, run_recorded, state_name};
use bd_core::bd_command::shell_word;
use bd_core::context::{Environment, RepoContext};
use bd_core::error::CliError;
use bd_core::operation_record::{admit_planned, run_admitted};
use bd_core::output::SchemaVersion;
use bd_core::review_summary::PersonSummary;
use bd_core::services::{BdServices, github_service, patronus_service, session_key};
use bd_core::sync_command::sync;
use domain_policy::shown::{landed_by_content, pending_fixes};
use domain_types::branch_names::{review_branch_names, to_final_branch_name};
use domain_types::error::ErrorCode;
use domain_types::landing::{PlannedEffect, Subject};
use domain_types::operation::{EffectStatus, Intent};
use domain_types::patronus::PatronusRobot;
use domain_types::patronus::{PatronusReviewRequirement, ReviewReadiness, UnavailableReadinessReason};
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestState};
use domain_types::session::{Fact, SubjectReadiness};
use git_ops::revs::resolve_commit;
use operations::land::LandHost;
use operations::land_execute::{Accepted, Steps, refusal_of, run_planned};
use operations::needs::needs;
use operations::pull_request::one_shot_planned;
use operations::shown::{MarkDecision, MarkFacts, ShownLandings, ShownText, extend_plan, mark_plan, open_plan, shown_refusal};
use remote_facts::pull_requests::SIGNED_OUT_MESSAGE;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use sync_core::github_repository::resolve_github_repository_with;
use sync_core::patronus::no_target_message;

#[derive(Debug, Default, clap::Args)]
pub struct RequestReviewArgs {
  /// The branch whose pull request is reviewed
  pub branch: String,
  /// Ask this GitHub login (repeatable) instead of the suggested code owners; Patronus is not asked then
  #[arg(long = "reviewer", value_name = "LOGIN")]
  pub reviewers: Vec<String>,
  /// Push the commits the remote lacks first, so the reviewers see them; without it such commits refuse the request
  /// (`needs_push`)
  #[arg(long)]
  pub push: bool,
  /// Print whom it would ask and what it would push, and ask nobody
  #[arg(long, alias = "check")]
  pub dry_run: bool,
  /// The review after landing (show of ship/show/ask). Of a branch not in master yet, marked ahead of time: its draft
  /// opens now asking nobody, its body recording the reviewers (`--reviewer`, else one of each owner group of what it
  /// changes), and once anything lands the branch (its Safe Push, a child's or the whole stack's, a safe-merge from its
  /// pull request) it opens on what landed and asks them. Of a branch something landed: its draft opens, one open
  /// already takes the fixes of the branch that landed since, else asks again those it asked who have not approved; one
  /// that landed without a draft gets it now, ready, its reviewers asked. A branch with a shown review needs no flag
  #[arg(long, conflicts_with = "push")]
  pub after_landing: bool,
}

/// `brd request-review`
#[derive(Debug, Serialize, JsonSchema)]
pub struct RequestReviewOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub branch: String,
  pub pull_request: ReviewedPullRequest,
  /// The code-owner review before the request, as `brd safe-push` names it (`missing`, `approved`, `not_required`,
  /// `unknown`); `null` with `--reviewer`, which doesn't ask Patronus
  pub readiness: Option<String>,
  /// Whom it asks: the `--reviewer` logins, else one suggested code owner of each missing group nobody is asked for
  /// yet; with `--dry-run`, whom it would ask
  pub reviewers: Vec<PersonSummary>,
  /// The branches `--push` pushed first, root first; with `--dry-run`, what it would push; absent when the remote has
  /// them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub pushed: Vec<String>,
  /// Whom GitHub asked to review; empty with `--dry-run`
  pub requested: Vec<PersonSummary>,
  /// Whom GitHub refused (the author, a user without access to the repository), with its reason
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub refused: Vec<RefusedSummary>,
  /// The reviewers the pull request asks after the request: those asked before and those asked now
  pub waiting_for: Vec<PersonSummary>,
  /// The missing groups nobody is asked for whose suggested code owners Patronus names no GitHub login of: pass
  /// `--reviewer` for them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub groups_without_reviewer: Vec<String>,
  /// With `--after-landing`: the shown review it opened or asked again
  #[serde(skip_serializing_if = "Option::is_none")]
  pub shown: Option<ShownOpened>,
}

/// The shown review `--after-landing` opened, or created on a branch that landed without one
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ShownOpened {
  /// `draft` before, `in_review`, `changes_requested` or `approved`; `null` for one it creates
  pub state: Option<String>,
  /// It was marked now ahead of the landing: a draft that opens once anything landed the branch
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub marked: bool,
  /// The branch it shows and the one it goes into, their fork point
  pub head: String,
  pub base: String,
  /// The commit the Safe Push landed, which its head shows; `null` when it was not moved
  #[serde(skip_serializing_if = "Option::is_none")]
  pub landed: Option<String>,
  /// It was opened now (`--after-landing` of a branch that landed without one)
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub created: bool,
  /// The fixes of the branch that landed after it opened, which go onto its head
  #[serde(skip_serializing_if = "Option::is_none")]
  pub extended: Option<ShownExtended>,
}

/// The extend of a shown review by the fixes of its branch that landed after it opened
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ShownExtended {
  /// The merged Safe Pushes of the fixes, by robot id, oldest first
  pub fixes: Vec<String>,
  /// Its head now, the fixes that applied on top of its commits; `null` with `--dry-run`
  pub head: Option<String>,
  /// The fixes, by robot id, that don't apply on its commits (a commit of one that conflicts has no commit of master of
  /// its identity): a comment names each, its head stays where the fix found it, and the later fixes apply all the same
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub not_applied: Vec<String>,
  /// The fixes whose commits conflicted on its commits and took the files they conflict in as master has them after the
  /// fix: a comment names each with those files
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub took_files: Vec<ShownTookFiles>,
}

/// A fix whose commits took the files they conflict in from what landed them on master
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ShownTookFiles {
  /// The merged Safe Push of the fix
  pub robot: String,
  /// The files, as master has them after the fix
  pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReviewedPullRequest {
  pub number: u32,
  pub url: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RefusedSummary {
  pub login: String,
  pub full_name: Option<String>,
  /// What GitHub said
  pub reason: String,
}

/// An unapplied branch has no pull request to review: its commits are not in HEAD
const UNAPPLIED_REVIEW: &str = "nobody was asked to review: reapply it first (`brd reapply`)";

pub fn request_review(context: &RepoContext, args: &RequestReviewArgs, environment: &Environment) -> Result<RequestReviewOutput, CliError> {
  if args.after_landing {
    return request_after_landing(context, args, environment);
  }
  let name = args.branch.as_str();
  // the name must be one a virtual branch can have
  to_final_branch_name(&context.prefix, name).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))?;
  let reviewers = logins_of(&args.reviewers)?;
  // the push URL of origin is read once, for this and the Space repository of the review
  let configured = context.config()?.github_repository();
  let repository = resolve_github_repository_with(&context.git, &context.repository, configured, context.origin_url()?.as_deref())?
    .ok_or_else(|| CliError::new(ErrorCode::InvalidArguments, UNKNOWN_GITHUB_REPOSITORY))?;
  // Patronus names the code owners (without --reviewer) and the robots that hold the push (--push)
  let space = context.space_repository()?;
  let patronus = match (&space, reviewers.is_empty()) {
    (None, true) => {
      return Err(CliError::new(
        ErrorCode::NotOnSpace,
        format!("The repository is not on Space: Patronus can't name the code owners; pass --reviewer <login> to ask someone for {name}"),
      ));
    }
    (Some(_), true) => Some(Arc::new(patronus_service(environment)?)),
    (Some(_), false) if args.push => patronus_service(environment).ok().map(Arc::new),
    _ => None,
  };
  let github = Arc::new(github_service(context, environment));

  // the sync, without the lock (its refs move by CAS), says whether the branch has work its remote lacks
  let synced = sync(context)?;
  if !synced.branches.iter().any(|branch| branch.name == name) {
    // a branch that landed and left the table keeps its shown review: its open or extend
    let read = read_shown_of(
      context,
      environment,
      name,
      &Intent::OpenShownReview {
        branch: name.to_owned(),
        reviewers: Vec::new(),
      },
      synced,
    )?;
    if read.review.is_none() {
      return Err(unknown_or_unapplied(context, name, UNAPPLIED_REVIEW));
    }
    return shown_review_request(context, args, environment, read);
  }
  let target = synced
    .report
    .target_branch
    .clone()
    .ok_or_else(|| CliError::new(ErrorCode::InvalidArguments, no_target_message(&synced.baseline, synced.report.github.as_ref())))?;
  let table = compared_table(context, &synced);
  let services = BdServices {
    patronus: patronus.clone(),
    space: space.clone(),
    github_repository: Some(repository.clone()),
    target: Some(target.clone()),
    ..BdServices::new(context, Arc::clone(&github))
  };
  let subject = Subject::branch(name);
  let intent = Intent::RequestReview {
    subject: subject.clone(),
    reviewers: reviewers.clone(),
    push_first: args.push,
  };
  // what the plan reads, as the app's session reads it (`operations::needs`): the pull requests, the robots that hold
  // the push with --push, the review of the branch without --reviewer
  let mut facts = SessionFacts::new(context, synced.table.clone(), table.clone(), services.sources());
  facts.read(context, &needs(&intent))?;
  if args.push {
    facts.derive();
  }
  let session_facts = facts.session.facts();
  let listed = session_facts.pull_requests.view();
  require_listing(listed)?;
  let open = listed.pull_requests.get(name).filter(|pull_request| pull_request.state == PullRequestState::Open).cloned();
  let Some(pull_request) = open else {
    if !listed.shown_reviews.contains_key(name) {
      return Err(no_pull_request(context, name));
    }
    // its review is the shown one: its open or extend, and its reviewers asked again
    let read = read_shown_of(
      context,
      environment,
      name,
      &Intent::OpenShownReview {
        branch: name.to_owned(),
        reviewers: Vec::new(),
      },
      synced,
    )?;
    return shown_review_request(context, args, environment, read);
  };
  let review = if reviewers.is_empty() {
    Some(review_of(session_facts.readiness.fact(&subject.key()))?)
  } else {
    None
  };
  let host = LandHost {
    table: &table,
    baseline: &synced.landed_section,
    target: &target,
    github: true,
    gate: &[],
    target_conflict: None,
  };
  let planned = one_shot_planned(&facts.session, &host, &intent).ok_or_else(|| no_pull_request(context, name))?;

  let full_name_of = |login: &str| review.as_ref().and_then(|(requirement, _)| review_full_name(requirement, login));
  let summary = |login: &str| PersonSummary {
    login: Some(login.to_string()),
    full_name: full_name_of(login),
  };
  let asked: Vec<String> = planned
    .plan
    .effects
    .iter()
    .find_map(|effect| match effect {
      PlannedEffect::RequestReviewers { reviewers, .. } => Some(reviewers.clone()),
      _ => None,
    })
    .unwrap_or_default();
  let pushes: Vec<String> = planned
    .plan
    .effects
    .iter()
    .find_map(|effect| match effect {
      PlannedEffect::Push { branches, .. } => Some(branches.clone()),
      _ => None,
    })
    .unwrap_or_default();
  let mut output = RequestReviewOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: args.branch.clone(),
    pull_request: ReviewedPullRequest {
      number: pull_request.number,
      url: pull_request.url.clone(),
    },
    readiness: review.as_ref().map(|(_, readiness)| readiness_names(readiness).0.to_string()),
    reviewers: asked.iter().map(|login| summary(login)).collect(),
    pushed: pushes.clone(),
    requested: Vec::new(),
    refused: Vec::new(),
    waiting_for: Vec::new(),
    groups_without_reviewer: review.as_ref().map_or_default(|(_, readiness)| without_reviewer(readiness, &pull_request)),
    shown: None,
  };
  if let Some(error) = refusal_of(&planned, facts.session.robots()) {
    let refused = CliError::from(error);
    // the push is the user's to approve: the reviewers see what it pushes
    return Err(if refused.code == ErrorCode::NeedsPush {
      refused.with_next_step(format!("{} request-review {} --push", context.bd, shell_word(name)), true)
    } else {
      refused
    });
  }
  let mut waiting: Vec<String> = pull_request
    .reviewers
    .iter()
    .filter(|reviewer| reviewer.requested)
    .map(|reviewer| reviewer.login.clone())
    .collect();
  if args.dry_run || (asked.is_empty() && pushes.is_empty()) {
    output.waiting_for = waiting.iter().map(|login| summary(login)).collect();
    return Ok(output);
  }

  let start = BdStart {
    session: &facts.session,
    land: None,
    sources: facts.sources(),
    key: session_key(context),
    text: None,
  };
  let space = space.map_or_default(|space| space.name);
  let run = start_services(context, patronus, space, Some((Arc::clone(&github), repository)), target.clone(), &synced.baseline)?;
  // busy while another process runs what the request needs; the record leaves once the run ended
  let entry = admit_planned(context, &run, &planned)?;
  let accepted = Accepted {
    fingerprint: planned.plan.fingerprint.clone(),
    confirmations: Vec::new(),
  };
  let steps = Steps::none().with_record(Some(entry.handle()));
  let request = run_planned(&start, &run, &steps, entry.id().clone(), planned, &accepted, false);
  let started = run_admitted(context, entry, request).map_err(CliError::from)?;
  // the request got to its end unless its last effect did not (a retarget GitHub refused of a branch the push takes
  // along stops nothing): then the first failure says why
  let effects = &started.outcome.effects;
  if effects.last().is_some_and(|last| last.status != EffectStatus::Done) {
    let stopped = effects.iter().find_map(|record| match &record.status {
      EffectStatus::Failed { error } | EffectStatus::Skipped { reason: Some(error) } => Some(error.clone()),
      _ => None,
    });
    return Err(stopped.map_or_else(
      || CliError::new(ErrorCode::NetworkError, "GitHub did not answer the request: see the pull request"),
      CliError::from,
    ));
  }
  if let Some(requested) = started.requested {
    output.refused = requested
      .refused
      .iter()
      .map(|refused| RefusedSummary {
        login: refused.login.clone(),
        full_name: full_name_of(&refused.login),
        reason: refused.reason.clone(),
      })
      .collect();
    for login in &requested.requested {
      if !waiting.iter().any(|it| it.eq_ignore_ascii_case(login)) {
        waiting.push(login.clone());
      }
    }
    output.requested = requested.requested.iter().map(|login| summary(login)).collect();
  }
  output.waiting_for = waiting.iter().map(|login| summary(login)).collect();
  Ok(output)
}

/// The `--reviewer` logins, trimmed, each once
pub(crate) fn logins_of(reviewers: &[String]) -> Result<Vec<String>, CliError> {
  let mut logins: Vec<String> = Vec::new();
  for login in reviewers.iter().map(|login| login.trim()).filter(|login| !login.is_empty()) {
    if !logins.iter().any(|it| it.eq_ignore_ascii_case(login)) {
      logins.push(login.to_string());
    }
  }
  if logins.is_empty() && !reviewers.is_empty() {
    return Err(CliError::new(ErrorCode::InvalidArguments, "--reviewer needs a GitHub login"));
  }
  Ok(logins)
}

/// "auth has no open pull request: …"
fn no_pull_request(context: &RepoContext, name: &str) -> CliError {
  CliError::new(
    ErrorCode::NoPullRequest,
    format!(
      "{name} has no open pull request: `{bd} safe-push {branch} --create-pull-request` pushes the branch and opens it, with `--reviewer`; a branch a Safe Push landed is reviewed after landing: `{bd} request-review {branch} --after-landing`",
      bd = context.bd,
      branch = shell_word(name)
    ),
  )
}

/// `brd request-review <branch> --after-landing` (`RequestReviewArgs::after_landing`): the sync, then the shown review
/// of the branch (`shown_review_request`), its mark ahead of time read as the Safe Push with it is refused
fn request_after_landing(context: &RepoContext, args: &RequestReviewArgs, environment: &Environment) -> Result<RequestReviewOutput, CliError> {
  let name = args.branch.as_str();
  let read = read_shown(
    context,
    environment,
    name,
    &Intent::MarkShownReview {
      branch: name.to_owned(),
      reviewers: logins_of(&args.reviewers)?,
    },
  )?;
  shown_review_request(context, args, environment, read)
}

/// The request of the review after landing of the branch of `args` over what `read` read, recorded in the operations
/// registry as the app's run holds it: its draft opens once something landed it since it opened
/// (`domain_policy::shown::landed_head`, `operations::shown::open_plan`), the fixes that landed since it opened go onto
/// it (`extend_plan`), else it asks again those it asked who neither approved nor are asked now; `--reviewer` adds
/// reviewers to it. Without one (`operations::shown::mark_plan`), a branch that landed gets it now, ready on what landed;
/// one the table has that did not land yet is marked ahead of time, refused as the Safe Push with its review is
fn shown_review_request(context: &RepoContext, args: &RequestReviewArgs, environment: &Environment, mut read: ShownRead) -> Result<RequestReviewOutput, CliError> {
  let name = args.branch.as_str();
  let asked_logins = logins_of(&args.reviewers)?;
  let (head, base) = review_branch_names(&context.prefix, name).map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("{error:#}")))?;
  let robots: Vec<&PatronusRobot> = read.robots.iter().collect();
  let content = if read.robots_known {
    landed_by_content(&read.synced.landed_section)
  } else {
    BTreeMap::new()
  };
  let landings = ShownLandings {
    table: Some(&read.table),
    landed: &content,
    pull_requests: &read.pull_requests,
    target: &read.target,
    robots: &robots,
  };
  let (plan, state, created) = if let Some(review) = &read.review {
    let fixes = pending_fixes(review, name, robots.iter().copied());
    let plan = if fixes.is_empty() {
      open_plan(review, name, landings.landed_of(Some(review), name).as_ref(), robots.iter().copied(), &asked_logins)
    } else {
      extend_plan(review, name, &fixes, &asked_logins)
    };
    (plan.map_err(CliError::from)?, Some(state_name(&review.state).to_owned()), false)
  } else {
    let mut landed = landings.landed_of(None, name);
    let tip = read.table.tip_of(name).map(str::to_owned);
    // a branch that left the table: its remote branch tells the commit that landed
    if let Some(landed) = landed.as_mut().filter(|landed| landed.head.is_none()) {
      let remote = format!(
        "refs/remotes/origin/{}",
        to_final_branch_name(&context.prefix, name).map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("{error:#}")))?
      );
      landed.head = resolve_commit(&context.git, &context.repository, &remote)?;
    }
    let source = landed.as_ref().and_then(|landed| landed.head.clone()).or_else(|| tip.clone()).ok_or_else(|| {
        CliError::new(
          ErrorCode::NoShownReview,
          format!("Nothing landed {name}, and the table has no such branch: a review after landing is of a branch the table has or one that landed; a branch not in master yet is reviewed in its pull request (`{} request-review {}`)", context.bd, shell_word(name)),
        )
      })?;
    let reviewers = if asked_logins.is_empty() {
      let target_ref = format!("refs/remotes/{}", read.synced.baseline);
      let (_, candidates) = show_reviewers(context, environment, &read.services.github, &read.services.repository, &source, &target_ref)?;
      if candidates.reviewers.is_empty() {
        return Err(CliError::new(
          ErrorCode::NoReviewer,
          format!(
            "Nobody to ask for the review after landing of {name}: nobody of the owner groups of what it changed is available with a GitHub login; ask the user whom, then --reviewer <login>"
          ),
        ));
      }
      candidates.reviewers
    } else {
      asked_logins
    };
    // the text of the branch while the table has it, else its name
    let draft = read
      .synced
      .branches
      .iter()
      .any(|branch| branch.name == name)
      .then(|| unpushed_pull_request_draft(context, &read.synced, name, None).ok())
      .flatten();
    read.services.text = Some(ShownText {
      title: draft.as_ref().map_or_else(|| name.to_owned(), |draft| draft.title.clone()),
      body: draft.and_then(|draft| draft.body).unwrap_or_default(),
      assign_self: true,
      target: read.target.clone(),
    });
    let open = read.pull_requests.get(name).is_some_and(|pull_request| pull_request.state == PullRequestState::Open);
    let mark = MarkFacts {
      branch: name,
      reviewers: &reviewers,
      review: None,
      landed: landed.as_ref(),
      tip: tip.as_deref(),
      decision: read.mark.as_ref().map(|(blockers, waits)| MarkDecision {
        blockers,
        waits,
        pull_request_open: open,
      }),
    };
    let plan = mark_plan(&mark).map_err(CliError::from)?;
    if let Some(refusal) = shown_refusal(&plan, name) {
      return Err(CliError::from(refusal));
    }
    (plan, None, true)
  };
  let marked = plan.effects.iter().any(|effect| matches!(effect, PlannedEffect::CreateShownReview { draft: true, .. }));
  let asked: Vec<String> = plan
    .effects
    .iter()
    .find_map(|effect| match effect {
      PlannedEffect::RequestReviewers { reviewers, .. } | PlannedEffect::CreateShownReview { reviewers, .. } => Some(reviewers.clone()),
      _ => None,
    })
    .unwrap_or_default();
  let landed = plan.effects.iter().find_map(|effect| match effect {
    PlannedEffect::PushReview { tip, .. } => Some(tip.clone()),
    _ => None,
  });
  let extended = plan.effects.iter().find_map(|effect| match effect {
    PlannedEffect::ExtendReview { fixes, .. } => Some(ShownExtended {
      fixes: fixes.iter().map(|fix| fix.robot.clone()).collect(),
      head: None,
      not_applied: Vec::new(),
      took_files: Vec::new(),
    }),
    _ => None,
  });
  let person = |login: &str| PersonSummary {
    login: Some(login.to_owned()),
    full_name: None,
  };
  let (number, url) = read
    .review
    .as_ref()
    .map_or((0, String::new()), |review| (review.pull_request.number, review.pull_request.url.clone()));
  let mut output = RequestReviewOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: args.branch.clone(),
    pull_request: ReviewedPullRequest { number, url },
    readiness: None,
    reviewers: asked.iter().map(|login| person(login)).collect(),
    pushed: Vec::new(),
    requested: Vec::new(),
    refused: Vec::new(),
    waiting_for: Vec::new(),
    groups_without_reviewer: Vec::new(),
    shown: Some(ShownOpened {
      state,
      marked,
      head,
      base,
      landed,
      created,
      extended,
    }),
  };
  if args.dry_run || plan.effects.is_empty() {
    return Ok(output);
  }
  let run = run_recorded(context, &read.services, &plan)?;
  if let (Some(shown), Some(extension)) = (output.shown.as_mut(), &run.extended)
    && let Some(extended) = shown.extended.as_mut()
  {
    extended.head = Some(extension.head.clone());
    extended.not_applied = run.not_applied.iter().map(|fix| fix.robot.clone()).collect();
    extended.took_files = run
      .taken
      .iter()
      .map(|taken| ShownTookFiles {
        robot: taken.fix.robot.clone(),
        files: taken.files.clone(),
      })
      .collect();
  }
  if let Some(created) = &run.created {
    output.pull_request = ReviewedPullRequest {
      number: created.pull_request.number,
      url: created.pull_request.url.clone(),
    };
  }
  let requested = run.requested.map_or_else(
    || {
      run.created.map_or_default(|created| {
        created
          .pull_request
          .reviewers
          .iter()
          .filter(|reviewer| reviewer.requested)
          .map(|reviewer| reviewer.login.clone())
          .collect()
      })
    },
    |requested| {
      output.refused = requested
        .refused
        .iter()
        .map(|refused| RefusedSummary {
          login: refused.login.clone(),
          full_name: None,
          reason: refused.reason.clone(),
        })
        .collect();
      requested.requested
    },
  );
  output.requested = requested.iter().map(|login| person(login)).collect();
  output.waiting_for.clone_from(&output.requested);
  Ok(output)
}

/// The code-owner review of the branch the session read (the pre-check with the pull request, asked again): a pre-check
/// that failed is an error, a Patronus that doesn't let the user in `not_logged_in`
fn review_of(fact: Option<&Fact<SubjectReadiness>>) -> Result<(PatronusReviewRequirement, ReviewReadiness), CliError> {
  let answer = fact.and_then(|fact| fact.value.as_ref());
  match answer.map(|answer| (answer.requirement.clone(), &answer.readiness)) {
    Some((Some(requirement), readiness)) => Ok((requirement, readiness.clone())),
    Some((
      None,
      ReviewReadiness::Unavailable {
        reason: UnavailableReadinessReason::NotLoggedIn,
        message,
      },
    )) => Err(CliError::new(ErrorCode::NotLoggedIn, message.clone())),
    Some((None, ReviewReadiness::Unavailable { message, .. })) => Err(CliError::new(ErrorCode::PatronusError, message.clone())),
    _ => Err(CliError::new(ErrorCode::PatronusError, "Patronus gave no answer for the review")),
  }
}

/// The missing groups whose suggested code owners have no GitHub login to ask (`domain_policy::review::candidates`)
fn without_reviewer(readiness: &ReviewReadiness, pull_request: &PullRequest) -> Vec<String> {
  domain_policy::review::candidates(Some(readiness), pull_request).without_reviewer
}

/// The full name Patronus knows for a GitHub login
fn review_full_name(requirement: &PatronusReviewRequirement, login: &str) -> Option<String> {
  requirement
    .groups
    .iter()
    .flat_map(|group| &group.reviewers)
    .find(|member| member.github_login.as_deref().is_some_and(|it| it.eq_ignore_ascii_case(login)))
    .map(|member| member.full_name.clone())
    .filter(|name| !name.is_empty())
}

/// The listing of the pull requests stands, else why GitHub can't list them (`availability_error`)
pub(crate) fn require_listing(listed: &domain_types::session::PullRequestsView) -> Result<(), CliError> {
  match listed.head.value.as_ref() {
    Some(PullRequestAvailability::Available { .. }) => Ok(()),
    other => Err(availability_error(other.unwrap_or(&PullRequestAvailability::SignedOut))),
  }
}

/// Why the pull requests of the branches can't be read: `not_logged_in` without a token GitHub accepts
pub(crate) fn availability_error(availability: &PullRequestAvailability) -> CliError {
  match availability {
    PullRequestAvailability::SignedOut => CliError::new(ErrorCode::NotLoggedIn, SIGNED_OUT_MESSAGE),
    PullRequestAvailability::NotAuthenticated { source } => CliError::new(
      ErrorCode::NotLoggedIn,
      format!(
        "GitHub rejected the token from {}: tell the user to sign in to GitHub again (in the app, or gh auth login), or to set a valid GH_TOKEN",
        source.label()
      ),
    ),
    PullRequestAvailability::NotGitHub => CliError::new(ErrorCode::InvalidArguments, UNKNOWN_GITHUB_REPOSITORY),
    PullRequestAvailability::RateLimited { .. } => CliError::new(ErrorCode::NetworkError, "GitHub refused the request for its rate limit: try again later"),
    PullRequestAvailability::SsoRequired { url } => CliError::new(
      ErrorCode::NotLoggedIn,
      match url {
        Some(url) => format!("The organization requires SAML SSO: authorize the token at {url}"),
        None => "The organization requires SAML SSO: tell the user to authorize the token for the organization on github.com, or to sign in to GitHub again".to_string(),
      },
    ),
    PullRequestAvailability::NoAccess { message, .. } | PullRequestAvailability::Error { message } => CliError::new(ErrorCode::NetworkError, message.clone()),
    PullRequestAvailability::Available { .. } => CliError::new(ErrorCode::NetworkError, "GitHub listed the pull requests"),
  }
}
