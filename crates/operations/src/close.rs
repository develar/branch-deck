//! The closes of pull requests of the virtual branches (docs/design/landing.md, "Every pull request goes into master"):
//! the close the user asks with its reason (`Intent::ClosePullRequest`: the pull request of a whole stack that no
//! longer matches it), and the closes of the pull requests a merged robot landed with it (`Intent::CloseLandedPullRequests`,
//! D7), which the page asks for by the news of the merge in the session of the robot's repository. Which pull requests
//! the merge closes is the rule of the news (`domain_policy::landed::pull_requests_to_close` over the session's
//! table, `FactStore::closes`); the comment of each names the robot ("Landed in master with #7, the Safe Push of
//! user/virtual/oauth, which is built on this branch."). Each close holds the state of its pull request
//! (`PullRequestField::State`), is one `GET`, a comment and a `PATCH` (`close_pull_request`: a closed one gets no second
//! comment, a merged one can't be closed), goes into the session at once (`PullRequestWrite::Closed`), and one GitHub
//! refuses stops none of the others. `bd` closes none: `bd robot` lists them (`closes`).

use crate::engine::{check_admitted, outcome_of, run};
use crate::events::EventSink;
use crate::land_execute::Accepted;
use crate::pull_request::{find, github_of, write_plan};
use crate::registry::{Answer, OperationRequest, Registry};
use crate::robot_start::github_failure;
use domain_types::branch_names::{to_final_branch_name, to_stack_branch_name};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{LandedPullRequest, PlannedEffect, Subject};
use domain_types::operation::{EffectRecord, EffectStatus, Intent, NewsKey, OperationId, OperationResult, OperationStep, Outcome, Plan, PullRequestField, Resource};
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::pull_request::{PullRequest, PullRequestState};
use domain_types::reason::Reason;
use domain_types::session::{PullRequestsView, Version};
use remote_facts::pull_request_writes::close_pull_request;
use remote_facts::service_sources::ServiceHost;
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::session::{SessionKey, Sessions};
use std::sync::Arc;
use tracing::instrument;

/// The plan of the close of the pull request `number` with `comment` over `pull_requests`, the listing of the session:
/// `[ClosePullRequest]` of an open one, nothing of a closed one (it gets no second comment), a merged one refused;
/// `None` for one the listing lacks. Its fingerprint holds the pull request as listed
#[must_use]
pub fn close_plan(pull_requests: &PullRequestsView, number: u32, comment: &str) -> Option<Plan> {
  let (branch, pull_request) = find(pull_requests, number)?;
  let (effects, blockers) = match pull_request.state {
    PullRequestState::Open => (
      vec![PlannedEffect::ClosePullRequest {
        branch: branch.to_string(),
        number,
        comment: comment.to_string(),
      }],
      Vec::new(),
    ),
    PullRequestState::Closed => (Vec::new(), Vec::new()),
    PullRequestState::Merged => (Vec::new(), vec![Reason::PullRequestMerged { number }]),
  };
  let intent = Intent::ClosePullRequest {
    number,
    comment: comment.to_string(),
  };
  Some(write_plan(intent, effects, blockers, Some(number), [pull_request], []))
}

/// The plan of the closes of the open pull requests the merged `robot` landed with it, `closes` as the news of its merge
/// names them (`FactStore::closes`), each found in `pull_requests` with a comment that names the robot (`landed_comment`,
/// its branches under `prefix`); `None` for a robot that did not merge (a Dry Run pushes nothing)
#[must_use]
pub fn close_landed_plan(robot: &PatronusRobot, closes: &[LandedPullRequest], pull_requests: &PullRequestsView, prefix: &str) -> Option<Plan> {
  if robot.status != PatronusRobotStatus::Success || robot.push_mode == PatronusPushMode::DryRun {
    return None;
  }
  let comment = landed_comment(robot, prefix);
  let closed: Vec<&PullRequest> = closes.iter().filter_map(|landed| find(pull_requests, landed.number).map(|(_, found)| found)).collect();
  let effects = closes
    .iter()
    .map(|landed| PlannedEffect::ClosePullRequest {
      branch: landed.branch.clone(),
      number: landed.number,
      comment: comment.clone(),
    })
    .collect();
  let intent = Intent::CloseLandedPullRequests { robot_id: robot.id.clone() };
  Some(write_plan(intent, effects, Vec::new(), None, closed, [robot]))
}

/// "Landed in master with #7, the Safe Push of user/virtual/oauth, which is built on this branch." / "Landed in master
/// with the Safe Push of the whole air stack (user/stack/air), which holds this branch."
#[must_use]
pub fn landed_comment(robot: &PatronusRobot, prefix: &str) -> String {
  let (by, what) = match (&robot.branch_name, &robot.stack_root) {
    (Some(name), _) => (
      format!("the Safe Push of {}", to_final_branch_name(prefix, name).unwrap_or_else(|_| name.clone())),
      "which is built on this branch",
    ),
    (None, root) => {
      let root = root.as_deref().unwrap_or_default();
      let stack = to_stack_branch_name(prefix, root).unwrap_or_else(|_| root.to_string());
      (format!("the Safe Push of {} ({stack})", Subject::whole_stack(root).words()), "which holds this branch")
    }
  };
  match &robot.pull_request {
    Some(pull_request) => format!("Landed in {} with #{}, {by}, {what}.", robot.target_branch, pull_request.number),
    None => format!("Landed in {} with {by}, {what}.", robot.target_branch),
  }
}

/// The plan of `intent` (a close, or the closes of a merged robot) in the open session of `key`, over the listing of its
/// pull requests and its robots as they are now; `None` for a session that is not open, a pull request it lists not, a
/// robot it lists not or that did not merge
pub async fn prepare_close(sessions: &Sessions, key: &SessionKey, intent: &Intent) -> Option<Plan> {
  let (prefix, intent) = (key.prefix.clone(), intent.clone());
  sessions
    .read(key, move |state, facts| match &intent {
      Intent::ClosePullRequest { number, comment } => close_plan(facts.pull_requests.view(), *number, comment),
      Intent::CloseLandedPullRequests { robot_id } => {
        let robot = facts.robots.view().robots.get(robot_id)?;
        close_landed_plan(robot, &facts.closes(state, robot_id), facts.pull_requests.view(), &prefix)
      }
      _ => None,
    })
    .await
    .flatten()
}

/// What closes ask the registry for: the state of each pull request they close (a second close of the same joins, a
/// retarget or a review request of it runs beside)
pub(crate) fn close_request(intent: &Intent, plan: Option<&Plan>) -> OperationRequest {
  let resources = plan
    .map(|plan| {
      plan.effects.iter().filter_map(|effect| match effect {
        PlannedEffect::ClosePullRequest { number, .. } => Some(Resource::PullRequest {
          number: *number,
          field: PullRequestField::State,
        }),
        _ => None,
      })
    })
    .into_iter()
    .flatten()
    .collect();
  OperationRequest {
    intent: intent.clone(),
    resources,
    step: OperationStep::Writing,
    moves: Vec::new(),
  }
}

/// Runs the close or the closes of a merged robot (`intent`) in the session of `key` on the plan its caller accepted:
/// admitted over the operations of the session (it holds the state of each pull request it closes), planned again, then
/// each close with the GitHub service and repository `services` resolves, into the session at once; a close GitHub
/// refuses is the status of its effect, and the others go on
///
/// # Errors
/// A pull request or a robot the session has no plan of, a merged pull request, a changed plan
#[instrument(skip_all)]
pub async fn execute_close(registry: &Registry, key: &SessionKey, intent: Intent, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  let sessions = registry.sessions();
  let request = close_request(&intent, prepare_close(sessions, key, &intent).await.as_ref());
  run(registry, key, request, events, async |guard| {
    let plan = prepare_close(sessions, key, &intent).await.ok_or_else(|| no_plan(&intent))?;
    if let Some(Reason::PullRequestMerged { number }) = plan.blockers.first() {
      return Err(ApiError::new(ErrorCode::Landed, format!("Nothing was closed: pull request #{number} is merged")));
    }
    check_admitted(guard, &accepted, &plan)?;
    let mut version = None;
    let mut records = Vec::with_capacity(plan.effects.len());
    for effect in plan.effects {
      let status = match &effect {
        PlannedEffect::ClosePullRequest { number, comment, .. } => match close(services, key, *number, comment).await {
          Ok(()) => {
            version = sessions.write_pull_request(key, PullRequestWrite::Closed { number: *number }).await.or(version);
            EffectStatus::Done
          }
          Err(error) => EffectStatus::Failed { error },
        },
        _ => EffectStatus::Skipped { reason: None },
      };
      records.push(EffectRecord { effect, status });
    }
    Ok(close_outcome(guard.id().clone(), &intent, records, version))
  })
  .await
}

/// The intent has no plan in the session: "Nothing was closed: the session lists no pull request #12"
fn no_plan(intent: &Intent) -> ApiError {
  match intent {
    Intent::CloseLandedPullRequests { robot_id } => ApiError::new(ErrorCode::InvalidArguments, format!("Nothing was closed: the session lists no merged robot {robot_id}")),
    Intent::ClosePullRequest { number, .. } => ApiError::new(ErrorCode::NoPullRequest, format!("Nothing was closed: the session lists no pull request #{number}")),
    _ => ApiError::new(ErrorCode::InvalidArguments, "No close of a pull request"),
  }
}

/// The comment and the close of the pull request `number` with the GitHub service and repository of the session of `key`
async fn close(services: &dyn ServiceHost, key: &SessionKey, number: u32, comment: &str) -> Result<(), ApiError> {
  let (github, repository) = github_of(services, key).await?;
  close_pull_request(&github, &key.worktree, &repository, number, comment).await.map_err(|error| {
    let failure = github_failure(error);
    ApiError::new(failure.code, format!("Cannot close pull request #{number}: {}", failure.message))
  })
}

/// The outcome of closes: the closes of a merged robot are a state of its merge, whose toast says them
fn close_outcome(op: OperationId, intent: &Intent, effects: Vec<EffectRecord>, version: Option<Version>) -> Outcome {
  let news_key = match intent {
    Intent::CloseLandedPullRequests { robot_id } => Some(NewsKey::Robot { robot_id: robot_id.clone() }),
    _ => None,
  };
  outcome_of(op, effects, news_key, version, OperationResult::None)
}

#[cfg(test)]
#[path = "close_test.rs"]
mod close_test;
