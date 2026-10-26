//! The writes of pull requests an operation makes (docs/design/landing.md: every pull request of a virtual branch goes
//! into the target): the retarget of an open pull request into the target (`PlannedEffect::Retarget`,
//! `domain_policy::pull_request::base_of`), the first effect of a start (D11) and of a push (D3a), before the push of its
//! branch, so GitHub never compares the pushed commits with a parent's branch the pull request no longer goes into.
//!
//! The retarget the user asks of a pull request (`Intent::Retarget`: row 5 of the next step, its "⋯" for another base,
//! the Safe Push form) is an operation of its own: planned over the pull requests of the session (`retarget_plan`: an
//! open one whose base is not the target), it holds the base of the pull request (`PullRequestField::Base`), so a review
//! request of the same pull request runs beside it; its `PATCH` goes into the session at once
//! (`PullRequestWrite::Retargeted`, newer than every listing that started before).
//!
//! The creation of the pull request of a subject (`Intent::CreatePullRequest`, D6) and a review request of its open one
//! (`Intent::RequestReview`, D5) push first what the remote lacks of it (`push_step`: the push of the push button,
//! `derive::action_push_of`, the stack branch of a whole stack; the retargets of the pull requests it pushes into a
//! virtual branch before it), then write: the creation (GitHub waits for a mirror right after the push; a pull request
//! it already has of the head is found, `created: false`), "Continue on GitHub" the wait until GitHub has the pushed
//! branch (`WaitForMirror`), the request of the reviewers. They run as a start runs (`land_execute::run_planned`): the
//! witness and the lock only for the freeze of the push, the writes after it; what they wrote goes into the session.
//! The creation holds the head of its subject (`Resource::Ref`), so a second one waits and is decided again; a review
//! request the reviewers of its pull request (`PullRequestField::Reviewers`), so a retarget of it runs beside, unless its
//! push retargets the pull request too, which holds its base.

use crate::engine::{outcome_of, run, run_planned_in};
use crate::events::EventSink;
use crate::land::{LandHost, with_one_shot_sources};
use crate::land_execute::{Accepted, Planned, PushedRef};
use crate::land_session::pull_request_start;
use crate::registry::{Answer, OperationRequest, Registry};
use crate::robot_start::github_failure;
use crate::witness::{Witness, check};
use domain_policy::pull_request::base_of;
use domain_policy::reasons::stack_not_built;
use domain_policy::review::{Candidates, candidates};
use domain_types::branch_names::{baseline_on_remote, to_final_branch_name};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{PlannedEffect, Subject};
use domain_types::operation::{
  CreatePullRequest, EffectRecord, EffectStatus, Fingerprint, Intent, OperationId, OperationResult, OperationStep, Outcome, Plan, PullRequestField, Resource, RunningMove,
};
use domain_types::patronus::{PatronusPushMode, PatronusRobot};
use domain_types::pull_request::{PullRequest, PullRequestBase, PullRequestState};
use domain_types::reason::Reason;
use domain_types::session::PullRequestsView;
use github::service::GitHubService;
use remote_facts::patronus_command::UNKNOWN_GITHUB_REPOSITORY_MESSAGE;
use remote_facts::pull_request_writes::retarget_pull_request;
use remote_facts::pull_requests::GitHubWriteError;
use remote_facts::service_sources::ServiceHost;
use repository_model::derive::{ActionPushView, LandSources, action_push_of, with_pull_request_sources};
use repository_model::one_shot::OneShot;
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::session::{SessionKey, Sessions};
use std::collections::BTreeMap;
use std::sync::Arc;
use sync_core::github_repository::GitHubRepository;
use tracing::instrument;

/// Moves the pull request `number` into `base` (`PATCH pulls/<n>`)
///
/// # Errors
/// What GitHub refused or failed with
#[instrument(skip(github, repository))]
pub async fn retarget(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, number: u32, base: &str) -> Result<(), GitHubWriteError> {
  retarget_pull_request(github, repository_path, repository, number, base).await
}

/// Each of `retargets` (the number of a pull request and its new base), one `PATCH` each, in their order, with how each
/// went: one GitHub refuses stops none of the others
pub async fn retarget_each(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, retargets: &[(u32, &str)]) -> Vec<Result<(), GitHubWriteError>> {
  let mut answers = Vec::with_capacity(retargets.len());
  for (number, base) in retargets {
    answers.push(retarget(github, repository_path, repository, *number, base).await);
  }
  answers
}

/// The pull request `number` among `pull_requests` (of a virtual branch, or of the stack branch of a whole stack) with
/// the branch or the root it is of
pub(crate) fn find(pull_requests: &PullRequestsView, number: u32) -> Option<(&str, &PullRequest)> {
  pull_requests
    .pull_requests
    .iter()
    .chain(&pull_requests.stack_pull_requests)
    .find(|(_, pull_request)| pull_request.number == number)
    .map(|(name, pull_request)| (name.as_str(), pull_request))
}

/// The plan of the retarget of the pull request `number` into `target` ("master") over `pull_requests`, the listing of
/// the session: `[Retarget]` of an open one into another base, nothing of one into the target already; a merged one is
/// refused; `None` for one the listing lacks or a closed one. Its fingerprint holds the pull request as listed
#[must_use]
pub fn retarget_plan(pull_requests: &PullRequestsView, number: u32, target: &str) -> Option<Plan> {
  let (branch, pull_request) = find(pull_requests, number)?;
  let mut blockers = Vec::new();
  let mut effects = Vec::new();
  match pull_request.state {
    PullRequestState::Closed => return None,
    PullRequestState::Merged => blockers.push(Reason::PullRequestMerged { number }),
    PullRequestState::Open if pull_request.base_ref_name != target => effects.push(PlannedEffect::Retarget {
      branch: branch.to_string(),
      number,
      from: pull_request.base_ref_name.clone(),
      to: target.to_string(),
    }),
    PullRequestState::Open => {}
  }
  Some(write_plan(Intent::Retarget { number }, effects, blockers, Some(number), [pull_request], []))
}

/// A plan of writes of pull requests that pushes nothing (a retarget, closes, a review), its fingerprint over the pull
/// requests and the robots it read; `pull_request` the one it is of
pub(crate) fn write_plan<'a>(
  intent: Intent,
  effects: Vec<PlannedEffect>,
  blockers: Vec<Reason>,
  pull_request: Option<u32>,
  pull_requests: impl IntoIterator<Item = &'a PullRequest>,
  robots: impl IntoIterator<Item = &'a PatronusRobot>,
) -> Plan {
  let mut plan = Plan {
    intent,
    fingerprint: Fingerprint(String::new()),
    lands: Vec::new(),
    effects,
    confirmations: Vec::new(),
    blockers,
    waits: Vec::new(),
    alternatives: None,
    pull_request,
    approvals_skipped_by: None,
  };
  plan.fingerprint = Witness::of(BTreeMap::new(), pull_requests, robots).fingerprint(&plan);
  plan
}

/// The plan of the retarget of the pull request `number` in the open session of `key`, into the branch of origin its
/// baseline tracks (`retarget_plan`)
pub async fn prepare_retarget(sessions: &Sessions, key: &SessionKey, number: u32) -> Option<Plan> {
  sessions
    .read(key, move |state, facts| {
      // before the first sync names the baseline, nothing names the target
      let target = baseline_on_remote(&state.header().baseline_branch).to_string();
      if target.is_empty() {
        return None;
      }
      retarget_plan(facts.pull_requests.view(), number, &target)
    })
    .await
    .flatten()
}

/// What a retarget asks the registry for: the base of the pull request, which a review request of it does not hold
pub(crate) fn retarget_request(number: u32) -> OperationRequest {
  OperationRequest {
    intent: Intent::Retarget { number },
    resources: vec![Resource::PullRequest {
      number,
      field: PullRequestField::Base,
    }],
    step: OperationStep::Retargeting,
    moves: Vec::new(),
  }
}

/// Runs the retarget of the pull request `number` in the session of `key` on the plan its caller accepted: admitted
/// over the operations of the session (it holds the base of the pull request: a review request of it runs beside, a
/// second retarget joins it), planned again, then the `PATCH` with the GitHub service and repository `services`
/// resolves; the new base into the session. A `PATCH` GitHub refused is the status of its effect in the outcome
///
/// # Errors
/// A pull request the session lacks, a changed plan, a refusal (a merged one), no GitHub repository of the branches
#[instrument(skip_all, fields(number))]
pub async fn execute_retarget(registry: &Registry, key: &SessionKey, number: u32, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  let sessions = registry.sessions();
  run(registry, key, retarget_request(number), events, async |guard| {
    let began = std::time::Instant::now();
    let plan = prepare_retarget(sessions, key, number).await.ok_or_else(|| {
      ApiError::new(
        ErrorCode::NoPullRequest,
        format!("Nothing was retargeted: the session lists no open pull request #{number}"),
      )
    })?;
    if let Some(Reason::PullRequestMerged { .. }) = plan.blockers.first() {
      return Err(ApiError::new(ErrorCode::Landed, format!("Nothing was retargeted: pull request #{number} is merged")));
    }
    check(&accepted.fingerprint, &plan.fingerprint)?;
    let mut version = None;
    let mut records = Vec::with_capacity(plan.effects.len());
    for effect in plan.effects.clone() {
      let status = match &effect {
        PlannedEffect::Retarget { number, to, .. } => {
          let written = write_retarget(services, key, *number, to).await;
          match written {
            Ok(()) => {
              let write = PullRequestWrite::Retargeted {
                number: *number,
                base: to.clone(),
              };
              version = sessions.write_pull_request(key, write).await.or(version);
              EffectStatus::Done
            }
            Err(error) => EffectStatus::Failed { error },
          }
        }
        _ => EffectStatus::Skipped { reason: None },
      };
      records.push(EffectRecord { effect, status });
    }
    Ok(retarget_outcome(guard.id().clone(), records, version, began))
  })
  .await
}

/// The GitHub service of the session of `key` and the GitHub repository its branches are mirrored to
///
/// # Errors
/// No GitHub repository of the branches, a host without GitHub
pub(crate) async fn github_of(services: &dyn ServiceHost, key: &SessionKey) -> Result<(Arc<GitHubService>, GitHubRepository), ApiError> {
  let repository = services
    .github_repository(key)
    .await?
    .ok_or_else(|| ApiError::new(ErrorCode::InvalidArguments, UNKNOWN_GITHUB_REPOSITORY_MESSAGE))?;
  Ok((services.github(key)?, repository))
}

/// The `PATCH` of the retarget with the GitHub service and repository of the session of `key`
async fn write_retarget(services: &dyn ServiceHost, key: &SessionKey, number: u32, base: &str) -> Result<(), ApiError> {
  let (github, repository) = github_of(services, key).await?;
  retarget(&github, &key.worktree, &repository, number, base).await.map_err(|error| {
    let failure = github_failure(error);
    ApiError::new(failure.code, format!("Cannot retarget pull request #{number} to {base}: {}", failure.message))
  })
}

/// The outcome of a retarget: the retarget of the user's own pull request is no event of an object with a toast of its own
fn retarget_outcome(op: OperationId, effects: Vec<EffectRecord>, version: Option<domain_types::session::Version>, began: std::time::Instant) -> Outcome {
  outcome_of(op, effects, None, version, OperationResult::None, began)
}

/// What the plan of a pull request or a review request reads: the sources of the session, and the push the branch of
/// its subject needs first (`derive::action_push_of`, in the order of the push button; a whole stack pushes its stack
/// branch, which no other branch goes with)
pub struct PullRequestFacts<'s, 'a> {
  pub sources: &'s LandSources<'a>,
  pub push: ActionPushView,
}

/// The push a plan of a subject makes first, when its remote lacks work of it: the retargets of the pull requests of the
/// branches it pushes that go into a virtual branch (D3a), then the push
struct PushStep<'a> {
  /// The branch locally and on the remote, without `refs/heads/`: the head of its pull request
  head: String,
  /// `Retarget`s, then `Push`; empty when the remote has it all
  effects: Vec<PlannedEffect>,
  pushes: Vec<PushedRef>,
  /// The tips of the refs the push moves and leases
  refs: BTreeMap<String, Option<String>>,
  /// The pull requests it retargets, as listed
  retargeted: Vec<&'a PullRequest>,
  /// Why the push the remote needs can't be made for the user (`ActionPushView::blocker`)
  blocker: Option<Reason>,
}

/// The push `facts` say `subject` needs first (`PushStep`); `None` for a subject the table lacks, or a whole stack
/// that doesn't fork
fn push_step<'a>(facts: &PullRequestFacts<'_, 'a>, subject: &Subject) -> Option<PushStep<'a>> {
  let sources = facts.sources;
  let full = |branch: &str| to_final_branch_name(sources.prefix, branch).ok();
  let mut refs = BTreeMap::new();
  let mut lease = |full_name: &str, tip: Option<String>, remote: Option<String>| {
    refs.insert(format!("refs/heads/{full_name}"), tip);
    refs.insert(format!("refs/remotes/origin/{full_name}"), remote);
  };
  match subject {
    Subject::Branch { name } => {
      sources.table.branch(name)?;
      let head = full(name)?;
      let mut pushes = Vec::with_capacity(facts.push.branches.len());
      for branch in &facts.push.branches {
        let full_name = full(branch)?;
        let base = sources
          .table
          .branch(branch)
          .and_then(|info| info.stack.as_ref())
          .and_then(|stack| stack.effective_base.as_deref());
        let remote = sources.remote.branches.get(branch).and_then(|remote| remote.remote_tip.clone());
        lease(&full_name, sources.table.tips.get(&full_name).cloned(), remote);
        pushes.push(PushedRef {
          name: branch.clone(),
          full_name,
          parent_ref: base.and_then(full),
          whole_stack: false,
          unapplied: false,
        });
      }
      // the pull requests of what the push sends go into the target first, where every pull request of a virtual branch
      // goes (D3a): GitHub never compares the pushed commits with a branch the pull request no longer goes into
      let retargets: Vec<(&String, &PullRequest)> = facts
        .push
        .branches
        .iter()
        .filter_map(|branch| {
          let pull_request = sources
            .pull_requests
            .pull_requests
            .get(branch)
            .filter(|pull_request| pull_request.state == PullRequestState::Open)?;
          matches!(base_of(pull_request, sources.target, sources.prefix), PullRequestBase::Virtual { .. }).then_some((branch, pull_request))
        })
        .collect();
      let mut effects: Vec<PlannedEffect> = retargets
        .iter()
        .map(|(branch, pull_request)| PlannedEffect::Retarget {
          branch: (*branch).clone(),
          number: pull_request.number,
          from: pull_request.base_ref_name.clone(),
          to: sources.target.to_string(),
        })
        .collect();
      let retargeted = retargets.into_iter().map(|(_, pull_request)| pull_request).collect();
      if !pushes.is_empty() {
        effects.push(PlannedEffect::Push {
          branches: facts.push.branches.clone(),
          force: false,
        });
      }
      Some(PushStep {
        head,
        blocker: facts.push.blocker.clone(),
        effects,
        pushes,
        refs,
        retargeted,
      })
    }
    Subject::WholeStack { root } => {
      let stack = sources.table.stack(root)?;
      let remote = sources.remote.stacks.get(root);
      let pushes_it = remote.is_none_or(|remote| !remote.known || !remote.up_to_date);
      // a stack branch that is no build of its commits is no pull request of them, pushed or not
      let mut blocker = stack_not_built(stack);
      let mut effects = Vec::new();
      let mut pushes = Vec::new();
      if pushes_it {
        let held = sources.view.stacks.get(root).and_then(|facts| facts.landing.hold.clone());
        let drops = remote.filter(|remote| remote.drops > 0).map(|remote| Reason::DropsRemote {
          branch: root.clone(),
          behind: remote.drops,
        });
        blocker = blocker.or(held).or(drops);
        lease(&stack.branch_name, stack.tip.clone(), remote.and_then(|remote| remote.remote_tip.clone()));
        pushes.push(PushedRef {
          name: root.clone(),
          full_name: stack.branch_name.clone(),
          parent_ref: None,
          whole_stack: true,
          unapplied: false,
        });
        effects.push(PlannedEffect::Push {
          branches: vec![root.clone()],
          force: false,
        });
      }
      Some(PushStep {
        head: stack.branch_name.clone(),
        effects,
        pushes,
        refs,
        retargeted: Vec::new(),
        blocker,
      })
    }
  }
}

/// A plan of `intent` of `subject`, with what its run needs
fn planned(intent: Intent, subject: &Subject, first: PushStep<'_>, mut effects: Vec<PlannedEffect>, blockers: Vec<Reason>, pull_requests: &[&PullRequest]) -> Planned {
  let mut all = first.effects;
  all.append(&mut effects);
  let number = match &intent {
    Intent::RequestReview { .. } => pull_requests.first().map(|pull_request| pull_request.number),
    _ => None,
  };
  let witness = Witness::of(first.refs, pull_requests.iter().copied().chain(first.retargeted), std::iter::empty::<&PatronusRobot>());
  let mut plan = Plan {
    intent,
    fingerprint: Fingerprint(String::new()),
    lands: Vec::new(),
    effects: all,
    confirmations: Vec::new(),
    blockers,
    waits: Vec::new(),
    alternatives: None,
    pull_request: number,
    approvals_skipped_by: None,
  };
  plan.fingerprint = witness.fingerprint(&plan);
  Planned {
    plan,
    witness,
    pushes: first.pushes,
    source_ref: format!("refs/heads/{}", first.head),
    merge_request: None,
    notes: Vec::new(),
    subject: subject.clone(),
    // no robot: the mode is of none
    mode: PatronusPushMode::RebaseAutosquash,
  }
}

/// The plan of the creation of the pull request of `create.subject` over `facts`: its push first while its remote lacks
/// work of it (`push_step`: the retargets, the push), then the creation (`PlannedEffect::CreatePullRequest`), or with
/// `on_github` the wait until GitHub has the pushed branch (`PlannedEffect::WaitForMirror`, none without a push: GitHub
/// has what the remote has). A pull request GitHub already has of the head is found by the creation (`created: false`),
/// never refused here: the listing may lag. `None` for a subject the table lacks
#[must_use]
pub fn create_planned(facts: &PullRequestFacts<'_, '_>, create: &CreatePullRequest) -> Option<Planned> {
  let first = push_step(facts, &create.subject)?;
  let blockers: Vec<Reason> = first.blocker.iter().cloned().collect();
  let pushes = !first.pushes.is_empty();
  let then = match (create.on_github, pushes) {
    (false, _) => vec![PlannedEffect::CreatePullRequest],
    (true, true) => vec![PlannedEffect::WaitForMirror { branch: first.head.clone() }],
    (true, false) => Vec::new(),
  };
  Some(planned(Intent::CreatePullRequest(create.clone()), &create.subject, first, then, blockers, &[]))
}

/// The open pull request of `subject` (of its branch, or of the stack branch of its whole stack) among `facts`
fn open_pull_request_of<'a>(facts: &PullRequestFacts<'_, 'a>, subject: &Subject) -> Option<&'a PullRequest> {
  let pull_requests = facts.sources.pull_requests;
  match subject {
    Subject::Branch { name } => pull_requests.pull_requests.get(name),
    Subject::WholeStack { root } => pull_requests.stack_pull_requests.get(root),
  }
  .filter(|pull_request| pull_request.state == PullRequestState::Open)
}

/// The plan of the review request of the open pull request of `subject` over `facts`, asking `reviewers` (none: the
/// code owners Request Review… preselects, `domain_policy::review::candidates` over the readiness of the subject): its
/// push first while the remote lacks work of the subject, so the reviewers see the commits (D5), only when `push_first`
/// says so (else `NeedsPush` refuses it), then the request (`PlannedEffect::RequestReviewers`; none when each code owner
/// of a missing group is asked already). A draft refuses it (a draft asks nobody), and so does a missing group whose code
/// owners have no GitHub login (`NoReviewer`). `None` for a subject without an open pull request
#[must_use]
pub fn review_planned(facts: &PullRequestFacts<'_, '_>, subject: &Subject, reviewers: &[String], push_first: bool) -> Option<Planned> {
  let pull_request = open_pull_request_of(facts, subject)?;
  let first = push_step(facts, subject)?;
  let mut blockers = Vec::new();
  if pull_request.is_draft {
    blockers.push(Reason::Draft { number: pull_request.number });
  }
  let (asked, without_reviewer) = if reviewers.is_empty() {
    let readiness = (facts.sources.readiness)(&subject.key())
      .and_then(|fact| fact.value.as_ref())
      .map(|answer| &answer.readiness);
    let Candidates { candidates, without_reviewer } = candidates(readiness, pull_request);
    let chosen = candidates.into_iter().filter(|candidate| candidate.preselected).map(|candidate| candidate.login).collect();
    (chosen, without_reviewer)
  } else {
    (unique_reviewers(reviewers), Vec::new())
  };
  // a missing group whose code owners have no login to ask is for the user to name; one asked already waits for them
  if asked.is_empty() && !without_reviewer.is_empty() {
    blockers.push(Reason::NoReviewer { groups: without_reviewer });
  }
  // a push the user decides (it drops remote commits) or that can't be made is said as such, `--push` or not
  if let Some(blocker) = &first.blocker {
    blockers.push(blocker.clone());
  } else if !first.pushes.is_empty() && !push_first {
    blockers.push(Reason::NeedsPush {
      branch: subject.name().to_string(),
    });
  }
  let intent = Intent::RequestReview {
    subject: subject.clone(),
    reviewers: reviewers.to_vec(),
    push_first,
  };
  // nobody left to ask (each code owner of a missing group is asked already): the request asks nobody, its push still
  // goes
  let request = (!asked.is_empty()).then_some(PlannedEffect::RequestReviewers {
    number: pull_request.number,
    reviewers: asked,
  });
  Some(planned(intent, subject, first, request.into_iter().collect(), blockers, &[pull_request]))
}

/// `logins` trimmed, each once (GitHub logins ignore the case), in their order
fn unique_reviewers(logins: &[String]) -> Vec<String> {
  let mut unique: Vec<String> = Vec::new();
  for login in logins.iter().map(|login| login.trim()).filter(|login| !login.is_empty()) {
    if !unique.iter().any(|known| known.eq_ignore_ascii_case(login)) {
      unique.push(login.to_string());
    }
  }
  unique
}

/// The plan of the pull request or review request `intent` in the open session of `key`, over the facts it holds now
/// (`create_planned`, `review_planned`); `None` for a session that is not open, before its first table and before the
/// first sync names the target, for a subject its table lacks, or one without an open pull request to review
pub async fn prepare_planned(sessions: &Sessions, key: &SessionKey, intent: &Intent) -> Option<Planned> {
  let (prefix, intent) = (key.prefix.clone(), intent.clone());
  sessions
    .read(key, move |state, facts| {
      with_pull_request_sources(state, facts, &prefix, |sources| {
        if sources.target.is_empty() {
          return None;
        }
        planned_of(sources, |name| action_push_of(state, facts, name), &intent)
      })
      .flatten()
    })
    .await
    .flatten()
}

/// The plan of `intent` over `sources`, `push` the push the branch of its subject needs first; `None` for another
/// intent
pub fn planned_of(sources: &LandSources<'_>, push: impl FnOnce(&str) -> ActionPushView, intent: &Intent) -> Option<Planned> {
  let subject = match intent {
    Intent::CreatePullRequest(create) => &create.subject,
    Intent::RequestReview { subject, .. } => subject,
    _ => return None,
  };
  let facts = PullRequestFacts {
    sources,
    push: match subject {
      Subject::Branch { name } => push(name),
      Subject::WholeStack { .. } => ActionPushView::default(),
    },
  };
  match intent {
    Intent::CreatePullRequest(create) => create_planned(&facts, create),
    Intent::RequestReview { subject, reviewers, push_first } => review_planned(&facts, subject, reviewers, *push_first),
    _ => None,
  }
}

/// The plan of the pull request or review request `intent` over the one-shot session of a command (`bd request-review`)
/// as it is now: no read; `None` as `prepare_planned`
#[must_use]
pub fn one_shot_planned(session: &OneShot, host: &LandHost<'_>, intent: &Intent) -> Option<Planned> {
  with_one_shot_sources(session, host, |sources| {
    planned_of(sources, |name| action_push_of(session.state(), session.facts(), name), intent)
  })
}

/// What a pull request intent asks the registry for: the head of its subject (a creation of it, or a push of it, waits
/// for the one before and is decided again), or the reviewers of the pull request a review request writes (a retarget
/// of it runs beside), with the refs its push writes and the bases it retargets (of `planned`, its plan as the session
/// has it now); the move of the row of its subject from the click on
pub(crate) fn pull_request_request(intent: &Intent, planned: Option<&Planned>) -> OperationRequest {
  let mut resources = Vec::new();
  let mut moves = Vec::new();
  if let Some(planned) = planned {
    let subject = planned.subject.key();
    if let Some(number) = planned.plan.pull_request {
      resources.push(Resource::PullRequest {
        number,
        field: PullRequestField::Reviewers,
      });
      moves.push(RunningMove::RequestingReview { subject, number });
    } else {
      resources.push(Resource::Ref {
        name: planned.source_ref.trim_start_matches("refs/heads/").to_string(),
      });
      moves.push(RunningMove::CreatingPullRequest { subject });
    }
    for pushed in &planned.pushes {
      let name = Resource::Ref { name: pushed.full_name.clone() };
      if !resources.contains(&name) {
        resources.push(name);
      }
    }
    for effect in &planned.plan.effects {
      if let PlannedEffect::Retarget { number, .. } = effect {
        resources.push(Resource::PullRequest {
          number: *number,
          field: PullRequestField::Base,
        });
      }
    }
  }
  OperationRequest {
    intent: intent.clone(),
    resources,
    step: OperationStep::Checking,
    moves,
  }
}

/// The intent has no plan in the session: "Nothing was pushed or created: the last sync has no auth"
fn no_plan(intent: &Intent) -> ApiError {
  match intent {
    Intent::RequestReview { subject, .. } => ApiError::new(
      ErrorCode::NoPullRequest,
      format!("Nothing was pushed or requested: {} has no open pull request", subject.words()),
    ),
    Intent::CreatePullRequest(create) => ApiError::new(
      ErrorCode::UnknownBranch,
      format!("Nothing was pushed or created: the last sync has no {}", create.subject.words()),
    ),
    _ => ApiError::new(ErrorCode::InvalidArguments, "No pull request intent"),
  }
}

/// Runs the creation of a pull request or a review request (`intent`) in the session of `key` on the plan its caller
/// accepted: admitted over the operations of the session with what its plan as the session has it now holds (a second
/// click of the same joins it), then planned again and run as a start runs (`land_execute::run_planned`: the witness,
/// the lock only for the freeze of its push, the retargets and the push after it, then its last effect) over the
/// services of `services` (`pull_request_start`: no Patronus needed), its steps in the view and its outcome to
/// `events`; the pull request it opened and the reviewers it asked go into the session
///
/// # Errors
/// What refuses it before its first effect, a subject the session has no plan of, a session that closed, no GitHub
/// repository of the branches
#[instrument(skip_all)]
pub async fn execute_pull_request(registry: &Registry, key: &SessionKey, intent: Intent, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  let sessions = registry.sessions();
  let planned = prepare_planned(sessions, key, &intent).await;
  let request = pull_request_request(&intent, planned.as_ref());
  run(registry, key, request, events, async |guard| {
    let began = std::time::Instant::now();
    // decided again once admitted: an operation it waited for changed what it decides on
    let planned = prepare_planned(sessions, key, &intent).await.ok_or_else(|| no_plan(&intent))?;
    run_planned_in(guard, pull_request_start(services, sessions, key).await?, planned, &accepted, began).await
  })
  .await
}

#[cfg(test)]
#[path = "pull_request_test.rs"]
mod pull_request_test;
