//! The rest of Reviews beside the requests by name (spec/repository/reviews.spec.md), read in the request of the review
//! requests while the Reviews view shows (`reviews_query`): the pull requests that ask the user's teams, that the user
//! reviewed, that the user opened, and the ones the user added by their links (aliases `p<number>` of `repository`).
//! A link is read alone when the user adds it (`read_linked_pull_request`), after `parse_pull_request_link` named its
//! number in the repository. As the review requests, every failure is an availability, not an error.

use crate::pull_requests::{NOT_ON_GITHUB_MESSAGE, Nodes, collect_reviewers, failure_message, visible_data};
use crate::review_requests::{GhData, GhPullRequest, GhSearch, request_of};
use crate::service_sources::ServiceHost;
use domain_policy::reviews::has_new_commits;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::review_request::{LinkedAnswer, LinkedPullRequest, ReviewEntry, ReviewInvolvement, ReviewSearch, ReviewsAnswer};
use domain_types::session::{FactFamily, FactRefreshReason};
use github::service::{GitHubFailure, GitHubService, RequestKind};
use repository_model::session::{SessionKey, Sessions};
use serde::Deserialize;
use std::fmt::Write as _;
use sync_core::github_repository::GitHubRepository;
use tracing::instrument;

/// The fields of a pull request of Reviews: those of a request, its state and draft, the latest review of each reviewer
/// and the open review requests (a team by its slug, which needs `read:org`)
macro_rules! review_pull_fragment {
  ($team:literal) => {
    concat!(
      "fragment ReviewPull on PullRequest {
  number title url bodyText state isDraft
  author { login }
  createdAt updatedAt
  additions deletions changedFiles
  reviewDecision
  headRefName headRefOid baseRefName baseRefOid
  isCrossRepository
  baseRepository { defaultBranchRef { name } }
  statusCheckRollup { state }
  viewerLatestReview { state submittedAt commit { oid } }
  latestReviews(first: 100) { nodes { author { login } state } }
  reviewRequests(first: 100) { nodes { requestedReviewer { __typename ... on User { login }",
      $team,
      " } } }
}
"
    )
  };
}

const REVIEW_PULL: &str = review_pull_fragment!(" ... on Team { slug }");
/// For a token without `read:org`, which GitHub refuses a team's slug
const REVIEW_PULL_WITHOUT_TEAMS: &str = review_pull_fragment!("");

/// The searches of Reviews in `repository` by their variables: the requests of the user's teams (GitHub's
/// `review-requested` has the requests by name too, which the answer leaves to Your Turn), the pull requests of others
/// the user reviewed, the user's own
#[must_use]
pub fn reviews_searches(repository: &GitHubRepository) -> [(&'static str, String); 3] {
  let base = format!("repo:{} is:pr is:open archived:false", repository.full_name());
  [
    ("team", format!("{base} draft:false review-requested:@me")),
    ("reviewed", format!("{base} draft:false reviewed-by:@me -author:@me")),
    ("authored", format!("{base} author:@me")),
  ]
}

/// The query of the review requests (`body`, the body of `REVIEW_REQUESTS_QUERY`) with the searches of Reviews and the
/// linked pull requests: one request, about 4 points
#[must_use]
pub fn reviews_query(body: &str, linked: &[u32], teams: bool) -> String {
  let mut query = String::from("query($q: String!, $team: String!, $reviewed: String!, $authored: String!, $owner: String!, $name: String!) {\n");
  query.push_str(body);
  for key in ["team", "reviewed", "authored"] {
    let _ = writeln!(query, "  {key}: search(query: ${key}, type: ISSUE, first: 50) {{ issueCount nodes {{ ...ReviewPull }} }}");
  }
  if !linked.is_empty() {
    query.push_str("  linked: repository(owner: $owner, name: $name) {\n");
    for number in linked {
      let _ = writeln!(query, "    p{number}: pullRequest(number: {number}) {{ ...ReviewPull }}");
    }
    query.push_str("  }\n");
  }
  query.push_str("}\n");
  query.push_str(if teams { REVIEW_PULL } else { REVIEW_PULL_WITHOUT_TEAMS });
  query
}

/// The rest of Reviews of an answer of `reviews_query`, taken out of `data`: the team requests without those by name,
/// the reviewed pull requests, the own ones, and the answer of each link of `linked`
pub(crate) fn reviews_answer(data: &mut GhData, viewer: &str, linked: &[LinkedPullRequest]) -> ReviewsAnswer {
  let team = data.team.take().map_or_default(|search| {
    let mut dropped = 0;
    let entries: Vec<ReviewEntry> = search
      .nodes
      .into_iter()
      .filter_map(|node| {
        let number = node.number?;
        let entry = team_entry(number, node, viewer);
        dropped += u32::from(entry.is_none());
        entry
      })
      .collect();
    ReviewSearch {
      total_count: search.issue_count.saturating_sub(dropped),
      entries,
    }
  });
  let searched = |search: Option<GhSearch>, entry: fn(u32, GhPullRequest) -> ReviewEntry| {
    search.map_or_default(|search| ReviewSearch {
      total_count: search.issue_count,
      entries: search.nodes.into_iter().filter_map(|node| Some(entry(node.number?, node))).collect(),
    })
  };
  let reviewed = searched(data.reviewed.take(), reviewed_entry);
  let authored = searched(data.authored.take(), authored_entry);
  let mut by_alias = data.linked.take().unwrap_or_default();
  let mut links = linked.to_vec();
  links.sort_by_key(|link| std::cmp::Reverse(link.linked_at));
  let linked = links
    .into_iter()
    .map(|link| {
      let node = by_alias.remove(&format!("p{}", link.number)).flatten();
      linked_answer(link, node)
    })
    .collect();
  ReviewsAnswer { team, reviewed, authored, linked }
}

/// A pull request that asks the user's teams; none when it asks the user by name (Your Turn has it)
fn team_entry(number: u32, mut node: GhPullRequest, viewer: &str) -> Option<ReviewEntry> {
  let requests: Vec<_> = Nodes::into_nodes(node.review_requests.take())
    .into_iter()
    .filter_map(|node| node.requested_reviewer)
    .collect();
  if requests
    .iter()
    .any(|request| request.login.as_deref().is_some_and(|login| login.eq_ignore_ascii_case(viewer)))
  {
    return None;
  }
  let teams = requests.into_iter().filter_map(|request| request.slug).collect();
  let mut request = request_of(number, node);
  // a team waits since the pull request was opened: its request event is not read
  request.requested_at = request.created_at;
  request.waiting_since = request.created_at;
  Some(ReviewEntry {
    request,
    involvement: ReviewInvolvement::Team { teams },
  })
}

fn reviewed_entry(number: u32, node: GhPullRequest) -> ReviewEntry {
  let request = request_of(number, node);
  ReviewEntry {
    involvement: ReviewInvolvement::Reviewed {
      state: request.reviewed_state,
      replied_at: None,
      new_commits: has_new_commits(&request),
    },
    request,
  }
}

fn authored_entry(number: u32, mut node: GhPullRequest) -> ReviewEntry {
  let reviews = Nodes::into_nodes(node.latest_reviews.take());
  let requests = Nodes::into_nodes(node.review_requests.take())
    .into_iter()
    .filter_map(|node| node.requested_reviewer)
    .collect();
  let is_draft = node.is_draft.unwrap_or(false);
  ReviewEntry {
    involvement: ReviewInvolvement::Authored {
      reviewers: collect_reviewers(reviews, requests),
      is_draft,
      branch: None,
    },
    request: request_of(number, node),
  }
}

/// What GitHub answered of `link`: open, merged or closed, or none of the number
fn linked_answer(link: LinkedPullRequest, node: Option<GhPullRequest>) -> LinkedAnswer {
  match linked_entry(link, node) {
    Some((entry, true)) => LinkedAnswer::Open { entry: Box::new(entry) },
    Some((_, false)) => LinkedAnswer::Closed { number: link.number },
    None => LinkedAnswer::Missing { number: link.number },
  }
}

/// The linked entry of `node`, and whether it is open; none without one
fn linked_entry(link: LinkedPullRequest, node: Option<GhPullRequest>) -> Option<(ReviewEntry, bool)> {
  let node = node?;
  let open = node.state.as_deref().is_none_or(|state| state == "OPEN");
  let entry = ReviewEntry {
    request: request_of(link.number, node),
    involvement: ReviewInvolvement::Linked { linked_at: link.linked_at },
  };
  Some((entry, open))
}

/// The number of the pull request of `repository` that `input` names: its URL (`https://github.com/o/r/pull/123`, also
/// with `/files` or an anchor after it), `o/r#123`, `#123` or `123`
///
/// # Errors
/// `invalid_arguments` for nothing that names a pull request, or one of another repository
pub fn parse_pull_request_link(input: &str, repository: &GitHubRepository) -> Result<u32, ApiError> {
  let input = input.trim();
  let not_a_link = || ApiError::new(ErrorCode::InvalidArguments, format!("{input} is no link of a pull request"));
  let (named, number) = if let Some(path) = input.strip_prefix("https://").or_else(|| input.strip_prefix("http://")) {
    let path = path.strip_prefix("www.").unwrap_or(path);
    let path = path.strip_prefix("github.com/").ok_or_else(not_a_link)?;
    let mut parts = path.split(['/', '#', '?']);
    let (owner, name, kind, number) = (parts.next(), parts.next(), parts.next(), parts.next());
    if kind != Some("pull") {
      return Err(not_a_link());
    }
    (Some((owner.unwrap_or_default(), name.unwrap_or_default())), number.unwrap_or_default())
  } else if let Some((full_name, number)) = input.split_once('#').filter(|(full_name, _)| !full_name.is_empty()) {
    let (owner, name) = full_name.split_once('/').ok_or_else(not_a_link)?;
    (Some((owner, name)), number)
  } else {
    (None, input.strip_prefix('#').unwrap_or(input))
  };
  let number: u32 = number.parse().ok().filter(|number| *number > 0).ok_or_else(not_a_link)?;
  if let Some((owner, name)) = named
    && !(owner.eq_ignore_ascii_case(&repository.owner) && name.eq_ignore_ascii_case(&repository.name))
  {
    return Err(ApiError::new(
      ErrorCode::InvalidArguments,
      format!("{owner}/{name}#{number} is in another repository: Reviews shows {}", repository.full_name()),
    ));
  }
  Ok(number)
}

/// The query of one linked pull request
fn linked_query(teams: bool) -> String {
  format!(
    "query($owner: String!, $name: String!, $number: Int!) {{\n  viewer {{ login }}\n  repository(owner: $owner, name: $name) {{ viewerPermission pullRequest(number: $number) {{ ...ReviewPull }} }}\n}}\n{}",
    if teams { REVIEW_PULL } else { REVIEW_PULL_WITHOUT_TEAMS }
  )
}

#[derive(Debug, Deserialize)]
struct LinkedData {
  repository: Option<LinkedRepository>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinkedRepository {
  pull_request: Option<GhPullRequest>,
}

/// What GitHub says of the pull request `link` of `repository`, when the user adds it: the entry and whether it is
/// open; none for a number the repository lacks
///
/// # Errors
/// GitHub could not be asked
#[instrument(skip(github))]
pub async fn read_linked_pull_request(
  github: &GitHubService,
  repository_path: &str,
  repository: &GitHubRepository,
  link: LinkedPullRequest,
) -> Result<Option<(ReviewEntry, bool)>, ApiError> {
  let full_name = repository.full_name();
  let variables = serde_json::json!({"owner": repository.owner, "name": repository.name, "number": link.number});
  let (variables, full_name_ref) = (&variables, full_name.as_str());
  let answer = github
    .ask(repository_path, &full_name, RequestKind::Listing, |client, token| async move {
      let teams = !github.lacks_read_org(&token);
      let mut response = client.graphql::<LinkedData>(&token, &linked_query(teams), variables).await?;
      if teams && response.errors.iter().any(|error| error.kind.as_deref() == Some("INSUFFICIENT_SCOPES")) {
        github.note_lacks_read_org(&token);
        response = client.graphql::<LinkedData>(&token, &linked_query(false), variables).await?;
      }
      visible_data(response, full_name_ref, |data| data.repository.is_some())
    })
    .await;
  match answer {
    Ok(data) => Ok(linked_entry(link, data.repository.and_then(|repository| repository.pull_request))),
    Err(failure) => {
      let code = if matches!(failure, GitHubFailure::SignedOut) {
        ErrorCode::NotLoggedIn
      } else {
        ErrorCode::NetworkError
      };
      Err(ApiError::new(code, failure_message(&failure)))
    }
  }
}

/// The user adds a pull request to Reviews of the session of `key` by `input` (`parse_pull_request_link`) at `now` (unix
/// seconds), in the repository the session listed the review requests of, else the one the branches are mirrored to:
/// GitHub is asked for it, an open one joins the links of the repository the host keeps and the links of the session at
/// once, and the session reads Reviews again, which nothing waits for. Answers the entry and whether it was kept: a
/// merged or closed one opens but is not kept
///
/// # Errors
/// No GitHub repository, an input that names no pull request of it, a number it lacks (`no_pull_request`), a failure
/// of GitHub, a host that keeps no links
pub async fn link_pull_request<H: ServiceHost + ?Sized>(host: &H, sessions: &Sessions, key: &SessionKey, input: &str, now: u32) -> Result<(ReviewEntry, bool), ApiError> {
  let github = host.github(key)?;
  let repository = repository_of(host, sessions, key).await?;
  let number = parse_pull_request_link(input, &repository)?;
  let link = LinkedPullRequest { number, linked_at: now };
  let (entry, open) = read_linked_pull_request(&github, &key.worktree, &repository, link)
    .await?
    .ok_or_else(|| ApiError::new(ErrorCode::NoPullRequest, format!("{} has no pull request #{number}", repository.full_name())))?;
  if open {
    host.link_pull_request(key, &repository, link)?;
    sessions.linked(key, entry.clone()).await;
    sessions.refresh_later(key, FactFamily::ReviewRequests, FactRefreshReason::Manual);
  }
  Ok((entry, open))
}

/// The pull request `number` leaves the links of the repository of the session of `key` and the links of the session
/// at once; the session reads Reviews again, which nothing waits for
///
/// # Errors
/// No GitHub repository
pub async fn unlink_pull_request<H: ServiceHost + ?Sized>(host: &H, sessions: &Sessions, key: &SessionKey, number: u32) -> Result<(), ApiError> {
  let repository = repository_of(host, sessions, key).await?;
  host.forget_linked_pull_requests(key, &repository, &[number]);
  sessions.unlinked(key, number).await;
  sessions.refresh_later(key, FactFamily::ReviewRequests, FactRefreshReason::Manual);
  Ok(())
}

/// The repository the session listed the review requests of, else the one the branches are mirrored to
async fn repository_of<H: ServiceHost + ?Sized>(host: &H, sessions: &Sessions, key: &SessionKey) -> Result<GitHubRepository, ApiError> {
  if let Some(listed) = sessions.read(key, |_, facts| facts.review_requests.github_repository()).await.flatten() {
    return Ok(listed);
  }
  host
    .github_repository(key)
    .await?
    .ok_or_else(|| ApiError::new(ErrorCode::InvalidArguments, NOT_ON_GITHUB_MESSAGE))
}

#[cfg(test)]
#[path = "reviews_test.rs"]
mod reviews_test;
