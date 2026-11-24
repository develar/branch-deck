//! A review after landing (show of ship/show/ask) through `brd`: `brd safe-push --request-review` opens the draft of the
//! shown review before its robot starts, `brd request-review --after-landing` opens it once the robot merged, `brd
//! close-review` closes it once every reviewer approved; against the bare origin of a `Space`, a stateful GitHub whose
//! branches are the origin's, and a `CodeOwners` service of the owner groups

use bd_cli::fixture_test::{PREFIX, assert_error, next_steps, run_json};
use bd_cli::remote_fixture_test::{AUTH, Space, robot, shown_space};
use bd_core::context::Environment;
use codeowners::fake_codeowners::{group_json, member_json, real_lookup, start_fake_server};
use domain_types::fixtures::asked;
use domain_types::pull_request::ReviewerState;
use github::fake_repository::FakeGitHubRepository;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::Arc;

const HEAD: &str = "test-user/review/auth/head";
const BASE: &str = "test-user/review/auth/base";

/// `brd <args>` with the prefix of the fixtures
fn bd(environment: &Environment, args: &[&str]) -> (i32, Value) {
  let mut all = vec!["--prefix", PREFIX];
  all.extend_from_slice(args);
  run_json(environment, &all)
}

/// The `Space` of a review after landing (`shown_space`), whose `CodeOwners` service names alice the reviewer of the
/// group `Air Plugin`
fn space() -> (Space, Arc<FakeGitHubRepository>, Environment) {
  let (space, repository, environment) = shown_space();
  let (codeowners, codeowners_url) = space.runtime.block_on(start_fake_server());
  codeowners.set_group(
    "Air Plugin",
    group_json(
      "Air Plugin",
      &[member_json("me", Some("me"), "AVAILABLE", false), member_json("alice", Some("alice"), "AVAILABLE", false)],
      &[member_json("bob", Some("bob"), "AVAILABLE", false)],
    ),
  );
  let environment = Environment { codeowners_url, ..environment };
  (space, repository, environment)
}

/// The robot `r1` of auth merged at `tip`: `brd request-review auth --after-landing` opens its review; what it printed
fn open_after_landing(space: &Space, environment: &Environment, tip: &str) -> Value {
  space.set_robots(vec![robot("r1", "SUCCESS", "auth", tip)]);
  let (exit_code, output) = bd(environment, &["request-review", "auth", "--after-landing"]);
  assert_eq!(exit_code, 0, "{output}");
  output
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

  let output = open_after_landing(&space, &environment, &tip);
  assert_eq!(output["requested"], json!([{"login": "alice", "full_name": null}]));
  assert_eq!(output["shown"]["state"], json!("draft"));
  let pull = repository.state().pulls[0].pull_request.clone();
  assert!(!pull.is_draft);
  assert_eq!(asked(&pull), [("alice", true)]);

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

/// A fix of auth landed after its shown review opened: `brd robot` tells which review a running Safe Push of the branch
/// extends, `brd request-review auth` puts the fix on the head of the review, asks alice again and records the fix in
/// its body; a second one finds nothing more to add
#[test_log::test]
fn a_fix_that_landed_goes_onto_the_shown_review_and_its_reviewers_are_asked_again() {
  let (space, repository, environment) = space();
  space.publish_target();
  let (exit_code, output) = bd(&environment, &["safe-push", "auth", "--request-review"]);
  assert_eq!(exit_code, 0, "{output}");
  let tip = space.local_tip(AUTH);
  open_after_landing(&space, &environment, &tip);
  // alice requests changes; the robots land auth into master as a copy of their own
  repository.update(|state| {
    let reviewers = &mut state.pulls[0].pull_request.reviewers;
    reviewers[0].state = ReviewerState::ChangesRequested;
    reviewers[0].requested = false;
  });
  space.land_on_target("auth.txt", "auth\n", "Add auth");
  bd_cli::fixture_test::git(&space.repo, &["fetch", "-q", "origin", "master"]);

  // the fix runs: brd robot names the review it extends
  let mut running = robot("r2", "RUNNING", "auth", "0000000000000000000000000000000000000002");
  running["startDateTime"] = json!(bd_cli::remote_fixture_test::minutes_ago(30));
  space.fake.details.lock().unwrap().insert("r2".to_owned(), running.clone());
  space.set_robots(vec![robot("r1", "SUCCESS", "auth", &tip), running]);
  let (exit_code, output) = bd(&environment, &["robot", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["robot"]["id"].clone(), output["extends"].clone()), (json!("r2"), json!(21)), "{output}");

  // it merged: what it took is the fix on the target as the repository fetched it, and the target has a copy of it
  let target = bd_cli::fixture_test::git(&space.repo, &["rev-parse", "refs/remotes/origin/master"]);
  let landed_fix = space.land_on_target("auth.txt", "auth 2\n", "Fix auth");
  bd_cli::fixture_test::git(&space.repo, &["fetch", "-q", "origin", "master"]);
  let source = bd_cli::fixture_test::git(&space.repo, &["commit-tree", &format!("{landed_fix}^{{tree}}"), "-p", &target, "-m", "(auth) Fix auth"]);
  let mut merged = robot("r2", "SUCCESS", "auth", &source);
  merged["startDateTime"] = json!(bd_cli::remote_fixture_test::minutes_ago(30));
  space.fake.details.lock().unwrap().insert("r2".to_owned(), merged.clone());
  space.set_robots(vec![robot("r1", "SUCCESS", "auth", &tip), merged]);

  let (exit_code, output) = bd(&environment, &["request-review", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let head = space.remote_tip(HEAD);
  assert_eq!(output["shown"]["extended"], json!({"fixes": ["r2"], "head": head}));
  assert_eq!(output["requested"], json!([{"login": "alice", "full_name": null}]));
  // the fix on the commit the review showed, the base where it was
  assert_eq!(bd_cli::fixture_test::git(&space.repo, &["rev-parse", &format!("{head}^")]), tip);
  assert_eq!(bd_cli::fixture_test::git(&space.repo, &["show", &format!("{head}:auth.txt")]), "auth 2");
  let state = repository.state();
  let pull = &state.pulls[0];
  assert_eq!(asked(&pull.pull_request), [("alice", true)]);
  assert!(pull.body.contains(&format!("<!-- branch-deck show landed: {source} -->")), "{}", pull.body);

  // taken: nothing more to add, and the merged fix extends nothing
  let (exit_code, output) = bd(&environment, &["request-review", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["shown"].get("extended"), space.remote_tip(HEAD)), (None, head));
  let (exit_code, output) = bd(&environment, &["robot", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["robot"]["state"].clone(), output.get("extends")), (json!("success"), None), "{output}");
}

/// The owner group as the real `CodeOwners` service answers it (members only, one away on vacation): the one available
/// is asked
#[test_log::test]
fn a_member_away_is_not_asked_of_a_group_as_the_real_service_answers_it() {
  let (space, _, environment) = shown_space();
  let (codeowners, codeowners_url) = space.runtime.block_on(start_fake_server());
  codeowners.set_group("Air Plugin", real_lookup());
  let environment = Environment { codeowners_url, ..environment };
  let (exit_code, output) = bd(&environment, &["safe-push", "auth", "--request-review", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (output["shown"]["reviewers"].clone(), output["shown"]["groups"].clone()),
    (json!(["AlexPl292"]), json!(["Air Plugin"]))
  );
}
