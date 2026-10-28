//! The fetch of the pull request a review window shows (`Intent::FetchReview`; docs/design/git-operations.md, "Review
//! fetch"): an operation of the session of its repository, which names the pull request by its number and takes its head
//! and its base as the review requests of the session have them, the last a listing named. The core
//! (`sync_core::review_fetch::fetch_review_head`) fetches nothing when both commits are local, else writes only
//! `refs/branch-deck/review/<n>/{head,base}`, never under the repository lock, and answers the range to review. It holds
//! the review refs of its number (`Resource::Ref`): a second fetch of the same pull request joins it. Its progress goes
//! to its caller; `Registry::cancel` stops it (`canceled`), as does a fetch silent for 120 s. The refs of a pull request
//! no review window shows any more go with the next change of the windows of the session (`Sessions` prunes them,
//! `SessionHost::prune_review_refs`).

use crate::engine::{Hosts, outcome_of, plain_plan, run};
use crate::events::EventSink;
use crate::registry::{Answer, OperationRequest, Registry};
use crate::rewrite::blocking;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectRecord, EffectStatus, Intent, OperationResult, OperationStep, Plan, Resource};
use domain_types::review_request::FetchReviewHeadParams;
use repository_model::session::{SessionKey, Sessions};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use sync_core::review_fetch::{REVIEW_REF_PREFIX, fetch_review_head};
use tracing::instrument;

/// The plan of the fetch of the pull request `number` (`engine::prepare`): its one effect; it asks nothing to accept.
/// `None` for a session that is not open
pub async fn prepare_review_fetch(sessions: &Sessions, key: &SessionKey, number: u32) -> Option<Plan> {
  sessions.read(key, |_, _| ()).await?;
  Some(plain_plan(Intent::FetchReview { number }, vec![PlannedEffect::FetchReview { number }]))
}

/// What the fetch of the pull request `number` asks the registry for: its review refs
#[must_use]
pub fn review_fetch_request(number: u32) -> OperationRequest {
  OperationRequest {
    intent: Intent::FetchReview { number },
    resources: vec![Resource::Ref {
      name: format!("{REVIEW_REF_PREFIX}{number}"),
    }],
    step: OperationStep::Fetching,
    moves: Vec::new(),
  }
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
  run(registry, key, review_fetch_request(number), events, async |guard| {
    // the caller learns its operation, which the close of its window stops
    guard.stepper().step(OperationStep::Fetching);
    let request = sessions
      .read(key, move |_, facts| facts.review_requests.pull_request(number).cloned())
      .await
      .flatten()
      .ok_or_else(|| ApiError::new(ErrorCode::InvalidArguments, format!("No listing of the review requests names pull request #{number}")))?;
    let params = hosts.rewrites.review_fetch(FetchReviewHeadParams {
      repository_path: key.worktree.clone(),
      number,
      head_ref_name: request.head_ref_name,
      head_ref_oid: request.head_ref_oid,
      base_ref_name: request.base_ref_name,
      base_ref_oid: request.base_ref_oid,
      is_cross_repository: request.is_cross_repository,
    })?;
    let (git, stepper, canceled) = (hosts.services.git(), guard.stepper(), guard.canceled());
    let fetched = blocking(move || Ok(fetch_review_head(&git, &params, &mut |tick| stepper.progress(&tick), &|| canceled.load(Ordering::Relaxed)))).await?;
    let (status, result) = match fetched {
      Ok(range) => (EffectStatus::Done, OperationResult::ReviewRange(Box::new(range))),
      Err(error) => {
        let mut error = hosts.rewrites.rewrite_error(error);
        error.operation_id.get_or_insert_with(|| guard.id().0.clone());
        (EffectStatus::Failed { error }, OperationResult::None)
      }
    };
    let effects = vec![EffectRecord::new(PlannedEffect::FetchReview { number }, status)];
    Ok(outcome_of(guard.id().clone(), effects, None, None, result))
  })
  .await
}

#[cfg(test)]
#[path = "review_fetch_test.rs"]
mod review_fetch_test;
