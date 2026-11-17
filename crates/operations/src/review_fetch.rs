//! The fetch of the pull request a review window shows (`Intent::FetchReview`; docs/design/git-operations.md, "Review
//! fetch"): an operation of the session of its repository, which names the pull request by its number and takes its head
//! and its base as the review requests of the session have them, the last a listing named; the head it fetched goes to
//! the session (`Sessions::review_fetched`), which asks the readiness of the pull request from then on. The core
//! (`sync_core::review_fetch::fetch_review_head`) fetches nothing when both commits are local, else writes only
//! `refs/branch-deck/review/<n>/{head,base}`, never under the repository lock, and answers the range to review. It holds
//! the review refs of its number (`Resource::Ref`): a second fetch of the same pull request joins it. Its progress goes
//! to its caller; `Registry::cancel` stops it (`canceled`), as does a fetch silent for 120 s. Before it writes, it tells the
//! session (`Sessions::review_fetching`), whose prunes keep its refs from then on and which answers once the prunes it
//! decided before ended, a wait that a stop of the fetch ends, and a prune that hangs for 120 s: the window asks for the
//! fetch apart from its demand, which may reach the session later, and a listing that no longer names the pull request
//! in between would prune the refs of the review on screen. Its end goes
//! to the session too (`CountedFetch`): the head it brought, else that it ended without one (a failure, a stop, its
//! operation dropped), which the session drops while no window shows the pull request. The refs of a pull request no
//! review window shows any more go with the next change of the windows of the session (`Sessions` prunes them,
//! `SessionHost::prune_review_refs`).
//!
//! The fetch of a stack (`Intent::FetchReviewStack`, Review Stack) names its top and its bottom: its layers are the chain
//! between them as the review requests of the session link them now (`ReviewRequestsFact::stack`), its core
//! (`sync_core::review_fetch::fetch_review_stack`) one fetch of the head of the top with the base of the bottom, cut
//! into the layers, a restacked layer a fetch of its own. It holds the review refs of the top, where it writes: a fetch
//! of the top alone waits for it. Each layer's head goes to the session, as the head of a pull request a window shows.

use crate::engine::{Hosts, outcome_of, plain_plan, run};
use crate::events::EventSink;
use crate::registry::{Answer, OperationRequest, Registry, RunningGuard};
use crate::rewrite::blocking;
use anyhow::anyhow;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectRecord, EffectStatus, Intent, OperationId, OperationResult, OperationStep, Outcome, Plan, Resource};
use domain_types::review_request::{FetchReviewHeadParams, ReviewRequest, ReviewStack, ReviewStackLayer};
use git_executor::git_command_executor::StoppedCommand;
use repository_model::session::{SessionKey, Sessions};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use sync_core::review_fetch::{REVIEW_FETCH_IDLE_TIMEOUT, REVIEW_REF_PREFIX, fetch_review_head, fetch_review_stack};
use tracing::instrument;

/// The plan of the fetch of the pull request `number` (`engine::prepare`): its one effect; it asks nothing to accept.
/// `None` for a session that is not open
pub async fn prepare_review_fetch(sessions: &Sessions, key: &SessionKey, intent: Intent) -> Option<Plan> {
  sessions.read(key, |_, _| ()).await?;
  let number = written_number(&intent)?;
  Some(plain_plan(intent, vec![PlannedEffect::FetchReview { number }]))
}

/// The pull request whose review refs a review fetch writes: the one it fetches, of a stack the top
const fn written_number(intent: &Intent) -> Option<u32> {
  match intent {
    Intent::FetchReview { number } | Intent::FetchReviewStack { top: number, .. } => Some(*number),
    _ => None,
  }
}

/// What a review fetch asks the registry for: the review refs of the pull request it writes (of a stack its top)
#[must_use]
pub fn review_fetch_request(intent: Intent) -> OperationRequest {
  let resources = written_number(&intent)
    .map(|number| Resource::Ref {
      name: format!("{REVIEW_REF_PREFIX}{number}"),
    })
    .into_iter()
    .collect();
  OperationRequest {
    intent,
    resources,
    step: OperationStep::Fetching,
    moves: Vec::new(),
  }
}

/// What the fetch of `request` fetches: its head and its base as GitHub names them, through the host
fn fetch_params(key: &SessionKey, hosts: Hosts<'_>, request: ReviewRequest) -> Result<FetchReviewHeadParams, ApiError> {
  hosts.rewrites.review_fetch(FetchReviewHeadParams {
    repository_path: key.worktree.clone(),
    number: request.number,
    head_ref_name: request.head_ref_name,
    head_ref_oid: request.head_ref_oid,
    base_ref_name: request.base_ref_name,
    base_ref_oid: request.base_ref_oid,
    is_cross_repository: request.is_cross_repository,
  })
}

/// Fetches the pull request `number` of the session of `key` for its review (`Intent::FetchReview`): its head and its
/// base as the review requests of the session name them (the host may fill what a fixture leaves out,
/// `RewriteHost::review_fetch`), its progress and its outcome, the range, to `events`
///
/// # Errors
/// A pull request the session knows no listing of, a session that closed; a fetch that failed or was stopped is the
/// failed effect of its outcome
#[instrument(skip_all, fields(number))]
pub async fn execute_review_fetch(registry: &Registry, key: &SessionKey, number: u32, hosts: Hosts<'_>, events: Arc<dyn EventSink>) -> Answer {
  fetch_review_within(registry, key, number, hosts, events, REVIEW_FETCH_IDLE_TIMEOUT).await
}

/// `execute_review_fetch` whose wait for the prunes of its session ends after `limit` (`CountedFetch::start`)
///
/// # Errors
/// As `execute_review_fetch`
pub(crate) async fn fetch_review_within(registry: &Registry, key: &SessionKey, number: u32, hosts: Hosts<'_>, events: Arc<dyn EventSink>, limit: Duration) -> Answer {
  let sessions = registry.sessions();
  run(registry, key, review_fetch_request(Intent::FetchReview { number }), events, async |guard| {
    // the caller learns its operation, which the close of its window stops
    guard.stepper().step(OperationStep::Fetching);
    let request = sessions
      .read(key, move |_, facts| facts.review_requests.pull_request(number).cloned())
      .await
      .flatten()
      .ok_or_else(|| ApiError::new(ErrorCode::InvalidArguments, format!("No listing of the review requests names pull request #{number}")))?;
    let params = fetch_params(key, hosts, request)?;
    // the refs it writes stay from now, whether or not the demand of its window reached the session yet
    let counted = match CountedFetch::start(sessions, key, number, guard, limit).await {
      Ok(counted) => counted,
      Err(stopped) => return Ok(fetch_outcome(guard.id(), hosts, number, Err(stopped))),
    };
    let (git, stepper, canceled) = (hosts.services.git(), guard.stepper(), guard.canceled());
    let fetched = blocking(move || Ok(fetch_review_head(&git, &params, &mut |tick| stepper.progress(&tick), &|| canceled.load(Ordering::Relaxed)))).await?;
    if let Ok(range) = &fetched {
      // the head the review refs hold now: the session asks the readiness of the pull request at it
      counted.brought();
      sessions.review_fetched(key, number, range.head.clone()).await;
    }
    Ok(fetch_outcome(guard.id(), hosts, number, fetched.map(|range| OperationResult::ReviewRange(Box::new(range)))))
  })
  .await
}

/// A review fetch of the pull request `number` (of a stack its top) as the session counts it, from its start
/// (`Sessions::review_fetching`) to its end: the head it brought (`brought`, then `Sessions::review_fetched`), else, as
/// it drops, that it ended without one (`Sessions::review_fetch_ended`): a failure, a stop (of its git or of its wait for
/// the prunes), the `?` of an error, its operation dropped while it waited
struct CountedFetch<'a> {
  sessions: &'a Sessions,
  key: &'a SessionKey,
  number: u32,
  brought: bool,
}

impl<'a> CountedFetch<'a> {
  /// Tells the session that the fetch starts, and resolves once its refs are safe to write: the prunes the session
  /// decided before ended. A stop of the operation (`Registry::cancel`) ends that wait, as for the git of the fetch
  /// (`StoppedCommand::Cancelled`, canceled), and so does a wait of `limit`, as long as the fetch may stay silent
  /// (`REVIEW_FETCH_IDLE_TIMEOUT`): a prune that hangs keeps no fetch. The fetch ended then, and a drop while it waits
  /// ends it all the same, as the start reached the session with the first poll
  ///
  /// # Errors
  /// Why the fetch ended before it wrote
  async fn start(sessions: &'a Sessions, key: &'a SessionKey, number: u32, guard: &RunningGuard, limit: Duration) -> anyhow::Result<Self> {
    let counted = Self {
      sessions,
      key,
      number,
      brought: false,
    };
    tokio::select! {
      // the start reaches the session first, so its end pairs with it
      biased;
      () = sessions.review_fetching(key, number) => Ok(counted),
      () = guard.stopped() => Err(StoppedCommand::Cancelled.into()),
      () = tokio::time::sleep(limit) => Err(anyhow!("The review refs of #{number} stayed busy with a prune for {limit:?}")),
    }
  }

  /// The fetch brought its head, which the caller tells the session next: its end is that
  fn brought(mut self) {
    self.brought = true;
  }
}

impl Drop for CountedFetch<'_> {
  fn drop(&mut self) {
    if !self.brought {
      self.sessions.review_fetch_ended(self.key, self.number);
    }
  }
}

/// The outcome of a review fetch of the pull request `number` (of a stack its top): its one effect, done with what it
/// answered, or failed with the error as the host words it
fn fetch_outcome(id: &OperationId, hosts: Hosts<'_>, number: u32, fetched: anyhow::Result<OperationResult>) -> Outcome {
  let (status, result) = match fetched {
    Ok(result) => (EffectStatus::Done, result),
    Err(error) => {
      let mut error = hosts.rewrites.rewrite_error(error);
      error.operation_id.get_or_insert_with(|| id.0.clone());
      (EffectStatus::Failed { error }, OperationResult::None)
    }
  };
  outcome_of(id.clone(), vec![EffectRecord::new(PlannedEffect::FetchReview { number }, status)], None, None, result)
}

/// Fetches the stack of `top` down to `bottom` of the session of `key` for its review (`Intent::FetchReviewStack`): its
/// layers as the review requests of the session link them now, each one's head and base as they name them (the host may
/// fill what a fixture leaves out), its progress and its outcome, the ranges of the stack and of each layer, to `events`
///
/// # Errors
/// A top the session knows no listing of, a session that closed; a fetch that failed or was stopped is the failed effect
/// of its outcome
#[instrument(skip_all, fields(top, bottom))]
pub async fn execute_review_stack_fetch(registry: &Registry, key: &SessionKey, top: u32, bottom: u32, hosts: Hosts<'_>, events: Arc<dyn EventSink>) -> Answer {
  let sessions = registry.sessions();
  run(registry, key, review_fetch_request(Intent::FetchReviewStack { top, bottom }), events, async |guard| {
    guard.stepper().step(OperationStep::Fetching);
    let requests = sessions.read(key, move |_, facts| facts.review_requests.stack(top, bottom)).await.unwrap_or_default();
    if requests.is_empty() {
      return Err(ApiError::new(
        ErrorCode::InvalidArguments,
        format!("No listing of the review requests names pull request #{top}"),
      ));
    }
    let params = requests.iter().map(|request| fetch_params(key, hosts, request.clone())).collect::<Result<Vec<_>, _>>()?;
    let counted = match CountedFetch::start(sessions, key, top, guard, REVIEW_FETCH_IDLE_TIMEOUT).await {
      Ok(counted) => counted,
      Err(stopped) => return Ok(fetch_outcome(guard.id(), hosts, top, Err(stopped))),
    };
    let (git, stepper, canceled) = (hosts.services.git(), guard.stepper(), guard.canceled());
    let fetched = blocking(move || Ok(fetch_review_stack(&git, &params, &mut |tick| stepper.progress(&tick), &|| canceled.load(Ordering::Relaxed)))).await?;
    let stack = fetched.map(|ranges| ReviewStack {
      range: ranges.range,
      layers: requests
        .into_iter()
        .zip(ranges.layers)
        .map(|(request, layer)| ReviewStackLayer {
          request,
          range: layer.range,
          restacked: layer.restacked,
        })
        .collect(),
    });
    if let Ok(stack) = &stack {
      // the head each layer shows: the session asks the readiness of each pull request at it, and decides Approve N Pull
      // Requests of the stack on them
      let layers = stack.layers.iter().map(|layer| (layer.request.number, layer.range.head.clone())).collect();
      counted.brought();
      sessions.review_stack_fetched(key, top, layers).await;
    }
    Ok(fetch_outcome(guard.id(), hosts, top, stack.map(|stack| OperationResult::ReviewStack(Box::new(stack)))))
  })
  .await
}

#[cfg(test)]
#[path = "review_fetch_test.rs"]
mod review_fetch_test;
