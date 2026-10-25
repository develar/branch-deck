use crate::branch_rules::read_branch_rules;
use crate::patronus_command_test::repository;
use domain_types::pull_request::BranchRules;
use github::fake_github::{FakeAnswer, start_fake_service};
use pretty_assertions::assert_eq;
use serde_json::json;
use test_log::test;

const RULES: &str = "/repos/acme/monorepo/rules/branches/master";

#[test(tokio::test)]
async fn a_pull_request_rule_that_dismisses_stale_reviews_on_push_is_found_among_the_rules_of_the_branch() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer(
    "GET",
    RULES,
    FakeAnswer::ok(json!([
      {"type": "deletion", "ruleset_id": 1},
      {"type": "pull_request", "ruleset_id": 2, "parameters": {"required_approving_review_count": 1, "dismiss_stale_reviews_on_push": true}},
    ])),
  );
  assert_eq!(
    read_branch_rules(&github, ".", &repository(), "master").await.unwrap(),
    BranchRules {
      dismiss_stale_reviews_on_push: true
    }
  );
  assert_eq!(fake.requests_to("GET", RULES)[0].query.as_deref(), Some("per_page=100"));
}

#[test(tokio::test)]
async fn a_branch_without_such_a_rule_keeps_the_approvals() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer(
    "GET",
    RULES,
    FakeAnswer::ok(json!([{"type": "pull_request", "parameters": {"dismiss_stale_reviews_on_push": false}}, {"type": "non_fast_forward"}])),
  );
  assert!(!read_branch_rules(&github, ".", &repository(), "master").await.unwrap().dismiss_stale_reviews_on_push);

  fake.answer("GET", RULES, FakeAnswer::ok(json!([])));
  assert!(!read_branch_rules(&github, ".", &repository(), "master").await.unwrap().dismiss_stale_reviews_on_push);
}

#[test(tokio::test)]
async fn a_failed_answer_is_an_error_not_a_guess() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer("GET", RULES, FakeAnswer::status(404, json!({"message": "Not Found"})));
  read_branch_rules(&github, ".", &repository(), "master").await.unwrap_err();
}
