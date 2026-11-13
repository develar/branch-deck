//! The fetch of the pull request a review window shows (`Intent::FetchReview`; docs/design/git-operations.md, "Review
//! fetch"): an operation of the session of its repository, which names the pull request by its number and takes its head
//! and its base as the review requests of the session have them, the last a listing named; the head it fetched goes to
//! the session (`Sessions::review_fetched`), which asks the readiness of the pull request from then on. The core
//! (`sync_core::review_fetch::fetch_review_head`) fetches nothing when both commits are local, else writes only
//! `refs/branch-deck/review/<n>/{head,base}`, never under the repository lock, and answers the range to review. It holds
//! the review refs of its number (`Resource::Ref`): a second fetch of the same pull request joins it. Its progress goes
//! to its caller; `Registry::cancel` stops it (`canceled`), as does a fetch silent for 120 s. The refs of a pull request
//! no review window shows any more go with the next change of the windows of the session (`Sessions` prunes them,
//! `SessionHost::prune_review_refs`).
//!
//! The fetch of a stack (`Intent::FetchReviewStack`, Review Stack) names its top and its bottom: its layers are the chain
//! between them as the review requests of the session link them now (`ReviewRequestsFact::stack`), its core
//! (`sync_core::review_fetch::fetch_review_stack`) one fetch of the head of the top with the base of the bottom, cut
//! into the layers, a restacked layer a fetch of its own. It holds the review refs of the top, where it writes: a fetch
//! of the top alone waits for it. Each layer's head goes to the session, as the head of a pull request a window shows.

use crate::engine::{Hosts, outcome_of, plain_plan, run};
use crate::events::EventSink;
use crate::registry::{Answer, OperationRequest, Registry};
use crate::rewrite::blocking;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectRecord, EffectStatus, Intent, OperationId, OperationResult, OperationStep, Outcome, Plan, Resource};
use domain_types::review_request::{FetchReviewHeadParams, ReviewRequest, ReviewStack, ReviewStackLayer};
use repository_model::session::{SessionKey, Sessions};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use sync_core::review_fetch::{REVIEW_REF_PREFIX, fetch_review_head, fetch_review_stack};
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
    let (git, stepper, canceled) = (hosts.services.git(), guard.stepper(), guard.canceled());
    let fetched = blocking(move || Ok(fetch_review_head(&git, &params, &mut |tick| stepper.progress(&tick), &|| canceled.load(Ordering::Relaxed)))).await?;
    if let Ok(range) = &fetched {
      // the head the review refs hold now: the session asks the readiness of the pull request at it
      sessions.review_fetched(key, number, range.head.clone()).await;
    }
    Ok(fetch_outcome(guard.id(), hosts, number, fetched.map(|range| OperationResult::ReviewRange(Box::new(range)))))
  })
  .await
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
      // the head each layer shows: the session asks the readiness of each pull request at it
      for layer in &stack.layers {
        sessions.review_fetched(key, layer.request.number, layer.range.head.clone()).await;
      }
    }
    Ok(fetch_outcome(guard.id(), hosts, top, stack.map(|stack| OperationResult::ReviewStack(Box::new(stack)))))
  })
  .await
}

#[cfg(test)]
#[path = "review_fetch_test.rs"]
mod review_fetch_test;
