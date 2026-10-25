use bd_cli::fixture_test::assert_covered;
use bd_cli::fixture_test::{PREFIX, assert_error, run_json};
use bd_cli::remote_fixture_test::{evaluated_review, list_pull_requests, needs_code_owner, own_pull_request};
use bd_core::context::Environment;
use github::fake_github::{FakeAnswer, FakeGitHub};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;

const REVIEWERS: &str = "/repos/acme/monorepo/pulls/7/requested_reviewers";

/// `bd request-review` with `args`
fn request_review(environment: &Environment, args: &[&str]) -> (i32, Value) {
  let mut all_args = vec!["--prefix", PREFIX, "request-review"];
  all_args.extend_from_slice(args);
  run_json(environment, &all_args)
}

fn requested_answer(logins: &[&str]) -> FakeAnswer {
  let reviewers: Vec<Value> = logins.iter().map(|login| json!({"login": login})).collect();
  FakeAnswer::status(201, json!({"number": 7, "requested_reviewers": reviewers}))
}

fn bodies(github: &FakeGitHub) -> Vec<Value> {
  github.requests_to("POST", REVIEWERS).iter().map(github::fake_github::FakeRequest::json).collect()
}

#[test_log::test]
fn asks_the_suggested_code_owner_of_the_missing_group_once() {
  let (space, github, environment) = needs_code_owner("NOT_ADDED");
  github.answer("POST", REVIEWERS, requested_answer(&["ikorennoy"]));
  let ilya = json!({"login": "ikorennoy", "full_name": "Ilya Korennoy"});

  // the check names whom it asks, and asks nobody
  let (exit_code, output) = request_review(&environment, &["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (output["readiness"].clone(), output["reviewers"].clone(), output["requested"].clone()),
    (json!("missing"), json!([ilya]), json!([]))
  );
  assert_eq!(output["pull_request"], json!({"number": 7, "url": "https://github.com/acme/monorepo/pull/7"}));
  assert_eq!(bodies(&github), [] as [Value; 0]);

  let (exit_code, output) = request_review(&environment, &["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let (_, schema) = run_json(&environment, &["schema", "--all"]);
  let schema = &schema["outputs"]["request_review"];
  assert_covered(schema, schema, &output, "request_review");
  assert_eq!(bodies(&github), [json!({"reviewers": ["ikorennoy"]})]);
  assert_eq!((output["requested"].clone(), output["waiting_for"].clone()), (json!([ilya]), json!([ilya])));
  assert_eq!(output.get("refused"), None);
  assert_eq!(space.fake.checked_pull_requests.lock().unwrap()[0], json!({"repository": "acme/monorepo", "number": 7}));

  // asked now: nobody is asked again, the pull request waits for him
  let mut asked = own_pull_request();
  asked["reviewRequests"] = json!({"nodes": [{"requestedReviewer": {"__typename": "User", "login": "ikorennoy"}}]});
  list_pull_requests(&github, &[asked]);
  *space.fake.pull_request_review.lock().unwrap() = Some(evaluated_review(false, "REVIEWING"));
  let (exit_code, output) = request_review(&environment, &["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["reviewers"].clone(), output["waiting_for"].clone()), (json!([]), json!([ilya])));
  assert_eq!(bodies(&github).len(), 1);
}

#[test_log::test]
fn asks_the_logins_it_is_given_and_names_whom_github_refuses() {
  let (space, github, environment) = needs_code_owner("NOT_ADDED");
  let author = FakeAnswer::status(422, json!({"message": "Review cannot be requested from pull request author."}));
  github.answer_in_turn("POST", REVIEWERS, vec![author.clone(), requested_answer(&["alice"]), author.clone()]);
  let checks = space.fake.check_calls.load(Ordering::SeqCst);

  let (exit_code, output) = request_review(&environment, &["auth", "--reviewer", "alice", "--reviewer", "me"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["readiness"], Value::Null);
  assert_eq!(output["requested"], json!([{"login": "alice", "full_name": null}]));
  assert_eq!(
    output["refused"],
    json!([{"login": "me", "full_name": null, "reason": "Review cannot be requested from pull request author."}])
  );
  assert_eq!(space.fake.check_calls.load(Ordering::SeqCst), checks, "--reviewer doesn't ask Patronus");

  // GitHub asked nobody: an error that names each with the reason
  github.answer("POST", REVIEWERS, author);
  let (exit_code, output) = request_review(&environment, &["auth", "--reviewer", "me"]);
  assert_error(&output, exit_code, (4, "review_refused"));
  assert_eq!(
    output["error"]["message"],
    json!("GitHub asked nobody to review pull request #7: me: Review cannot be requested from pull request author.")
  );
}

#[test_log::test]
fn a_branch_without_an_open_pull_request_or_a_code_owner_to_ask_is_refused() {
  let (space, github, environment) = needs_code_owner("NOT_ADDED");

  list_pull_requests(&github, &[]);
  let (exit_code, output) = request_review(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "no_pull_request"));
  assert!(
    output["error"]["message"].as_str().unwrap().contains("`bd safe-push auth --create-pull-request`"),
    "{output}"
  );

  let mut draft = own_pull_request();
  draft["isDraft"] = json!(true);
  list_pull_requests(&github, &[draft]);
  let (exit_code, output) = request_review(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "no_pull_request"));
  assert!(output["error"]["message"].as_str().unwrap().contains("is a draft"), "{output}");

  // Patronus doesn't say the GitHub login of the suggested code owner
  list_pull_requests(&github, &[own_pull_request()]);
  let mut review = evaluated_review(false, "NOT_ADDED");
  review["reviewerGroups"][0]["reviewers"][0]["githubUsername"] = Value::Null;
  *space.fake.pull_request_review.lock().unwrap() = Some(review);
  let (exit_code, output) = request_review(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "no_reviewer"));
  assert!(
    output["error"]["message"].as_str().unwrap().contains("IJPL Code Platform: pass --reviewer <login>"),
    "{output}"
  );
  assert_eq!(bodies(&github), [] as [Value; 0]);
}
