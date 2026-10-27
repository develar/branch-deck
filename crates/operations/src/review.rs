//! The review of a pull request of someone else and its Safe Push (`Intent::SubmitReview`,
//! spec/repository/review-requests.spec.md, "Reviews" and "Safe Push of a reviewed pull request"): the review of the
//! user on the commit the review shows (`PlannedEffect::SubmitReview`, one `POST pulls/<n>/reviews`), then for Approve
//! and Safe Push the `/patronus merge` comment (`PlannedEffect::StartRobot` by `StartRoute::Comment`, without
//! `--keep-source-branch`: the branch of someone else goes once merged), only while GitHub still shows the approved
//! commit (one `GET pulls/<n>`, else the start is skipped with `plan_changed`, and the approval stays). The bot's reply
//! is read on the clock of the session as for a start of the user's own branch (`robot_start::start_by_comment`); its
//! robot is looked for among the user's robots for `PATRONUS_REVIEWED_ROBOT_WAIT` after the reply (Patronus may count it
//! as the author's), and one found goes into the session. A review asks the session for the review requests again at
//! once, so the reviewed request leaves the list. The operation holds the review of the pull request
//! (`PullRequestField::Review`): a second write of the same pull request waits for it; the same one joins it. Nothing
//! of it reads the facts of the session: its plan is the intent's.

use crate::engine::{check_admitted, outcome_of, run};
use crate::events::EventSink;
use crate::land_execute::Accepted;
use crate::land_session::robots_since;
use crate::pull_request::{github_of, write_plan};
use crate::registry::{Answer, OperationRequest, Registry, RunningGuard};
use crate::robot_start::{CommentStart, RobotWait, github_request_failure, start_by_comment};
use domain_policy::refresh::PATRONUS_REVIEWED_ROBOT_WAIT;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{PlannedEffect, StartRoute};
use domain_types::operation::{EffectRecord, EffectStatus, Intent, NewsKey, OperationResult, OperationStep, Plan, PullRequestField, Resource, ReviewResult, StartComment};
use domain_types::patronus::{PatronusCommandSubject, PatronusPushMode, PatronusRobot};
use domain_types::pull_request::{PullRequestAvailability, PullRequestReviewEvent};
use domain_types::session::{FactFamily, FactRefreshReason};
use git_ops::revs::short_id;
use github::service::GitHubService;
use remote_facts::fact_sources::Read;
use remote_facts::pull_request_writes::{read_pull_request_head, submit_pull_request_review};
use remote_facts::pull_requests::GitHubWriteError;
use remote_facts::service_sources::ServiceHost;
use repository_model::session::{SessionKey, Sessions};
use std::collections::HashSet;
use std::sync::Arc;
use sync_core::github_repository::GitHubRepository;
use tracing::{Instrument, instrument};

/// The plan of `intent`, a review and its Safe Push: the review with its verdict (none: the user approved the commit
/// before), then with `then_land` the comment that starts the Safe Push
///
/// # Errors
/// `invalid_arguments` for no review and no Safe Push, or a Safe Push after a request for changes
pub fn review_plan(intent: &Intent) -> Result<Plan, ApiError> {
  let Intent::SubmitReview {
    number,
    verdict,
    commit,
    then_land,
    ..
  } = intent
  else {
    return Err(ApiError::new(ErrorCode::InvalidArguments, "No review of a pull request"));
  };
  if verdict.is_none() && !then_land || *verdict == Some(PullRequestReviewEvent::RequestChanges) && *then_land {
    return Err(ApiError::new(
      ErrorCode::InvalidArguments,
      "A review approves or requests changes, and only an approved pull request is Safe Pushed",
    ));
  }
  let review = verdict.map(|verdict| PlannedEffect::SubmitReview {
    number: *number,
    verdict,
    commit: commit.clone(),
  });
  let start = then_land.then_some(PlannedEffect::StartRobot {
    route: StartRoute::Comment { number: *number },
  });
  let effects = review.into_iter().chain(start).collect();
  Ok(write_plan(intent.clone(), effects, Vec::new(), Some(*number), [], []))
}

/// What a review asks the registry for: the review of its pull request (a second write of it waits, the same joins)
fn review_request(intent: &Intent, number: u32) -> OperationRequest {
  OperationRequest {
    intent: intent.clone(),
    resources: vec![Resource::PullRequest {
      number,
      field: PullRequestField::Review,
    }],
    step: OperationStep::Writing,
    moves: Vec::new(),
  }
}

/// Runs the review `intent` in the session of `key` on the plan its caller accepted: admitted over the operations of the
/// session, then the review and the Safe Push its plan orders, with the GitHub and Patronus `services` resolves; an
/// effect that fails skips the ones after it
///
/// # Errors
/// An invalid intent, a changed plan, no GitHub repository of the branches
#[instrument(skip_all)]
pub async fn execute_review(registry: &Registry, key: &SessionKey, intent: Intent, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  let plan = review_plan(&intent)?;
  let number = plan.pull_request.unwrap_or_default();
  let sessions = registry.sessions();
  run(registry, key, review_request(&intent, number), events, async |guard| {
    let began = std::time::Instant::now();
    check_admitted(guard, &accepted, &plan)?;
    let (github, repository) = review_github(sessions, services, key).await?;
    let mut review = Review {
      guard,
      sessions,
      services,
      key,
      github,
      repository,
      result: ReviewResult::default(),
      version: None,
    };
    let records = review.effects(&intent, plan.effects).await;
    let result = if review.result == ReviewResult::default() {
      OperationResult::None
    } else {
      OperationResult::Review(Box::new(review.result))
    };
    Ok(outcome_of(guard.id().clone(), records, Some(NewsKey::Review { number }), review.version, result, began))
  })
  .await
}

/// A review as it runs
struct Review<'a> {
  guard: &'a RunningGuard,
  sessions: &'a Arc<Sessions>,
  services: &'a dyn ServiceHost,
  key: &'a SessionKey,
  github: Arc<GitHubService>,
  repository: GitHubRepository,
  result: ReviewResult,
  version: Option<domain_types::session::Version>,
}

impl Review<'_> {
  /// Each effect in its order, until one fails or is skipped: the ones after it are skipped
  async fn effects(&mut self, intent: &Intent, effects: Vec<PlannedEffect>) -> Vec<EffectRecord> {
    let Intent::SubmitReview { commit, body, .. } = intent else {
      return Vec::new();
    };
    let mut stopped = false;
    let mut records = Vec::with_capacity(effects.len());
    for effect in effects {
      let status = match &effect {
        _ if stopped => EffectStatus::Skipped { reason: None },
        PlannedEffect::SubmitReview { number, verdict, commit } => self.submit(*number, *verdict, commit, body.as_deref()).await,
        PlannedEffect::StartRobot {
          route: StartRoute::Comment { number },
        } => self.start(*number, commit).await,
        _ => EffectStatus::Skipped { reason: None },
      };
      stopped = !matches!(status, EffectStatus::Done);
      records.push(EffectRecord { effect, status });
    }
    records
  }

  /// The review on GitHub; the session lists the review requests again at once, not waited for
  async fn submit(&self, number: u32, verdict: PullRequestReviewEvent, commit: &str, body: Option<&str>) -> EffectStatus {
    self.guard.step(OperationStep::Writing);
    let submitted = submit_pull_request_review(&self.github, &self.key.worktree, &self.repository, number, verdict, commit, body).await;
    if let Err(error) = submitted {
      return EffectStatus::Failed { error: review_failure(error) };
    }
    let (sessions, key) = (Arc::clone(self.sessions), self.key.clone());
    tokio::spawn(async move { sessions.refresh(&key, FactFamily::ReviewRequests, FactRefreshReason::Manual, None).await }.in_current_span());
    EffectStatus::Done
  }

  /// The `/patronus merge` comment, while GitHub shows the commit the user approved; its robot into the session
  async fn start(&mut self, number: u32, approved: &str) -> EffectStatus {
    self.guard.step(OperationStep::Starting);
    match read_pull_request_head(&self.github, &self.key.worktree, &self.repository, number).await {
      Ok(head) if head == approved => {}
      Ok(head) => {
        let reason = ApiError::new(
          ErrorCode::PlanChanged,
          format!("#{number} has new commits since you reviewed {}: review {} first", short_id(approved), short_id(&head)),
        );
        return EffectStatus::Skipped { reason: Some(reason) };
      }
      Err(error) => return EffectStatus::Failed { error: review_failure(error) },
    }
    let (patronus, space) = match self.patronus().await {
      Ok(patronus) => patronus,
      Err(error) => return EffectStatus::Failed { error },
    };
    let known = self
      .sessions
      .read(self.key, |_, facts| facts.robots.view().robots.keys().cloned().collect::<HashSet<_>>())
      .await
      .unwrap_or_default();
    let start = CommentStart {
      github: &self.github,
      repository_path: &self.key.worktree,
      repository: &self.repository,
      number,
      mode: PatronusPushMode::RebaseAutosquash,
      known,
      subject: PatronusCommandSubject::ReviewedPullRequest,
      robot_wait: RobotWait::For(PATRONUS_REVIEWED_ROBOT_WAIT),
    };
    let (prefix, new_api) = (self.key.prefix.as_str(), self.services.new_api(self.key));
    let listed = |since: u32| -> Read<'_, Result<Vec<PatronusRobot>, ApiError>> { Box::pin(robots_since(&patronus, &space, prefix, new_api, since)) };
    let clock = self.sessions.clock(self.key);
    match start_by_comment(start, clock.as_ref(), &listed).await {
      Ok(commented) => {
        if let Some(robot) = &commented.robot {
          self.version = self.sessions.write_robot(self.key, robot.clone()).await;
        }
        self.result = ReviewResult {
          comment: Some(StartComment {
            number,
            url: commented.comment_url,
            accepted: commented.accepted,
          }),
          robot: commented.robot,
        };
        EffectStatus::Done
      }
      Err(error) => EffectStatus::Failed { error },
    }
  }

  /// The Patronus service of the session and the Space repository the user's robots are listed in
  async fn patronus(&self) -> Result<(Arc<patronus::service::PatronusService>, String), ApiError> {
    let space = self
      .services
      .space_repository(self.key)
      .await?
      .ok_or_else(|| ApiError::new(ErrorCode::NotOnSpace, "The repository is not on Space: Patronus can't merge the pull request"))?;
    Ok((self.services.patronus(self.key)?, space.name))
  }
}

/// The GitHub service of the session of `key` and the repository its review requests were read from, else the one the
/// branches are mirrored to
///
/// # Errors
/// Neither is known, a host without GitHub
async fn review_github(sessions: &Sessions, services: &dyn ServiceHost, key: &SessionKey) -> Result<(Arc<GitHubService>, GitHubRepository), ApiError> {
  let listed = sessions
    .read(key, |_, facts| match &facts.review_requests.view().head.value {
      Some(PullRequestAvailability::Available { owner, name }) => Some(GitHubRepository {
        owner: owner.clone(),
        name: name.clone(),
      }),
      _ => None,
    })
    .await
    .flatten();
  match listed {
    Some(repository) => Ok((services.github(key)?, repository)),
    None => github_of(services, key).await,
  }
}

/// A write or a read of the review GitHub refused, in its words ("User can only have one pending review per pull
/// request"), or one the core refused before asking (no commit id, a request for changes without a comment)
fn review_failure(error: GitHubWriteError) -> ApiError {
  match error {
    GitHubWriteError::GitHub(failure) => github_request_failure(&failure),
    GitHubWriteError::Refused(message) => ApiError::new(ErrorCode::InvalidArguments, message),
  }
}

#[cfg(test)]
#[path = "review_test.rs"]
mod review_test;
