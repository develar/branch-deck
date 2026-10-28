//! Cancel Auto-Merge (`Intent::DisableAutoMerge`, spec/branch/landing-routes.spec.md, "Merge"): the auto-merge of an
//! open pull request of the session, which a start of the pull-request route enabled (`PlannedEffect::EnableAutoMerge`),
//! is disabled on GitHub, so GitHub merges it no more by itself. It holds the auto-merge of the pull request
//! (`PullRequestField::AutoMerge`), so a second cancel joins it and a retarget or a review request of the same pull
//! request runs beside; its write goes into the session at once (`PullRequestWrite::AutoMerge`).

use crate::{
  engine::{check_admitted, outcome_of, run},
  events::EventSink,
  land_execute::Accepted,
  pull_request::{find, github_of, write_plan},
  registry::{Answer, OperationRequest, Registry},
  robot_start::github_failure,
};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectRecord, EffectStatus, Intent, OperationResult, OperationStep, Plan, PullRequestField, Resource};
use domain_types::pull_request::PullRequestState;
use domain_types::reason::Reason;
use domain_types::session::PullRequestsView;
use remote_facts::pull_request_writes::{MergeWrite, disable_auto_merge};
use remote_facts::service_sources::ServiceHost;
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::session::{SessionKey, Sessions};
use std::sync::Arc;
use tracing::instrument;

/// The plan of the cancel of the auto-merge of the pull request `number` over `pull_requests`, the listing of the
/// session: `[DisableAutoMerge]` of an open one whose auto-merge is enabled, nothing of one without it; a merged one is
/// refused; `None` for one the listing lacks or a closed one. Its fingerprint holds the pull request as listed
#[must_use]
pub fn disable_plan(pull_requests: &PullRequestsView, number: u32) -> Option<Plan> {
  let (_, pull_request) = find(pull_requests, number).filter(|(_, pull_request)| pull_request.state != PullRequestState::Closed)?;
  let merged = pull_request.state == PullRequestState::Merged;
  let blockers = if merged { vec![Reason::PullRequestMerged { number }] } else { Vec::new() };
  let effects: Vec<PlannedEffect> = pull_request
    .auto_merge
    .filter(|_| !merged)
    .map(|_| PlannedEffect::DisableAutoMerge { number })
    .into_iter()
    .collect();
  Some(write_plan(Intent::DisableAutoMerge { number }, effects, blockers, Some(number), [pull_request], []))
}

/// The plan of the cancel of the auto-merge of the pull request `number` in the open session of `key` (`disable_plan`)
pub async fn prepare_disable(sessions: &Sessions, key: &SessionKey, number: u32) -> Option<Plan> {
  sessions.read(key, move |_, facts| disable_plan(facts.pull_requests.view(), number)).await.flatten()
}

/// What a cancel of an auto-merge asks the registry for: the auto-merge of the pull request
pub(crate) fn disable_request(number: u32) -> OperationRequest {
  OperationRequest {
    intent: Intent::DisableAutoMerge { number },
    resources: vec![Resource::PullRequest {
      number,
      field: PullRequestField::AutoMerge,
    }],
    step: OperationStep::Writing,
    moves: Vec::new(),
  }
}

/// Runs the cancel of the auto-merge of the pull request `number` in the session of `key` on the plan its caller
/// accepted: admitted over the operations of the session, planned again, then the mutation with the GitHub service and
/// repository `services` resolves; the auto-merge off into the session. What GitHub refused is the status of its effect
///
/// # Errors
/// A pull request the session lacks, a changed plan, a merged one, no GitHub repository of the branches
#[instrument(skip_all, fields(number))]
pub async fn execute_disable(registry: &Registry, key: &SessionKey, number: u32, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  let sessions = registry.sessions();
  run(registry, key, disable_request(number), events, async |guard| {
    let nothing = |why: String| format!("Nothing was canceled: {why}");
    let plan = prepare_disable(sessions, key, number)
      .await
      .ok_or_else(|| ApiError::new(ErrorCode::NoPullRequest, nothing(format!("the session lists no open pull request #{number}"))))?;
    if !plan.blockers.is_empty() {
      return Err(ApiError::new(ErrorCode::Landed, nothing(format!("pull request #{number} is merged"))));
    }
    check_admitted(guard, &accepted, &plan)?;
    // one effect at most: the cancel of its auto-merge, none of a pull request without one
    let Some(effect) = plan.effects.first().cloned() else {
      return Ok(outcome_of(guard.id().clone(), Vec::new(), None, None, OperationResult::None));
    };
    let (status, version) = if let Err(error) = write_disable(services, key, number).await {
      (EffectStatus::Failed { error }, None)
    } else {
      let version = sessions.write_pull_request(key, PullRequestWrite::AutoMerge { number, method: None }).await;
      (EffectStatus::Done, version)
    };
    Ok(outcome_of(
      guard.id().clone(),
      vec![EffectRecord::new(effect, status)],
      None,
      version,
      OperationResult::None,
    ))
  })
  .await
}

/// The mutation of the cancel with the GitHub service and repository of the session of `key`
async fn write_disable(services: &dyn ServiceHost, key: &SessionKey, number: u32) -> Result<(), ApiError> {
  let (github, repository) = github_of(services, key).await?;
  let written = disable_auto_merge(&github, &key.worktree, &repository, number).await;
  written.map(|_: MergeWrite| ()).map_err(|error| {
    let failure = github_failure(error);
    ApiError::new(failure.code, format!("Cannot cancel the auto-merge of pull request #{number}: {}", failure.message))
  })
}

#[cfg(test)]
#[path = "auto_merge_test.rs"]
mod auto_merge_test;
