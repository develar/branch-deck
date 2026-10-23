//! The code-owner review a Safe Push needs (PAT-A-109), asked before the start. Branches reviewed in GitHub pull
//! requests are checked with the pull request the robot would start from (`review_check`): Patronus evaluates its
//! reviews as that robot would, and the caller caches the answer (`remote_facts::review_readiness`). A repository only on
//! Space asks in the cheap order (`review_requirement`): the check without a merge request first (no Space call); only
//! when approvals are missing, the merge request of the branch, then the check with it, cached here by the commits it
//! checked. The pre-check is the heaviest call of the Patronus API.

use crate::actions::{ActionError, availability_message};
use crate::client::{GitHubPullRequestDto, MergeRequestDto, PatronusError, PreMergeCheckRequest, PreMergeCheckResponse, PreMergeCommitDto};
use crate::service::PatronusService;
use domain_types::patronus::{
  PatronusAvailability, PatronusMergeRequest, PatronusReviewRequirement, PatronusReviewer, PatronusReviewerGroup, ReviewPathsSource, ReviewerApprovalState,
};
use std::time::{Duration, Instant};
use tracing::instrument;

/// No approval needed: holds until the commits change
const NOT_REQUIRED_TTL: Duration = Duration::from_secs(600);
/// A Recheck asks at once
const REQUIRED_TTL: Duration = Duration::from_secs(120);
/// A branch without a merge request may get one in the web UI
const NO_MERGE_REQUEST_TTL: Duration = Duration::from_secs(300);

/// The message of a 503 of the pre-check: the server gave up on it (after 10 s)
pub const REVIEW_CHECK_UNAVAILABLE: &str = "The review check of Patronus is unavailable right now";

/// What the pre-check of a branch is about: its commits on the target
pub struct ReviewSubject<'a> {
  pub project_key: &'a str,
  pub repository: &'a str,
  pub target_branch: &'a str,
  /// The pushed name, e.g. `user/virtual/IJPL-1`
  pub source_branch: &'a str,
  /// Newest first
  pub commits: &'a [PreMergeCommitDto],
  /// The parent of the oldest commit: with the tip it names the commits checked
  pub base: &'a str,
}

impl PatronusService {
  /// Whether the code owners still have to approve the branch. `recheck` asks again instead of a cached answer.
  #[instrument(skip(self, subject), fields(source_branch = subject.source_branch, commits = subject.commits.len()))]
  pub async fn review_requirement(&self, subject: &ReviewSubject<'_>, recheck: bool) -> Result<PatronusReviewRequirement, ActionError> {
    let tip = subject.commits.first().map_or("", |commit| commit.hash.as_str());
    let key = format!("{}/{}/{tip}/{}", subject.repository, subject.target_branch, subject.base);
    if !recheck
      && let Some((fetched_at, ttl, requirement)) = self.caches.reviews.get(&key)
      && fetched_at.elapsed() < ttl
    {
      return Ok(requirement);
    }

    let asked_at = Instant::now();
    let token = self.action_token()?;
    let without_review = self.check(&token, subject, None).await?;
    let requirement = if without_review.approval_required {
      match self.merge_request(subject, recheck).await? {
        Some(merge_request) => {
          let with_review = self.check(&token, subject, Some(&merge_request.review_key)).await?;
          to_requirement(&with_review, Some(merge_request))
        }
        None => to_requirement(&without_review, None),
      }
    } else {
      to_requirement(&without_review, None)
    };
    let ttl = if requirement.required { REQUIRED_TTL } else { NOT_REQUIRED_TTL };
    // a background answer replaces no answer kept since it was asked (a Recheck beside it): that one is newer
    let newer_kept = !recheck && self.caches.reviews.get(&key).is_some_and(|(fetched_at, _, _)| fetched_at >= asked_at);
    if !newer_kept {
      self.caches.reviews.insert(key, (Instant::now(), ttl, requirement.clone()));
    }
    Ok(requirement)
  }

  /// The review of the branch as the robot started from `pull_request` would decide it, or, without one, as a start
  /// through the API would (it counts no approval of a pull request). Not cached: the caller keys the answer by what
  /// Patronus decides on. A `background` check is not sent while Patronus backs off after a failure (`settle`, shared
  /// with the robot lists) or while it rejects the token.
  #[instrument(skip(self, subject), fields(source_branch = subject.source_branch, commits = subject.commits.len()))]
  pub async fn review_check(
    &self,
    subject: &ReviewSubject<'_>,
    pull_request: Option<GitHubPullRequestDto<'_>>,
    background: bool,
  ) -> Result<PatronusReviewRequirement, PatronusAvailability> {
    let token = self.token(background).map_err(|result| result.availability)?;
    let request = check_request(subject, None, pull_request);
    // the backoff repeats the message of the failure to the checks it holds back
    let response = self.client().pre_merge_check(&token.value, &request).await.map_err(|error| match error {
      PatronusError::Http { status: 503, code, .. } => PatronusError::Http {
        status: 503,
        code,
        description: Some(REVIEW_CHECK_UNAVAILABLE.to_string()),
      },
      error => error,
    });
    self.settle(response, &token.stamp).map(|response| to_requirement(&response, None))
  }

  async fn check(&self, token: &crate::token::Token, subject: &ReviewSubject<'_>, review_key: Option<&str>) -> Result<PreMergeCheckResponse, ActionError> {
    let request = check_request(subject, review_key, None);
    let response = self.client().pre_merge_check(&token.value, &request).await;
    if let Err(PatronusError::Http { status: 503, .. }) = response {
      return Err(REVIEW_CHECK_UNAVAILABLE.to_string());
    }
    self.settle(response, &token.stamp).map_err(availability_message)
  }

  /// The open merge request of the branch: a found one is kept for the session, none is asked again after 5 minutes
  /// (or on a recheck)
  async fn merge_request(&self, subject: &ReviewSubject<'_>, recheck: bool) -> Result<Option<MergeRequestDto>, ActionError> {
    let key = merge_request_key(subject.repository, subject.source_branch, subject.target_branch);
    if let Some((fetched_at, found)) = self.caches.merge_requests.get(&key)
      && (found.is_some() || (!recheck && fetched_at.elapsed() < NO_MERGE_REQUEST_TTL))
    {
      return Ok(found);
    }
    let token = self.action_token()?;
    let response = self
      .client()
      .find_merge_requests(&token.value, subject.repository, subject.source_branch, subject.target_branch)
      .await;
    let found = self.settle(response, &token.stamp).map_err(availability_message)?.into_iter().next();
    self.caches.merge_requests.insert(key, (Instant::now(), found.clone()));
    Ok(found)
  }
}

const fn check_request<'a>(subject: &ReviewSubject<'a>, review_key: Option<&'a str>, pull_request: Option<GitHubPullRequestDto<'a>>) -> PreMergeCheckRequest<'a> {
  PreMergeCheckRequest {
    space_project_key: subject.project_key,
    space_repository_name: subject.repository,
    target_branch: subject.target_branch,
    commits: subject.commits,
    space_review_key: review_key,
    github_pull_request: pull_request,
  }
}

fn merge_request_key(repository: &str, source_branch: &str, target_branch: &str) -> String {
  format!("{repository}/{source_branch}/{target_branch}")
}

/// The answer as the app shows it. Without a pull request only the groups whose approval is missing are listed; with
/// one (the groups carry `approved`) every group the changes need, the approved ones too.
pub fn to_requirement(response: &PreMergeCheckResponse, merge_request: Option<MergeRequestDto>) -> PatronusReviewRequirement {
  let evaluated = response.reviewer_groups.iter().any(|group| group.approved.is_some());
  PatronusReviewRequirement {
    required: response.approval_required,
    merge_request: merge_request.map(to_merge_request),
    groups: if response.approval_required || evaluated {
      response
        .reviewer_groups
        .iter()
        .map(|group| PatronusReviewerGroup {
          name: group.group_name.clone(),
          paths: group.paths.as_ref().map_or_default(|paths| sorted_paths(paths)),
          branch_wide: group.branch_wide.unwrap_or(false),
          paths_source: if group.paths.is_some() || group.branch_wide == Some(true) {
            ReviewPathsSource::Patronus
          } else {
            ReviewPathsSource::Unknown
          },
          reviewers: group
            .reviewers
            .iter()
            .map(|reviewer| PatronusReviewer {
              full_name: reviewer.full_name.clone(),
              username: reviewer.space_username.clone().filter(|username| !username.is_empty()),
              github_login: reviewer.github_username.clone().filter(|login| !login.is_empty()),
              available: reviewer.is_available,
              suggested: reviewer.is_suggested,
              approval_state: reviewer.approval_state.as_deref().and_then(parse_approval_state),
            })
            .collect(),
          required_approvals: group.required_approvals,
          collected_approvals: group.collected_approvals,
          approved: group.approved,
          self_approval_allowed: group.self_approval_allowed,
        })
        .collect()
    } else {
      Vec::new()
    },
    skipped: false,
  }
}

/// None for a state this version doesn't know: the member is shown, not counted
fn parse_approval_state(state: &str) -> Option<ReviewerApprovalState> {
  match state {
    "APPROVED" => Some(ReviewerApprovalState::Approved),
    "AUTHOR_SELF_APPROVED" => Some(ReviewerApprovalState::AuthorSelfApproved),
    "REVIEWING" => Some(ReviewerApprovalState::Reviewing),
    "NOT_ADDED" => Some(ReviewerApprovalState::NotAdded),
    "NOT_ELIGIBLE" => Some(ReviewerApprovalState::NotEligible),
    _ => None,
  }
}

fn sorted_paths(paths: &[String]) -> Vec<String> {
  let mut paths = paths.to_vec();
  paths.sort_unstable();
  paths.dedup();
  paths
}

fn to_merge_request(merge_request: MergeRequestDto) -> PatronusMergeRequest {
  PatronusMergeRequest {
    project_key: merge_request.project_key,
    review_key: merge_request.review_key,
    url: merge_request.url,
  }
}
