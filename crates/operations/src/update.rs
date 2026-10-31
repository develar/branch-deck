//! The family of Update Project (docs/design/git-operations.md, "Update project"; docs/design/operations.md, "Update
//! Project"): the fetch of the baseline (`Intent::FetchBaseline`, the Fetch of a merge toast), the update of the
//! checked out branch onto it (`Intent::UpdateProject`, ⌘T) and the dismissal of what landed with an update
//! (`Intent::DismissLanded`), as operations of the session, run by the cores `bd update` runs.
//!
//! A fetch holds the tracking ref of the baseline (`Resource::Ref`): a second Fetch of any window joins it, and the
//! fetch lock of the repository joins a fetch of another process (`sync_core::fetch_lock`). An update holds nothing
//! while it fetches: its fetch is the `FetchBaseline` of the session (`fetch_operation`), which joins a Fetch that runs
//! and which a Fetch joins while it runs, so the rewrites of the session run during the network. Then it holds HEAD
//! (`RunningGuard::hold`, `Resource::Head`): it waits behind the rewrites of the process before it, its step `Queued`,
//! and the ones after it wait for it. Its check (the replay, the resolution by words, the landed candidates) runs before
//! the repository lock on the HEAD and the upstream it read once; under the lock only the witness, the local changes and
//! the move (`update_core::update_project`), a wait for another holder its step `WaitingForLock`, which
//! `Registry::cancel` stops (`canceled`, nothing changed but the fetch). The sync that shows what the fetch and the move
//! changed is asked last (`SyncReason::AfterRewrite`), after every update whose fetch finished, and the operation ends
//! with that request: a rewrite queued behind it runs beside that sync. The answer of an applied update comes once that
//! sync ended (`Outcome::version_after`), its undo the token `update:<previous>..<new>` (`UndoKind::Update`). The fetch
//! is the first effect: a failure from it on is the failed effect of an outcome, the effects after it skipped.

use crate::engine::{Hosts, outcome_of, plain_plan, run};
use crate::events::EventSink;
use crate::registry::{Answer, OperationRequest, Registry, RunningGuard, Stepper};
use crate::rewrite::{StepWaiter, blocking, wait_stopped};
use domain_types::error::ApiError;
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectRecord, EffectStatus, FetchedBaseline, Intent, OperationEvent, OperationResult, OperationStep, Outcome, Plan, Resource};
use domain_types::rewrite::{UndoKind, UndoToken};
use domain_types::session::SyncReason;
use domain_types::update_project::UpdateProjectResult;
use history_rewrite::repository_lock::LockWait;
use repository_model::session::{SessionKey, Sessions, SyncTicket};
use std::sync::{Arc, Mutex, PoisonError};
use sync_core::baseline_fetch::{FetchBaselineParams, fetch_baseline_core};
use sync_core::landed::dismiss_landed;
use sync_core::transfer_progress::TransferProgress;
use tracing::instrument;
use update_core::update_project::{UpdatePhase, UpdateProjectParams, UpdateUndoToken, update_project_core};

/// The baseline of the session of `key` as its last grouping named it (`origin/master`); empty before it
async fn baseline_of(sessions: &Sessions, key: &SessionKey) -> String {
  sessions.read(key, |state, _| state.header().baseline_branch.clone()).await.unwrap_or_default()
}

/// What an intent of the family does, in order, for the baseline `upstream`
fn effects_of(intent: &Intent, upstream: &str) -> Vec<PlannedEffect> {
  let fetch = || PlannedEffect::Fetch { upstream: upstream.to_owned() };
  match intent {
    Intent::FetchBaseline => vec![fetch(), PlannedEffect::Sync],
    Intent::UpdateProject { fetch: true } => vec![fetch(), PlannedEffect::Update, PlannedEffect::Sync],
    Intent::UpdateProject { fetch: false } => vec![PlannedEffect::Update, PlannedEffect::Sync],
    _ => vec![PlannedEffect::DismissLanded],
  }
}

/// The plan of `intent`, an intent of the family, in the session of `key` (`engine::prepare`): its effects, for a window
/// that shows what it does; it asks nothing to accept, and its run decides on what it reads then. `None` for a session
/// that is not open
pub async fn prepare_update(sessions: &Sessions, key: &SessionKey, intent: Intent) -> Option<Plan> {
  let upstream = sessions.read(key, |state, _| state.header().baseline_branch.clone()).await?;
  let effects = effects_of(&intent, &upstream);
  Some(plain_plan(intent, effects))
}

/// What an intent of the family planned on the baseline `upstream` asks the registry for: a fetch the tracking ref of
/// the baseline (none before the first grouping names it), an update without a fetch HEAD (with one, nothing until its
/// fetch ended), a dismissal nothing
#[must_use]
pub fn update_request(intent: &Intent, upstream: &str) -> OperationRequest {
  let (resources, step) = match intent {
    Intent::FetchBaseline => {
      let tracking = (!upstream.is_empty()).then(|| Resource::Ref {
        name: format!("refs/remotes/{upstream}"),
      });
      (tracking.into_iter().collect(), OperationStep::Fetching)
    }
    Intent::UpdateProject { fetch: true } => (Vec::new(), OperationStep::Fetching),
    Intent::UpdateProject { fetch: false } => (vec![Resource::Head], OperationStep::Checking),
    _ => (Vec::new(), OperationStep::Writing),
  };
  OperationRequest {
    intent: intent.clone(),
    resources,
    step,
    moves: Vec::new(),
  }
}

/// The upstream an intent of the family planned as `plan` fetches: the one of its `Fetch` effect, empty without one
#[must_use]
pub fn planned_upstream(plan: &Plan) -> &str {
  plan
    .effects
    .iter()
    .find_map(|effect| match effect {
      PlannedEffect::Fetch { upstream } => Some(upstream.as_str()),
      _ => None,
    })
    .unwrap_or_default()
}

/// Runs the Fetch of the baseline of the session of `key` (`Intent::FetchBaseline`): its fetch, then the sync that
/// shows what it fetched, asked and not waited for; its progress and its outcome to `events`
///
/// # Errors
/// What refused it before its fetch: a session that closed
#[instrument(skip_all)]
pub async fn execute_fetch(registry: &Registry, key: &SessionKey, hosts: Hosts<'_>, events: Arc<dyn EventSink>) -> Answer {
  fetch_operation(registry, key, hosts, events, true).await
}

/// The fetch of the session of `key` as the operation `FetchBaseline`, its progress and outcome to `events`: a request
/// that joins the one that runs (the Fetch of a toast during the fetch of an update, an update during a Fetch) gets its
/// outcome; `sync`: its own run asks the sync after it (the fetch of an update leaves it to the update, which syncs once
/// at its end)
fn fetch_operation<'a>(registry: &'a Registry, key: &'a SessionKey, hosts: Hosts<'a>, events: Arc<dyn EventSink>, sync: bool) -> Fetching<'a> {
  Box::pin(fetch_run(registry, key, hosts, events, sync))
}

/// The run of `fetch_operation`, its type erased: it runs inside the run of an update too
type Fetching<'a> = std::pin::Pin<Box<dyn Future<Output = Answer> + Send + 'a>>;

async fn fetch_run(registry: &Registry, key: &SessionKey, hosts: Hosts<'_>, events: Arc<dyn EventSink>, sync: bool) -> Answer {
  let sessions = registry.sessions();
  let upstream = baseline_of(sessions, key).await;
  let request = update_request(&Intent::FetchBaseline, &upstream);
  run(registry, key, request, events, async |guard| {
    let fetched = fetch(guard, hosts, key).await;
    let upstream = fetched.as_ref().map_or(upstream, |fetched| fetched.upstream.clone());
    let fetch_status = fetched
      .as_ref()
      .map_or_else(|error| EffectStatus::Failed { error: named(error, guard) }, |_| EffectStatus::Done);
    let synced = fetched.is_ok() && sync;
    if synced {
      drop(sessions.sync(key, SyncReason::AfterRewrite));
    }
    let effects = vec![
      EffectRecord::new(PlannedEffect::Fetch { upstream }, fetch_status),
      EffectRecord::new(PlannedEffect::Sync, if synced { EffectStatus::Done } else { EffectStatus::Skipped { reason: None } }),
    ];
    let result = fetched.map_or(OperationResult::None, |fetched| OperationResult::Fetch(Box::new(fetched)));
    Ok(outcome_of(guard.id().clone(), effects, None, None, result))
  })
  .await
}

/// The fetch of the operation of `guard` on the blocking pool, its progress to its caller: the baseline it fetched, when,
/// and how much git said it received
///
/// # Errors
/// The failed fetch as the host words it; a local baseline, which has nothing to fetch
async fn fetch(guard: &RunningGuard, hosts: Hosts<'_>, key: &SessionKey) -> Result<FetchedBaseline, ApiError> {
  let (git, stepper, canceled) = (hosts.services.git(), guard.stepper(), guard.canceled());
  let params = FetchBaselineParams {
    repository_path: key.worktree.clone(),
  };
  let fetched = blocking(move || {
    let mut transferred = None;
    // a wait for the fetch of another process ends when the user stops the update
    let canceled = || canceled.load(std::sync::atomic::Ordering::Relaxed);
    let on_tick = &mut |tick: TransferProgress| {
      if tick.transferred.is_some() {
        transferred.clone_from(&tick.transferred);
      }
      stepper.progress(&tick);
    };
    let fetched = fetch_baseline_core(&git, &params, on_tick, &canceled);
    Ok(fetched.map(|fetched| FetchedBaseline {
      upstream: fetched.upstream,
      fetched_at: fetched.fetched_at,
      transferred,
    }))
  })
  .await?;
  fetched.map_err(|error| hosts.rewrites.rewrite_error(error))
}

/// The progress of a fetch an update runs or joins, told as the update's to the caller of the update
struct Relay(Stepper);

impl EventSink for Relay {
  fn send(&self, event: OperationEvent) {
    if let OperationEvent::Progress {
      percent,
      line,
      transferred,
      speed,
      ..
    } = event
    {
      self.0.progress(&TransferProgress {
        percent: u8::try_from(percent).unwrap_or(100),
        transferred,
        speed,
        line,
      });
    }
  }
}

/// Runs Update Project in the session of `key` (the module's header): with `fetch`, the fetch first (the
/// `FetchBaseline` of the session, joined when one runs), then HEAD held and its core without a fetch, then the sync;
/// its steps in the view, its progress and its outcome to `events`. The operation ends with the request of the sync;
/// the answer of an applied update comes once that sync ended, so the window that asked keeps its run until then
///
/// # Errors
/// What refused it before its first effect: a session that closed; without a fetch the core's failure, `canceled` after
/// a stopped wait for the lock
#[instrument(skip_all, fields(fetch))]
pub async fn execute_update(registry: &Registry, key: &SessionKey, fetch: bool, hosts: Hosts<'_>, events: Arc<dyn EventSink>) -> Answer {
  let sessions = registry.sessions();
  let request = update_request(&Intent::UpdateProject { fetch }, "");
  let asked = Mutex::new(None);
  let mut answer = run(registry, key, request, events, async |guard| {
    let mut effects = Vec::new();
    if fetch {
      let fetched = fetch_operation(registry, key, hosts, Arc::new(Relay(guard.stepper())), false).await?;
      let record = fetched
        .effects
        .into_iter()
        .next()
        .unwrap_or_else(|| EffectRecord::new(PlannedEffect::Sync, EffectStatus::Done));
      let failed = !matches!(record.status, EffectStatus::Done);
      effects.push(record);
      if failed {
        effects.extend([PlannedEffect::Update, PlannedEffect::Sync].map(|effect| EffectRecord::new(effect, EffectStatus::Skipped { reason: None })));
        return Ok(outcome_of(guard.id().clone(), effects, None, None, OperationResult::None));
      }
      // the check and the move need HEAD: after the rewrites of the process before it, before the ones after it
      guard.hold(vec![Resource::Head], OperationStep::Checking).await?;
    }
    let updated = update(guard, key, hosts).await;
    let applied = updated.as_ref().is_ok_and(|result| result.applied.is_some());
    // the fetch moved the baseline also when nothing moved here: integration, the remote status, what landed
    let sync = (fetch || applied).then(|| sessions.sync(key, SyncReason::AfterRewrite));
    let sync_status = if sync.is_some() { EffectStatus::Done } else { EffectStatus::Skipped { reason: None } };
    let (status, result, undo) = match updated {
      Ok(result) => {
        let undo = undo_of(&result);
        (EffectStatus::Done, OperationResult::Update(Box::new(result)), undo)
      }
      // nothing was changed by it: before its first effect without a fetch, the failed update after one
      Err(error) if !fetch => return Err(error),
      Err(error) => (EffectStatus::Failed { error: named(&error, guard) }, OperationResult::None, None),
    };
    effects.push(EffectRecord::new(PlannedEffect::Update, status));
    effects.push(EffectRecord::new(PlannedEffect::Sync, sync_status));
    if applied {
      *asked.lock().unwrap_or_else(PoisonError::into_inner) = sync;
    }
    Ok(Outcome {
      undo,
      ..outcome_of(guard.id().clone(), effects, None, None, result)
    })
  })
  .await;
  let sync = asked.into_inner().unwrap_or_else(PoisonError::into_inner);
  if let (Ok(outcome), Some(sync)) = (&mut answer, sync) {
    outcome.version_after = synced(sync).await;
  }
  answer
}

/// `error` of an effect of the operation of `guard`, naming it, as an error of the operation does (`engine::run`): the
/// page tells it under that id, and Activity tells the entry of the operation no more
fn named(error: &ApiError, guard: &RunningGuard) -> ApiError {
  let mut error = error.clone();
  error.operation_id.get_or_insert_with(|| guard.id().0.clone());
  error
}

/// The version of the view after the last patch of the sync `sync`; `None` when it failed or was deferred, whose toast
/// comes at once
async fn synced(sync: SyncTicket) -> Option<domain_types::session::Version> {
  sync.wait().await.ok().map(|answer| answer.version)
}

/// The undo of what `result` moved: the branch back from its new head to the one before (`bd undo update:…`)
fn undo_of(result: &UpdateProjectResult) -> Option<UndoToken> {
  result.applied.as_ref().map(|applied| UndoToken {
    kind: UndoKind::Update,
    token: UpdateUndoToken {
      previous_head: result.check.head.clone(),
      new_head: applied.new_head.clone(),
    }
    .to_string(),
  })
}

/// The check and the move of the update of the operation of `guard` (`update_project_core` without a fetch) on the
/// blocking pool, the lock waited for as long as its holder lives (`StepWaiter`): its steps the check and the move
///
/// # Errors
/// The core's failure as the host words it; `canceled` once the user stopped the wait for the lock
async fn update(guard: &RunningGuard, key: &SessionKey, hosts: Hosts<'_>) -> Result<UpdateProjectResult, ApiError> {
  let (git, stepper) = (hosts.services.git(), guard.stepper());
  let params = UpdateProjectParams {
    repository_path: key.worktree.clone(),
    fetch: false,
  };
  let waiter = Arc::new(StepWaiter::new(guard, OperationStep::Updating));
  let core_waiter = Arc::clone(&waiter);
  let updated = blocking(move || {
    let on_progress = |progress: update_core::update_project::UpdateProgress| match progress.phase {
      UpdatePhase::Checking => stepper.step(OperationStep::Checking),
      UpdatePhase::Updating => stepper.step(OperationStep::Updating),
      // the waiter tells whom it waits for, and the fetch is the operation's own
      UpdatePhase::Waiting | UpdatePhase::Fetching => {}
    };
    Ok(update_project_core(&git, &params, &on_progress, LockWait::WhileHeld(&*core_waiter)))
  })
  .await?;
  match updated {
    Ok(result) => Ok(result),
    Err(_) if waiter.ended_by_cancel() => Err(wait_stopped()),
    Err(error) => Err(hosts.rewrites.rewrite_error(error)),
  }
}

/// Clears the report of what landed with an update in the session of `key` (`Intent::DismissLanded`): the user has seen
/// it, in every window and `bd`
///
/// # Errors
/// A failed write of the report, as the host words it
#[instrument(skip_all)]
pub async fn execute_dismiss(registry: &Registry, key: &SessionKey, hosts: Hosts<'_>, events: Arc<dyn EventSink>) -> Answer {
  let request = update_request(&Intent::DismissLanded, "");
  run(registry, key, request, events, async |guard| {
    let (git, path) = (hosts.services.git(), key.worktree.clone());
    let dismissed = blocking(move || Ok(dismiss_landed(&git, &path))).await?;
    dismissed.map_err(|error| hosts.rewrites.rewrite_error(error))?;
    let effects = vec![EffectRecord::new(PlannedEffect::DismissLanded, EffectStatus::Done)];
    Ok(outcome_of(guard.id().clone(), effects, None, None, OperationResult::None))
  })
  .await
}

#[cfg(test)]
#[path = "update_test.rs"]
mod update_test;
