//! The review of a pull request of someone else and its Safe Push (`Intent::SubmitReview`,
//! openspec/specs/repository/review-requests/spec.md, "Reviews" and "Safe Push of a reviewed pull request"): the review of the
//! user on the commit the review shows (`PlannedEffect::SubmitReview`, one `POST pulls/<n>/reviews`), then for Approve
//! and Safe Push the `/patronus merge` comment (`PlannedEffect::StartRobot` by `StartRoute::Comment`, without
//! `--keep-source-branch`: the branch of someone else goes once merged), only while GitHub still shows the approved
//! commit (one `GET pulls/<n>`, else the start is skipped with `plan_changed`, and the approval stays). The bot's reply
//! is read on the clock of the session as for a start of the user's own branch (`robot_start::start_by_comment`); its
//! robot is looked for among the user's robots for `PATRONUS_REVIEWED_ROBOT_WAIT` after the reply (Patronus may count it
//! as the author's), and one found goes into the session. A review asks the session for the review requests again at
//! once, so the reviewed request leaves the list. The operation holds the review of the pull request
//! (`PullRequestField::Review`): a second write of the same pull request waits for it; the same one joins it. Its plan
//! is the intent's; only a Safe Push reads the session: it runs where the session offers it, as the menus decide it
//! (`repository_model::review_view::review_actions_of`, `domain_policy::actions::review_actions`), else its plan and
//! its run refuse it before the review with the code of its blocker (`safe_push_refusal`), so a window of a state
//! before (Patronus had not answered, the pull request went into another base) or another client approves nothing.
//!
//! Approve N Pull Requests of a stack a review window shows (`Intent::ApproveStack`, "Review window: Approve N Pull
//! Requests approves every layer the window shows") holds the review of every layer: its plan is an approval of each
//! layer on the head the window showed, but those the session knows the user approved on it
//! (`domain_policy::stack_approval`); before the first write GitHub's head of every layer is read again (one `GET
//! pulls/<n>` each, side by side), and the layer whose head moved, with every layer above it, is skipped with the
//! reason by its number (`plan_changed`), the layers below it approved.

use crate::engine::{check_admitted, outcome_of, run};
use crate::events::EventSink;
use crate::land_execute::Accepted;
use crate::land_session::robots_since;
use crate::pull_request::{github_of, write_plan};
use crate::registry::{Answer, OperationRequest, Registry, RunningGuard};
use crate::robot_start::{CommentStart, NoRobot, RobotWait, github_request_failure, start_by_comment};
use domain_policy::refresh::PATRONUS_REVIEWED_ROBOT_WAIT;
use domain_policy::stack_approval::{LayerCheck, LayerFacts, head_checks, stack_approval};
use domain_types::actions::{ActionBlocker, ActionId, Availability};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{PlannedEffect, StartRoute};
use domain_types::next_step::StepFact;
use domain_types::operation::{
  EffectRecord, EffectStatus, Intent, NewsKey, OperationResult, OperationStep, Plan, PullRequestField, Resource, ReviewResult, ReviewedLayer, StartComment,
};
use domain_types::patronus::{PatronusCommandSubject, PatronusPushMode, PatronusRobot};
use domain_types::pull_request::PullRequestReviewEvent;
use domain_types::session::{FactFamily, FactRefreshReason};
use git_ops::revs::short_id;
use github::service::GitHubService;
use remote_facts::fact_sources::Read;
use remote_facts::pull_request_writes::{read_pull_request_head, submit_pull_request_review};
use remote_facts::pull_requests::GitHubWriteError;
use remote_facts::service_sources::ServiceHost;
use repository_model::review_view::{pull_key, review_actions_of, reviews_view};
use repository_model::session::{SessionKey, Sessions};
use std::collections::{BTreeMap, HashMap, HashSet};
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

/// The plan of the review `intent` in the session of `key` (`review_plan`), refused as its run refuses it where the
/// session offers its Safe Push not (`safe_push_refusal`); `None` for a Safe Push in a session that is not open
///
/// # Errors
/// As `review_plan`; the refusal of its Safe Push
pub async fn prepare_review(sessions: &Sessions, key: &SessionKey, intent: &Intent) -> Result<Option<Plan>, ApiError> {
  let plan = review_plan(intent)?;
  let Some(number) = safe_pushed(intent) else {
    return Ok(Some(plan));
  };
  let Some(offer) = offered_safe_push(sessions, key, number).await else {
    return Ok(None);
  };
  safe_push_refusal(number, offer).map_or(Ok(Some(plan)), Err)
}

/// The pull request whose Safe Push the review `intent` starts (`then_land`)
const fn safe_pushed(intent: &Intent) -> Option<u32> {
  match intent {
    Intent::SubmitReview { number, then_land: true, .. } => Some(*number),
    _ => None,
  }
}

/// What the session offers of the Safe Push of a reviewed pull request, as the menus have it
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SafePushOffer {
  /// The session has no actions of the pull request: none it lists to review
  NoPullRequest,
  /// It offers no Safe Push of it: Patronus merges nothing in the repository
  NoSafePush,
  /// Its Safe Push, enabled, disabled with its blocker, or running
  Offered(Availability),
}

/// The Safe Push of the pull request `number` in the session of `key` as the menus have it now (`review_actions_of`
/// over the view of Reviews); `None` for a session that is not open
async fn offered_safe_push(sessions: &Sessions, key: &SessionKey, number: u32) -> Option<SafePushOffer> {
  sessions
    .read(key, move |state, facts| {
      // the own pull requests that are the user's move change no action: no steps
      let reviews = reviews_view(state, facts, &BTreeMap::new());
      let actions = review_actions_of(state, facts, &reviews).remove(&pull_key(number));
      actions.map_or(SafePushOffer::NoPullRequest, |actions| {
        actions
          .into_iter()
          .find(|action| action.id == ActionId::SafePushPullRequest)
          .map_or(SafePushOffer::NoSafePush, |action| SafePushOffer::Offered(action.availability))
      })
    })
    .await
}

/// What refuses the Safe Push of the review `intent` in the session of `key` once its operation was admitted, before
/// the review: `safe_push_refusal` of what the session offers now. Nothing for a review without one
///
/// # Errors
/// The refusal of `safe_push_refusal`; `invalid_arguments` for a session that is not open
async fn refuse_unoffered_safe_push(sessions: &Sessions, key: &SessionKey, intent: &Intent) -> Result<(), ApiError> {
  let Some(number) = safe_pushed(intent) else {
    return Ok(());
  };
  let offer = offered_safe_push(sessions, key, number)
    .await
    .ok_or_else(|| ApiError::new(ErrorCode::InvalidArguments, "The repository has no session to review in"))?;
  safe_push_refusal(number, offer).map_or(Ok(()), Err)
}

/// The refusal of the Safe Push of the reviewed pull request `number` by what the session offers of it, with the code
/// `bd` would fail with and the words of the menu; `None` where it runs (or a start of it joins the one that runs).
/// Nothing was approved or started then
#[must_use]
pub fn safe_push_refusal(number: u32, offer: SafePushOffer) -> Option<ApiError> {
  let refusal = |code: ErrorCode, words: String| Some(ApiError::new(code, format!("{words}: nothing was approved or started")));
  let reason = match offer {
    SafePushOffer::NoPullRequest => return refusal(ErrorCode::NoPullRequest, format!("The repository lists no pull request #{number} to review")),
    SafePushOffer::NoSafePush => return refusal(ErrorCode::NotOnSpace, "Patronus merges no pull request of this repository".to_owned()),
    SafePushOffer::Offered(Availability::Disabled { reason }) => reason,
    SafePushOffer::Offered(Availability::Enabled | Availability::Running { .. } | Availability::Queued { .. } | Availability::Waits { .. }) => return None,
  };
  match reason {
    ActionBlocker::Pending { fact: StepFact::Patronus } => refusal(ErrorCode::PlanChanged, "Waits for the answer of Patronus".to_owned()),
    ActionBlocker::TargetUnread => refusal(ErrorCode::PlanChanged, "Reading the branch Patronus pushes into".to_owned()),
    ActionBlocker::Cached => refusal(ErrorCode::PlanChanged, "Waits for the first sync: the rows are the frame of an earlier run".to_owned()),
    ActionBlocker::NoTarget => refusal(ErrorCode::NotOnSpace, "Patronus pushes into a branch of origin: the baseline is none".to_owned()),
    ActionBlocker::OtherBase { number, base, target } => refusal(
      ErrorCode::PullRequestOtherBase,
      format!("Patronus starts a Safe Push only from a pull request into {target}: #{number} goes into {base}"),
    ),
    ActionBlocker::OwnPullRequest => refusal(ErrorCode::ReviewRefused, format!("GitHub takes no review of its author: #{number} is the user's own")),
    ActionBlocker::Shown { number } => refusal(
      ErrorCode::PullRequestOtherBase,
      format!("#{number} is a shown review: what it shows is in master already, and its review gates no Safe Push"),
    ),
    // no other blocker refuses the Safe Push of a pull request to review (`review_actions`)
    other => refusal(ErrorCode::PlanChanged, format!("The Safe Push of #{number} waits: {other:?}")),
  }
}

/// What a review asks the registry for: the review of each pull request it writes (a second write of one waits, the
/// same intent joins); of Approve N Pull Requests every layer of the stack
pub(crate) fn review_request(intent: &Intent) -> OperationRequest {
  let numbers = match intent {
    Intent::SubmitReview { number, .. } => vec![*number],
    Intent::ApproveStack { layers, .. } => layers.iter().map(|layer| layer.number).collect(),
    _ => Vec::new(),
  };
  OperationRequest {
    intent: intent.clone(),
    resources: numbers
      .into_iter()
      .map(|number| Resource::PullRequest {
        number,
        field: PullRequestField::Review,
      })
      .collect(),
    step: OperationStep::Writing,
    moves: Vec::new(),
  }
}

/// The plan of Approve N Pull Requests (`Intent::ApproveStack`) over `known`, each layer of the intent as the session
/// knows it: an approval of each layer it approves (`domain_policy::stack_approval`), the bottom first, on the head the
/// window showed; a layer approved on that head, or the user's own, is no effect
///
/// # Errors
/// `invalid_arguments` for another intent, or a stack without layers
pub fn approve_stack_plan(intent: &Intent, known: &[LayerFacts<'_>]) -> Result<Plan, ApiError> {
  let Intent::ApproveStack { layers, .. } = intent else {
    return Err(ApiError::new(ErrorCode::InvalidArguments, "No approval of a stack of pull requests"));
  };
  if layers.is_empty() {
    return Err(ApiError::new(ErrorCode::InvalidArguments, "A stack to approve has layers"));
  }
  let effects = stack_approval(known)
    .approves
    .into_iter()
    .map(|layer| PlannedEffect::SubmitReview {
      number: layer.number,
      verdict: PullRequestReviewEvent::Approve,
      commit: layer.commit,
    })
    .collect();
  Ok(write_plan(intent.clone(), effects, Vec::new(), None, [], []))
}

/// The plan of Approve N Pull Requests in the session of `key` (`approve_stack_plan`), each layer with the user's
/// approval of it the session has; `None` for a session that is not open
///
/// # Errors
/// As `approve_stack_plan`
pub async fn prepare_approve_stack(sessions: &Sessions, key: &SessionKey, intent: &Intent) -> Result<Option<Plan>, ApiError> {
  let Intent::ApproveStack { layers, .. } = intent else {
    return approve_stack_plan(intent, &[]).map(Some);
  };
  let layers = layers.clone();
  let known = sessions
    .read(key, move |_, facts| {
      let fact = &facts.review_requests;
      layers
        .iter()
        .map(|layer| {
          let request = fact.pull_request(layer.number);
          (
            layer.clone(),
            request.and_then(|request| fact.approved(request)).map(str::to_owned),
            fact.is_own(layer.number),
          )
        })
        .collect::<Vec<_>>()
    })
    .await;
  let Some(known) = known else { return Ok(None) };
  let facts: Vec<LayerFacts<'_>> = known
    .iter()
    .map(|(layer, approved, own)| LayerFacts {
      number: layer.number,
      head: &layer.commit,
      approved: approved.as_deref(),
      own: *own,
    })
    .collect();
  approve_stack_plan(intent, &facts).map(Some)
}

/// Runs Approve N Pull Requests (`Intent::ApproveStack`) in the session of `key` on the plan its caller accepted: planned
/// again once admitted, GitHub's head of every layer read side by side, then the approval of each layer of the plan
/// bottom first, each refused by its number where a head moved (`domain_policy::stack_approval::head_checks`); a
/// refused review of one layer stops none of the others. A read of the heads that fails approves nothing
///
/// # Errors
/// An invalid intent, a changed plan, no GitHub repository of the requests, a read of a head that failed
#[instrument(skip_all, fields(top))]
pub async fn execute_approve_stack(registry: &Registry, key: &SessionKey, intent: Intent, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  let Intent::ApproveStack { top, layers } = &intent else {
    return Err(ApiError::new(ErrorCode::InvalidArguments, "No approval of a stack of pull requests"));
  };
  tracing::Span::current().record("top", top);
  let sessions = registry.sessions();
  run(registry, key, review_request(&intent), events, async |guard| {
    let plan = prepare_approve_stack(sessions, key, &intent)
      .await?
      .ok_or_else(|| ApiError::new(ErrorCode::InvalidArguments, "The repository has no session to review in"))?;
    check_admitted(guard, &accepted, &plan)?;
    if plan.effects.is_empty() {
      return Ok(outcome_of(guard.id().clone(), Vec::new(), None, None, OperationResult::None));
    }
    let (github, repository) = review_github(sessions, services, key).await?;
    guard.step(OperationStep::Checking);
    let heads = read_heads(&github, &key.worktree, &repository, layers).await.map_err(review_failure)?;
    let checks: HashMap<u32, LayerCheck> = layers.iter().map(|layer| layer.number).zip(head_checks(layers, &heads)).collect();
    let review = Review {
      guard,
      sessions,
      services,
      key,
      github,
      repository,
      result: ReviewResult::default(),
      version: None,
    };
    let mut records = Vec::with_capacity(plan.effects.len());
    for effect in plan.effects {
      let status = match &effect {
        PlannedEffect::SubmitReview { number, verdict, commit } => match checks.get(number) {
          Some(LayerCheck::Approve) => review.submit(*number, *verdict, commit, None).await,
          Some(LayerCheck::Moved { head }) => refused(format!(
            "The head of #{number} moved from {} to {} since the window fetched it: fetch the stack again to read the new commits",
            short_id(commit),
            short_id(head)
          )),
          Some(LayerCheck::BelowMoved { below }) => refused(format!("#{number} is built on #{below}, whose head moved since the window fetched it")),
          None => EffectStatus::Skipped { reason: None },
        },
        _ => EffectStatus::Skipped { reason: None },
      };
      records.push(EffectRecord::new(effect, status));
    }
    Ok(outcome_of(guard.id().clone(), records, None, None, OperationResult::None))
  })
  .await
}

/// A layer the heads refuse: skipped with the reason, by its number
fn refused(message: String) -> EffectStatus {
  EffectStatus::Skipped {
    reason: Some(ApiError::new(ErrorCode::PlanChanged, message)),
  }
}

/// GitHub's head of each of `layers` now, in their order: one `GET pulls/<n>` each, side by side
async fn read_heads(github: &Arc<GitHubService>, repository_path: &str, repository: &GitHubRepository, layers: &[ReviewedLayer]) -> Result<Vec<String>, GitHubWriteError> {
  let mut reads = tokio::task::JoinSet::new();
  for (index, layer) in layers.iter().enumerate() {
    let (github, repository_path, repository, number) = (Arc::clone(github), repository_path.to_owned(), repository.clone(), layer.number);
    reads.spawn(async move { (index, read_pull_request_head(&github, &repository_path, &repository, number).await) }.in_current_span());
  }
  let mut heads = vec![String::new(); layers.len()];
  while let Some(read) = reads.join_next().await {
    let (index, head) = read.map_err(|error| GitHubWriteError::Refused(format!("The read of a head of the stack stopped: {error}")))?;
    heads[index] = head?;
  }
  Ok(heads)
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
  run(registry, key, review_request(&intent), events, async |guard| {
    // the Safe Push as the session offers it once admitted, whatever plan the caller showed: no review of a pull
    // request whose Safe Push it would not start
    refuse_unoffered_safe_push(sessions, key, &intent).await?;
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
    Ok(outcome_of(guard.id().clone(), records, Some(NewsKey::Review { number }), review.version, result))
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
      records.push(EffectRecord::new(effect, status));
    }
    records
  }

  /// The review on GitHub, into the session; the session lists the review requests again at once (with the rest of
  /// Reviews while a window shows it), not waited for
  async fn submit(&self, number: u32, verdict: PullRequestReviewEvent, commit: &str, body: Option<&str>) -> EffectStatus {
    self.guard.step(OperationStep::Writing);
    let submitted = submit_pull_request_review(&self.github, &self.key.worktree, &self.repository, number, verdict, commit, body).await;
    if let Err(error) = submitted {
      return EffectStatus::Failed { error: review_failure(error) };
    }
    // the actions of the pull request know the review until a listing gives it
    self.sessions.write_review(self.key, number, verdict, commit.to_owned()).await;
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
      Err(NoRobot::Refused(error)) => EffectStatus::Failed { error },
      // the post went out and its answer was lost: the comment may be there, never posted again
      Err(NoRobot::Lost(_)) => EffectStatus::Unknown,
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
  let listed = sessions.read(key, |_, facts| facts.review_requests.github_repository()).await.flatten();
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
