//! The state of a YouTrack issue (`Intent::SetIssueState`, spec/repository/youtrack.spec.md, "Mark Fixed"): Mark
//! Fixed sets the state field of an open issue to Fixed, and its undo is the same intent, back to the value the field
//! had, only while the issue is still in the state the change set (`expected`). One command of YouTrack after a read of
//! the issue (`YouTrackService::set_state`), with the YouTrack of the app; the operation holds the issue
//! (`Resource::Issue`), so a second click joins it and its undo waits for it. Its outcome tells the event of the issue
//! (`NewsKey::Issue`): the toast of the change and of its undo is one.

use crate::engine::{outcome_of, run};
use crate::events::EventSink;
use crate::land_execute::Accepted;
use crate::pull_request::write_plan;
use crate::registry::{Answer, OperationRequest, Registry};
use crate::witness::check;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectRecord, EffectStatus, Intent, NewsKey, OperationResult, OperationStep, Plan, Resource};
use remote_facts::service_sources::ServiceHost;
use repository_model::session::SessionKey;
use std::sync::Arc;
use tracing::instrument;

/// The plan of `intent`, the change of the state of an issue: the change itself, its plan the intent's
///
/// # Errors
/// `invalid_arguments` for another intent
pub fn issue_plan(intent: &Intent) -> Result<Plan, ApiError> {
  let Intent::SetIssueState { key, state, .. } = intent else {
    return Err(ApiError::new(ErrorCode::InvalidArguments, "No change of the state of an issue"));
  };
  let effects = vec![PlannedEffect::SetIssueState {
    key: key.clone(),
    state: state.clone(),
  }];
  Ok(write_plan(intent.clone(), effects, Vec::new(), None, [], []))
}

/// Runs the change of the state of an issue (`intent`) in the session of `key` on the plan its caller accepted, with the
/// YouTrack `services` names: admitted over the operations of the session (it holds the issue), then the command; one
/// YouTrack refuses (the issue moved on, it has no state field, the sign-in expired) is the status of its effect
///
/// # Errors
/// Another intent, a changed plan, a host without YouTrack
#[instrument(skip_all)]
pub async fn execute_issue(registry: &Registry, key: &SessionKey, intent: Intent, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  let plan = issue_plan(&intent)?;
  let Intent::SetIssueState { key: issue, state, expected } = &intent else {
    return Err(ApiError::new(ErrorCode::InvalidArguments, "No change of the state of an issue"));
  };
  let request = OperationRequest {
    intent: intent.clone(),
    resources: vec![Resource::Issue { key: issue.clone() }],
    step: OperationStep::Writing,
    moves: Vec::new(),
  };
  run(registry, key, request, events, async |guard| {
    let began = std::time::Instant::now();
    check(&accepted.fingerprint, &plan.fingerprint)?;
    let youtrack = services.youtrack(key)?;
    let (status, result) = match youtrack.set_state(issue, state, expected.as_deref()).await {
      Ok(change) => (EffectStatus::Done, OperationResult::Issue(Box::new(change))),
      Err(message) => (
        EffectStatus::Failed {
          error: ApiError::internal(message),
        },
        OperationResult::None,
      ),
    };
    let effects = plan.effects.into_iter().map(|effect| EffectRecord { effect, status: status.clone() }).collect();
    Ok(outcome_of(guard.id().clone(), effects, Some(NewsKey::Issue { key: issue.clone() }), None, result, began))
  })
  .await
}

#[cfg(test)]
#[path = "issue_test.rs"]
mod issue_test;
