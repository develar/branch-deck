//! The pull requests of others that wait for the user's review in the GitHub repository of the branches, through one
//! `gh api graphql` search: requested by name only (`user-review-requested:@me`), not through a team. Like
//! `pull_requests.rs`, every failure is an availability, not an error.

use crate::pull_requests::{GH_TIMEOUT, GitHubRepository, classify_gh_failure, find_gh, parse_review_decision, resolve_github_repository, run_gh_output};
use crate::remote_url::split_owner_and_name;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;
use sync_types::pull_request::{ChecksState, PullRequestAvailability};
use sync_types::review_request::{GetReviewRequestsParams, ReviewRequest, ReviewRequestsResult};
use sync_utils::child_process::ChildOutput;
use sync_utils::timestamp::parse_rfc3339_timestamp;
use tracing::{instrument, warn};

/// The search costs 1 point of the 5000 an hour. `last: 20` review requests reach the newest one of the user also on a
/// pull request that asked many reviewers one by one.
pub const REVIEW_REQUESTS_QUERY: &str = "query($q: String!) {
  viewer { login }
  search(query: $q, type: ISSUE, first: 50) {
    issueCount
    nodes {
      ... on PullRequest {
        number title url bodyText
        author { login }
        createdAt updatedAt
        additions deletions changedFiles
        reviewDecision
        headRefName headRefOid baseRefName baseRefOid
        isCrossRepository
        baseRepository { defaultBranchRef { name } }
        statusCheckRollup { state }
        viewerLatestReview { submittedAt commit { oid } }
        timelineItems(itemTypes: [REVIEW_REQUESTED_EVENT], last: 20) {
          nodes {
            ... on ReviewRequestedEvent {
              createdAt
              actor { login }
              requestedReviewer { ... on User { login } }
            }
          }
        }
      }
    }
  }
}";

/// The open, non-draft pull requests of `repository` the user is asked to review by name
pub fn review_requests_search(repository: &GitHubRepository) -> String {
  format!(
    "repo:{}/{} is:pr is:open archived:false draft:false user-review-requested:@me",
    repository.owner, repository.name
  )
}

/// The pull requests waiting for the user's review in the GitHub repository of the branches
#[instrument(skip(git_executor), fields(repository_path = %params.repository_path))]
pub fn get_review_requests(git_executor: &GitCommandExecutor, params: &GetReviewRequestsParams) -> Result<ReviewRequestsResult> {
  get_review_requests_with(git_executor, params, find_gh().as_deref(), GH_TIMEOUT)
}

/// `get_review_requests` with the `gh` binary (None = not installed) and the timeout given. A known
/// `github_repository` skips resolving it, which costs up to 3 git processes.
pub fn get_review_requests_with(git_executor: &GitCommandExecutor, params: &GetReviewRequestsParams, gh: Option<&Path>, timeout: Duration) -> Result<ReviewRequestsResult> {
  let known = params.github_repository.as_deref().and_then(split_owner_and_name).map(|(owner, name)| GitHubRepository {
    owner: owner.to_string(),
    name: name.to_string(),
  });
  let repository = match known {
    Some(repository) => repository,
    None => match resolve_github_repository(git_executor, &params.repository_path)? {
      Some(repository) => repository,
      None => return Ok(ReviewRequestsResult::unavailable(PullRequestAvailability::NotGitHub)),
    },
  };
  let Some(gh) = gh else {
    return Ok(ReviewRequestsResult::unavailable(PullRequestAvailability::GhMissing));
  };

  let query = format!("query={REVIEW_REQUESTS_QUERY}");
  let search = format!("q={}", review_requests_search(&repository));
  let args = ["api", "graphql", "-f", &query, "-f", &search];
  let output = match run_gh_output(gh, &params.repository_path, &args, timeout) {
    Ok(output) => output,
    Err(availability) => return Ok(ReviewRequestsResult::unavailable(availability)),
  };
  Ok(answer_of(&output, &args, repository))
}

/// The result of a finished `gh api graphql`: a failed run that still printed `data` (a partial answer) is read
fn answer_of(output: &ChildOutput, args: &[&str], repository: GitHubRepository) -> ReviewRequestsResult {
  let response = serde_json::from_slice::<GhResponse>(&output.stdout);
  if let Ok(response) = &response
    && response.errors.iter().flatten().any(|error| error.kind.as_deref() == Some("RATE_LIMITED"))
  {
    return ReviewRequestsResult::unavailable(PullRequestAvailability::RateLimited);
  }
  if !output.status.success() && is_rate_limited(&output.stderr) {
    return ReviewRequestsResult::unavailable(PullRequestAvailability::RateLimited);
  }
  match response {
    Ok(GhResponse { data: Some(data), errors }) => {
      for error in errors.iter().flatten() {
        warn!(kind = error.kind.as_deref(), message = error.message.as_deref(), "gh api graphql answered with an error");
      }
      let total_count = data.search.issue_count;
      ReviewRequestsResult {
        availability: PullRequestAvailability::Available {
          owner: repository.owner,
          name: repository.name,
        },
        total_count,
        requests: to_review_requests(data),
      }
    }
    _ if !output.status.success() => ReviewRequestsResult::unavailable(classify_gh_failure(output, args)),
    Ok(GhResponse { data: None, .. }) => unexpected("no data"),
    Err(e) => unexpected(&e.to_string()),
  }
}

fn unexpected(reason: &str) -> ReviewRequestsResult {
  warn!(reason, "unexpected gh api graphql output");
  ReviewRequestsResult::unavailable(PullRequestAvailability::Error {
    message: format!("Unexpected gh output: {reason}"),
  })
}

/// A primary (`API rate limit exceeded`) or secondary (`secondary rate limit`, HTTP 403 or 429) rate limit
fn is_rate_limited(stderr: &str) -> bool {
  stderr.to_ascii_lowercase().contains("rate limit") || stderr.contains("HTTP 429")
}

/// The requests by the parsed search, without those the user reviewed after the newest request: the search index lags
/// behind a submitted review
fn to_review_requests(data: GhData) -> Vec<ReviewRequest> {
  let viewer = data.viewer.login;
  data
    .search
    .nodes
    .into_iter()
    // a node of another type (an issue) is an empty object
    .filter_map(|node| node.number.map(|number| (number, node)))
    .filter_map(|(number, node)| to_review_request(number, node, &viewer))
    .collect()
}

fn to_review_request(number: u32, node: GhPullRequest, viewer: &str) -> Option<ReviewRequest> {
  let created_at = parse_time(node.created_at.as_deref());
  // the newest request of the viewer by name; the events come oldest first
  let request = node
    .timeline_items
    .and_then(|items| items.nodes)
    .unwrap_or_default()
    .into_iter()
    .flatten()
    .rev()
    .find(|event| {
      event
        .requested_reviewer
        .as_ref()
        .and_then(|reviewer| reviewer.login.as_deref())
        .is_some_and(|login| login.eq_ignore_ascii_case(viewer))
    });
  let (waiting_since, requested_by) = match request {
    Some(event) => (parse_time(event.created_at.as_deref()).max(created_at), event.actor.and_then(|actor| actor.login)),
    None => (created_at, None),
  };
  // a pending review (not submitted) is no review
  let review = node
    .viewer_latest_review
    .and_then(|review| Some((parse_time(Some(review.submitted_at.as_deref()?)), review.commit.and_then(|commit| commit.oid))));
  if review.as_ref().is_some_and(|(submitted_at, _)| *submitted_at >= waiting_since) {
    return None;
  }
  let default_branch = node.base_repository.and_then(|repository| repository.default_branch_ref).and_then(|branch| branch.name);
  Some(ReviewRequest {
    number,
    title: node.title.unwrap_or_default(),
    url: node.url.unwrap_or_default(),
    body_text: node.body_text.unwrap_or_default(),
    author: node.author.and_then(|author| author.login),
    requested_by,
    created_at,
    updated_at: parse_time(node.updated_at.as_deref()),
    waiting_since,
    additions: node.additions.unwrap_or(0),
    deletions: node.deletions.unwrap_or(0),
    changed_files: node.changed_files.unwrap_or(0),
    checks_state: node.status_check_rollup.and_then(|rollup| parse_checks_state(rollup.state.as_deref())),
    review_decision: parse_review_decision(node.review_decision.as_deref()),
    re_requested: review.is_some(),
    reviewed_commit: review.and_then(|(_, commit)| commit),
    base_is_default: default_branch.as_deref().is_some_and(|name| name == node.base_ref_name.as_deref().unwrap_or_default()),
    head_ref_name: node.head_ref_name.unwrap_or_default(),
    head_ref_oid: node.head_ref_oid.unwrap_or_default(),
    base_ref_name: node.base_ref_name.unwrap_or_default(),
    base_ref_oid: node.base_ref_oid.unwrap_or_default(),
    is_cross_repository: node.is_cross_repository.unwrap_or(false),
  })
}

fn parse_time(value: Option<&str>) -> u32 {
  value.and_then(parse_rfc3339_timestamp).unwrap_or(0)
}

/// The state of the status check rollup of the head commit
fn parse_checks_state(state: Option<&str>) -> Option<ChecksState> {
  match state? {
    "SUCCESS" => Some(ChecksState::Passing),
    "FAILURE" | "ERROR" => Some(ChecksState::Failing),
    "PENDING" | "EXPECTED" => Some(ChecksState::Pending),
    _ => None,
  }
}

// ---- the GraphQL answer; nulls are possible everywhere a node is gone or hidden ----

#[derive(Debug, Deserialize)]
struct GhResponse {
  data: Option<GhData>,
  errors: Option<Vec<GhError>>,
}

#[derive(Debug, Deserialize)]
struct GhError {
  #[serde(rename = "type")]
  kind: Option<String>,
  message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhData {
  viewer: GhViewer,
  search: GhSearch,
}

#[derive(Debug, Deserialize)]
struct GhViewer {
  login: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhSearch {
  issue_count: u32,
  #[serde(default, deserialize_with = "skip_nulls")]
  nodes: Vec<GhPullRequest>,
}

fn skip_nulls<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<GhPullRequest>, D::Error> {
  let nodes: Option<Vec<Option<GhPullRequest>>> = Deserialize::deserialize(deserializer)?;
  Ok(nodes.unwrap_or_default().into_iter().flatten().collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPullRequest {
  number: Option<u32>,
  title: Option<String>,
  url: Option<String>,
  body_text: Option<String>,
  author: Option<GhLogin>,
  created_at: Option<String>,
  updated_at: Option<String>,
  additions: Option<u32>,
  deletions: Option<u32>,
  changed_files: Option<u32>,
  review_decision: Option<String>,
  head_ref_name: Option<String>,
  head_ref_oid: Option<String>,
  base_ref_name: Option<String>,
  base_ref_oid: Option<String>,
  is_cross_repository: Option<bool>,
  base_repository: Option<GhRepository>,
  status_check_rollup: Option<GhRollup>,
  viewer_latest_review: Option<GhReview>,
  timeline_items: Option<GhTimeline>,
}

#[derive(Debug, Deserialize)]
struct GhLogin {
  login: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhRepository {
  default_branch_ref: Option<GhBranch>,
}

#[derive(Debug, Deserialize)]
struct GhBranch {
  name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRollup {
  state: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhReview {
  submitted_at: Option<String>,
  commit: Option<GhCommit>,
}

#[derive(Debug, Deserialize)]
struct GhCommit {
  oid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhTimeline {
  nodes: Option<Vec<Option<GhReviewRequestedEvent>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhReviewRequestedEvent {
  created_at: Option<String>,
  actor: Option<GhLogin>,
  /// A user has a login, a team or a bot an empty object here
  requested_reviewer: Option<GhLogin>,
}

#[cfg(test)]
#[path = "review_requests_test.rs"]
mod review_requests_test;
