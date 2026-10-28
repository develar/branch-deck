//! The frame every family of intents runs in (docs/design/rust-domain-authority.md, "Планирование и исполнение
//! операций", "Наблюдаемость"): `run` admits the operation in its session through the registry, runs the body of its
//! family once it is admitted and ends it with the body's answer, or answers what the operation of the same intent it
//! joined answered. The caller hears the outcome either way (`OperationEvent::Done`). The admission, the body and the
//! end run inside the root span of the operation (`operation{op operation_id tool subject}`, its id the one of its
//! record, its outcome and its entry in the journal), the body counting what it costs into the tally of the operation,
//! and an error leaves with the id. `prepare` asks the family of an intent for its plan,
//! and `execute` runs it on the plan its caller accepted: a start (`execute_land`, `land_execute::execute`), a restart
//! and a cancel of a robot (`robot`), a retarget of a pull request, the creation of one and a review request
//! (`pull_request`), the closes (`close`), a review (`review`), the state of an issue (`issue`), a push (`push`: of a
//! branch alone, of a stack, up to a commit, of an unapplied branch, of the stack branch of a whole stack), a rewrite
//! (`rewrite`), the fetch of the baseline, Update Project and the dismissal of what landed (`update`) and the fetch of
//! a review (`review_fetch`). A start
//! reserves what its plan lands (`Resource::Lands`), the refs
//! it pushes and the robot it cancels: a second start of the same subject joins it, one that lands a part of it waits for
//! it and is decided again. An operation that waited in the queue runs on the plan the ones it waited for left, while
//! the user accepted what it asks (`Accepted::decided_again`): a push and the Safe Push pressed right after it are one
//! push and one start. A start of a repository that lands by a push into the target reserves the remote-tracking ref of
//! the target on the remote its plan names (`refs/remotes/origin/master`: two landings into it wait for each other, a
//! fetch of it waits for the push) and runs without Patronus; once its push landed, Update Project with its fetch
//! follows as an operation of its own (`domain_policy::land_direct::update_after`), admitted as any update, its events
//! to the caller of the start: the start holds no HEAD in its network part.

use crate::events::EventSink;
use crate::land_execute::{self, Accepted, Planned, StartServices, Steps};
use crate::land_session::{SessionStart, pull_request_start, session_start};
use crate::registry::{Admitted, Answer, OperationRequest, Registry, RunningGuard};
use crate::rewrite::{RewriteHost, execute_rewrite, prepare_rewrite, rewrite_request};
use crate::witness::{Witness, check};
use crate::{auto_merge, close, forget, issue, land, pull_request, push, review, review_fetch, robot, update};
use domain_policy::land_direct::update_after;
use domain_types::branch_names::{to_final_branch_name, to_stack_branch_name};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{Land, PlannedEffect, Subject};
use domain_types::operation::{
  EffectRecord, Intent, NewsKey, OperationEvent, OperationId, OperationResult, OperationStats, OperationStep, Outcome, Plan, PullRequestField, Resource, RunningMove,
};
use domain_types::rewrite::RewriteIntent;
use domain_types::session::Version;
use remote_facts::service_sources::ServiceHost;
use repository_model::session::{SessionKey, Sessions};
use std::sync::Arc;
use sync_core::transfer_progress::TransferProgress;
use tracing::Instrument;

/// What the operations of a host run with: the services of its reads and of its effects on the remote, and what its
/// rewrites run with besides (the app's and the E2E server's services are both)
#[derive(Clone, Copy)]
pub struct Hosts<'a> {
  pub services: &'a dyn ServiceHost,
  pub rewrites: &'a dyn RewriteHost,
}

/// Runs `request` in the session of `key` as a new operation, inside its root span: `body` once the registry admitted
/// it (at once, or after the operations it waits for), counting what it costs, or the answer of the operation of the
/// same intent it joined; `events` hears its steps and its outcome. An error of its own names it (`operation_id`)
pub async fn run(registry: &Registry, key: &SessionKey, request: OperationRequest, events: Arc<dyn EventSink>, body: impl AsyncFnOnce(&RunningGuard) -> Answer) -> Answer {
  let id = sync_utils::operation_id::OperationId::new();
  let op = OperationId(id.to_string());
  let span = id.operation_span(&words(&request.intent.kind()), &words(&registry.tool()), request.intent.subject().as_deref());
  let named = |answer: Answer| {
    answer.map_err(|mut error| {
      error.operation_id.get_or_insert_with(|| op.0.clone());
      error
    })
  };
  async {
    match registry.admit(key, op.clone(), request, Arc::clone(&events)).await {
      Err(error) => named(Err(error)),
      Ok(Admitted::Run(guard)) => {
        let answer = named(guard.tally().counting(body(&guard)).await);
        guard.finish(answer).await
      }
      Ok(Admitted::Joined(joined)) => {
        let answer = joined.answer().await;
        if let Ok(outcome) = &answer {
          events.send(OperationEvent::Done {
            outcome: Box::new(outcome.clone()),
          });
        }
        answer
      }
    }
  }
  .instrument(span)
  .await
}

/// The name serde gives `value`, a unit variant: `safe_push`, `app`
fn words(value: &impl serde::Serialize) -> String {
  serde_json::to_value(value).ok().and_then(|value| value.as_str().map(str::to_owned)).unwrap_or_default()
}

/// The plan of `intent` in the open session of `key` (the `prepare` of its family): a read, no lock, no effect; `None`
/// for a session that is not open or has no answer of it yet (a start before the Patronus view, a subject its table
/// lacks, a robot it offers no restart of, a pull request it lists not, a review request of a subject without an open
/// pull request)
///
/// # Errors
/// `invalid_arguments` for an invalid review
pub async fn prepare(sessions: &Sessions, key: &SessionKey, intent: Intent) -> Result<Option<Plan>, ApiError> {
  match intent {
    Intent::Land(start) => Ok(land::prepare(sessions, key, start).await),
    Intent::RestartRobot { robot_id } => Ok(robot::prepare_restart(sessions, key, &robot_id).await.map(|planned| planned.plan)),
    Intent::CancelRobot { robot_id } => Ok(robot::prepare_cancel(sessions, key, &robot_id).await.map(|(plan, _)| plan)),
    Intent::Retarget { number } => Ok(pull_request::prepare_retarget(sessions, key, number).await),
    Intent::CreatePullRequest(_) | Intent::RequestReview { .. } => Ok(pull_request::prepare_planned(sessions, key, &intent).await.map(|planned| planned.plan)),
    Intent::ClosePullRequest { .. } | Intent::CloseLandedPullRequests { .. } => Ok(close::prepare_close(sessions, key, &intent).await),
    Intent::DisableAutoMerge { number } => Ok(auto_merge::prepare_disable(sessions, key, number).await),
    Intent::SubmitReview { .. } => review::review_plan(&intent).map(Some),
    Intent::SetIssueState { .. } => issue::issue_plan(&intent).map(Some),
    Intent::Push(push) => Ok(push::prepare_push(sessions, key, &push).await.map(|planned| planned.plan)),
    Intent::ForgetStart { .. } => forget::forget_plan(&intent).map(Some),
    Intent::Rewrite(rewrite) => Ok(prepare_rewrite(sessions, key, *rewrite).await),
    intent @ (Intent::FetchBaseline | Intent::UpdateProject { .. } | Intent::DismissLanded) => Ok(update::prepare_update(sessions, key, intent).await),
    Intent::FetchReview { number } => Ok(review_fetch::prepare_review_fetch(sessions, key, number).await),
    // only `bd` runs it: no plan the app could run
    Intent::Pick { .. } => Ok(None),
  }
}

/// Runs `intent` in the session of `key` on the plan its caller accepted (`accepted`: the fingerprint of the plan it
/// showed, the confirmations the user accepted of it), over the services `services` resolves for the session (the
/// app's and the E2E server's `Hosts`): admitted over the operations of the session, planned again, then its
/// effects; its steps in the view, its progress and its outcome to `events`. A rewrite asks nothing to accept: the table
/// it names its commits on is in its intent (`rewrite::execute_rewrite`)
///
/// # Errors
/// What refused it before its first effect, a session that is not open; `invalid_arguments` for an intent only `bd` runs
/// (a pick, a regroup of a plan, a move of a hunk: `runs_only_in_bd`)
pub async fn execute(registry: &Registry, key: &SessionKey, intent: Intent, accepted: Accepted, hosts: Hosts<'_>, events: Arc<dyn EventSink>) -> Answer {
  let services = hosts.services;
  match intent {
    // the run of a start is a large future: on the heap
    Intent::Land(land) => Box::pin(execute_land(registry, key, land, accepted, hosts, events)).await,
    Intent::RestartRobot { robot_id } => Box::pin(robot::execute_restart(registry, key, &robot_id, accepted, services, events)).await,
    Intent::CancelRobot { robot_id } => robot::execute_cancel(registry, key, &robot_id, accepted, services, events).await,
    Intent::Retarget { number } => pull_request::execute_retarget(registry, key, number, accepted, services, events).await,
    intent @ (Intent::CreatePullRequest(_) | Intent::RequestReview { .. }) => Box::pin(pull_request::execute_pull_request(registry, key, intent, accepted, services, events)).await,
    intent @ (Intent::ClosePullRequest { .. } | Intent::CloseLandedPullRequests { .. }) => close::execute_close(registry, key, intent, accepted, services, events).await,
    Intent::DisableAutoMerge { number } => auto_merge::execute_disable(registry, key, number, accepted, services, events).await,
    intent @ Intent::SubmitReview { .. } => Box::pin(review::execute_review(registry, key, intent, accepted, services, events)).await,
    intent @ Intent::SetIssueState { .. } => issue::execute_issue(registry, key, intent, accepted, services, events).await,
    Intent::Push(push) => Box::pin(push::execute_push(registry, key, push, accepted, services, events)).await,
    intent @ Intent::ForgetStart { .. } => forget::execute_forget(registry, key, intent, accepted, events).await,
    // the run of a rewrite is a large future: on the heap
    Intent::Rewrite(rewrite) => Box::pin(execute_rewrite(registry, key, *rewrite, hosts, events)).await,
    Intent::FetchBaseline => Box::pin(update::execute_fetch(registry, key, hosts, events)).await,
    Intent::UpdateProject { fetch } => Box::pin(update::execute_update(registry, key, fetch, hosts, events)).await,
    Intent::DismissLanded => update::execute_dismiss(registry, key, hosts, events).await,
    Intent::FetchReview { number } => review_fetch::execute_review_fetch(registry, key, number, hosts, events).await,
    intent @ Intent::Pick { .. } => Err(runs_only_in_bd(&intent)),
  }
}

/// Whether `plan`, decided again once the operation of `guard` was admitted, is the plan its caller accepted, or after a
/// wait in the queue the one the operations it waited for left (`Accepted::decided_again`)
///
/// # Errors
/// `plan_changed` of its intent
pub(crate) fn check_admitted(guard: &RunningGuard, accepted: &Accepted, plan: &Plan) -> Result<(), ApiError> {
  check(&accepted.decided_again(plan, guard.queued()).fingerprint, &plan.fingerprint, &plan.intent)
}

/// Where the steps of the operation of `guard` go: the view and its caller, the progress of its push to the caller, and
/// whether the user stopped it
pub(crate) fn steps_of(guard: &RunningGuard) -> Steps {
  let stepper = guard.stepper();
  let progress = guard.stepper();
  Steps::new(
    Arc::new(move |step| stepper.step(step)),
    Arc::new(move |tick: &TransferProgress| progress.progress(tick)),
    guard.canceled(),
  )
  .with_record(guard.record())
}

/// Runs `planned`, decided again once its operation `guard` was admitted, on `start` (the host and the services of the
/// session, `session_start` or `pull_request_start`) as a start runs (`land_execute::run_planned`), its steps in the view:
/// on the plan the wait left after it `waited` (in the queue, or for the sync of a push), on the one `accepted` names
/// else (`Accepted::decided_again`); answers its outcome
pub(crate) async fn run_planned_in(guard: &RunningGuard, start: (SessionStart, StartServices), planned: Planned, accepted: &Accepted, waited: bool) -> Answer {
  let (host, services) = start;
  let steps = steps_of(guard);
  let accepted = accepted.decided_again(&planned.plan, waited);
  // the run of a plan is a large future: on the heap
  let started = Box::pin(land_execute::run_planned(&host, &services, &steps, guard.id().clone(), planned, &accepted)).await?;
  Ok(started.outcome)
}

/// Runs the start `land` of the app in the session of `key` on the plan its caller accepted (`accepted`):
/// admitted over the operations of the session with what the plan as the session has it now reserves, then run
/// (`land_execute::execute`) over the session and the services of `hosts` (`session_start`; a push into the target,
/// which starts no robot, `pull_request_start`) with its steps in the view and its progress and outcome to `events`, on
/// what the operations it queued behind left (the push of its branch pressed just before it: the start pushes nothing
/// more); stopped by `Registry::cancel`. Once a push into the target landed, Update Project follows as an operation of
/// its own (`land_direct::update_after`), its steps in the view and, after the start's `Done`, its events and its own
/// `Done` to `events`; the answer is the start's, after the update: an update refused before its first effect (a lock
/// another process holds) sends no `Done` of its own
///
/// # Errors
/// What refused the start before its first effect (`land_execute::execute`), a lock another process holds, a session
/// that closed or has no sync yet
pub async fn execute_land(registry: &Registry, key: &SessionKey, land: Land, accepted: Accepted, hosts: Hosts<'_>, events: Arc<dyn EventSink>) -> Answer {
  let services = hosts.services;
  let planned = land::prepare(registry.sessions(), key, land.clone()).await;
  let request = land_request(&key.prefix, &land, planned.as_ref());
  let pushes_into_target = planned.as_ref().is_some_and(pushes_into_target);
  let answer = run(registry, key, request, Arc::clone(&events), async |guard| {
    let (host, run) = if pushes_into_target {
      pull_request_start(services, registry.sessions(), key).await?
    } else {
      session_start(services, registry.sessions(), key).await?
    };
    let steps = steps_of(guard);
    let started = land_execute::execute(&host, &run, &steps, guard.id().clone(), &land, &accepted, guard.queued()).await?;
    Ok(started.outcome)
  })
  .await;
  if let Ok(outcome) = &answer
    && let Some(Intent::UpdateProject { fetch }) = update_after(&outcome.effects)
  {
    // the caller hears its steps, the progress of its fetch and its outcome after the start's `Done`, on the same
    // channel (the toast of the push says how the update went); the answer is the start's
    drop(Box::pin(update::execute_update(registry, key, fetch, hosts, events)).await);
  }
  answer
}

/// The plan pushes the copy of its subject into the target, or GitHub merges its pull request: no robot of Patronus
/// starts (`bd land` too)
#[must_use]
pub fn pushes_into_target(plan: &Plan) -> bool {
  plan.effects.iter().any(|effect| {
    matches!(
      effect,
      PlannedEffect::PushBaseline { .. } | PlannedEffect::MergePullRequest { .. } | PlannedEffect::EnableAutoMerge { .. }
    )
  })
}

/// What the start `land` of a session of `prefix` asks the registry for: what its plan (as the session has it now)
/// reserves, the move of its row from the click on
fn land_request(prefix: &str, land: &Land, plan: Option<&Plan>) -> OperationRequest {
  OperationRequest {
    intent: Intent::Land(land.clone()),
    resources: plan.map_or_default(|plan| reserved(prefix, &land.subject, plan)),
    step: OperationStep::Checking,
    moves: vec![RunningMove::Starting {
      subject: land.subject.name().to_string(),
      mode: land.mode,
    }],
  }
}

/// What the run of `planned` in a session of `prefix` asks the registry for, by the family of its intent, as the app's
/// admission of that intent asks it: `bd` records the run of a plan it made with it in the operations registry of the
/// repository (`registry_file`), so one plan holds the same resources in the app and in `bd`
#[must_use]
pub fn request_of(prefix: &str, planned: &Planned) -> OperationRequest {
  let plan = &planned.plan;
  match &plan.intent {
    Intent::Land(land) => land_request(prefix, land, Some(plan)),
    Intent::Push(push) => push::push_request(prefix, push, Some(planned)),
    Intent::RestartRobot { robot_id } => robot::restart_request(robot_id, Some(planned)),
    Intent::CancelRobot { robot_id } => robot::cancel_request(robot_id),
    Intent::Retarget { number } => pull_request::retarget_request(*number),
    intent @ (Intent::CreatePullRequest(_) | Intent::RequestReview { .. }) => pull_request::pull_request_request(intent, Some(planned)),
    intent @ (Intent::ClosePullRequest { .. } | Intent::CloseLandedPullRequests { .. }) => close::close_request(intent, Some(plan)),
    Intent::DisableAutoMerge { number } => auto_merge::disable_request(*number),
    intent @ Intent::SubmitReview { number, .. } => review::review_request(intent, *number),
    intent @ Intent::SetIssueState { key, .. } => issue::issue_request(intent, key),
    intent @ Intent::ForgetStart { .. } => forget::forget_request(intent),
    Intent::Rewrite(rewrite) => rewrite_request(prefix, rewrite),
    intent @ (Intent::FetchBaseline | Intent::UpdateProject { .. } | Intent::DismissLanded) => update::update_request(intent, update::planned_upstream(plan)),
    Intent::FetchReview { number } => review_fetch::review_fetch_request(*number),
    intent @ Intent::Pick { .. } => pick_request(intent),
  }
}

/// What `intent`, of a family whose request names no plan, asks the registry for in a session of `prefix`, as the
/// app's admission of it asks: `bd` records the run of its rewrites, its update and its pick with it in the operations
/// registry of the repository (`registry_file`), which the view and the journal show; `None` for an intent whose
/// resources its plan names (a start, a push, an action on the remote: `request_of`)
#[must_use]
pub fn unplanned_request(prefix: &str, intent: &Intent) -> Option<OperationRequest> {
  match intent {
    Intent::Rewrite(rewrite) => Some(rewrite_request(prefix, rewrite)),
    Intent::UpdateProject { .. } | Intent::DismissLanded => Some(update::update_request(intent, "")),
    Intent::Pick { .. } => Some(pick_request(intent)),
    _ => None,
  }
}

/// What the pick `intent` of `bd` holds: HEAD, which it moves under the repository lock
fn pick_request(intent: &Intent) -> OperationRequest {
  OperationRequest {
    intent: intent.clone(),
    resources: vec![Resource::Head],
    step: OperationStep::Rewriting,
    moves: Vec::new(),
  }
}

/// The refusal of `intent` that only `bd` runs (a pick, a regroup of a plan, a move of a hunk): its record in the
/// operations registry is the view's and the journal's, the engine of the app has no run of it
pub(crate) fn runs_only_in_bd(intent: &Intent) -> ApiError {
  let what = match intent {
    Intent::Rewrite(rewrite) if matches!(**rewrite, RewriteIntent::Group { .. }) => "the regroup of a plan (`bd group`)",
    Intent::Rewrite(_) => "the move of a hunk (`bd move-hunk`)",
    _ => "a pick of commits (`bd pick`)",
  };
  ApiError::new(ErrorCode::InvalidArguments, format!("Only bd runs {what}: the app shows its record and runs none"))
}

/// What a start of `subject` reserves: the branches its plan lands, the refs its push writes, the robot it cancels
fn reserved(prefix: &str, subject: &Subject, plan: &Plan) -> Vec<Resource> {
  let mut resources: Vec<Resource> = plan.lands.iter().map(|branch| Resource::Lands { branch: branch.clone() }).collect();
  for effect in &plan.effects {
    match effect {
      PlannedEffect::Push { branches, .. } => {
        let refs = match subject {
          Subject::WholeStack { root } => to_stack_branch_name(prefix, root).into_iter().collect::<Vec<_>>(),
          Subject::Branch { .. } => branches.iter().filter_map(|branch| to_final_branch_name(prefix, branch).ok()).collect(),
        };
        resources.extend(refs.into_iter().map(|name| Resource::Ref { name }));
      }
      PlannedEffect::CancelRobot { robot_id } => resources.push(Resource::Robot { id: robot_id.clone() }),
      // the remote-tracking ref of the target: a fetch of it, and another landing into it, wait for the push
      PlannedEffect::PushBaseline { remote, target, .. } => resources.push(Resource::Ref {
        name: format!("refs/remotes/{remote}/{target}"),
      }),
      // its state: a close of it waits; its auto-merge: a cancel of it waits
      PlannedEffect::MergePullRequest { number, .. } => resources.push(Resource::PullRequest {
        number: *number,
        field: PullRequestField::State,
      }),
      PlannedEffect::EnableAutoMerge { number, .. } => resources.push(Resource::PullRequest {
        number: *number,
        field: PullRequestField::AutoMerge,
      }),
      _ => {}
    }
  }
  resources
}

/// The plan of an intent that asks nothing to accept and lands nothing: its `effects`, for a window that shows what it
/// does; its run decides on what it reads then
pub(crate) fn plain_plan(intent: Intent, effects: Vec<PlannedEffect>) -> Plan {
  Plan {
    effects,
    fingerprint: Witness::default().fingerprint(&intent),
    intent,
    lands: Vec::new(),
    confirmations: Vec::new(),
    blockers: Vec::new(),
    waits: Vec::new(),
    alternatives: None,
    pull_request: None,
    approvals_skipped_by: None,
  }
}

/// The outcome of the operation `op` that ran `effects`, telling the event of `news_key`, with what it answered
/// (`result`) and the version of the view after what it changed; what it cost is the registry's to fill
/// (`RunningGuard::finish`)
pub(crate) fn outcome_of(op: OperationId, effects: Vec<EffectRecord>, news_key: Option<NewsKey>, version_after: Option<Version>, result: OperationResult) -> Outcome {
  Outcome {
    op,
    effects,
    news_key,
    version_after,
    result,
    undo: None,
    stats: OperationStats::default(),
  }
}

#[cfg(test)]
#[path = "engine_log_test.rs"]
mod engine_log_test;
