use crate::pull_requests::{GetPullRequestsParams, GitHubRepository, get_pull_requests_with, parse_github_repository, parse_pull_requests, parse_space_mirror, run_gh};
use pretty_assertions::assert_eq;
use std::time::Duration;
use sync_types::pull_request::{ChecksState, ChecksSummary, PullRequestAvailability, PullRequestState, ReviewDecision, Reviewer, ReviewerState};
use test_log::test;
use test_utils::fake_executable::fake_executable;
use test_utils::git_test_utils::TestRepo;

/// `gh pr list --json …` output, trimmed to what the parser reads
const FIXTURE: &str = r#"[
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
    "statusCheckRollup": [
      {"__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SUCCESS", "workflowName": "CI"},
      {"__typename": "CheckRun", "name": "tests", "status": "COMPLETED", "conclusion": "FAILURE", "workflowName": "CI"},
      {"__typename": "CheckRun", "name": "lint", "status": "IN_PROGRESS", "conclusion": ""},
      {"__typename": "StatusContext", "context": "safe-push", "state": "ERROR", "targetUrl": "https://example.com"}
    ],
    "latestReviews": [
      {"author": {"login": "alice"}, "state": "APPROVED", "body": ""},
      {"author": {"login": "bob"}, "state": "COMMENTED", "body": "nit"},
      {"author": {"login": "carol"}, "state": "DISMISSED", "body": ""}
    ],
    "reviewRequests": [
      {"__typename": "User", "login": "bob"},
      {"__typename": "Team", "name": "IJ Platform", "slug": "ij-platform"}
    ]
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
    "updatedAt": "2026-10-15T09:30:00Z",
    "statusCheckRollup": [],
    "latestReviews": [],
    "reviewRequests": []
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
    "updatedAt": "2026-10-14T09:30:00Z",
    "statusCheckRollup": [],
    "latestReviews": [],
    "reviewRequests": []
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
    "statusCheckRollup": null,
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
    "updatedAt": "2026-10-01T08:00:00Z",
    "statusCheckRollup": [],
    "latestReviews": [],
    "reviewRequests": []
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
    "statusCheckRollup": [{"__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SKIPPED"}],
    "latestReviews": [],
    "reviewRequests": []
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
    "statusCheckRollup": [{"__typename": "StatusContext", "context": "ci", "state": "PENDING"}],
    "latestReviews": [],
    "reviewRequests": []
  }
]"#;

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
  let json = r#"[
    {"number": 1, "title": "a", "url": "u1", "state": "CLOSED", "isDraft": false, "reviewDecision": "", "headRefName": "p/virtual/x",
     "headRefOid": "a", "headRepositoryOwner": {"login": "o"}, "isCrossRepository": false, "updatedAt": "2026-01-01T00:00:00Z",
     "statusCheckRollup": [{"__typename": "StatusContext", "context": "ci", "state": "PENDING"}], "latestReviews": [], "reviewRequests": []},
    {"number": 2, "title": "b", "url": "u2", "state": "CLOSED", "isDraft": false, "reviewDecision": "", "headRefName": "p/virtual/x",
     "headRefOid": "b", "headRepositoryOwner": {"login": "o"}, "isCrossRepository": false, "updatedAt": "2026-01-02T00:00:00.123Z",
     "statusCheckRollup": [], "latestReviews": [], "reviewRequests": []}
  ]"#;
  let pull_requests = parse_pull_requests(json.as_bytes(), "p", "o").unwrap();
  let x = &pull_requests["x"];
  assert_eq!(x.number, 2);
  assert_eq!(x.review_decision, None);
  assert_eq!(x.updated_at, 1767312000);
}

#[test]
fn a_pull_request_patronus_closed_with_the_merged_label_is_merged() {
  let json = r#"[
    {"number": 1, "title": "a", "url": "u1", "state": "CLOSED", "isDraft": false, "reviewDecision": "", "headRefName": "p/virtual/x",
     "headRefOid": "a", "headRepositoryOwner": {"login": "o"}, "isCrossRepository": false, "updatedAt": "2026-01-01T00:00:00Z",
     "statusCheckRollup": [], "latestReviews": [], "reviewRequests": [], "labels": [{"name": "merged"}]},
    {"number": 2, "title": "b", "url": "u2", "state": "CLOSED", "isDraft": false, "reviewDecision": "", "headRefName": "p/virtual/y",
     "headRefOid": "b", "headRepositoryOwner": {"login": "o"}, "isCrossRepository": false, "updatedAt": "2026-01-01T00:00:00Z",
     "statusCheckRollup": [], "latestReviews": [], "reviewRequests": [], "labels": [{"name": "bug"}]}
  ]"#;
  let pull_requests = parse_pull_requests(json.as_bytes(), "p", "o").unwrap();
  assert_eq!(pull_requests["x"].state, PullRequestState::Merged);
  assert_eq!(pull_requests["y"].state, PullRequestState::Closed);
}

#[test]
fn pending_checks_without_failures_are_pending() {
  let json = r#"[{"number": 1, "title": "a", "url": "u", "state": "OPEN", "isDraft": false, "reviewDecision": "CHANGES_REQUESTED",
    "headRefName": "p/virtual/x", "headRefOid": "a", "headRepositoryOwner": {"login": "o"}, "isCrossRepository": false,
    "updatedAt": "2026-01-01T00:00:00Z", "statusCheckRollup": [
      {"__typename": "CheckRun", "name": "build", "status": "QUEUED", "conclusion": ""},
      {"__typename": "StatusContext", "context": "ci", "state": "SUCCESS"},
      {"__typename": "StatusContext", "context": "legacy", "state": "EXPECTED"}
    ], "latestReviews": [{"author": {"login": "alice"}, "state": "CHANGES_REQUESTED"}], "reviewRequests": []}]"#;
  let x = &parse_pull_requests(json.as_bytes(), "p", "o").unwrap()["x"];
  assert_eq!(x.review_decision, Some(ReviewDecision::ChangesRequested));
  let checks = x.checks.as_ref().unwrap();
  assert_eq!((checks.state, checks.passed, checks.failed, checks.pending), (ChecksState::Pending, 1, 0, 2));
  assert_eq!(x.reviewers[0].state, ReviewerState::ChangesRequested);
}

#[test]
fn rejects_output_that_is_not_a_pull_request_list() {
  assert!(parse_pull_requests(b"To get started with GitHub CLI, please run:  gh auth login", "p", "o").is_err());
  assert!(parse_pull_requests(b"[]", "p", "o").unwrap().is_empty());
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

fn params(repo: &TestRepo) -> GetPullRequestsParams {
  GetPullRequestsParams {
    repository_path: repo.path().to_str().unwrap().to_string(),
    branch_prefix: "develar".to_string(),
  }
}

#[test]
fn lists_pull_requests_of_the_push_remote_repository() {
  let repo = github_repo();
  // pushes go elsewhere than fetches: the push URL decides
  repo.set_config("remote.origin.pushurl", "https://github.com/JetBrains/ultimate-push.git").unwrap();
  let dir = tempfile::tempdir().unwrap();
  let fixture = dir.path().join("fixture.json");
  std::fs::write(&fixture, FIXTURE).unwrap();
  let args_file = dir.path().join("args");
  let env_file = dir.path().join("env");
  let gh = fake_executable(
    dir.path(),
    "gh",
    &format!(
      "printf '%s\\n' \"$@\" > '{}'\necho \"$GH_PROMPT_DISABLED $GH_NO_UPDATE_NOTIFIER $NO_COLOR\" > '{}'\ncat '{}'",
      args_file.display(),
      env_file.display(),
      fixture.display()
    ),
  );

  let result = get_pull_requests_with(repo.git_executor(), &params(&repo), Some(&gh), Duration::from_secs(10)).unwrap();
  // the repository the branches are pushed to, for a link to create a pull request
  assert_eq!(
    result.availability,
    PullRequestAvailability::Available {
      owner: "JetBrains".to_string(),
      name: "ultimate-push".to_string()
    }
  );
  assert_eq!(result.pull_requests.keys().collect::<Vec<_>>(), vec!["auth", "done", "draft-work"]);

  let args = std::fs::read_to_string(&args_file).unwrap();
  assert_eq!(
    args.lines().take(11).collect::<Vec<_>>(),
    vec![
      "pr",
      "list",
      "--repo",
      "github.com/JetBrains/ultimate-push",
      "--author",
      "@me",
      "--state",
      "all",
      "--limit",
      "50",
      "--json"
    ]
  );
  assert!(args.contains("headRefOid,headRepositoryOwner,isCrossRepository"), "{args}");
  assert_eq!(std::fs::read_to_string(&env_file).unwrap().trim(), "1 1 1");
}

#[test]
fn a_remote_outside_github_does_not_run_gh() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  let dir = tempfile::tempdir().unwrap();
  let marker = dir.path().join("ran");
  let gh = fake_executable(dir.path(), "gh", &format!("touch '{}'", marker.display()));

  // no origin at all
  let result = get_pull_requests_with(repo.git_executor(), &params(&repo), Some(&gh), Duration::from_secs(10)).unwrap();
  assert_eq!(result.availability, PullRequestAvailability::NotGitHub);

  repo.add_remote("origin", "git@gitlab.com:JetBrains/ultimate.git").unwrap();
  let result = get_pull_requests_with(repo.git_executor(), &params(&repo), Some(&gh), Duration::from_secs(10)).unwrap();
  assert_eq!(result.availability, PullRequestAvailability::NotGitHub);
  assert!(!marker.exists());
}

/// A checkout whose `origin` is a Space repository: Space mirrors the pushed branches to GitHub
fn mirrored_repo() -> TestRepo {
  let repo = TestRepo::new();
  repo.add_remote("origin", "ssh://git@git.example.com/team/monorepo.git").unwrap();
  repo
}

/// Lists the pull requests of `repo` with a fake `gh`: the availability, and the repository `gh` was asked for
fn list_with_fake_gh(repo: &TestRepo) -> (PullRequestAvailability, Option<String>) {
  let dir = tempfile::tempdir().unwrap();
  let args_file = dir.path().join("args");
  let gh = fake_executable(dir.path(), "gh", &format!("printf '%s\\n' \"$@\" > '{}'\necho '[]'", args_file.display()));
  let result = get_pull_requests_with(repo.git_executor(), &params(repo), Some(&gh), Duration::from_secs(10)).unwrap();
  let asked = std::fs::read_to_string(&args_file).ok().and_then(|args| args.lines().nth(3).map(str::to_string));
  (result.availability, asked)
}

fn acme_monorepo() -> (PullRequestAvailability, Option<String>) {
  (
    PullRequestAvailability::Available {
      owner: "acme".to_string(),
      name: "monorepo".to_string(),
    },
    Some("github.com/acme/monorepo".to_string()),
  )
}

#[test]
fn the_configured_github_repository_wins_over_origin() {
  let repo = mirrored_repo();
  repo.create_commit("Initial commit", "README.md", "# Test");
  assert_eq!(list_with_fake_gh(&repo), (PullRequestAvailability::NotGitHub, None));

  repo.set_config("branchdeck.githubRepository", "acme/monorepo").unwrap();
  assert_eq!(list_with_fake_gh(&repo), acme_monorepo());
}

#[test]
fn reads_the_mirror_from_the_committed_space_settings() {
  let repo = mirrored_repo();
  repo.create_commit(
    "Space settings",
    ".space/repository-settings.json",
    r#"{"version": "1.1", "mirror": {"url": "git@github.com:acme/monorepo.git", "useKeyAuth": true}}"#,
  );
  assert_eq!(list_with_fake_gh(&repo), acme_monorepo());

  // a file of the working tree is not read: a checkout without the commit has no mirror
  repo.git_executor().execute_command(&["update-ref", "-d", "HEAD"], repo.path().to_str().unwrap()).unwrap();
  assert_eq!(list_with_fake_gh(&repo), (PullRequestAvailability::NotGitHub, None));
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

#[test]
fn reports_a_missing_gh() {
  let repo = github_repo();
  let result = get_pull_requests_with(repo.git_executor(), &params(&repo), None, Duration::from_secs(10)).unwrap();
  assert_eq!(result.availability, PullRequestAvailability::GhMissing);

  // a configured path that is gone
  let dir = tempfile::tempdir().unwrap();
  let result = get_pull_requests_with(repo.git_executor(), &params(&repo), Some(&dir.path().join("gh")), Duration::from_secs(10)).unwrap();
  assert_eq!(result.availability, PullRequestAvailability::GhMissing);
  assert!(result.pull_requests.is_empty());
}

#[test]
fn classifies_gh_failures() {
  let dir = tempfile::tempdir().unwrap();
  let cwd = dir.path().to_str().unwrap();
  let timeout = Duration::from_secs(10);

  let gh = fake_executable(dir.path(), "gh", "echo 'To get started with GitHub CLI, please run:  gh auth login' >&2\nexit 4");
  assert_eq!(run_gh(&gh, cwd, &[], timeout), Err(PullRequestAvailability::NotAuthenticated));

  let gh = fake_executable(
    dir.path(),
    "gh",
    "echo 'HTTP 401: Bad credentials (https://api.github.com/graphql)' >&2\necho 'Try authenticating with:  gh auth login' >&2\nexit 1",
  );
  assert_eq!(run_gh(&gh, cwd, &[], timeout), Err(PullRequestAvailability::NotAuthenticated));

  let gh = fake_executable(
    dir.path(),
    "gh",
    "echo >&2\necho 'error connecting to api.github.com' >&2\necho 'check your internet connection' >&2\nexit 1",
  );
  assert_eq!(
    run_gh(&gh, cwd, &[], timeout),
    Err(PullRequestAvailability::Error {
      message: "error connecting to api.github.com".to_string(),
    })
  );

  let gh = fake_executable(dir.path(), "gh", "exit 3");
  assert_eq!(
    run_gh(&gh, cwd, &[], timeout),
    Err(PullRequestAvailability::Error {
      message: "gh exited with code 3".to_string(),
    })
  );
}

#[test]
fn kills_gh_after_the_timeout() {
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_executable(dir.path(), "gh", "exec sleep 30");
  let started = std::time::Instant::now();
  let result = run_gh(&gh, dir.path().to_str().unwrap(), &[], Duration::from_millis(300));
  assert_eq!(
    result,
    Err(PullRequestAvailability::Error {
      message: "gh did not answer within 300ms".to_string(),
    })
  );
  assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
}

#[test]
fn reads_large_output_while_waiting() {
  // more than a pipe buffer: gh would block on a full stdout if it were read only after the exit
  let dir = tempfile::tempdir().unwrap();
  let big = format!("[{}]", vec![r#"{"x": "0123456789012345678901234567890123456789"}"#; 10_000].join(","));
  let fixture = dir.path().join("big.json");
  std::fs::write(&fixture, &big).unwrap();
  let gh = fake_executable(dir.path(), "gh", &format!("cat '{}'", fixture.display()));
  let output = run_gh(&gh, dir.path().to_str().unwrap(), &[], Duration::from_secs(10)).unwrap();
  assert_eq!(output.len(), big.len());
}
