use crate::review_requests::list_review_requests;
use crate::reviews::{parse_pull_request_link, read_linked_pull_request, reviews_query};
use domain_types::error::ErrorCode;
use domain_types::pull_request::{Reviewer, ReviewerState};
use domain_types::review_request::{LinkedAnswer, LinkedPullRequest, ReviewInvolvement, ReviewsAsk};
use github::fake_github::{FakeAnswer, start_fake_service};
use pretty_assertions::assert_eq;
use sync_core::github_repository::GitHubRepository;
use test_log::test;

fn repository() -> GitHubRepository {
  GitHubRepository {
    owner: "acme".to_string(),
    name: "monorepo".to_string(),
  }
}

/// A pull request of the answer with the fields the parser reads
fn pull(number: u32, author: &str, extra: &serde_json::Value) -> serde_json::Value {
  let mut pull = serde_json::json!({
    "number": number,
    "title": format!("Change {number}"),
    "url": format!("https://github.com/acme/monorepo/pull/{number}"),
    "bodyText": "",
    "state": "OPEN",
    "isDraft": false,
    "author": {"login": author},
    "createdAt": "2026-10-05T07:00:00Z",
    "updatedAt": "2026-10-06T07:00:00Z",
    "additions": 1, "deletions": 0, "changedFiles": 1,
    "reviewDecision": "REVIEW_REQUIRED",
    "headRefName": format!("{author}/change-{number}"),
    "headRefOid": "head",
    "baseRefName": "master",
    "baseRefOid": "base",
    "isCrossRepository": false,
    "baseRepository": {"defaultBranchRef": {"name": "master"}},
    "statusCheckRollup": null,
    "viewerLatestReview": null,
    "latestReviews": {"nodes": []},
    "reviewRequests": {"nodes": []}
  });
  for (key, value) in extra.as_object().unwrap() {
    pull[key] = value.clone();
  }
  pull
}

fn answer() -> serde_json::Value {
  serde_json::json!({"data": {
    "viewer": {"login": "carol"},
    "repository": {"viewerPermission": "READ"},
    "search": {"issueCount": 0, "nodes": []},
    "team": {"issueCount": 3, "nodes": [
      pull(101, "alice", &serde_json::json!({"reviewRequests": {"nodes": [{"requestedReviewer": {"__typename": "Team", "slug": "ide-platform"}}]}})),
      // asked by name too: Your Turn has it
      pull(102, "alice", &serde_json::json!({"reviewRequests": {"nodes": [{"requestedReviewer": {"__typename": "User", "login": "Carol"}}]}})),
    ]},
    "reviewed": {"issueCount": 1, "nodes": [
      pull(103, "bob", &serde_json::json!({"viewerLatestReview": {"state": "CHANGES_REQUESTED", "submittedAt": "2026-10-05T08:00:00Z", "commit": {"oid": "older"}}})),
    ]},
    "authored": {"issueCount": 1, "nodes": [
      pull(104, "carol", &serde_json::json!({"isDraft": true, "latestReviews": {"nodes": [{"author": {"login": "dave"}, "state": "APPROVED"}]}, "reviewRequests": {"nodes": [{"requestedReviewer": {"__typename": "User", "login": "erin"}}]}})),
    ]},
    "linked": {
      "p105": pull(105, "frank", &serde_json::json!({})),
      "p106": pull(106, "frank", &serde_json::json!({"state": "MERGED"})),
      "p107": null
    }
  }})
}

#[test(tokio::test)]
async fn reads_the_rest_of_reviews_in_the_request_of_the_review_requests() {
  let (fake, github) = start_fake_service("t0ken").await;
  fake.answer("POST", "/graphql", FakeAnswer::ok(answer()));
  let linked = [105, 106, 107].map(|number| LinkedPullRequest { number, linked_at: number });
  let ask = ReviewsAsk { linked: linked.to_vec() };
  let result = list_review_requests(&github, ".", &repository(), Some(&ask)).await;
  let requests = fake.requests_to("POST", "/graphql");
  assert_eq!(requests.len(), 1, "one request");
  let request = requests[0].json();
  assert_eq!(request["query"], reviews_query(crate::review_requests::REVIEW_REQUESTS_BODY, &[105, 106, 107], true));
  assert_eq!(
    request["variables"]["team"],
    "repo:acme/monorepo is:pr is:open archived:false draft:false review-requested:@me"
  );
  assert_eq!(
    request["variables"]["reviewed"],
    "repo:acme/monorepo is:pr is:open archived:false draft:false reviewed-by:@me -author:@me"
  );
  assert_eq!(request["variables"]["authored"], "repo:acme/monorepo is:pr is:open archived:false author:@me");

  let others = result.others.expect("the rest of Reviews");
  // the request by name is Your Turn's: GitHub's count less it
  assert_eq!(others.team.total_count, 2);
  assert_eq!(others.team.entries.len(), 1);
  assert_eq!(
    others.team.entries[0].involvement,
    ReviewInvolvement::Team {
      teams: vec!["ide-platform".to_owned()]
    }
  );
  assert_eq!(others.team.entries[0].request.waiting_since, others.team.entries[0].request.created_at);
  assert_eq!(
    others.reviewed.entries[0].involvement,
    ReviewInvolvement::Reviewed {
      state: Some(ReviewerState::ChangesRequested),
      replied_at: None,
      new_commits: true,
    }
  );
  assert_eq!(
    others.authored.entries[0].involvement,
    ReviewInvolvement::Authored {
      reviewers: vec![
        Reviewer {
          login: "dave".to_owned(),
          state: ReviewerState::Approved,
          requested: false,
        },
        Reviewer {
          login: "erin".to_owned(),
          state: ReviewerState::Requested,
          requested: true,
        },
      ],
      is_draft: true,
      branch: None,
    }
  );
  // the newest link first: missing, merged, open
  let linked: Vec<(u32, &str)> = others
    .linked
    .iter()
    .map(|answer| {
      let kind = match answer {
        LinkedAnswer::Open { .. } => "open",
        LinkedAnswer::Closed { .. } => "closed",
        LinkedAnswer::Missing { .. } => "missing",
      };
      (answer.number(), kind)
    })
    .collect();
  assert_eq!(linked, vec![(107, "missing"), (106, "closed"), (105, "open")]);
}

#[test(tokio::test)]
async fn without_the_reviews_view_the_request_is_the_one_of_your_turn() {
  let (fake, github) = start_fake_service("t0ken").await;
  fake.answer("POST", "/graphql", FakeAnswer::ok(answer()));
  let result = list_review_requests(&github, ".", &repository(), None).await;
  assert_eq!(result.others, None);
  assert_eq!(fake.requests_to("POST", "/graphql")[0].json()["query"], crate::review_requests::REVIEW_REQUESTS_QUERY);
}

#[test(tokio::test)]
async fn a_token_without_read_org_is_asked_again_without_the_teams() {
  let refused = FakeAnswer::ok(serde_json::json!({"data": null, "errors": [{"type": "INSUFFICIENT_SCOPES", "message": "read:org"}]}));
  let (fake, github) = start_fake_service("t0ken").await;
  fake.answer_in_turn("POST", "/graphql", vec![refused, FakeAnswer::ok(answer())]);
  let ask = ReviewsAsk::default();
  let result = list_review_requests(&github, ".", &repository(), Some(&ask)).await;
  assert!(result.others.is_some());
  let requests = fake.requests_to("POST", "/graphql");
  assert_eq!(requests.len(), 2);
  assert_eq!(requests[1].json()["query"], reviews_query(crate::review_requests::REVIEW_REQUESTS_BODY, &[], false));
}

#[test]
fn a_link_names_a_pull_request_of_the_repository() {
  let number = |input: &str| parse_pull_request_link(input, &repository()).map_err(|error| error.code);
  assert_eq!(number("https://github.com/acme/monorepo/pull/123"), Ok(123));
  assert_eq!(number("https://github.com/Acme/Monorepo/pull/123/files#diff-1"), Ok(123));
  assert_eq!(number(" acme/monorepo#123 "), Ok(123));
  assert_eq!(number("#123"), Ok(123));
  assert_eq!(number("123"), Ok(123));
  assert_eq!(number("https://github.com/other/repo/pull/123"), Err(ErrorCode::InvalidArguments));
  assert_eq!(number("other/repo#123"), Err(ErrorCode::InvalidArguments));
  assert_eq!(number("https://github.com/acme/monorepo/issues/123"), Err(ErrorCode::InvalidArguments));
  assert_eq!(number("#0"), Err(ErrorCode::InvalidArguments));
  assert_eq!(number("review this"), Err(ErrorCode::InvalidArguments));
}

#[test(tokio::test)]
async fn reads_a_link_the_user_adds() {
  let (fake, github) = start_fake_service("t0ken").await;
  fake.answer(
    "POST",
    "/graphql",
    FakeAnswer::ok(
      serde_json::json!({"data": {"viewer": {"login": "carol"}, "repository": {"viewerPermission": "READ", "pullRequest": pull(105, "frank", &serde_json::json!({}))}}}),
    ),
  );
  let link = LinkedPullRequest { number: 105, linked_at: 7 };
  let (entry, open) = read_linked_pull_request(&github, ".", &repository(), link).await.unwrap().expect("the pull request");
  assert!(open);
  assert_eq!(entry.involvement, ReviewInvolvement::Linked { linked_at: 7 });
  assert_eq!(entry.request.title, "Change 105");
  assert_eq!(fake.requests_to("POST", "/graphql")[0].json()["variables"]["number"], 105);
}
