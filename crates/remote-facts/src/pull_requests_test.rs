use crate::pull_requests::{PULL_REQUESTS_QUERY, PULL_REQUESTS_QUERY_WITHOUT_TEAMS, list_pull_requests, parse_pull_requests};
use domain_types::pull_request::{ChecksState, ChecksSummary, PullRequestAvailability, PullRequestState, PullRequestsResult, ReviewDecision, Reviewer, ReviewerState, TokenSource};
use git_executor::git_command_executor::GitCommandExecutor;
use github::fake_github::{FakeAnswer, FakeRequest, start_fake_service};
use github::service::GitHubService;
use github::token::{TokenChain, TokenSources};
use pretty_assertions::assert_eq;
use sync_core::github_repository::{GitHubRepository, resolve_github_repository};
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn repository(owner: &str, name: &str) -> GitHubRepository {
  GitHubRepository::new(owner, name)
}

/// The `data` of `PULL_REQUESTS_QUERY`, trimmed to what the parser reads; a null node is skipped
const FIXTURE: &str = r#"{
  "repository": {"viewerPermission": "WRITE"},
  "search": {"nodes": [
    {
      "number": 760,
      "title": "Add login flow",
      "url": "https://github.com/JetBrains/ultimate/pull/760",
      "state": "OPEN",
      "isDraft": false,
      "reviewDecision": "APPROVED",
      "headRefName": "develar/virtual/auth",
      "headRefOid": "1111111111111111111111111111111111111111",
      "baseRefName": "master",
      "author": {"login": "develar"},
      "headRepositoryOwner": {"id": "MDEyOk9yZ2FuaXphdGlvbjg3ODQzNw==", "login": "JetBrains"},
      "isCrossRepository": false,
      "updatedAt": "2026-10-14T09:30:00Z",
      "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": [
        {"__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SUCCESS", "workflowName": "CI"},
        {"__typename": "CheckRun", "name": "tests", "status": "COMPLETED", "conclusion": "FAILURE", "workflowName": "CI"},
        {"__typename": "CheckRun", "name": "lint", "status": "IN_PROGRESS", "conclusion": ""},
        {"__typename": "StatusContext", "context": "safe-push", "state": "ERROR", "targetUrl": "https://example.com"}
      ]}}}}]},
      "latestReviews": {"nodes": [
        {"author": {"login": "alice"}, "state": "APPROVED", "body": ""},
        {"author": {"login": "bob"}, "state": "COMMENTED", "body": "nit"},
        {"author": {"login": "carol"}, "state": "DISMISSED", "body": ""},
        {"author": {"login": "dave"}, "state": "APPROVED", "body": ""}
      ]},
      "reviewRequests": {"nodes": [
        {"requestedReviewer": {"__typename": "User", "login": "bob"}},
        {"requestedReviewer": {"__typename": "User", "login": "dave"}},
        {"requestedReviewer": {"__typename": "Team", "name": "IJ Platform", "slug": "ij-platform"}}
      ]}
    },
    {
      "number": 700,
      "title": "Old attempt at auth",
      "url": "https://github.com/JetBrains/ultimate/pull/700",
      "state": "CLOSED",
      "isDraft": false,
      "reviewDecision": "",
      "headRefName": "develar/virtual/auth",
      "headRefOid": "2222222222222222222222222222222222222222",
      "headRepositoryOwner": {"login": "JetBrains"},
      "isCrossRepository": false,
      "updatedAt": "2026-10-15T09:30:00Z"
    },
    {
      "number": 701,
      "title": "Fork of the same branch name",
      "url": "https://github.com/JetBrains/ultimate/pull/701",
      "state": "OPEN",
      "isDraft": false,
      "reviewDecision": null,
      "headRefName": "develar/virtual/fork",
      "headRefOid": "3333333333333333333333333333333333333333",
      "headRepositoryOwner": {"login": "someone"},
      "isCrossRepository": true,
      "updatedAt": "2026-10-14T09:30:00Z"
    },
    {
      "number": 702,
      "title": "Draft without checks",
      "url": "https://github.com/JetBrains/ultimate/pull/702",
      "state": "OPEN",
      "isDraft": true,
      "reviewDecision": "REVIEW_REQUIRED",
      "headRefName": "develar/virtual/draft-work",
      "headRefOid": "4444444444444444444444444444444444444444",
      "headRepositoryOwner": {"login": "jetbrains"},
      "isCrossRepository": false,
      "updatedAt": "2026-10-13T08:00:00Z",
      "commits": {"nodes": [{"commit": {"statusCheckRollup": null}}]},
      "latestReviews": null,
      "reviewRequests": null
    },
    {
      "number": 650,
      "title": "Not a virtual branch",
      "url": "https://github.com/JetBrains/ultimate/pull/650",
      "state": "MERGED",
      "isDraft": false,
      "reviewDecision": "APPROVED",
      "headRefName": "develar/manual-branch",
      "headRefOid": "5555555555555555555555555555555555555555",
      "headRepositoryOwner": {"login": "JetBrains"},
      "isCrossRepository": false,
      "updatedAt": "2026-10-01T08:00:00Z"
    },
    {
      "number": 640,
      "title": "Merged, then closed again",
      "url": "https://github.com/JetBrains/ultimate/pull/640",
      "state": "MERGED",
      "isDraft": false,
      "reviewDecision": "APPROVED",
      "headRefName": "develar/virtual/done",
      "headRefOid": "6666666666666666666666666666666666666666",
      "headRepositoryOwner": {"login": "JetBrains"},
      "isCrossRepository": false,
      "updatedAt": "2026-10-01T08:00:00Z",
      "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": [
        {"__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SKIPPED"}
      ]}}}}]}
    },
    {
      "number": 641,
      "title": "Closed later",
      "url": "https://github.com/JetBrains/ultimate/pull/641",
      "state": "CLOSED",
      "isDraft": false,
      "reviewDecision": "",
      "headRefName": "develar/virtual/done",
      "headRefOid": "7777777777777777777777777777777777777777",
      "headRepositoryOwner": {"login": "JetBrains"},
      "isCrossRepository": false,
      "updatedAt": "2026-10-02T08:00:00Z",
      "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": [
        {"__typename": "StatusContext", "context": "ci", "state": "PENDING"}
      ]}}}}]}
    },
    null
  ]}
}"#;

/// The data of the search with one open pull request per branch, each with `count` failed check runs
fn failing_checks_fixture(branches: &[(&str, usize)]) -> String {
  let nodes: Vec<String> = branches
    .iter()
    .enumerate()
    .map(|(index, (branch, count))| {
      let checks: Vec<String> = (1..=*count)
        .map(|check| format!(r#"{{"__typename": "CheckRun", "name": "check-{check}", "status": "COMPLETED", "conclusion": "FAILURE"}}"#))
        .collect();
      format!(
        r#"{{"number": {number}, "title": "{branch}", "url": "https://github.com/JetBrains/ultimate/pull/{number}", "state": "OPEN", "isDraft": false,
          "reviewDecision": "", "headRefName": "develar/virtual/{branch}", "headRefOid": "1111111111111111111111111111111111111111",
          "headRepositoryOwner": {{"login": "JetBrains"}}, "isCrossRepository": false, "updatedAt": "2026-10-14T09:30:00Z",
          "commits": {{"nodes": [{{"commit": {{"statusCheckRollup": {{"contexts": {{"nodes": [{checks}]}}}}}}}}]}}}}"#,
        number = 800 + index,
        checks = checks.join(", ")
      )
    })
    .collect();
  format!(r#"{{"repository": {{"viewerPermission": "WRITE"}}, "search": {{"nodes": [{}]}}}}"#, nodes.join(", "))
}

/// The failed checks of a pull request it names: all of them up to one more than the limit, never "and 1 more"; from
/// two more on, the first three (`list_cut::kept`)
#[test]
fn the_failed_checks_of_a_pull_request_are_named_whole_when_only_one_is_past_the_limit() {
  let fixture = failing_checks_fixture(&[("three", 3), ("four", 4), ("five", 5)]);
  let pull_requests = parse_pull_requests(fixture.as_bytes(), "develar", "JetBrains").unwrap();
  let names = |branch: &str| {
    let checks = pull_requests[branch].checks.as_ref().unwrap();
    (checks.failed, checks.failed_names.clone())
  };
  let listed = |count: usize| (1..=count).map(|check| format!("check-{check}")).collect::<Vec<_>>();
  assert_eq!(names("three"), (3, listed(3)));
  assert_eq!(names("four"), (4, listed(4)));
  assert_eq!(names("five"), (5, listed(3)));
}

#[test]
fn parses_and_matches_pull_requests_by_virtual_branch() {
  let pull_requests = parse_pull_requests(FIXTURE.as_bytes(), "develar", "JetBrains").unwrap();
  // the fork and the non-virtual branch are not ours
  assert_eq!(pull_requests.keys().collect::<Vec<_>>(), vec!["auth", "done", "draft-work"]);

  // the open pull request wins over a newer closed one of the same branch
  let auth = &pull_requests["auth"];
  assert_eq!(auth.number, 760);
  assert_eq!(auth.title, "Add login flow");
  assert_eq!(auth.url, "https://github.com/JetBrains/ultimate/pull/760");
  assert_eq!(auth.state, PullRequestState::Open);
  assert!(!auth.is_draft);
  assert_eq!(auth.review_decision, Some(ReviewDecision::Approved));
  assert_eq!(auth.head_ref_name, "develar/virtual/auth");
  assert_eq!(auth.base_ref_name, "master");
  assert_eq!(auth.head_ref_oid, "1111111111111111111111111111111111111111");
  assert_eq!(auth.updated_at, 1791970200);
  assert_eq!(
    auth.checks,
    Some(ChecksSummary {
      state: ChecksState::Failing,
      passed: 1,
      failed: 2,
      pending: 1,
      failed_names: vec!["tests".to_string(), "safe-push".to_string()],
    })
  );
  // a dismissed review says nothing; bob was asked again after commenting, dave after approving: a re-request keeps
  // the review, whose approval still counts for Patronus
  assert_eq!(
    auth.reviewers,
    vec![
      Reviewer {
        login: "alice".to_string(),
        state: ReviewerState::Approved,
        requested: false,
      },
      Reviewer {
        login: "bob".to_string(),
        state: ReviewerState::Commented,
        requested: true,
      },
      Reviewer {
        login: "dave".to_string(),
        state: ReviewerState::Approved,
        requested: true,
      },
      Reviewer {
        login: "ij-platform".to_string(),
        state: ReviewerState::Requested,
        requested: true,
      },
    ]
  );
  assert_eq!(auth.author.as_deref(), Some("develar"));

  // the owner matches case-insensitively; nulls are empty
  let draft = &pull_requests["draft-work"];
  assert!(draft.is_draft);
  assert_eq!(draft.review_decision, Some(ReviewDecision::ReviewRequired));
  assert_eq!(draft.checks, None);
  assert_eq!(draft.reviewers, Vec::<Reviewer>::new());
  assert_eq!(draft.author, None);

  // merged wins over a newer closed one; a skipped check passes
  let done = &pull_requests["done"];
  assert_eq!(done.number, 640);
  assert_eq!(done.state, PullRequestState::Merged);
  assert_eq!(done.checks.as_ref().map(|checks| (checks.state, checks.passed)), Some((ChecksState::Passing, 1)));
}

#[test]
fn closed_pull_requests_prefer_the_newest_and_empty_decisions_are_none() {
  let json = r#"{"search": {"nodes": [{"number": 1, "title": "a", "url": "u1", "state": "CLOSED", "isDraft": false, "reviewDecision": "", "headRefName": "p/virtual/x", "headRefOid": "a", "headRepositoryOwner": {"login": "o"}, "isCrossRepository": false, "updatedAt": "2026-01-01T00:00:00Z", "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": [{"__typename": "StatusContext", "context": "ci", "state": "PENDING"}]}}}}]}, "latestReviews": {"nodes": []}, "reviewRequests": {"nodes": []}}, {"number": 2, "title": "b", "url": "u2", "state": "CLOSED", "isDraft": false, "reviewDecision": "", "headRefName": "p/virtual/x", "headRefOid": "b", "headRepositoryOwner": {"login": "o"}, "isCrossRepository": false, "updatedAt": "2026-01-02T00:00:00.123Z", "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": []}}}}]}, "latestReviews": {"nodes": []}, "reviewRequests": {"nodes": []}}]}}"#;
  let pull_requests = parse_pull_requests(json.as_bytes(), "p", "o").unwrap();
  let x = &pull_requests["x"];
  assert_eq!(x.number, 2);
  assert_eq!(x.review_decision, None);
  assert_eq!(x.updated_at, 1767312000);
}

#[test]
fn a_pull_request_patronus_closed_with_the_merged_label_is_merged() {
  let json = r#"{"search": {"nodes": [{"number": 1, "title": "a", "url": "u1", "state": "CLOSED", "isDraft": false, "reviewDecision": "", "headRefName": "p/virtual/x", "headRefOid": "a", "headRepositoryOwner": {"login": "o"}, "isCrossRepository": false, "updatedAt": "2026-01-01T00:00:00Z", "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": []}}}}]}, "latestReviews": {"nodes": []}, "reviewRequests": {"nodes": []}, "labels": {"nodes": [{"name": "merged"}]}}, {"number": 2, "title": "b", "url": "u2", "state": "CLOSED", "isDraft": false, "reviewDecision": "", "headRefName": "p/virtual/y", "headRefOid": "b", "headRepositoryOwner": {"login": "o"}, "isCrossRepository": false, "updatedAt": "2026-01-01T00:00:00Z", "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": []}}}}]}, "latestReviews": {"nodes": []}, "reviewRequests": {"nodes": []}, "labels": {"nodes": [{"name": "bug"}]}}]}}"#;
  let pull_requests = parse_pull_requests(json.as_bytes(), "p", "o").unwrap();
  assert_eq!(pull_requests["x"].state, PullRequestState::Merged);
  assert_eq!(pull_requests["y"].state, PullRequestState::Closed);
}

#[test]
fn pending_checks_without_failures_are_pending() {
  let json = r#"{"search": {"nodes": [{"number": 1, "title": "a", "url": "u", "state": "OPEN", "isDraft": false, "reviewDecision": "CHANGES_REQUESTED", "headRefName": "p/virtual/x", "headRefOid": "a", "headRepositoryOwner": {"login": "o"}, "isCrossRepository": false, "updatedAt": "2026-01-01T00:00:00Z", "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": [{"__typename": "CheckRun", "name": "build", "status": "QUEUED", "conclusion": ""}, {"__typename": "StatusContext", "context": "ci", "state": "SUCCESS"}, {"__typename": "StatusContext", "context": "legacy", "state": "EXPECTED"}]}}}}]}, "latestReviews": {"nodes": [{"author": {"login": "alice"}, "state": "CHANGES_REQUESTED"}]}, "reviewRequests": {"nodes": []}}]}}"#;
  let x = &parse_pull_requests(json.as_bytes(), "p", "o").unwrap()["x"];
  assert_eq!(x.review_decision, Some(ReviewDecision::ChangesRequested));
  let checks = x.checks.as_ref().unwrap();
  assert_eq!((checks.state, checks.passed, checks.failed, checks.pending), (ChecksState::Pending, 1, 0, 2));
  assert_eq!(x.reviewers[0].state, ReviewerState::ChangesRequested);
}

#[test]
fn rejects_output_that_is_not_a_pull_request_list() {
  parse_pull_requests(b"<html>", "p", "o").unwrap_err();
  assert!(parse_pull_requests(br#"{"search": {"nodes": []}}"#, "p", "o").unwrap().is_empty());
}

/// Lists the pull requests of `repository` with the token `t0ken`, GitHub answering `answer`
async fn list(answer: FakeAnswer, repository: &GitHubRepository) -> (PullRequestsResult, Vec<FakeRequest>) {
  let (fake, github) = start_fake_service("t0ken").await;
  fake.answer("POST", "/graphql", answer);
  let result = list_pull_requests(&github, ".", repository, "develar").await;
  (result, fake.requests_to("POST", "/graphql"))
}

#[test(tokio::test)]
async fn lists_pull_requests_of_the_push_remote_repository() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.add_remote("origin", "git@github.com:JetBrains/ultimate.git").unwrap();
  // pushes go elsewhere than fetches: the push URL decides
  repo.set_config("remote.origin.pushurl", "https://github.com/JetBrains/ultimate-push.git").unwrap();
  let repository = resolve_github_repository(repo.git_executor(), repo.path().to_str().unwrap()).unwrap().unwrap();
  assert_eq!(repository, self::repository("JetBrains", "ultimate-push"));

  let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
  let (result, requests) = list(FakeAnswer::ok(serde_json::json!({ "data": fixture })), &repository).await;
  // the repository the branches are pushed to, for a link to create a pull request
  assert_eq!(
    result.availability,
    PullRequestAvailability::Available {
      owner: "JetBrains".to_string(),
      name: "ultimate-push".to_string()
    }
  );
  assert_eq!(result.pull_requests.keys().collect::<Vec<_>>(), vec!["auth", "done", "draft-work"]);

  let request = &requests[0];
  assert_eq!(request.authorization.as_deref(), Some("Bearer t0ken"));
  assert_eq!(request.json()["query"], PULL_REQUESTS_QUERY);
  assert_eq!(
    request.json()["variables"],
    serde_json::json!({"q": "repo:JetBrains/ultimate-push is:pr author:@me sort:updated-desc", "owner": "JetBrains", "name": "ultimate-push"})
  );
}

/// A pull request of a node of the search: `head` on the remote, `state`, updated at `updated_at`
fn node(number: u32, head: &str, state: &str, updated_at: &str) -> serde_json::Value {
  serde_json::json!({"number": number, "title": format!("#{number}"), "url": format!("https://github.com/o/r/pull/{number}"), "state": state,
    "isDraft": false, "reviewDecision": null, "headRefName": head, "headRefOid": "1111111111111111111111111111111111111111",
    "baseRefName": "master", "headRepositoryOwner": {"login": "o"}, "isCrossRepository": false, "updatedAt": updated_at})
}

/// The pull request of a stack branch (`{prefix}/stack/<root>`, the whole stack) is the stack's, by its root, with the
/// preference of one per branch; never a row's, also when a branch has the root's name
#[test(tokio::test)]
async fn the_pull_request_of_a_stack_branch_is_the_stacks_by_its_root() {
  let nodes = vec![
    node(10, "develar/virtual/auth", "OPEN", "2026-10-14T09:30:00Z"),
    node(11, "develar/stack/auth", "CLOSED", "2026-10-15T09:30:00Z"),
    node(12, "develar/stack/auth", "OPEN", "2026-10-14T09:30:00Z"),
    node(13, "alice/stack/auth", "OPEN", "2026-10-16T09:30:00Z"),
  ];
  let (result, _) = list(
    FakeAnswer::ok(serde_json::json!({"data": {"repository": {"viewerPermission": "WRITE"}, "search": {"nodes": nodes}}})),
    &repository("o", "r"),
  )
  .await;
  let numbers = |pull_requests: &std::collections::BTreeMap<String, domain_types::pull_request::PullRequest>| {
    pull_requests.iter().map(|(name, pull_request)| (name.clone(), pull_request.number)).collect::<Vec<_>>()
  };
  assert_eq!(numbers(&result.pull_requests), vec![("auth".to_string(), 10)]);
  assert_eq!(numbers(&result.stack_pull_requests), vec![("auth".to_string(), 12)]);
  assert_eq!(result.stack_pull_requests["auth"].head_ref_name, "develar/stack/auth");
}

#[test(tokio::test)]
async fn without_a_token_github_is_not_asked() {
  let (fake, client) = github::fake_github::start_fake().await;
  let github = GitHubService::new(client, TokenChain::new(TokenSources::default(), GitCommandExecutor::new()));
  let result = list_pull_requests(&github, ".", &repository("acme", "monorepo"), "develar").await;
  assert_eq!(result, PullRequestsResult::unavailable(PullRequestAvailability::SignedOut));
  assert!(fake.requests().is_empty());
}

#[test(tokio::test)]
async fn a_repository_the_token_can_not_see_is_no_access_not_an_empty_list() {
  let repository = repository("acme", "monorepo");
  let (result, _) = list(
    FakeAnswer::ok(serde_json::json!({"data": {"repository": null, "search": {"nodes": []}}, "errors": [{"type": "NOT_FOUND", "message": "Could not resolve to a Repository with the name 'acme/monorepo'."}]})),
    &repository,
  )
  .await;
  assert_eq!(
    result.availability,
    PullRequestAvailability::NoAccess {
      source: TokenSource::Environment,
      message: "no repository acme/monorepo".to_string()
    }
  );

  // an organization with SAML SSO the token is not authorized for leaves its data out
  let (result, _) = list(
    FakeAnswer::ok(serde_json::json!({"data": {"repository": null, "search": {"nodes": []}}})).header("x-github-sso", "partial-results; organizations=1"),
    &repository,
  )
  .await;
  assert_eq!(result.availability, PullRequestAvailability::SsoRequired { url: None });
}

#[test(tokio::test)]
async fn a_token_without_read_org_is_asked_again_without_teams() {
  let (fake, github) = start_fake_service("t0ken").await;
  fake.answer_in_turn(
    "POST",
    "/graphql",
    vec![
      FakeAnswer::ok(
        serde_json::json!({"data": null, "errors": [{"type": "INSUFFICIENT_SCOPES", "message": "The 'slug' field requires one of the following scopes: ['read:org']"}]}),
      ),
      FakeAnswer::ok(serde_json::json!({"data": {"repository": {"viewerPermission": "READ"}, "search": {"nodes": []}}})),
    ],
  );
  let result = list_pull_requests(&github, ".", &repository("acme", "monorepo"), "develar").await;
  assert!(matches!(result.availability, PullRequestAvailability::Available { .. }), "{result:?}");
  let queries: Vec<serde_json::Value> = fake.requests_to("POST", "/graphql").iter().map(|request| request.json()["query"].clone()).collect();
  assert_eq!(queries, [serde_json::json!(PULL_REQUESTS_QUERY), serde_json::json!(PULL_REQUESTS_QUERY_WITHOUT_TEAMS)]);
  assert!(PULL_REQUESTS_QUERY.contains("... on Team { slug }") && !PULL_REQUESTS_QUERY_WITHOUT_TEAMS.contains("Team"));

  // the missing scope is remembered for the token: the next refresh asks once, without teams (3 searches, not 4)
  let result = list_pull_requests(&github, ".", &repository("acme", "monorepo"), "develar").await;
  assert!(matches!(result.availability, PullRequestAvailability::Available { .. }), "{result:?}");
  let queries: Vec<serde_json::Value> = fake.requests_to("POST", "/graphql").iter().map(|request| request.json()["query"].clone()).collect();
  assert_eq!(
    queries,
    [
      serde_json::json!(PULL_REQUESTS_QUERY),
      serde_json::json!(PULL_REQUESTS_QUERY_WITHOUT_TEAMS),
      serde_json::json!(PULL_REQUESTS_QUERY_WITHOUT_TEAMS)
    ]
  );
}

#[test(tokio::test)]
async fn classifies_failures() {
  let repository = repository("acme", "monorepo");
  let (result, _) = list(FakeAnswer::status(401, serde_json::json!({"message": "Bad credentials"})), &repository).await;
  assert_eq!(result.availability, PullRequestAvailability::NotAuthenticated { source: TokenSource::Environment });

  let (result, _) = list(
    FakeAnswer::status(403, serde_json::json!({"message": "API rate limit exceeded"}))
      .header("x-ratelimit-remaining", "0")
      .header("x-ratelimit-reset", "1800000000"),
    &repository,
  )
  .await;
  assert_eq!(result.availability, PullRequestAvailability::RateLimited { retry_at: 1_800_000_000 });

  let (result, _) = list(FakeAnswer::status(422, serde_json::json!({"message": "Validation Failed"})), &repository).await;
  assert_eq!(
    result.availability,
    PullRequestAvailability::Error {
      message: "Validation Failed".to_string()
    }
  );
}
