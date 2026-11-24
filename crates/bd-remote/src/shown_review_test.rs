//! A review after landing (show of ship/show/ask) through `brd`: `brd safe-push --request-review` opens the draft of the
//! shown review before its robot starts, `brd request-review --after-landing` opens it once the robot merged, `brd
//! close-review` closes it once every reviewer approved; against the bare origin of a `Space`, a stateful GitHub whose
//! branches are the origin's, and a `CodeOwners` service of the owner groups

use bd_cli::fixture_test::{PREFIX, assert_error, next_steps, run_json};
use bd_cli::remote_fixture_test::{AUTH, Space, robot};
use bd_core::context::Environment;
use codeowners::fake_codeowners::{group_json, member_json, start_fake_server};
use domain_types::pull_request::{GitHubUser, ReviewerState};
use github::fake_github::{FakeGitHub, serve};
use github::fake_repository::{FakeGitHubRepository, RepositoryState};
use github::token::TokenSources;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::Arc;
use test_utils::git_test_utils::{append_config, git_command};
use test_utils::history::HistoryCommit;

const HEAD: &str = "test-user/review/auth/head";
const BASE: &str = "test-user/review/auth/base";

/// `brd <args>` with the prefix of the fixtures
fn bd(environment: &Environment, args: &[&str]) -> (i32, Value) {
  let mut all = vec!["--prefix", PREFIX];
  all.extend_from_slice(args);
  run_json(environment, &all)
}

/// A `Space` whose baseline names the owner group `Air Plugin` of every path, with auth on it; its GitHub is the
/// stateful fake of `acme/monorepo` (the token's user `me`, alice and bob can be asked), whose branches are the ones of
/// the origin; the `CodeOwners` service names alice the reviewer of the group
fn space() -> (Space, Arc<FakeGitHubRepository>, Environment) {
  let space = Space::with_baseline(
    &[("README.md", "readme\n"), ("OWNERSHIP", "/**\nowner 'Air Plugin'\n")],
    &[HistoryCommit::new("(auth) Add auth", &[("auth.txt", "auth\n")])],
  );
  append_config(space.repo.path(), &[("branchdeck.githubRepository", "acme/monorepo")]);
  let repository = Arc::new(FakeGitHubRepository::default());
  let user = |login: &str| GitHubUser {
    login: login.to_owned(),
    name: None,
  };
  repository.set(RepositoryState {
    owner: "acme".to_owned(),
    name: "monorepo".to_owned(),
    token: "t".to_owned(),
    viewer: "me".to_owned(),
    users: vec![user("alice"), user("bob")],
    next_number: 21,
    ..RepositoryState::default()
  });
  let remote = space.remote.path().to_path_buf();
  repository.set_branch_tip(Box::new(move |branch| {
    let output = git_command()
      .args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")])
      .current_dir(&remote)
      .output()
      .unwrap();
    output.status.success().then(|| String::from_utf8(output.stdout).unwrap().trim().to_owned())
  }));
  let url = space.runtime.block_on(serve(Arc::new(FakeGitHub::with_repository(Arc::clone(&repository)))));
  let (codeowners, codeowners_url) = space.runtime.block_on(start_fake_server());
  codeowners.set_group(
    "Air Plugin",
    group_json(
      "Air Plugin",
      &[member_json("me", Some("me"), "AVAILABLE", false), member_json("alice", Some("alice"), "AVAILABLE", false)],
      &[member_json("bob", Some("bob"), "AVAILABLE", false)],
    ),
  );
  let environment = Environment {
    github_api_url: url,
    github_tokens: TokenSources::fixed("t"),
    codeowners_url,
    ..space.environment()
  };
  (space, repository, environment)
}

#[test_log::test]
fn a_check_names_whom_the_review_after_landing_asks_of_the_owner_groups_and_refuses_a_branch_with_a_pull_request() {
  let (space, repository, environment) = space();
  let (exit_code, output) = bd(&environment, &["safe-push", "auth", "--request-review", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  // the reviewers of Air Plugin before its members, never the author
  assert_eq!(
    output["shown"],
    json!({"reviewers": ["alice"], "groups": ["Air Plugin"], "head": HEAD, "base": BASE, "number": null, "url": null})
  );
  assert_eq!((output.get("refusals"), output["mode"].clone()), (None, json!("safe_push")));
  let (exit_code, output) = bd(&environment, &["safe-push", "auth", "--request-review", "--reviewer", "bob", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["shown"]["reviewers"], json!(["bob"]));
  // nothing went out
  assert_eq!((space.fake.starts.lock().unwrap().len(), space.remote_tip(HEAD)), (0, String::new()));

  // a branch with an open pull request has its review there: ask, not show
  repository.update(|state| {
    state.pulls.push(github::fake_repository::FakePull {
      pull_request: domain_types::pull_request::PullRequest {
        head_ref_name: AUTH.to_owned(),
        author: Some("me".to_owned()),
        ..domain_types::fixtures::pull_request("auth", 7)
      },
      body: String::new(),
      comments: Vec::new(),
      assignees: Vec::new(),
      reviews: Vec::new(),
    });
  });
  let (exit_code, output) = bd(&environment, &["safe-push", "auth", "--request-review", "--reviewer", "bob"]);
  assert_error(&output, exit_code, (4, "pull_request_open"));
  assert_eq!(next_steps(&output["error"]).len(), 1);
  assert_eq!(space.fake.starts.lock().unwrap().len(), 0);
  // --reviewer needs a review mode
  let (exit_code, output) = bd(&environment, &["safe-push", "auth", "--reviewer", "bob"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
}

#[test_log::test]
fn the_draft_opens_before_the_start_its_reviewers_are_asked_once_it_merged_and_it_closes_once_they_approved() {
  let (space, repository, environment) = space();
  let (exit_code, output) = bd(&environment, &["safe-push", "auth", "--request-review"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["started_via"].clone(), output["robot"]["id"].clone()), (json!("api"), json!("started")));
  assert_eq!((output["shown"]["number"].clone(), output["shown"]["created"].clone()), (json!(21), json!(true)));
  // its head is the commit the robot takes, its base the fork point on the target
  let tip = space.local_tip(AUTH);
  assert_eq!(space.remote_tip(HEAD), tip);
  let fork = bd_cli::fixture_test::git(&space.repo, &["rev-parse", "refs/remotes/origin/master"]);
  assert_eq!(space.remote_tip(BASE), fork);
  let state = repository.state();
  let draft = &state.pulls[0];
  assert!(draft.pull_request.is_draft);
  assert_eq!((draft.pull_request.head_ref_name.as_str(), draft.pull_request.base_ref_name.as_str()), (HEAD, BASE));
  // nobody asked while the robot runs; its body records whom its open asks
  assert_eq!(draft.pull_request.reviewers, []);
  assert!(draft.body.contains("<!-- branch-deck show reviewers: alice -->"), "{}", draft.body);

  // before the robot merged, it does not open
  let (exit_code, output) = bd(&environment, &["request-review", "auth", "--after-landing"]);
  assert_error(&output, exit_code, (4, "no_shown_review"));

  space.set_robots(vec![robot("r1", "SUCCESS", "auth", &tip)]);
  let (exit_code, output) = bd(&environment, &["request-review", "auth", "--after-landing"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["requested"], json!([{"login": "alice", "full_name": null}]));
  assert_eq!(output["shown"]["state"], json!("draft"));
  let pull = repository.state().pulls[0].pull_request.clone();
  assert!(!pull.is_draft);
  assert_eq!(
    pull.reviewers.iter().map(|reviewer| (reviewer.login.as_str(), reviewer.requested)).collect::<Vec<_>>(),
    [("alice", true)]
  );

  // alice has not approved yet: the close waits, unless the user says so
  let (exit_code, output) = bd(&environment, &["close-review", "auth"]);
  assert_error(&output, exit_code, (4, "review_not_approved"));
  assert_eq!(next_steps(&output["error"]), []);
  repository.update(|state| {
    let reviewers = &mut state.pulls[0].pull_request.reviewers;
    reviewers[0].state = ReviewerState::Approved;
    reviewers[0].requested = false;
  });
  let (exit_code, output) = bd(&environment, &["close-review", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (output["state"].clone(), output["approved_by"].clone(), output["closed"].clone()),
    (json!("approved"), json!(["alice"]), json!(true))
  );
  let state = repository.state();
  assert_eq!(state.pulls[0].pull_request.state, domain_types::pull_request::PullRequestState::Closed);
  assert_eq!(
    state.pulls[0].comments.last().unwrap().body,
    "Reviewed after landing: approved by alice. The change is in master."
  );
  assert_eq!((space.remote_tip(HEAD), space.remote_tip(BASE)), (String::new(), String::new()));
  // closed: none left to close
  let (exit_code, output) = bd(&environment, &["close-review", "auth"]);
  assert_error(&output, exit_code, (4, "no_shown_review"));
}
