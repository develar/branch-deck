use crate::pull_requests::{GitHubRepository, parse_known_github_repository};
use crate::review_requests::{REVIEW_REQUESTS_QUERY, list_review_requests};
use github::fake_github::{FakeAnswer, FakeRequest, start_fake_service};
use pretty_assertions::assert_eq;
use sync_types::pull_request::{ChecksState, PullRequestAvailability, ReviewDecision, TokenSource};
use sync_types::review_request::{ReviewRequest, ReviewRequestsResult};
use test_log::test;

/// The answer to the review requests query, trimmed to what the parser reads
const FIXTURE: &str = r#"{"data": {
  "viewer": {"login": "Carol"},
  "repository": {"viewerPermission": "READ"},
  "search": {"issueCount": 63, "nodes": [
    {
      "number": 840,
      "title": "Load the registry under a lock",
      "url": "https://github.com/acme/monorepo/pull/840",
      "bodyText": "Fixes a race.\nSecond line",
      "author": {"login": "alice"},
      "createdAt": "2026-10-05T07:34:26Z",
      "updatedAt": "2026-10-07T09:03:01Z",
      "additions": 24, "deletions": 10, "changedFiles": 1,
      "reviewDecision": "REVIEW_REQUIRED",
      "headRefName": "alice/registry-race",
      "headRefOid": "1111111111111111111111111111111111111111",
      "baseRefName": "master",
      "baseRefOid": "2222222222222222222222222222222222222222",
      "isCrossRepository": false,
      "baseRepository": {"defaultBranchRef": {"name": "master"}},
      "statusCheckRollup": {"state": "SUCCESS"},
      "viewerLatestReview": null,
      "timelineItems": {"nodes": [
        {"createdAt": "2026-10-05T07:34:27Z", "actor": {"login": "alice"}, "requestedReviewer": {"login": "bob"}},
        {"createdAt": "2026-10-06T10:00:00Z", "actor": {"login": "alice"}, "requestedReviewer": {"login": "carol"}},
        {"createdAt": "2026-10-07T09:03:01Z", "actor": {"login": "bob"}, "requestedReviewer": {"login": "carol"}},
        {"createdAt": "2026-10-08T09:00:00Z", "actor": {"login": "alice"}, "requestedReviewer": {}}
      ]}
    },
    {
      "number": 830,
      "title": "Sort the report",
      "url": "https://github.com/acme/monorepo/pull/830",
      "bodyText": "",
      "author": null,
      "createdAt": "2026-10-01T08:00:00Z",
      "updatedAt": "2026-10-02T08:00:00Z",
      "additions": 3, "deletions": 0, "changedFiles": 2,
      "reviewDecision": null,
      "headRefName": "feature/report",
      "headRefOid": "3333333333333333333333333333333333333333",
      "baseRefName": "release/253",
      "baseRefOid": "4444444444444444444444444444444444444444",
      "isCrossRepository": true,
      "baseRepository": {"defaultBranchRef": {"name": "master"}},
      "statusCheckRollup": {"state": "FAILURE"},
      "viewerLatestReview": {"submittedAt": "2026-10-01T12:00:00Z", "commit": {"oid": "5555555555555555555555555555555555555555"}},
      "timelineItems": {"nodes": [
        {"createdAt": "2026-10-02T08:00:00Z", "actor": {"login": "dave"}, "requestedReviewer": {"login": "carol"}}
      ]}
    },
    {
      "number": 820,
      "title": "Reviewed after the request: the search lags",
      "url": "https://github.com/acme/monorepo/pull/820",
      "bodyText": "",
      "author": {"login": "erin"},
      "createdAt": "2026-10-01T08:00:00Z",
      "updatedAt": "2026-10-03T08:00:00Z",
      "additions": 1, "deletions": 1, "changedFiles": 1,
      "reviewDecision": "APPROVED",
      "headRefName": "erin/fix",
      "headRefOid": "6666666666666666666666666666666666666666",
      "baseRefName": "master",
      "baseRefOid": "2222222222222222222222222222222222222222",
      "isCrossRepository": false,
      "baseRepository": {"defaultBranchRef": {"name": "master"}},
      "statusCheckRollup": null,
      "viewerLatestReview": {"submittedAt": "2026-10-03T08:00:00Z", "commit": {"oid": "6666666666666666666666666666666666666666"}},
      "timelineItems": {"nodes": [
        {"createdAt": "2026-10-01T08:00:01Z", "actor": {"login": "erin"}, "requestedReviewer": {"login": "carol"}}
      ]}
    },
    {
      "number": 810,
      "title": "A pending review is no review",
      "url": "https://github.com/acme/monorepo/pull/810",
      "bodyText": "",
      "author": {"login": "frank"},
      "createdAt": "2026-09-30T08:00:00Z",
      "updatedAt": "2026-09-30T09:00:00Z",
      "additions": 0, "deletions": 5, "changedFiles": 1,
      "reviewDecision": "CHANGES_REQUESTED",
      "headRefName": "frank/cleanup",
      "headRefOid": "7777777777777777777777777777777777777777",
      "baseRefName": "master",
      "baseRefOid": "2222222222222222222222222222222222222222",
      "isCrossRepository": false,
      "baseRepository": {"defaultBranchRef": null},
      "statusCheckRollup": {"state": "EXPECTED"},
      "viewerLatestReview": {"submittedAt": null, "commit": {"oid": "7777777777777777777777777777777777777777"}},
      "timelineItems": {"nodes": []}
    },
    {},
    null
  ]}
}}"#;

fn repository() -> GitHubRepository {
  GitHubRepository {
    owner: "acme".to_string(),
    name: "monorepo".to_string(),
  }
}

/// The review requests of acme/monorepo with the token `t0ken`, GitHub answering `answer`
async fn list(answer: FakeAnswer) -> (ReviewRequestsResult, Vec<FakeRequest>) {
  let (fake, github) = start_fake_service("t0ken").await;
  fake.answer("POST", "/graphql", answer);
  let result = list_review_requests(&github, ".", &repository()).await;
  (result, fake.requests_to("POST", "/graphql"))
}

fn fixture() -> FakeAnswer {
  FakeAnswer::ok(serde_json::from_str(FIXTURE).unwrap())
}

fn available() -> PullRequestAvailability {
  PullRequestAvailability::Available {
    owner: "acme".to_string(),
    name: "monorepo".to_string(),
  }
}

#[test(tokio::test)]
async fn maps_the_review_requests_of_the_search() {
  let (result, _) = list(fixture()).await;

  assert_eq!(result.availability, available());
  // all the search found, also beyond the first 50
  assert_eq!(result.total_count, 63);
  // the lagging #820 is dropped, the issue and the null node are skipped
  assert_eq!(result.requests.iter().map(|request| request.number).collect::<Vec<_>>(), vec![840, 830, 810]);

  // waiting since the newest request of the viewer (the login matches case-insensitively), asked by its actor
  assert_eq!(
    result.requests[0],
    ReviewRequest {
      number: 840,
      title: "Load the registry under a lock".to_string(),
      url: "https://github.com/acme/monorepo/pull/840".to_string(),
      body_text: "Fixes a race.\nSecond line".to_string(),
      author: Some("alice".to_string()),
      requested_by: Some("bob".to_string()),
      created_at: 1791185666,
      updated_at: 1791363781,
      waiting_since: 1791363781,
      additions: 24,
      deletions: 10,
      changed_files: 1,
      checks_state: Some(ChecksState::Passing),
      review_decision: Some(ReviewDecision::ReviewRequired),
      re_requested: false,
      reviewed_commit: None,
      head_ref_name: "alice/registry-race".to_string(),
      head_ref_oid: "1111111111111111111111111111111111111111".to_string(),
      base_ref_name: "master".to_string(),
      base_ref_oid: "2222222222222222222222222222222222222222".to_string(),
      is_cross_repository: false,
      base_is_default: true,
    }
  );

  // reviewed before the newest request: re-requested, with the reviewed commit; a deleted author is None
  let re_requested = &result.requests[1];
  assert!(re_requested.re_requested);
  assert_eq!(re_requested.reviewed_commit.as_deref(), Some("5555555555555555555555555555555555555555"));
  assert_eq!(re_requested.author, None);
  assert_eq!(re_requested.requested_by.as_deref(), Some("dave"));
  assert_eq!(re_requested.checks_state, Some(ChecksState::Failing));
  assert_eq!(re_requested.review_decision, None);
  assert!(re_requested.is_cross_repository);
  assert!(!re_requested.base_is_default);

  // no request event: waiting since the pull request was opened; a pending review does not count
  let pending = &result.requests[2];
  assert_eq!(pending.waiting_since, pending.created_at);
  assert_eq!(pending.requested_by, None);
  assert!(!pending.re_requested);
  assert_eq!(pending.reviewed_commit, None);
  assert_eq!(pending.checks_state, Some(ChecksState::Pending));
  assert_eq!(pending.review_decision, Some(ReviewDecision::ChangesRequested));
  // without a default branch the base is not the default one
  assert!(!pending.base_is_default);
}

#[test(tokio::test)]
async fn asks_for_the_direct_requests_of_the_repository() {
  let (_, requests) = list(fixture()).await;
  let request = requests[0].json();
  assert_eq!(request["query"], REVIEW_REQUESTS_QUERY);
  assert_eq!(
    request["variables"],
    serde_json::json!({"q": "repo:acme/monorepo is:pr is:open archived:false draft:false user-review-requested:@me", "owner": "acme", "name": "monorepo"})
  );
  assert_eq!(requests[0].authorization.as_deref(), Some("Bearer t0ken"));
}

#[test]
fn a_known_github_repository_skips_resolving_it() {
  assert_eq!(parse_known_github_repository(Some("acme/monorepo")), Some(repository()));
  // a malformed one is resolved as if none were given
  assert_eq!(parse_known_github_repository(Some("acme")), None);
  assert_eq!(parse_known_github_repository(None), None);
}

#[test(tokio::test)]
async fn a_partial_answer_is_read() {
  // GraphQL errors next to data
  let partial = FIXTURE.replacen(r#"{"data": {"#, r#"{"errors": [{"type": "FORBIDDEN", "message": "Resource protected"}], "data": {"#, 1);
  let (result, _) = list(FakeAnswer::ok(serde_json::from_str(&partial).unwrap())).await;
  assert_eq!(result.availability, available());
  assert_eq!(result.requests.len(), 3);
}

#[test(tokio::test)]
async fn a_rate_limit_is_rate_limited() {
  let (result, _) = list(
    FakeAnswer::ok(serde_json::json!({"data": null, "errors": [{"type": "RATE_LIMITED", "message": "API rate limit exceeded for user ID 1."}]}))
      .header("x-ratelimit-reset", "1800000000"),
  )
  .await;
  assert_eq!(result.availability, PullRequestAvailability::RateLimited { retry_at: 1_800_000_000 });

  let (result, _) = list(FakeAnswer::status(403, serde_json::json!({"message": "You have exceeded a secondary rate limit."})).header("retry-after", "0")).await;
  assert!(matches!(result.availability, PullRequestAvailability::RateLimited { .. }), "{result:?}");
}

#[test(tokio::test)]
async fn classifies_the_other_failures() {
  let (result, _) = list(FakeAnswer::status(401, serde_json::json!({"message": "Bad credentials"}))).await;
  assert_eq!(result.availability, PullRequestAvailability::NotAuthenticated { source: TokenSource::Environment });

  let (result, _) = list(FakeAnswer::ok(serde_json::json!("not the answer"))).await;
  assert!(
    matches!(&result.availability, PullRequestAvailability::Error { message } if message.starts_with("Unexpected GitHub answer")),
    "{result:?}"
  );
  let (result, _) = list(FakeAnswer::ok(serde_json::json!({"data": null}))).await;
  assert_eq!(
    result,
    ReviewRequestsResult::unavailable(PullRequestAvailability::Error {
      message: "Unexpected GitHub answer: no data".to_string(),
    })
  );
}
