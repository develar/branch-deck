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

use crate::engine::run;
use crate::events::EventSink;
use crate::land_execute::Accepted;
use crate::registry::{Answer, OperationRequest, Registry};
use crate::robot_start::github_failure;
use crate::witness::{Witness, check};
use domain_types::branch_names::baseline_on_remote;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::PlannedEffect;
use domain_types::operation::{
  EffectRecord, EffectStatus, Fingerprint, Intent, OperationId, OperationResult, OperationStats, OperationStep, Outcome, Plan, PullRequestField, Resource,
};
use domain_types::patronus::PatronusRobot;
use domain_types::pull_request::{PullRequest, PullRequestState};
use domain_types::reason::Reason;
use domain_types::session::PullRequestsView;
use github::service::GitHubService;
use remote_facts::pull_request_writes::retarget_pull_request;
use remote_facts::pull_requests::GitHubWriteError;
use remote_facts::service_sources::ServiceHost;
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::session::{SessionKey, Sessions};
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
fn find(pull_requests: &PullRequestsView, number: u32) -> Option<(&str, &PullRequest)> {
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
  let mut plan = Plan {
    intent: Intent::Retarget { number },
    fingerprint: Fingerprint(String::new()),
    lands: Vec::new(),
    effects,
    confirmations: Vec::new(),
    blockers,
    waits: Vec::new(),
    alternatives: None,
    pull_request: Some(number),
    approvals_skipped_by: None,
  };
  plan.fingerprint = Witness::of(std::collections::BTreeMap::new(), [pull_request], std::iter::empty::<&PatronusRobot>()).fingerprint(&plan);
  Some(plan)
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

/// The `PATCH` of the retarget with the GitHub service and repository of the session of `key`
async fn write_retarget(services: &dyn ServiceHost, key: &SessionKey, number: u32, base: &str) -> Result<(), ApiError> {
  let repository = services
    .github_repository(key)
    .await?
    .ok_or_else(|| ApiError::new(ErrorCode::InvalidArguments, "The GitHub repository of the branches is unknown"))?;
  let github = services.github(key)?;
  retarget(&github, &key.worktree, &repository, number, base).await.map_err(|error| {
    let failure = github_failure(error);
    ApiError::new(failure.code, format!("Cannot retarget pull request #{number} to {base}: {}", failure.message))
  })
}

/// The outcome of a retarget
fn retarget_outcome(op: OperationId, effects: Vec<EffectRecord>, version: Option<domain_types::session::Version>, began: std::time::Instant) -> Outcome {
  Outcome {
    op,
    effects,
    // the retarget of the user's own pull request is no event of an object with a toast of its own
    news_key: None,
    version_after: version,
    result: OperationResult::None,
    stats: OperationStats {
      millis: u32::try_from(began.elapsed().as_millis()).unwrap_or(u32::MAX),
      ..OperationStats::default()
    },
  }
}
