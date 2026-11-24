//! The frame every family of intents runs in (docs/design/rust-domain-authority.md, "Планирование и исполнение
//! операций", "Наблюдаемость"): `run` admits the operation in its session through the registry, runs the body of its
//! family once it is admitted and ends it with the body's answer, or answers what the operation of the same intent it
//! joined answered. The caller hears the outcome either way (`OperationEvent::Done`). The admission, the body and the
//! end run inside the root span of the operation (`operation{op operation_id tool subject}`, its id the one of its
//! record, its outcome and its entry in the journal), the body counting what it costs into the tally of the operation,
//! and an error leaves with the id. `prepare` asks the family of an intent for its plan,
//! and `execute` runs it on the plan its caller accepted: a start (`execute_land`, `land_execute::execute`), a restart
//! and a cancel of a robot (`robot`), a retarget of a pull request, the creation of one and a review request
//! (`pull_request`), the closes (`close`), a review and Approve N Pull Requests of a stack (`review`), the state of an
//! issue (`issue`), a push (`push`: of a branch alone, of a stack, up to a commit, of an unapplied branch, of the stack
//! branch of a whole stack), a rewrite
//! (`rewrite`), the fetch of the baseline, Update Project and the dismissal of what landed (`update`) and the fetch of
//! a review (`review_fetch`). A start
//! reserves what its plan lands (`Resource::Lands`), the refs
//! it pushes and the robot it cancels: a second start of the same subject joins it, one that lands a part of it waits for
//! it and is decided again. An operation that waited in the queue runs on the plan the ones it waited for left, while
//! the user accepted what it asks (`Accepted::decided_again`): a push and the Safe Push pressed right after it are one
//! push and one start. A start of a repository that lands by a push into the target reserves the remote-tracking ref of
//! the target on the remote its plan names (`refs/remotes/origin/master`: two landings into it wait for each other, a
//! fetch of it waits for the push) and runs without Patronus; after its push, the follow-up intent of
//! `domain_policy::land_direct::update_after` runs under the same guard, which holds the target through Update Project
//! and its sync. It adds HEAD after any fetch and sends the start outcome before the update outcome on the same channel.

use crate::events::EventSink;
use crate::land_execute::{self, Accepted, NotRun, Planned, StartHost, StartServices, Steps};
use crate::land_session::{SessionStart, StartFetch, pull_request_start, session_start};
use crate::registry::{Admitted, Answer, OperationRequest, Registry, RunningGuard};
use crate::rewrite::{RewriteHost, execute_rewrite, prepare_rewrite, rewrite_request};
use crate::run_frame::{self, SyncWait, wait_for_facts};
use crate::witness::{Witness, check, plan_changed};
use crate::{auto_merge, close, forget, issue, land, pull_request, push, review, review_fetch, robot, shown, update};
use domain_policy::land_direct::update_after;
use domain_types::branch_names::{to_final_branch_name, to_stack_branch_name};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{Land, PlannedEffect, Subject};
use domain_types::operation::{
  EffectRecord, Intent, NewsKey, OperationEvent, OperationId, OperationResult, OperationStats, OperationStep, Outcome, Plan, PullRequestField, Resource, RunningMove,
};
use domain_types::rewrite::RewriteIntent;
use domain_types::session::{SyncState, Version};
use remote_facts::service_sources::ServiceHost;
use repository_model::action_view::{FrameReads, reads_cached_frame};
use repository_model::session::{SessionKey, Sessions};
use repository_model::table::TableState;
use std::sync::Arc;
use sync_core::transfer_progress::TransferProgress;
use tracing::{Instrument, debug};

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
/// `invalid_arguments` for an invalid review; the refusal of the Safe Push of a review the session offers not
/// (`review::safe_push_refusal`), of a restart it offers not (`robot::restart_refusal_of`), of a park of a branch a robot
/// holds (`rewrite::park_refusal`)
pub async fn prepare(sessions: &Sessions, key: &SessionKey, intent: Intent) -> Result<Option<Plan>, ApiError> {
  match intent {
    Intent::Land(start) => Ok(land::prepare(sessions, key, start).await),
    Intent::RestartRobot { robot_id } => Ok(robot::prepare_restart(sessions, key, &robot_id).await?.map(|planned| planned.plan)),
    Intent::CancelRobot { robot_id } => Ok(robot::prepare_cancel(sessions, key, &robot_id).await.map(|(plan, _)| plan)),
    Intent::Retarget { number } => Ok(pull_request::prepare_retarget(sessions, key, number).await),
    Intent::CreatePullRequest(_) | Intent::RequestReview { .. } | Intent::LinkStack { .. } => {
      Ok(pull_request::prepare_planned(sessions, key, &intent).await.map(|planned| planned.plan))
    }
    Intent::ClosePullRequest { .. } | Intent::CloseLandedPullRequests { .. } => Ok(close::prepare_close(sessions, key, &intent).await),
    Intent::DisableAutoMerge { number } => Ok(auto_merge::prepare_disable(sessions, key, number).await),
    Intent::OpenShownReview { .. } | Intent::CloseShownReview { .. } => shown::prepare_shown(sessions, key, &intent).await,
    Intent::SubmitReview { .. } => review::prepare_review(sessions, key, &intent).await,
    Intent::ApproveStack { .. } => review::prepare_approve_stack(sessions, key, &intent).await,
    Intent::SetIssueState { .. } => issue::issue_plan(&intent).map(Some),
    Intent::Push(push) => Ok(push::prepare_push(sessions, key, &push).await.map(|planned| planned.plan)),
    Intent::ForgetStart { .. } => forget::forget_plan(&intent).map(Some),
    Intent::Rewrite(rewrite) => prepare_rewrite(sessions, key, *rewrite).await,
    intent @ (Intent::FetchBaseline | Intent::UpdateProject { .. } | Intent::DismissLanded { .. }) => Ok(update::prepare_update(sessions, key, intent).await),
    intent @ (Intent::FetchReview { .. } | Intent::FetchReviewStack { .. }) => Ok(review_fetch::prepare_review_fetch(sessions, key, intent).await),
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
  refuse_on_cached_frame(registry.sessions(), key, &intent).await?;
  match intent {
    // the run of a start is a large future: on the heap
    Intent::Land(land) => Box::pin(execute_land(registry, key, land, accepted, hosts, events)).await,
    Intent::RestartRobot { robot_id } => Box::pin(robot::execute_restart(registry, key, &robot_id, accepted, services, events)).await,
    Intent::CancelRobot { robot_id } => robot::execute_cancel(registry, key, &robot_id, accepted, services, events).await,
    Intent::Retarget { number } => pull_request::execute_retarget(registry, key, number, accepted, services, events).await,
    intent @ (Intent::CreatePullRequest(_) | Intent::RequestReview { .. } | Intent::LinkStack { .. }) => {
      Box::pin(pull_request::execute_pull_request(registry, key, intent, accepted, services, events)).await
    }
    intent @ (Intent::ClosePullRequest { .. } | Intent::CloseLandedPullRequests { .. }) => close::execute_close(registry, key, intent, accepted, services, events).await,
    Intent::DisableAutoMerge { number } => auto_merge::execute_disable(registry, key, number, accepted, services, events).await,
    intent @ (Intent::OpenShownReview { .. } | Intent::CloseShownReview { .. }) => Box::pin(shown::execute_shown(registry, key, intent, accepted, services, events)).await,
    intent @ Intent::SubmitReview { .. } => Box::pin(review::execute_review(registry, key, intent, accepted, services, events)).await,
    intent @ Intent::ApproveStack { .. } => Box::pin(review::execute_approve_stack(registry, key, intent, accepted, services, events)).await,
    intent @ Intent::SetIssueState { .. } => issue::execute_issue(registry, key, intent, accepted, services, events).await,
    Intent::Push(push) => Box::pin(push::execute_push(registry, key, push, accepted, services, events)).await,
    intent @ Intent::ForgetStart { .. } => forget::execute_forget(registry, key, intent, accepted, events).await,
    // the run of a rewrite is a large future: on the heap
    Intent::Rewrite(rewrite) => Box::pin(execute_rewrite(registry, key, *rewrite, hosts, events)).await,
    Intent::FetchBaseline => Box::pin(update::execute_fetch(registry, key, hosts, events)).await,
    Intent::UpdateProject { fetch, epoch, keep } => Box::pin(update::execute_update_keeping(registry, key, (fetch, epoch, keep), hosts, events)).await,
    Intent::DismissLanded { epoch } => update::execute_dismiss(registry, key, epoch, hosts, events).await,
    Intent::FetchReview { number } => review_fetch::execute_review_fetch(registry, key, number, hosts, events).await,
    Intent::FetchReviewStack { top, bottom } => review_fetch::execute_review_stack_fetch(registry, key, top, bottom, hosts, events).await,
    intent @ Intent::Pick { .. } => Err(runs_only_in_bd(&intent)),
  }
}

/// What a remote intent reads of its session that the cached frame of an earlier run may hold, as its actions read it
/// (`action_view::FrameReads`); `None` for an intent that reads none of it or waits for the first sync itself (a push,
/// a rewrite, an update), and for the closes of what a landing landed, which the news of this run ask (a frame tells
/// none)
const fn frame_reads(intent: &Intent) -> Option<FrameReads> {
  match intent {
    Intent::Land(_)
    | Intent::Retarget { .. }
    | Intent::CreatePullRequest(_)
    | Intent::LinkStack { .. }
    | Intent::RequestReview { .. }
    | Intent::ClosePullRequest { .. }
    | Intent::DisableAutoMerge { .. }
    | Intent::OpenShownReview { .. }
    | Intent::CloseShownReview { .. } => Some(FrameReads::Landing),
    Intent::RestartRobot { .. } | Intent::CancelRobot { .. } => Some(FrameReads::Robots),
    Intent::SubmitReview { .. } | Intent::ApproveStack { .. } => Some(FrameReads::Reviews),
    _ => None,
  }
}

/// Refuses a remote intent pressed on a frame nobody checked yet (`action_view::reads_cached_frame`): the table of its
/// session is the cached frame of an earlier run, or a fact it reads is a value of it, so a plan of it would take the
/// checks, the reviews and the robots of yesterday for those of now. Its actions wait there (`ActionBlocker::Cached`):
/// this holds the same for a caller that did not read them (a key, a toast)
///
/// # Errors
/// `frame_refusal`, before anything is admitted or written
pub(crate) async fn refuse_on_cached_frame(sessions: &Sessions, key: &SessionKey, intent: &Intent) -> Result<(), ApiError> {
  let Some(reads) = frame_reads(intent) else {
    return Ok(());
  };
  let refused = sessions
    .read(key, move |state, facts| reads_cached_frame(state, facts, reads).then(|| frame_refusal(state)))
    .await
    .flatten();
  refused.map_or(Ok(()), Err)
}

/// The refusal of an intent on the cached frame of an earlier run in `state`. While the table is that frame because its
/// first sync failed or git deferred it, the error of that sync (`no_identity`, `operation_in_progress`, …): no sync runs
/// that the intent could wait for, and the error says what to fix. Otherwise `plan_changed` in the words of its actions
/// there: it waits for the first sync
pub(crate) fn frame_refusal(state: &TableState) -> ApiError {
  let failed = match state.sync() {
    SyncState::Failed { error } | SyncState::Deferred { error } if state.cached() => Some(error.clone()),
    _ => None,
  };
  failed.unwrap_or_else(|| ApiError::new(ErrorCode::PlanChanged, "Waits for the first sync: the rows are the frame of an earlier run"))
}

/// Whether `plan`, decided again once the operation of `guard` was admitted, is the plan its caller accepted, or after a
/// wait in the queue the one the operations it waited for left (`Accepted::decided_again`)
///
/// # Errors
/// `plan_changed` of its intent
pub(crate) fn check_admitted(guard: &RunningGuard, accepted: &Accepted, plan: &Plan) -> Result<(), ApiError> {
  check_waited(accepted, plan, guard.queued())
}

/// `check_admitted` of a plan decided after a wait `waited` names: in the queue at its admission, or for what it held
/// more once admitted (`Holding::cover`)
///
/// # Errors
/// `plan_changed` of its intent
pub(crate) fn check_waited(accepted: &Accepted, plan: &Plan, waited: bool) -> Result<(), ApiError> {
  check(&accepted.decided_again(plan, waited).fingerprint, &plan.fingerprint, &plan.intent)
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

/// How an operation decides its plan once admitted: on what the operations and the syncs it `waited` for left, and
/// whether a move of its branches before its first effect may still have it decided again (once per operation)
pub(crate) struct Replan {
  pub(crate) waited: bool,
  again: bool,
}

impl Replan {
  /// The plan of an operation that `waited` for facts once admitted (`run_frame::wait_for_facts`)
  pub(crate) const fn after(waited: bool) -> Self {
    Self { waited, again: true }
  }
}

/// Runs `planned`, decided again once the operation of `guard` was admitted, on `start` (the host and the services of
/// the session, `session_start` or `pull_request_start`) as a start runs (`land_execute::run_planned`), its steps in the
/// view: on the plan the wait left after it waited (`Replan::waited`: in the queue, or for the sync of a push), on the
/// one `accepted` names else (`Accepted::decided_again`); answers its outcome. A plan whose local branches alone moved
/// before its first effect (`NotRun::BranchesMoved`: a sync moved them since the table it was decided on) waits for the
/// sync that shows them (`run_frame::catch_up`), once per operation, and answers `None`: the caller decides it again
/// after that wait and runs it here once more; a second move refuses it
pub(crate) async fn run_planned_in(
  guard: &RunningGuard,
  (sessions, key): (&Sessions, &SessionKey),
  start: &(SessionStart, StartServices),
  planned: Planned,
  accepted: &Accepted,
  replan: &mut Replan,
) -> Option<Answer> {
  let (host, services) = start;
  let steps = steps_of(guard);
  let decided = accepted.decided_again(&planned.plan, replan.waited);
  // the run of a plan is a large future: on the heap
  match Box::pin(land_execute::try_planned(host, services, &steps, guard.id().clone(), planned, &decided, replan.waited)).await {
    Ok(started) => Some(Ok(started.outcome)),
    Err(NotRun::BranchesMoved(error)) if replan.again => {
      debug!(error = %error.message, "a branch of the plan moved before its first effect: decided again on the sync that shows it");
      *replan = Replan { waited: true, again: false };
      run_frame::catch_up(sessions, key, guard).await.err().map(Err)
    }
    Err(not_run) => Some(Err(not_run.into_error())),
  }
}

/// What an operation holds once admitted, and whether it held more once already (`cover`)
pub(crate) struct Holding {
  held: Vec<Resource>,
  spent: bool,
}

/// What `Holding::cover` found of a plan decided once admitted
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cover {
  /// It holds all the plan needs: the plan runs
  Held,
  /// It held what the plan needs more, after the operations of this process that held it (`waited`: it waited for
  /// them): the plan is decided again
  Again { waited: bool },
}

impl Holding {
  /// What the admission of an operation held (`OperationRequest::resources`)
  pub(crate) const fn new(held: Vec<Resource>) -> Self {
    Self { held, spent: false }
  }

  /// Whether the operation of `guard` holds what `needed` names, the resources its plan decided once admitted holds by
  /// the builder of its admission (`land_request`, `push_request`, …): HEAD, which a rewrite of this process holds and
  /// the wait for facts waited for, aside. Held: the plan runs. The first time it lacks some, it holds them
  /// (`RunningGuard::hold`: after the operations of this process that hold one, `busy` while another process does) and
  /// the plan is decided again on what they left; once more, the operation of `intent` is refused before its first
  /// effect: its plan moved under it twice. One hold per run of an operation, every plan of it decided again shares it
  /// (a start planned again after `BaselineMoved`)
  ///
  /// # Errors
  /// `plan_changed` of `intent` at the second time, nothing done; what `RunningGuard::hold` fails with
  pub(crate) async fn cover(&mut self, guard: &RunningGuard, needed: &[Resource], intent: &Intent) -> Result<Cover, ApiError> {
    let missing: Vec<Resource> = needed
      .iter()
      .filter(|resource| !matches!(resource, Resource::Head) && !self.held.contains(resource))
      .cloned()
      .collect();
    if missing.is_empty() {
      return Ok(Cover::Held);
    }
    if self.spent {
      return Err(plan_changed(intent, Some("it needs what it held not, after it held more once")));
    }
    self.spent = true;
    debug!(missing = ?missing, "the plan decided once admitted needs more than the admission held: it holds it and is decided again");
    let waited = guard.hold(missing.clone(), OperationStep::Checking).await?;
    self.held = with_all(std::mem::take(&mut self.held), missing);
    Ok(Cover::Again { waited })
  }

  /// `cover`, whether its hold waited added to `waited`: `true` where the plan runs, `false` where it is decided again
  ///
  /// # Errors
  /// As `cover`
  pub(crate) async fn covers(&mut self, guard: &RunningGuard, needed: &[Resource], intent: &Intent, waited: &mut bool) -> Result<bool, ApiError> {
    match self.cover(guard, needed, intent).await? {
      Cover::Held => Ok(true),
      Cover::Again { waited: queued } => {
        *waited |= queued;
        Ok(false)
      }
    }
  }
}

/// Runs the start `land` of the app in the session of `key` on the plan its caller accepted (`accepted`):
/// admitted over the operations of the session with what the plan as the session has it now reserves, then planned again
/// over the session and the services of `hosts`, held what that plan reserves more (`Holding::cover`), and run
/// (`land_execute::run_planned`) over the host of its route (`session_start`; a push into the target, which starts no
/// robot, `pull_request_start`)
/// with its steps in the view and its progress and outcome to `events`, on
/// the table of the last sync and what the operations it queued behind left (the push of its branch pressed just before
/// it: the start pushes nothing more), after the sync of the writes of the session (`SyncWait::Writes`: the one an
/// Update Project before it asked, which rebuilds the branches it pushes), the target fetched meanwhile
/// (`StartFetch`); a plan whose branches a sync moved before its first effect is decided once more on the table of
/// that sync (`run_frame::catch_up`); stopped by `Registry::cancel`. Once a push into the target landed, Update Project follows under the start's
/// guard (`land_direct::update_after`): its steps and, after the start's `Done`, its outcome go to `events`, while the
/// target stays held until its sync; the answer is the start's. The journal contains one operation for both phases
///
/// # Errors
/// What refused the start before its first effect (`land_execute::run_planned`), a robot of Patronus without its service
/// (`not_on_space`), a lock another process holds, a session that closed or has no sync yet
pub async fn execute_land(registry: &Registry, key: &SessionKey, land: Land, accepted: Accepted, hosts: Hosts<'_>, events: Arc<dyn EventSink>) -> Answer {
  let services = hosts.services;
  let sessions = registry.sessions();
  let planned = land::prepare_planned(sessions, key, land.clone()).await;
  let request = land_request(&key.prefix, &land, planned.as_ref());
  let mut holding = Holding::new(request.resources.clone());
  // the target its checks replay onto, fetched beside its wait for facts: a start that lands by a push into the
  // target replays nothing
  let prefetch = planned.as_ref().is_some_and(|planned| !pushes_into_target(&planned.plan));
  // what the plan of the click waited for: a fact it had answered that a sync makes the session read again is waited for
  let waited_before = planned.as_ref().map(|planned| planned.plan.waits.clone());
  let fetch = StartFetch::default();
  run(registry, key, request, Arc::clone(&events), async |guard| {
    // A start lands the table of the last sync, and the one the sync of a write of the session (an Update Project, a
    // rewrite) commits: that sync rebuilds the branches it pushes; it waits for no other sync
    let facts = wait_for_facts(sessions, key, guard, vec![Resource::Head], SyncWait::Writes);
    let fetched = async {
      if prefetch {
        // an error of the fetch is a note of the check that reads it
        let _ = fetch.fetched(&services.git(), &key.worktree).await;
      }
    };
    // a sync of the writes that failed leaves the table of the last one: the witness tells the branches it moved
    let (facts, ()) = tokio::join!(facts, fetched);
    let mut waited = facts.waited;
    // planned over the host that needs no Patronus, then run over the host of the route its plan decided: a robot of
    // Patronus needs its service (`session_start` refuses where there is none), a push into the target does not
    let (mut host, mut run) = pull_request_start(services, sessions, key).await?;
    let of_this_start = |host: &mut SessionStart| {
      host.exclude_operation(guard.id().clone());
      host.share_fetch(fetch.clone());
      if let Some(waits) = &waited_before {
        host.answered_before(waits);
      }
    };
    of_this_start(&mut host);
    let steps = steps_of(guard);
    steps.step(OperationStep::Checking);
    let (mut planned, queued) = covered_plan(&host, &land, guard, &mut holding).await?;
    waited |= queued;
    let into_target = pushes_into_target(&planned.plan);
    if !into_target {
      (host, run) = session_start(services, sessions, key).await?;
      of_this_start(&mut host);
    }
    // one plan decided again per start: after the update of its baseline (`BaselineMoved`), or after the sync that
    // moved its branches before its first effect (`NotRun::BranchesMoved`); over what the admission and the first plan
    // held, one hold of more for both plans, on the host of the route the first plan decided
    let mut decided = accepted.decided_again(&planned.plan, waited);
    let mut again = true;
    let started = loop {
      let changed = match land_execute::try_planned(&host, &run, &steps, guard.id().clone(), planned, &decided, waited).await {
        Ok(started) => break started,
        Err(NotRun::Refused(error)) if again && waited && error.code == ErrorCode::BaselineMoved => {
          run_frame::refresh_baseline(sessions, key, guard, hosts).await?;
          "it lands by another route after the update of its baseline"
        }
        Err(NotRun::BranchesMoved(error)) if again => {
          debug!(error = %error.message, "a branch of the start moved before its first effect: decided again on the sync that shows it");
          run_frame::catch_up(sessions, key, guard).await?;
          "it lands by another route after the sync that moved its branches"
        }
        Err(not_run) => return Err(not_run.into_error()),
      };
      again = false;
      (planned, _) = covered_plan(&host, &land, guard, &mut holding).await?;
      if pushes_into_target(&planned.plan) != into_target {
        return Err(plan_changed(&planned.plan.intent, Some(changed)));
      }
      decided = accepted.decided_again(&planned.plan, true);
      // the plan decided again runs as asked: no retry signal of the target any more
      waited = false;
    };
    if let Some(intent) = update_after(&started.outcome.effects) {
      // the start's outcome comes first on the channel; the follow-up keeps the target held until its sync ends
      guard.report(&started.outcome);
      let _ = run_frame::follow_up(registry, key, guard, intent, hosts, &events).await;
    }
    Ok(started.outcome)
  })
  .await
}

/// The plan of the start `land` decided once admitted over `host`, held by `holding` (`Holding::cover`: what it reserves
/// more is held once, and the start planned again), with whether the hold waited in the queue
///
/// # Errors
/// A subject the session lacks, what `Holding::cover` refuses it with
async fn covered_plan(host: &SessionStart, land: &Land, guard: &RunningGuard, holding: &mut Holding) -> Result<(Planned, bool), ApiError> {
  let mut queued = false;
  loop {
    let planned = host.plan(land).await?.ok_or_else(|| land_execute::unknown_subject(&land.subject))?;
    if holding
      .covers(guard, &land_request(&host.key.prefix, land, Some(&planned)).resources, &planned.plan.intent, &mut queued)
      .await?
    {
      return Ok((planned, queued));
    }
  }
}

/// The plan pushes the copy of its subject into the target, or GitHub merges its pull request: no robot of Patronus
/// starts (`brd land` too)
#[must_use]
pub fn pushes_into_target(plan: &Plan) -> bool {
  plan.effects.iter().any(|effect| {
    matches!(
      effect,
      PlannedEffect::PushBaseline { .. } | PlannedEffect::MergePullRequest { .. } | PlannedEffect::EnableAutoMerge { .. } | PlannedEffect::MergeStack { .. }
    )
  })
}

/// What the start `land` of a session of `prefix` asks the registry for: what its plan (as the session has it now)
/// reserves, the branch it lands without one (the frame of an earlier run, a subject the table lacks: what its plan
/// once admitted needs more it holds then, `Holding::cover`), the move of its row from the click on
pub(crate) fn land_request(prefix: &str, land: &Land, planned: Option<&Planned>) -> OperationRequest {
  OperationRequest {
    intent: Intent::Land(land.clone()),
    resources: planned.map_or_else(
      || {
        vec![Resource::Lands {
          branch: land.subject.name().to_string(),
        }]
      },
      |planned| reserved(prefix, &land.subject, planned),
    ),
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
    Intent::Land(land) => land_request(prefix, land, Some(planned)),
    Intent::Push(push) => push::push_request(prefix, push, Some(planned)),
    Intent::RestartRobot { robot_id } => robot::restart_request(robot_id, Some(planned)),
    Intent::CancelRobot { robot_id } => robot::cancel_request(robot_id),
    Intent::Retarget { number } => pull_request::retarget_request(*number),
    intent @ (Intent::CreatePullRequest(_) | Intent::RequestReview { .. } | Intent::LinkStack { .. }) => pull_request::pull_request_request(intent, Some(planned)),
    intent @ (Intent::ClosePullRequest { .. } | Intent::CloseLandedPullRequests { .. }) => close::close_request(intent, Some(plan)),
    Intent::DisableAutoMerge { number } => auto_merge::disable_request(*number),
    intent @ (Intent::OpenShownReview { .. } | Intent::CloseShownReview { .. }) => shown::shown_request(prefix, intent, Some(plan)),
    intent @ (Intent::SubmitReview { .. } | Intent::ApproveStack { .. }) => review::review_request(intent),
    intent @ Intent::SetIssueState { key, .. } => issue::issue_request(intent, key),
    intent @ Intent::ForgetStart { .. } => forget::forget_request(intent),
    Intent::Rewrite(rewrite) => rewrite_request(prefix, rewrite),
    intent @ (Intent::FetchBaseline | Intent::UpdateProject { .. } | Intent::DismissLanded { .. }) => update::update_request(intent, update::planned_upstream(plan)),
    intent @ (Intent::FetchReview { .. } | Intent::FetchReviewStack { .. }) => review_fetch::review_fetch_request(intent.clone()),
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
    Intent::UpdateProject { .. } | Intent::DismissLanded { .. } => Some(update::update_request(intent, "")),
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
    Intent::Rewrite(rewrite) if matches!(**rewrite, RewriteIntent::Group { .. }) => "the regroup of a plan (`brd group`)",
    Intent::Rewrite(_) => "the move of a hunk (`brd move-hunk`)",
    _ => "a pick of commits (`brd pick`)",
  };
  ApiError::new(ErrorCode::InvalidArguments, format!("Only brd runs {what}: the app shows its record and runs none"))
}

/// What a start of `subject` reserves: the branches its plan lands, the refs its push writes, what its effects on the
/// remote hold (`remote_holds`: the robot it cancels, the base of a pull request it retargets), the remote-tracking ref
/// of the target it pushes into and the fields of the pull requests GitHub merges
fn reserved(prefix: &str, subject: &Subject, planned: &Planned) -> Vec<Resource> {
  let plan = &planned.plan;
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
      // the remote-tracking ref of the target: a fetch of it, and another landing into it, wait for the push
      PlannedEffect::PushBaseline { remote, target, .. } => resources.push(Resource::Ref {
        name: format!("refs/remotes/{remote}/{target}"),
      }),
      // its state: a close of it waits; its auto-merge: a cancel of it waits
      PlannedEffect::MergePullRequest { number, .. } => resources.push(Resource::PullRequest {
        number: *number,
        field: PullRequestField::State,
      }),
      // the state of the layer and of each layer below it, which the merge merges with it
      PlannedEffect::MergeStack { number, layers, .. } => resources.extend(layers.iter().chain([number]).map(|number| Resource::PullRequest {
        number: *number,
        field: PullRequestField::State,
      })),
      PlannedEffect::EnableAutoMerge { number, .. } => resources.push(Resource::PullRequest {
        number: *number,
        field: PullRequestField::AutoMerge,
      }),
      _ => {}
    }
  }
  with_all(resources, remote_holds(planned))
}

/// What the effects of `planned` on the robots and the pull requests hold, of every family alike: the base of the pull
/// request each retarget moves and of each open pull request of the stack of GitHub a link brings in line (a retarget of
/// one waits for it), the robot it cancels, and the robot it restarts with what that robot lands (the branches of its
/// plan, else its subject): a Safe Push of the branch waits for the restart, so Patronus never runs two robots of it
pub(crate) fn remote_holds(planned: &Planned) -> Vec<Resource> {
  let base = |number: u32| Resource::PullRequest {
    number,
    field: PullRequestField::Base,
  };
  let mut resources = Vec::new();
  for effect in &planned.plan.effects {
    match effect {
      PlannedEffect::Retarget { number, .. } => resources.push(base(*number)),
      PlannedEffect::LinkStack { chain, .. } => resources.extend(
        chain
          .iter()
          .filter_map(|branch| planned.stack_pull_requests.get(branch))
          .map(|pull_request| base(pull_request.number)),
      ),
      PlannedEffect::CancelRobot { robot_id } => resources.push(Resource::Robot { id: robot_id.clone() }),
      PlannedEffect::RestartRobot { robot_id } => {
        resources.push(Resource::Robot { id: robot_id.clone() });
        if planned.plan.lands.is_empty() {
          resources.push(Resource::Lands {
            branch: planned.subject.name().to_string(),
          });
        } else {
          resources.extend(planned.plan.lands.iter().map(|branch| Resource::Lands { branch: branch.clone() }));
        }
      }
      _ => {}
    }
  }
  resources
}

/// `resources` with each of `more` it lacks, in their order: each resource once
pub(crate) fn with_all(mut resources: Vec<Resource>, more: impl IntoIterator<Item = Resource>) -> Vec<Resource> {
  for resource in more {
    if !resources.contains(&resource) {
      resources.push(resource);
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

#[cfg(test)]
#[path = "cover_test.rs"]
mod cover_test;
