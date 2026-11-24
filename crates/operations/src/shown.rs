//! The shown reviews of ship/show/ask after their start (docs/design/landing.md, "Show: a review after landing";
//! openspec/specs/branch/shown-reviews/spec.md). A Safe Push with a review after landing opens the draft before its
//! robot starts (`PlannedEffect::PushReview`, `CreateShownReview` of `land_execute`); then:
//!
//! - `Intent::OpenShownReview`, once the robot merged (the news `ShownReviewLanded`, `brd request-review
//!   --after-landing`): the branches of the review go to the commit the robot landed and its fork point (one atomic push,
//!   leased on what the remote has; none when they are there), the draft is marked ready for review, and the reviewers
//!   the record of its body names are asked. A review ready already only asks again those it asked who have not
//!   approved and are asked no more;
//! - `Intent::CloseShownReview`, once every reviewer it asked approved (the news `ShownReviewApproved`, `brd
//!   close-review`): the pull request is closed with a comment that names who approved it, and its branches are deleted,
//!   those the remote has, each leased on what it has. One not approved is refused unless the user says so.
//!
//! Each holds the fields of the pull request it writes and the branches of the review (`shown_request`); its effects
//! run in their order (`run_shown`), the first that fails skips the rest. The app runs them in the session of the
//! repository (`execute_shown`), `brd` over its one-shot reads with the same plans and the same run.

use crate::engine::{check_admitted, outcome_of, run};
use crate::events::EventSink;
use crate::land_execute::Accepted;
use crate::pull_request::{github_of, write_plan};
use crate::registry::{Answer, OperationRequest, Registry};
use crate::robot_start::github_failure;
use domain_policy::shown::{close_comment, close_refusal, landing_robot, reviewers_record};
use domain_types::branch_names::{baseline_on_remote, review_branch_names};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectRecord, EffectStatus, Intent, OperationResult, OperationStep, Plan, PullRequestField, Resource};
use domain_types::patronus::PatronusRobot;
use domain_types::pull_request::{CreatedPullRequest, RequestedReviewers, ReviewerState, ShownReview, ShownState};
use domain_types::reason::Reason;
use git_executor::git_command_executor::GitCommandExecutor;
use github::service::GitHubService;
use remote_facts::pull_request_create::{CreatePullRequestParams, create_pull_request};
use remote_facts::pull_request_writes::{close_pull_request, mark_ready_for_review, request_pull_request_reviewers};
use remote_facts::pull_requests::GitHubWriteError;
use remote_facts::service_sources::ServiceHost;
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::session::{SessionKey, Sessions};
use std::fmt::Write as _;
use std::sync::Arc;
use sync_core::github_repository::GitHubRepository;
use sync_core::review_head::{delete_review, push_review};
use sync_utils::span::in_current_span;
use tracing::instrument;

/// The body of the draft of a shown review: the text of the branch, what a shown review is, and the record of the
/// reviewers its open asks (`domain_policy::shown::reviewers_record`)
#[must_use]
pub fn shown_body(body: &str, reviewers: &[String], target: &str) -> String {
  let mut text = String::new();
  if !body.trim().is_empty() {
    text.push_str(body.trim_end());
    text.push_str("\n\n");
  }
  let _ = write!(
    text,
    "Shown after landing: these commits land in {target} with the Safe Push of the branch, and the review follows there. \
     This pull request goes into their fork point: it cannot be merged, and it closes once every reviewer approved."
  );
  text.push_str("\n\n");
  text.push_str(&reviewers_record(reviewers));
  text
}

/// The plan of the open of the shown review `review` of `branch` (module docs) over `robot`, the merged Safe Push that
/// landed it (`landing_robot`): a draft pushes its branches to the commit the robot took, is marked ready and asks its
/// reviewers; one ready asks again those it asked who neither approved nor are asked now
///
/// # Errors
/// `no_shown_review` for a draft whose branch no merged Safe Push landed yet, or one that records no reviewer
pub fn open_plan(review: &ShownReview, branch: &str, robot: Option<&PatronusRobot>) -> Result<Plan, ApiError> {
  let pull_request = &review.pull_request;
  let number = pull_request.number;
  let intent = Intent::OpenShownReview { branch: branch.to_owned() };
  if review.reviewers.is_empty() {
    return Err(ApiError::new(
      ErrorCode::NoShownReview,
      format!("The shown review #{number} of {branch} records no reviewer to ask: ask them on its page"),
    ));
  }
  let effects = if pull_request.is_draft {
    let Some(source) = robot.and_then(|robot| robot.source_revision.clone()) else {
      return Err(ApiError::new(
        ErrorCode::NoShownReview,
        format!("No merged Safe Push landed {branch} yet: its shown review #{number} opens once one did"),
      ));
    };
    vec![
      PlannedEffect::PushReview {
        branch: branch.to_owned(),
        tip: source,
      },
      PlannedEffect::MarkReady { number },
      PlannedEffect::RequestReviewers {
        number,
        reviewers: review.reviewers.clone(),
      },
    ]
  } else {
    let again: Vec<String> = review
      .reviewers
      .iter()
      .filter(|login| {
        !pull_request
          .reviewers
          .iter()
          .any(|reviewer| reviewer.login.eq_ignore_ascii_case(login) && (reviewer.requested || reviewer.state == ReviewerState::Approved))
      })
      .cloned()
      .collect();
    if again.is_empty() {
      Vec::new()
    } else {
      vec![PlannedEffect::RequestReviewers { number, reviewers: again }]
    }
  };
  Ok(write_plan(intent, effects, Vec::new(), Some(number), [pull_request], robot))
}

/// The plan of the shown review of `branch`, which a Safe Push at `source` landed without one (`brd request-review
/// --after-landing`): its branches pushed to `source` and its fork point, its pull request opened ready with `reviewers`
/// asked
#[must_use]
pub fn create_plan(branch: &str, source: &str, reviewers: &[String]) -> Plan {
  let effects = vec![
    PlannedEffect::PushReview {
      branch: branch.to_owned(),
      tip: source.to_owned(),
    },
    PlannedEffect::CreateShownReview {
      branch: branch.to_owned(),
      reviewers: reviewers.to_vec(),
      draft: false,
    },
  ];
  write_plan(Intent::OpenShownReview { branch: branch.to_owned() }, effects, Vec::new(), None, [], [])
}

/// The plan of the close of the shown review `review` of `branch`: its close with the comment that names who approved it
/// (in `target`), then the delete of its branches; refused (a blocker) while a reviewer it asked has not approved, unless
/// `allow_unapproved`
#[must_use]
pub fn close_plan(review: &ShownReview, branch: &str, allow_unapproved: bool, target: &str) -> Plan {
  let number = review.pull_request.number;
  let effects = vec![
    PlannedEffect::ClosePullRequest {
      branch: branch.to_owned(),
      number,
      comment: close_comment(review, target),
    },
    PlannedEffect::DeleteReview { branch: branch.to_owned() },
  ];
  let intent = Intent::CloseShownReview {
    branch: branch.to_owned(),
    allow_unapproved,
  };
  write_plan(
    intent,
    effects,
    close_refusal(review, allow_unapproved).into_iter().collect(),
    Some(number),
    [&review.pull_request],
    [],
  )
}

/// What an open or a close of the shown review of a branch under `prefix` asks the registry for: the reviewers (an open)
/// or the state (a close) of its pull request, and its branches, which it pushes or deletes
#[must_use]
pub fn shown_request(prefix: &str, intent: &Intent, plan: Option<&Plan>) -> OperationRequest {
  let (branch, field) = match intent {
    Intent::OpenShownReview { branch } => (branch.as_str(), PullRequestField::Reviewers),
    Intent::CloseShownReview { branch, .. } => (branch.as_str(), PullRequestField::State),
    _ => ("", PullRequestField::State),
  };
  let mut resources: Vec<Resource> =
    review_branch_names(prefix, branch).map_or_else(|_| Vec::new(), |(head, base)| vec![Resource::Ref { name: head }, Resource::Ref { name: base }]);
  resources.extend(plan.and_then(|plan| plan.pull_request).map(|number| Resource::PullRequest { number, field }));
  OperationRequest {
    intent: intent.clone(),
    resources,
    step: OperationStep::Writing,
    moves: Vec::new(),
  }
}

/// What the effects of a shown review run with
#[derive(Clone)]
pub struct ShownServices {
  pub git: GitCommandExecutor,
  pub repository_path: String,
  pub prefix: String,
  /// The remote its branches go to ("origin")
  pub remote: String,
  /// The baseline its fork point is on ("origin/master")
  pub baseline: String,
  pub github: Arc<GitHubService>,
  pub repository: GitHubRepository,
  /// The title and the text of the branch, of a shown review it opens (`CreateShownReview`); `None` where it opens none
  pub text: Option<ShownText>,
}

/// The title and the text of the branch a shown review opens with, its body adding what a shown review is and the record
/// of its reviewers (`shown_body`), and whether it is assigned to the user
#[derive(Clone, Debug, Default)]
pub struct ShownText {
  pub title: String,
  pub body: String,
  pub assign_self: bool,
  /// The branch of origin the robots push into ("master"), which the body names
  pub target: String,
}

/// What a run of the effects of a shown review did
#[derive(Debug, Default)]
pub struct ShownRun {
  pub records: Vec<EffectRecord>,
  /// Whom its review request asked
  pub requested: Option<RequestedReviewers>,
  /// The pull request it opened (or found open), `CreateShownReview`
  pub created: Option<CreatedPullRequest>,
}

/// The effects of `plan` in their order (module docs); the first that fails skips the rest
pub async fn run_shown(services: &ShownServices, plan: &Plan) -> ShownRun {
  let mut run = ShownRun::default();
  let mut failed = false;
  for effect in &plan.effects {
    let status = if failed {
      EffectStatus::Skipped { reason: None }
    } else {
      match shown_effect(services, effect, &mut run).await {
        Ok(()) => EffectStatus::Done,
        Err(error) => {
          failed = true;
          EffectStatus::Failed { error }
        }
      }
    };
    run.records.push(EffectRecord::new(effect.clone(), status));
  }
  run
}

/// One effect of a shown review
async fn shown_effect(services: &ShownServices, effect: &PlannedEffect, run: &mut ShownRun) -> Result<(), ApiError> {
  let (github, path, repository) = (&services.github, services.repository_path.as_str(), &services.repository);
  let github_error = |what: String| {
    move |error: GitHubWriteError| {
      let failure = github_failure(error);
      ApiError::new(failure.code, format!("{what}: {}", failure.message))
    }
  };
  match effect {
    PlannedEffect::PushReview { branch, tip } => {
      let (git, path, remote, prefix, branch, tip) = (
        services.git.clone(),
        path.to_owned(),
        services.remote.clone(),
        services.prefix.clone(),
        branch.clone(),
        tip.clone(),
      );
      let target_ref = format!("refs/remotes/{}", services.baseline);
      blocking(move || push_review(&git, &path, &remote, &prefix, &branch, &tip, &target_ref).map(|_| ())).await
    }
    PlannedEffect::MarkReady { number } => mark_ready_for_review(github, path, repository, *number)
      .await
      .map(|_| ())
      .map_err(github_error(format!("Cannot mark pull request #{number} ready for review"))),
    PlannedEffect::RequestReviewers { number, reviewers } => {
      let requested = request_pull_request_reviewers(github, path, repository, *number, reviewers)
        .await
        .map_err(github_error(format!("Cannot ask the reviewers of pull request #{number}")))?;
      let nobody = requested.requested.is_empty();
      let refused: Vec<String> = requested.refused.iter().map(|refused| format!("{}: {}", refused.login, refused.reason)).collect();
      run.requested = Some(requested);
      if nobody {
        return Err(ApiError::new(
          ErrorCode::ReviewRefused,
          format!("GitHub asked nobody to review pull request #{number}: {}", refused.join("; ")),
        ));
      }
      Ok(())
    }
    PlannedEffect::ClosePullRequest { number, comment, .. } => close_pull_request(github, path, repository, *number, comment)
      .await
      .map_err(github_error(format!("Cannot close pull request #{number}"))),
    PlannedEffect::CreateShownReview { branch, reviewers, draft } => {
      let text = services
        .text
        .clone()
        .ok_or_else(|| ApiError::internal("A shown review opens with the text of its branch"))?;
      let (head, base) = review_branch_names(&services.prefix, branch).map_err(|error| ApiError::new(ErrorCode::InvalidArguments, format!("{error:#}")))?;
      let params = CreatePullRequestParams {
        repository_path: path.to_owned(),
        branch_name: branch.clone(),
        head,
        base,
        title: text.title,
        body: shown_body(&text.body, reviewers, &text.target),
        reviewers: if *draft { Vec::new() } else { reviewers.clone() },
        assign_self: text.assign_self,
        wait_for_mirror: true,
        draft: *draft,
      };
      let created = create_pull_request(github, repository, &params)
        .await
        .map_err(github_error(format!("Cannot open the shown review of {branch}")))?;
      run.created = Some(created);
      Ok(())
    }
    PlannedEffect::DeleteReview { branch } => {
      let (git, path, remote, prefix, branch) = (services.git.clone(), path.to_owned(), services.remote.clone(), services.prefix.clone(), branch.clone());
      blocking(move || delete_review(&git, &path, &remote, &prefix, &branch).map(|_| ())).await
    }
    other => Err(ApiError::internal(format!("A shown review makes no {other:?}"))),
  }
}

async fn blocking(work: impl FnOnce() -> anyhow::Result<()> + Send + 'static) -> Result<(), ApiError> {
  tokio::task::spawn_blocking(in_current_span(work))
    .await
    .map_err(|error| ApiError::internal(format!("The step of the shown review failed: {error}")))?
    .map_err(|error| ApiError::new(ErrorCode::PushRefused, format!("{error:#}")))
}

/// The refusal of a plan of a shown review by its first blocker, in the words of the engine
#[must_use]
pub fn shown_refusal(plan: &Plan, branch: &str) -> Option<ApiError> {
  match plan.blockers.first()? {
    Reason::ReviewNotApproved { number, waiting } if waiting.is_empty() => Some(ApiError::new(
      ErrorCode::ReviewNotApproved,
      format!("Nothing was closed: the shown review #{number} of {branch} asked nobody yet; --allow-unapproved closes it anyway, only when the user says so"),
    )),
    Reason::ReviewNotApproved { number, waiting } => Some(ApiError::new(
      ErrorCode::ReviewNotApproved,
      format!(
        "Nothing was closed: {} has not approved the shown review #{number} of {branch}; --allow-unapproved closes it anyway, only when the user says so",
        waiting.join(", ")
      ),
    )),
    other => Some(ApiError::new(ErrorCode::PlanChanged, format!("Nothing was done: {other:?}"))),
  }
}

/// The plan of `intent` (an open or a close of a shown review) in the open session of `key`, over its listing of the
/// shown reviews and its robots; `None` for a session that is not open
///
/// # Errors
/// `no_shown_review`: the session lists no open shown review of the branch, or (an open) no merged Safe Push landed it
pub async fn prepare_shown(sessions: &Sessions, key: &SessionKey, intent: &Intent) -> Result<Option<Plan>, ApiError> {
  let intent = intent.clone();
  let read = sessions
    .read(key, move |state, facts| {
      let target = baseline_on_remote(&state.header().baseline_branch).to_owned();
      let shown = &facts.pull_requests.view().shown_reviews;
      match &intent {
        Intent::OpenShownReview { branch } => {
          let review = shown.get(branch).ok_or_else(|| no_shown_review(branch))?;
          open_plan(review, branch, landing_robot(facts.robots.view().robots.values(), branch)).map(Some)
        }
        Intent::CloseShownReview { branch, allow_unapproved } => {
          let review = shown.get(branch).ok_or_else(|| no_shown_review(branch))?;
          Ok(Some(close_plan(review, branch, *allow_unapproved, &target)))
        }
        _ => Ok(None),
      }
    })
    .await;
  read.unwrap_or(Ok(None))
}

/// "auth has no open shown review"
#[must_use]
pub fn no_shown_review(branch: &str) -> ApiError {
  ApiError::new(ErrorCode::NoShownReview, format!("{branch} has no open shown review"))
}

/// Runs the open or the close of a shown review (`intent`) in the session of `key` on the plan its caller accepted:
/// admitted (`shown_request`), planned again, refused by its first blocker, then its effects (`run_shown`) with the
/// GitHub service and repository `services` resolves; what it changed of the shown review goes into the session at once
///
/// # Errors
/// No open shown review of the branch, no merged Safe Push of it (an open), one not approved (a close), a changed plan
#[instrument(skip_all)]
pub async fn execute_shown(registry: &Registry, key: &SessionKey, intent: Intent, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  let sessions = registry.sessions();
  let first = prepare_shown(sessions, key, &intent).await?;
  let request = shown_request(&key.prefix, &intent, first.as_ref());
  run(registry, key, request, events, async |guard| {
    let branch = intent.subject().unwrap_or_default();
    let plan = prepare_shown(sessions, key, &intent).await?.ok_or_else(|| no_shown_review(&branch))?;
    if let Some(refusal) = shown_refusal(&plan, &branch) {
      return Err(refusal);
    }
    check_admitted(guard, &accepted, &plan)?;
    let (github, repository) = github_of(services, key).await?;
    let baseline = sessions.read(key, |state, _| state.header().baseline_branch.clone()).await.unwrap_or_default();
    let shown_services = ShownServices {
      git: services.git(),
      repository_path: key.worktree.clone(),
      prefix: key.prefix.clone(),
      remote: "origin".to_owned(),
      baseline,
      github,
      repository,
      text: None,
    };
    let done = run_shown(&shown_services, &plan).await;
    let all_done = done.records.iter().all(|record| record.status == EffectStatus::Done);
    let write = match &intent {
      // closed: no longer an open shown review; opened: the listing tells the reviewers asked
      Intent::CloseShownReview { .. } if all_done => Some(PullRequestWrite::Shown {
        branch: branch.clone(),
        review: None,
      }),
      Intent::OpenShownReview { .. } if all_done => sessions
        .read(key, {
          let branch = branch.clone();
          move |_, facts| facts.pull_requests.view().shown_reviews.get(&branch).cloned()
        })
        .await
        .flatten()
        .map(|review| PullRequestWrite::Shown {
          branch: branch.clone(),
          review: Some(Box::new(opened(review))),
        }),
      _ => None,
    };
    let version = match write {
      Some(write) => sessions.write_pull_request(key, write).await,
      None => None,
    };
    let result = done.requested.map_or(OperationResult::None, |requested| OperationResult::Reviewers(Box::new(requested)));
    Ok(outcome_of(guard.id().clone(), done.records, None, version, result))
  })
  .await
}

/// `review` as its open left it: ready for review, waiting for every reviewer it asked
#[must_use]
pub fn opened(mut review: ShownReview) -> ShownReview {
  review.pull_request.is_draft = false;
  review.state = ShownState::InReview {
    waiting: review.reviewers.clone(),
  };
  review
}

#[cfg(test)]
#[path = "shown_test.rs"]
mod shown_test;
