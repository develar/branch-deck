use crate::review_requests::{REVIEW_REQUESTS_QUERY, get_review_requests_with};
use pretty_assertions::assert_eq;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sync_types::pull_request::{ChecksState, PullRequestAvailability, ReviewDecision};
use sync_types::review_request::{GetReviewRequestsParams, ReviewRequest, ReviewRequestsResult};
use test_log::test;
use test_utils::fake_executable::fake_executable;
use test_utils::git_test_utils::TestRepo;

const TIMEOUT: Duration = Duration::from_secs(10);

/// `gh api graphql` output for the review requests query, trimmed to what the parser reads
const FIXTURE: &str = r#"{"data": {
  "viewer": {"login": "Carol"},
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

fn params(repo: &TestRepo, github_repository: Option<&str>) -> GetReviewRequestsParams {
  GetReviewRequestsParams {
    repository_path: repo.path().to_str().unwrap().to_string(),
    github_repository: github_repository.map(str::to_string),
  }
}

fn github_repo() -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.add_remote("origin", "git@github.com:acme/monorepo.git").unwrap();
  repo
}

/// A fake `gh` that writes its arguments (one per line) to `<dir>/args` and runs `body`
fn fake_gh(dir: &Path, body: &str) -> PathBuf {
  fake_executable(dir, "gh", &format!("printf '%s\\n' \"$@\" > '{}'\n{body}", dir.join("args").display()))
}

/// A fake `gh` that prints `json` and exits with `exit_code`, `stderr` on stderr
fn answering_gh(dir: &Path, json: &str, stderr: &str, exit_code: i32) -> PathBuf {
  let fixture = dir.join("answer.json");
  std::fs::write(&fixture, json).unwrap();
  fake_gh(dir, &format!("cat '{}'\necho '{stderr}' >&2\nexit {exit_code}", fixture.display()))
}

fn available() -> PullRequestAvailability {
  PullRequestAvailability::Available {
    owner: "acme".to_string(),
    name: "monorepo".to_string(),
  }
}

#[test]
fn maps_the_review_requests_of_the_search() {
  let repo = github_repo();
  let dir = tempfile::tempdir().unwrap();
  let gh = answering_gh(dir.path(), FIXTURE, "", 0);
  let result = get_review_requests_with(repo.git_executor(), &params(&repo, None), Some(&gh), TIMEOUT).unwrap();

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

#[test]
fn asks_for_the_direct_requests_of_the_repository() {
  let repo = github_repo();
  let dir = tempfile::tempdir().unwrap();
  let gh = answering_gh(dir.path(), FIXTURE, "", 0);
  get_review_requests_with(repo.git_executor(), &params(&repo, None), Some(&gh), TIMEOUT).unwrap();

  let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
  let args: Vec<&str> = args.lines().collect();
  assert_eq!(&args[..3], ["api", "graphql", "-f"]);
  let query = format!("query={REVIEW_REQUESTS_QUERY}");
  let query_lines = query.lines().count();
  assert_eq!(args[3..3 + query_lines].join("\n"), query);
  assert_eq!(
    &args[3 + query_lines..],
    ["-f", "q=repo:acme/monorepo is:pr is:open archived:false draft:false user-review-requested:@me"]
  );
}

#[test]
fn a_known_github_repository_skips_resolving_it() {
  // no remote at all: only the given repository can name it
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  let dir = tempfile::tempdir().unwrap();
  let gh = answering_gh(dir.path(), FIXTURE, "", 0);
  let result = get_review_requests_with(repo.git_executor(), &params(&repo, Some("acme/monorepo")), Some(&gh), TIMEOUT).unwrap();
  assert_eq!(result.availability, available());

  // a malformed one is resolved as if none were given
  let result = get_review_requests_with(repo.git_executor(), &params(&repo, Some("acme")), Some(&gh), TIMEOUT).unwrap();
  assert_eq!(result.availability, PullRequestAvailability::NotGitHub);
}

#[test]
fn a_repository_outside_github_never_runs_gh() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.add_remote("origin", "ssh://git@git.example.com/team/monorepo.git").unwrap();
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_gh(dir.path(), "echo '{}'");
  let result = get_review_requests_with(repo.git_executor(), &params(&repo, None), Some(&gh), TIMEOUT).unwrap();
  assert_eq!(result, ReviewRequestsResult::unavailable(PullRequestAvailability::NotGitHub));
  assert!(!dir.path().join("args").exists());

  // a missing gh is told after the repository
  let result = get_review_requests_with(repo.git_executor(), &params(&repo, Some("acme/monorepo")), None, TIMEOUT).unwrap();
  assert_eq!(result.availability, PullRequestAvailability::GhMissing);
}

#[test]
fn a_partial_answer_is_read() {
  let repo = github_repo();
  let dir = tempfile::tempdir().unwrap();
  // GraphQL errors next to data: gh exits with 1 and prints the answer
  let partial = FIXTURE.replacen(r#"{"data": {"#, r#"{"errors": [{"type": "FORBIDDEN", "message": "Resource protected"}], "data": {"#, 1);
  let gh = answering_gh(dir.path(), &partial, "GraphQL: Resource protected", 1);
  let result = get_review_requests_with(repo.git_executor(), &params(&repo, None), Some(&gh), TIMEOUT).unwrap();
  assert_eq!(result.availability, available());
  assert_eq!(result.requests.len(), 3);
}

#[test]
fn a_rate_limit_is_rate_limited() {
  let repo = github_repo();
  let dir = tempfile::tempdir().unwrap();
  let check = |gh: &Path| get_review_requests_with(repo.git_executor(), &params(&repo, None), Some(gh), TIMEOUT).unwrap().availability;

  let gh = answering_gh(
    dir.path(),
    r#"{"data": null, "errors": [{"type": "RATE_LIMITED", "message": "API rate limit exceeded for user ID 1."}]}"#,
    "GraphQL: API rate limit exceeded for user ID 1.",
    1,
  );
  assert_eq!(check(&gh), PullRequestAvailability::RateLimited);

  let gh = answering_gh(dir.path(), "", "HTTP 403: You have exceeded a secondary rate limit.", 1);
  assert_eq!(check(&gh), PullRequestAvailability::RateLimited);

  let gh = answering_gh(dir.path(), "", "HTTP 429: Too Many Requests", 1);
  assert_eq!(check(&gh), PullRequestAvailability::RateLimited);
}

#[test]
fn classifies_the_other_failures_as_gh_does() {
  let repo = github_repo();
  let dir = tempfile::tempdir().unwrap();
  let check = |gh: &Path| get_review_requests_with(repo.git_executor(), &params(&repo, None), Some(gh), TIMEOUT).unwrap();

  let gh = answering_gh(dir.path(), "", "To get started with GitHub CLI, please run:  gh auth login", 4);
  assert_eq!(check(&gh).availability, PullRequestAvailability::NotAuthenticated);

  let gh = answering_gh(
    dir.path(),
    r#"{"message": "Bad credentials"}"#,
    "HTTP 401: Bad credentials (https://api.github.com/graphql)",
    1,
  );
  assert_eq!(check(&gh).availability, PullRequestAvailability::NotAuthenticated);

  let gh = answering_gh(dir.path(), "", "error connecting to api.github.com", 1);
  assert_eq!(
    check(&gh),
    ReviewRequestsResult::unavailable(PullRequestAvailability::Error {
      message: "error connecting to api.github.com".to_string(),
    })
  );

  // a successful run that is not the answer
  let gh = answering_gh(dir.path(), "not json", "", 0);
  assert!(matches!(check(&gh).availability, PullRequestAvailability::Error { message } if message.starts_with("Unexpected gh output")));
  let gh = answering_gh(dir.path(), r#"{"data": null}"#, "", 0);
  assert_eq!(
    check(&gh).availability,
    PullRequestAvailability::Error {
      message: "Unexpected gh output: no data".to_string(),
    }
  );
}
