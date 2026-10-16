use crate::pull_requests::{
  GitHubRepository, PULL_REQUESTS_QUERY, PULL_REQUESTS_QUERY_WITHOUT_TEAMS, list_pull_requests, parse_github_repository, parse_pull_requests, parse_space_mirror,
  resolve_github_repository,
};
use git_executor::git_command_executor::GitCommandExecutor;
use github::fake_github::{FakeAnswer, FakeRequest, start_fake_service};
use github::service::GitHubService;
use github::token::{TokenChain, TokenSources};
use pretty_assertions::assert_eq;
use sync_types::pull_request::{ChecksState, ChecksSummary, PullRequestAvailability, PullRequestState, PullRequestsResult, ReviewDecision, Reviewer, ReviewerState, TokenSource};
use test_log::test;
use test_utils::git_test_utils::TestRepo;

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
        {"author": {"login": "carol"}, "state": "DISMISSED", "body": ""}
      ]},
      "reviewRequests": {"nodes": [
        {"requestedReviewer": {"__typename": "User", "login": "bob"}},
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
  // a dismissed review says nothing; bob was asked again after commenting
  assert_eq!(
    auth.reviewers,
    vec![
      Reviewer {
        login: "alice".to_string(),
        state: ReviewerState::Approved,
      },
      Reviewer {
        login: "bob".to_string(),
        state: ReviewerState::Requested,
      },
      Reviewer {
        login: "ij-platform".to_string(),
        state: ReviewerState::Requested,
      },
    ]
  );

  // the owner matches case-insensitively; nulls are empty
  let draft = &pull_requests["draft-work"];
  assert!(draft.is_draft);
  assert_eq!(draft.review_decision, Some(ReviewDecision::ReviewRequired));
  assert_eq!(draft.checks, None);
  assert_eq!(draft.reviewers, Vec::<Reviewer>::new());

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
  assert!(parse_pull_requests(b"<html>", "p", "o").is_err());
  assert!(parse_pull_requests(br#"{"search": {"nodes": []}}"#, "p", "o").unwrap().is_empty());
}

fn repository(owner: &str, name: &str) -> GitHubRepository {
  GitHubRepository {
    owner: owner.to_string(),
    name: name.to_string(),
  }
}

#[test]
fn parses_github_remote_urls() {
  for url in [
    "git@github.com:JetBrains/ultimate.git",
    "git@github.com:JetBrains/ultimate",
    "https://github.com/JetBrains/ultimate",
    "https://github.com/JetBrains/ultimate.git",
    "https://github.com/JetBrains/ultimate/",
    "https://user:token@github.com/JetBrains/ultimate.git",
    "ssh://git@github.com/JetBrains/ultimate.git",
    "ssh://git@ssh.github.com:443/JetBrains/ultimate.git",
    "git://github.com/JetBrains/ultimate.git",
    "  https://GitHub.com/JetBrains/ultimate.git\n",
  ] {
    assert_eq!(parse_github_repository(url), Some(repository("JetBrains", "ultimate")), "{url}");
  }
}

#[test]
fn rejects_remote_urls_outside_github() {
  for url in [
    "git@gitlab.com:JetBrains/ultimate.git",
    "https://git.jetbrains.team/ij/ultimate.git",
    "ssh://git@github.example.com/JetBrains/ultimate.git",
    "/Users/me/repos/ultimate.git",
    "../upstream.git",
    ".",
    "https://github.com/JetBrains",
    "https://github.com/JetBrains/ultimate/tree/master",
    "C:\\repos\\ultimate",
  ] {
    assert_eq!(parse_github_repository(url), None, "{url}");
  }
}

fn github_repo() -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.add_remote("origin", "git@github.com:JetBrains/ultimate.git").unwrap();
  repo
}

fn resolve(repo: &TestRepo) -> Option<GitHubRepository> {
  resolve_github_repository(repo.git_executor(), repo.path().to_str().unwrap()).unwrap()
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
  let repo = github_repo();
  // pushes go elsewhere than fetches: the push URL decides
  repo.set_config("remote.origin.pushurl", "https://github.com/JetBrains/ultimate-push.git").unwrap();
  let repository = resolve(&repo).unwrap();
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

#[test]
fn a_remote_outside_github_has_no_github_repository() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  // no origin at all
  assert_eq!(resolve(&repo), None);
  repo.add_remote("origin", "git@gitlab.com:JetBrains/ultimate.git").unwrap();
  assert_eq!(resolve(&repo), None);
}

/// A checkout whose `origin` is a Space repository: Space mirrors the pushed branches to GitHub
fn mirrored_repo() -> TestRepo {
  let repo = TestRepo::new();
  repo.add_remote("origin", "ssh://git@git.example.com/team/monorepo.git").unwrap();
  repo
}

#[test]
fn the_configured_github_repository_wins_over_origin() {
  let repo = mirrored_repo();
  repo.create_commit("Initial commit", "README.md", "# Test");
  assert_eq!(resolve(&repo), None);

  repo.set_config("branchdeck.githubRepository", "acme/monorepo").unwrap();
  assert_eq!(resolve(&repo), Some(repository("acme", "monorepo")));
}

#[test]
fn reads_the_mirror_from_the_committed_space_settings() {
  let repo = mirrored_repo();
  repo.create_commit(
    "Space settings",
    ".space/repository-settings.json",
    r#"{"version": "1.1", "mirror": {"url": "git@github.com:acme/monorepo.git", "useKeyAuth": true}}"#,
  );
  assert_eq!(resolve(&repo), Some(repository("acme", "monorepo")));

  // a file of the working tree is not read: a checkout without the commit has no mirror
  repo.git_executor().execute_command(&["update-ref", "-d", "HEAD"], repo.path().to_str().unwrap()).unwrap();
  assert_eq!(resolve(&repo), None);
}

#[test]
fn a_space_mirror_must_be_on_github() {
  assert_eq!(
    parse_space_mirror(r#"{"mirror": {"url": "https://github.com/acme/monorepo.git"}}"#),
    Some(repository("acme", "monorepo"))
  );
  assert_eq!(parse_space_mirror(r#"{"mirror": {"url": "git@gitlab.com:acme/monorepo.git"}}"#), None);
  assert_eq!(parse_space_mirror(r#"{"mirror": null}"#), None);
  assert_eq!(parse_space_mirror("not json"), None);
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
      message: "GitHub shows the token no repository acme/monorepo".to_string()
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
