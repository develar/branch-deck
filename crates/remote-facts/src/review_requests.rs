//! The pull requests of others that wait for the user's review in the GitHub repository of the branches, through one
//! GraphQL search: requested by name only (`user-review-requested:@me`), not through a team. Like `pull_requests.rs`,
//! every failure is an availability, not an error.

use crate::pull_requests::{GhReview as GhLatestReview, GhReviewRequestNode, Nodes, availability_of, parse_review_decision, visible_data};
use crate::reviews::{reviews_answer, reviews_query, reviews_searches};
use domain_types::pull_request::{ChecksState, PullRequestAvailability, ReviewerState};
use domain_types::review_request::{AnsweredRequest, BackToYou, ReviewRequest, ReviewRequestsResult, ReviewsAsk};
use github::service::{GitHubService, RequestKind};
use serde::Deserialize;
use sync_core::github_repository::GitHubRepository;
use sync_utils::issue_pattern::issue_lead;
use sync_utils::timestamp::parse_rfc3339_timestamp;
use tracing::instrument;

/// The body of the query of the requests by name, which the query of Reviews extends
macro_rules! review_requests_body {
  () => {
    "  viewer { login }
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
        viewerLatestReview { state submittedAt commit { oid } }
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
"
  };
}

/// The search costs 1 point of the 5000 an hour (1 + 50 + 50 connections, by the hundred). `last: 20` review requests
/// reach the newest one of the user also on a pull request that asked many reviewers one by one; `activity`, a timeline
/// of its own so that bot comments don't push the requests out, tells whether the user answered with a comment and the
/// other side moved since. `repository` tells "nothing to review" from "the token can't see the repository".
pub const REVIEW_REQUESTS_QUERY: &str = concat!("query($q: String!, $owner: String!, $name: String!) {\n", review_requests_body!(), "}");

/// The body of `REVIEW_REQUESTS_QUERY`, which the query of Reviews extends (`reviews::reviews_query`)
pub const REVIEW_REQUESTS_BODY: &str = review_requests_body!();

/// The open, non-draft pull requests of `repository` the user is asked to review by name
pub fn review_requests_search(repository: &GitHubRepository) -> String {
  format!("repo:{} is:pr is:open archived:false draft:false user-review-requested:@me", repository.full_name())
}

/// The pull requests waiting for the user's review in `repository`, the GitHub repository of the branches, with the
/// rest of Reviews in the same request when `also` asks for it (`reviews::reviews_query`); `repository_path` is where
/// git reads the credential config
#[instrument(skip(github, also))]
pub async fn list_review_requests(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, also: Option<&ReviewsAsk>) -> ReviewRequestsResult {
  let full_name = repository.full_name();
  let mut variables = serde_json::json!({"q": review_requests_search(repository), "owner": repository.owner, "name": repository.name});
  if also.is_some() {
    for (key, search) in reviews_searches(repository) {
      variables[key] = serde_json::Value::String(search);
    }
  }
  let linked: Vec<u32> = also.map_or_default(|also| also.linked.iter().map(|link| link.number).collect());
  let (variables, full_name_ref, linked_ref) = (&variables, full_name.as_str(), linked.as_slice());
  let answer = github
    .ask(repository_path, &full_name, RequestKind::Listing, |client, token| async move {
      if also.is_none() {
        let response = client.graphql::<GhData>(&token, REVIEW_REQUESTS_QUERY, variables).await?;
        return visible_data(response, full_name_ref, |data| data.repository.is_some());
      }
      // a token GitHub refused the teams once asks without them from then on, as the pull requests do
      if github.lacks_read_org(&token) {
        let response = client.graphql::<GhData>(&token, &reviews_query(REVIEW_REQUESTS_BODY, linked_ref, false), variables).await?;
        return visible_data(response, full_name_ref, |data| data.repository.is_some());
      }
      let mut response = client.graphql::<GhData>(&token, &reviews_query(REVIEW_REQUESTS_BODY, linked_ref, true), variables).await?;
      if response.errors.iter().any(|error| error.kind.as_deref() == Some("INSUFFICIENT_SCOPES")) {
        github.note_lacks_read_org(&token);
        response = client.graphql::<GhData>(&token, &reviews_query(REVIEW_REQUESTS_BODY, linked_ref, false), variables).await?;
      }
      visible_data(response, full_name_ref, |data| data.repository.is_some())
    })
    .await;
  match answer {
    Ok(mut data) => {
      let issue_count = data.search.issue_count;
      let viewer = data.viewer.login.clone();
      let others = also.map(|also| reviews_answer(&mut data, &viewer, &also.linked));
      let (requests, answered, dropped) = to_review_requests(data.search.nodes, &viewer);
      ReviewRequestsResult {
        availability: PullRequestAvailability::Available {
          owner: repository.owner.clone(),
          name: repository.name.clone(),
        },
        total_count: issue_count.saturating_sub(dropped),
        requests,
        answered,
        others,
      }
    }
    Err(failure) => ReviewRequestsResult::unavailable(availability_of(failure)),
  }
}

/// The requests by the parsed search, the ones the user answered with a comment, and how many of its pull requests
/// don't wait for the user: reviewed after the newest request (the search index lags behind a submitted review), or
/// answered with a comment the other side has not moved after (GitHub keeps the request then)
fn to_review_requests(nodes: Vec<GhPullRequest>, viewer: &str) -> (Vec<ReviewRequest>, Vec<AnsweredRequest>, u32) {
  let mut dropped = 0;
  let mut answered = Vec::new();
  let requests = nodes
    .into_iter()
    // a node of another type (an issue) is an empty object
    .filter_map(|node| node.number.map(|number| (number, node)))
    .filter_map(|(number, node)| match to_review_request(number, node, viewer) {
      Asked::Waits(request) => Some(*request),
      Asked::Answered(answer) => {
        dropped += 1;
        answered.push(*answer);
        None
      }
      Asked::Reviewed => {
        dropped += 1;
        None
      }
    })
    .collect();
  (requests, answered, dropped)
}

/// A request of the search as the user stands to it
enum Asked {
  Waits(Box<ReviewRequest>),
  /// The user answered it with a comment, and the other side has not moved since
  Answered(Box<AnsweredRequest>),
  /// The user reviewed it after the newest request
  Reviewed,
}

fn to_review_request(number: u32, mut node: GhPullRequest, viewer: &str) -> Asked {
  let created_at = parse_time(node.created_at.as_deref());
  // the newest request of the viewer by name; the events come oldest first
  let request = node
    .timeline_items
    .take()
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
  let review = submitted_review(&node);
  if review.as_ref().is_some_and(|(submitted_at, _, _)| *submitted_at >= requested_at) {
    return Asked::Reviewed;
  }
  // without the user's request event (a team request, an event beyond the last 20) an earlier comment must not hide it
  let activity: Vec<GhActivity> = node.activity.take().and_then(|activity| activity.nodes).unwrap_or_default().into_iter().flatten().collect();
  let (back_to_you, answered_at) = if asked_by_name {
    match turn_after_answer(&activity, requested_at) {
      Turn::Waiting => (None, None),
      Turn::Answered { at } => (None, Some(at)),
      Turn::Back(back) => (Some(back), None),
    }
  } else {
    (None, None)
  };
  let waiting_since = back_to_you.as_ref().map_or(requested_at, |back| back.at);
  let mut request = request_of(number, node);
  request.requested_by = requested_by;
  request.requested_at = requested_at;
  request.waiting_since = waiting_since;
  request.back_to_you = back_to_you;
  request.re_requested = review.is_some();
  match answered_at {
    Some(replied_at) => Asked::Answered(Box::new(AnsweredRequest { request, replied_at })),
    None => Asked::Waits(Box::new(request)),
  }
}

/// The user's latest submitted review of `node`: when, its commit and its state; a pending review is none
fn submitted_review(node: &GhPullRequest) -> Option<(u32, Option<String>, Option<ReviewerState>)> {
  let review = node.viewer_latest_review.as_ref()?;
  let submitted_at = parse_time(Some(review.submitted_at.as_deref()?));
  Some((
    submitted_at,
    review.commit.as_ref().and_then(|commit| commit.oid.clone()),
    parse_reviewer_state(review.state.as_deref()),
  ))
}

/// What GitHub says of the pull request of `node`, in the fields of a request: asked at its update, by nobody
pub(crate) fn request_of(number: u32, node: GhPullRequest) -> ReviewRequest {
  let created_at = parse_time(node.created_at.as_deref());
  let updated_at = parse_time(node.updated_at.as_deref());
  let review = submitted_review(&node);
  let default_branch = node.base_repository.and_then(|repository| repository.default_branch_ref).and_then(|branch| branch.name);
  let title = node.title.unwrap_or_default();
  ReviewRequest {
    number,
    title_lead: issue_lead(&title),
    title,
    url: node.url.unwrap_or_default(),
    body_text: node.body_text.unwrap_or_default(),
    author: node.author.and_then(|author| author.login),
    requested_by: None,
    created_at,
    updated_at,
    requested_at: updated_at,
    waiting_since: updated_at,
    back_to_you: None,
    additions: node.additions.unwrap_or(0),
    deletions: node.deletions.unwrap_or(0),
    changed_files: node.changed_files.unwrap_or(0),
    checks_state: node.status_check_rollup.and_then(|rollup| parse_checks_state(rollup.state.as_deref())),
    review_decision: parse_review_decision(node.review_decision.as_deref()),
    re_requested: false,
    reviewed_state: review.as_ref().and_then(|(_, _, state)| *state),
    reviewed_commit: review.and_then(|(_, commit, _)| commit),
    base_is_default: default_branch.as_deref().is_some_and(|name| name == node.base_ref_name.as_deref().unwrap_or_default()),
    head_ref_name: node.head_ref_name.unwrap_or_default(),
    head_ref_oid: node.head_ref_oid.unwrap_or_default(),
    base_ref_name: node.base_ref_name.unwrap_or_default(),
    base_ref_oid: node.base_ref_oid.unwrap_or_default(),
    is_cross_repository: node.is_cross_repository.unwrap_or(false),
  }
}

/// The state of a submitted review
pub(crate) fn parse_reviewer_state(state: Option<&str>) -> Option<ReviewerState> {
  match state? {
    "APPROVED" => Some(ReviewerState::Approved),
    "CHANGES_REQUESTED" => Some(ReviewerState::ChangesRequested),
    "COMMENTED" => Some(ReviewerState::Commented),
    _ => None,
  }
}

enum Turn {
  /// The user has not answered the request
  Waiting,
  /// The user answered with a comment at `at`, and the other side has not moved since
  Answered { at: u32 },
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
    None => Turn::Answered { at: answered_at },
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
  pub(crate) viewer: GhViewer,
  /// Null when the token can't see the repository
  pub(crate) repository: Option<serde_json::Value>,
  pub(crate) search: GhSearch,
  // the rest of Reviews (`reviews::reviews_query`)
  pub(crate) team: Option<GhSearch>,
  pub(crate) reviewed: Option<GhSearch>,
  pub(crate) authored: Option<GhSearch>,
  /// The linked pull requests by their aliases (`p123`); a number the repository lacks is null
  pub(crate) linked: Option<std::collections::HashMap<String, Option<GhPullRequest>>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GhViewer {
  pub(crate) login: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GhSearch {
  pub(crate) issue_count: u32,
  #[serde(default, deserialize_with = "skip_nulls")]
  pub(crate) nodes: Vec<GhPullRequest>,
}

fn skip_nulls<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<GhPullRequest>, D::Error> {
  let nodes: Option<Vec<Option<GhPullRequest>>> = Deserialize::deserialize(deserializer)?;
  Ok(nodes.unwrap_or_default().into_iter().flatten().collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GhPullRequest {
  pub(crate) number: Option<u32>,
  /// `OPEN`, `CLOSED`, `MERGED`: asked by Reviews only
  pub(crate) state: Option<String>,
  pub(crate) is_draft: Option<bool>,
  pub(crate) latest_reviews: Option<Nodes<GhLatestReview>>,
  pub(crate) review_requests: Option<Nodes<GhReviewRequestNode>>,
  title: Option<String>,
  url: Option<String>,
  body_text: Option<String>,
  pub(crate) author: Option<GhLogin>,
  created_at: Option<String>,
  updated_at: Option<String>,
  additions: Option<u32>,
  deletions: Option<u32>,
  changed_files: Option<u32>,
  review_decision: Option<String>,
  head_ref_name: Option<String>,
  pub(crate) head_ref_oid: Option<String>,
  base_ref_name: Option<String>,
  pub(crate) base_ref_oid: Option<String>,
  is_cross_repository: Option<bool>,
  base_repository: Option<GhRepository>,
  status_check_rollup: Option<GhRollup>,
  viewer_latest_review: Option<GhReview>,
  timeline_items: Option<GhTimeline>,
  activity: Option<GhActivityTimeline>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GhLogin {
  pub(crate) login: Option<String>,
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
  state: Option<String>,
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
