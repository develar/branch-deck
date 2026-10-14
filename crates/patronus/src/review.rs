//! The code-owner review a Safe Push needs (PAT-A-109), asked before the start in the cheap order: the check without a
//! merge request first (no Space call); only when approvals are missing, the merge request of the branch, then the check
//! with it. The pre-check is the heaviest call of the Patronus API, so its answer is cached by the commits it checked.

use crate::actions::{ActionError, availability_message};
use crate::client::{MergeRequestDto, PatronusError, PreMergeCheckRequest, PreMergeCheckResponse, PreMergeCommitDto};
use crate::service::PatronusService;
use std::time::{Duration, Instant};
use sync_types::patronus::{PatronusMergeRequest, PatronusReviewRequirement, PatronusReviewer, PatronusReviewerGroup};
use tracing::instrument;

/// No approval needed: holds until the commits change
const NOT_REQUIRED_TTL: Duration = Duration::from_secs(600);
/// Approvals may arrive while the user waits; Recheck asks at once
const REQUIRED_TTL: Duration = Duration::from_secs(120);
/// A branch without a merge request may get one in the web UI
const NO_MERGE_REQUEST_TTL: Duration = Duration::from_secs(300);

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
      && let Some((fetched_at, ttl, requirement)) = self.lock().reviews.get(&key)
      && fetched_at.elapsed() < *ttl
    {
      return Ok(requirement.clone());
    }

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
    self.lock().reviews.insert(key, (Instant::now(), ttl, requirement.clone()));
    Ok(requirement)
  }

  async fn check(&self, token: &crate::token::Token, subject: &ReviewSubject<'_>, review_key: Option<&str>) -> Result<PreMergeCheckResponse, ActionError> {
    let request = PreMergeCheckRequest {
      space_project_key: subject.project_key,
      space_repository_name: subject.repository,
      target_branch: subject.target_branch,
      commits: subject.commits,
      space_review_key: review_key,
    };
    let response = self.client().pre_merge_check(&token.value, &request).await;
    if let Err(PatronusError::Http { status: 503, .. }) = response {
      return Err("The review check of Patronus is unavailable right now".to_string());
    }
    self.settle(response, &token.stamp).map_err(availability_message)
  }

  /// The open merge request of the branch: a found one is kept for the session, none is asked again after 5 minutes
  /// (or on a recheck)
  async fn merge_request(&self, subject: &ReviewSubject<'_>, recheck: bool) -> Result<Option<MergeRequestDto>, ActionError> {
    let key = merge_request_key(subject.repository, subject.source_branch, subject.target_branch);
    if let Some((fetched_at, found)) = self.lock().merge_requests.get(&key)
      && (found.is_some() || (!recheck && fetched_at.elapsed() < NO_MERGE_REQUEST_TTL))
    {
      return Ok(found.clone());
    }
    let token = self.action_token()?;
    let response = self
      .client()
      .find_merge_requests(&token.value, subject.repository, subject.source_branch, subject.target_branch)
      .await;
    let found = self.settle(response, &token.stamp).map_err(availability_message)?.into_iter().next();
    self.lock().merge_requests.insert(key, (Instant::now(), found.clone()));
    Ok(found)
  }

  /// Opens the merge request of the branch on Space (by Patronus, for the user); it is remembered for the next check
  #[instrument(skip(self))]
  pub async fn create_merge_request(&self, project_key: &str, repository: &str, source_branch: &str, target_branch: &str) -> Result<PatronusMergeRequest, ActionError> {
    let token = self.action_token()?;
    let response = self
      .client()
      .create_merge_request(&token.value, project_key, repository, source_branch, target_branch)
      .await;
    let created = self.settle(response, &token.stamp).map_err(availability_message)?;
    let mut state = self.lock();
    state
      .merge_requests
      .insert(merge_request_key(repository, source_branch, target_branch), (Instant::now(), Some(created.clone())));
    // the requirements of the branch now depend on the merge request
    state.reviews.retain(|key, _| !key.starts_with(&format!("{repository}/{target_branch}/")));
    Ok(to_merge_request(created))
  }
}

fn merge_request_key(repository: &str, source_branch: &str, target_branch: &str) -> String {
  format!("{repository}/{source_branch}/{target_branch}")
}

fn to_requirement(response: &PreMergeCheckResponse, merge_request: Option<MergeRequestDto>) -> PatronusReviewRequirement {
  PatronusReviewRequirement {
    required: response.approval_required,
    merge_request: merge_request.map(to_merge_request),
    groups: if response.approval_required {
      response
        .reviewer_groups
        .iter()
        .map(|group| PatronusReviewerGroup {
          name: group.group_name.clone(),
          reviewers: group
            .reviewers
            .iter()
            .map(|reviewer| PatronusReviewer {
              full_name: reviewer.full_name.clone(),
              username: reviewer.space_username.clone(),
              available: reviewer.is_available,
              suggested: reviewer.is_suggested,
            })
            .collect(),
        })
        .collect()
    } else {
      Vec::new()
    },
    skipped: false,
  }
}

fn to_merge_request(merge_request: MergeRequestDto) -> PatronusMergeRequest {
  PatronusMergeRequest {
    project_key: merge_request.project_key,
    review_key: merge_request.review_key,
    url: merge_request.url,
  }
}
