//! The pull requests of others that wait for the user's review in the GitHub repository of the branches, through one
//! GraphQL search: requested by name only (`user-review-requested:@me`), not through a team. Like `pull_requests.rs`,
//! every failure is an availability, not an error.

use crate::pull_requests::{GitHubRepository, availability_of, parse_review_decision, visible_data};
use domain_types::pull_request::{ChecksState, PullRequestAvailability};
use domain_types::review_request::{BackToYou, ReviewRequest, ReviewRequestsResult};
use github::service::{GitHubService, RequestKind};
use serde::Deserialize;
use sync_utils::timestamp::parse_rfc3339_timestamp;
use tracing::instrument;

/// The search costs 1 point of the 5000 an hour (1 + 50 + 50 connections, by the hundred). `last: 20` review requests
/// reach the newest one of the user also on a pull request that asked many reviewers one by one; `activity`, a timeline
/// of its own so that bot comments don't push the requests out, tells whether the user answered with a comment and the
/// other side moved since. `repository` tells "nothing to review" from "the token can't see the repository".
pub const REVIEW_REQUESTS_QUERY: &str = "query($q: String!, $owner: String!, $name: String!) {
  viewer { login }
  repository(owner: $owner, name: $name) { viewerPermission }
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
        activity: timelineItems(itemTypes: [ISSUE_COMMENT, PULL_REQUEST_REVIEW, PULL_REQUEST_COMMIT, HEAD_REF_FORCE_PUSHED_EVENT], last: 30) {
          nodes {
            __typename
            ... on IssueComment { createdAt viewerDidAuthor bodyText author { __typename login } }
            ... on PullRequestReview { submittedAt viewerDidAuthor author { __typename login } }
            ... on PullRequestCommit { commit { committedDate } }
            ... on HeadRefForcePushedEvent { createdAt actor { __typename } }
          }
        }
      }
    }
  }
}";

/// The open, non-draft pull requests of `repository` the user is asked to review by name
pub fn review_requests_search(repository: &GitHubRepository) -> String {
  format!("repo:{} is:pr is:open archived:false draft:false user-review-requested:@me", repository.full_name())
}

/// The pull requests waiting for the user's review in `repository`, the GitHub repository of the branches;
/// `repository_path` is where git reads the credential config
#[instrument(skip(github))]
pub async fn list_review_requests(github: &GitHubService, repository_path: &str, repository: &GitHubRepository) -> ReviewRequestsResult {
  let full_name = repository.full_name();
  let variables = serde_json::json!({"q": review_requests_search(repository), "owner": repository.owner, "name": repository.name});
  let (variables, full_name_ref) = (&variables, full_name.as_str());
  let answer = github
    .ask(repository_path, &full_name, RequestKind::Listing, |client, token| async move {
      let response = client.graphql::<GhData>(&token, REVIEW_REQUESTS_QUERY, variables).await?;
      visible_data(response, full_name_ref, |data| data.repository.is_some())
    })
    .await;
  match answer {
    Ok(data) => {
      let issue_count = data.search.issue_count;
      let (requests, dropped) = to_review_requests(data);
      ReviewRequestsResult {
        availability: PullRequestAvailability::Available {
          owner: repository.owner.clone(),
          name: repository.name.clone(),
        },
        total_count: issue_count.saturating_sub(dropped),
        requests,
      }
    }
    Err(failure) => ReviewRequestsResult::unavailable(availability_of(failure)),
  }
}

/// The requests by the parsed search and how many of its pull requests don't wait for the user: reviewed after the
/// newest request (the search index lags behind a submitted review), or answered with a comment the other side has not
/// moved after (GitHub keeps the request then)
fn to_review_requests(data: GhData) -> (Vec<ReviewRequest>, u32) {
  let viewer = data.viewer.login;
  let mut dropped = 0;
  let requests = data
    .search
    .nodes
    .into_iter()
    // a node of another type (an issue) is an empty object
    .filter_map(|node| node.number.map(|number| (number, node)))
    .filter_map(|(number, node)| {
      let request = to_review_request(number, node, &viewer);
      dropped += u32::from(request.is_none());
      request
    })
    .collect();
  (requests, dropped)
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
  let asked_by_name = request.is_some();
  let (requested_at, requested_by) = match request {
    Some(event) => (parse_time(event.created_at.as_deref()).max(created_at), event.actor.and_then(|actor| actor.login)),
    None => (created_at, None),
  };
  // a pending review (not submitted) is no review
  let review = node
    .viewer_latest_review
    .and_then(|review| Some((parse_time(Some(review.submitted_at.as_deref()?)), review.commit.and_then(|commit| commit.oid))));
  if review.as_ref().is_some_and(|(submitted_at, _)| *submitted_at >= requested_at) {
    return None;
  }
  // without the user's request event (a team request, an event beyond the last 20) an earlier comment must not hide it
  let activity: Vec<GhActivity> = node.activity.and_then(|activity| activity.nodes).unwrap_or_default().into_iter().flatten().collect();
  let back_to_you = if asked_by_name {
    match turn_after_answer(&activity, requested_at) {
      Turn::Waiting => None,
      Turn::Answered => return None,
      Turn::Back(back) => Some(back),
    }
  } else {
    None
  };
  let waiting_since = back_to_you.as_ref().map_or(requested_at, |back| back.at);
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
    requested_at,
    waiting_since,
    back_to_you,
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

enum Turn {
  /// The user has not answered the request
  Waiting,
  /// The user answered with a comment, and the other side has not moved since
  Answered,
  /// The other side moved after the user's answer
  Back(BackToYou),
}

/// Whose turn a request of `requested_at` is by the timeline: the user's newest comment after the request (a command
/// such as `/patronus dry-run`, which the app posts as the user, is no answer), then the first move after it: a commit,
/// a force push, a comment or a submitted review of someone else; bots don't move. The order of the timeline is not
/// trusted (commits sit by their date), only the times; a commit made before the answer and pushed after it is missed,
/// as a plain push has no event.
fn turn_after_answer(activity: &[GhActivity], requested_at: u32) -> Turn {
  let answered_at = activity
    .iter()
    .filter_map(|item| match item {
      GhActivity::IssueComment {
        created_at,
        viewer_did_author: true,
        body_text,
        ..
      } if !body_text.as_deref().unwrap_or_default().trim_start().starts_with('/') => Some(parse_time(created_at.as_deref())),
      _ => None,
    })
    .filter(|at| *at >= requested_at)
    .max();
  let Some(answered_at) = answered_at else {
    return Turn::Waiting;
  };
  let first_move = activity
    .iter()
    .filter_map(|item| match item {
      GhActivity::PullRequestCommit { commit } => Some((parse_time(commit.as_ref()?.committed_date.as_deref()), None)),
      GhActivity::HeadRefForcePushedEvent { created_at, actor } if !actor.as_ref().is_some_and(GhActor::is_bot) => Some((parse_time(created_at.as_deref()), None)),
      GhActivity::IssueComment {
        created_at,
        viewer_did_author: false,
        author,
        ..
      } => person_reply(created_at.as_deref(), author.as_ref()),
      GhActivity::PullRequestReview {
        submitted_at,
        viewer_did_author: false,
        author,
      } => person_reply(submitted_at.as_deref(), author.as_ref()),
      _ => None,
    })
    .filter(|(at, _)| *at > answered_at)
    .min_by_key(|(at, _)| *at);
  match first_move {
    Some((at, replied_by)) => Turn::Back(BackToYou { at, replied_by }),
    None => Turn::Answered,
  }
}

/// A comment or a submitted review of a person (not a bot) at `at`; None without the time or by a bot
fn person_reply(at: Option<&str>, author: Option<&GhActor>) -> Option<(u32, Option<String>)> {
  let author = author?;
  if author.is_bot() {
    return None;
  }
  Some((parse_time(Some(at?)), Some(author.login.clone()?)))
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
pub struct GhData {
  viewer: GhViewer,
  /// Null when the token can't see the repository
  repository: Option<serde_json::Value>,
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
  activity: Option<GhActivityTimeline>,
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

#[derive(Debug, Deserialize)]
struct GhActivityTimeline {
  nodes: Option<Vec<Option<GhActivity>>>,
}

/// An item of the `activity` timeline; its fields are null where GitHub hides them
#[derive(Debug, Deserialize)]
#[serde(tag = "__typename")]
enum GhActivity {
  #[serde(rename_all = "camelCase")]
  IssueComment {
    created_at: Option<String>,
    #[serde(default)]
    viewer_did_author: bool,
    body_text: Option<String>,
    author: Option<GhActor>,
  },
  /// A pending review has no `submittedAt`
  #[serde(rename_all = "camelCase")]
  PullRequestReview {
    submitted_at: Option<String>,
    #[serde(default)]
    viewer_did_author: bool,
    author: Option<GhActor>,
  },
  PullRequestCommit {
    commit: Option<GhActivityCommit>,
  },
  #[serde(rename_all = "camelCase")]
  HeadRefForcePushedEvent {
    created_at: Option<String>,
    actor: Option<GhActor>,
  },
  #[serde(other)]
  Other,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhActivityCommit {
  committed_date: Option<String>,
}

/// The author of a comment or a review, the actor of a force push: a `User`, a `Bot`, a `Mannequin`…
#[derive(Debug, Deserialize)]
struct GhActor {
  #[serde(rename = "__typename")]
  type_name: Option<String>,
  login: Option<String>,
}

impl GhActor {
  fn is_bot(&self) -> bool {
    self.type_name.as_deref() == Some("Bot")
  }
}

#[cfg(test)]
#[path = "review_requests_test.rs"]
mod review_requests_test;
