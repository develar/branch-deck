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
      dismiss_stale_reviews_on_push: true,
      requires_pull_request: true,
      ..BranchRules::default()
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

/// Each rule a push into the branch meets is found by its type, among rules of other types, whatever its parameters
#[test(tokio::test)]
async fn the_rules_a_push_into_the_branch_meets_are_found_by_their_types() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer(
    "GET",
    RULES,
    FakeAnswer::ok(json!([
      {"type": "deletion", "ruleset_id": 1},
      {"type": "required_signatures", "ruleset_id": 1},
      {"type": "required_status_checks", "ruleset_id": 2, "parameters": {"required_status_checks": [{"context": "build"}], "strict_required_status_checks_policy": false}},
      {"type": "merge_queue", "ruleset_id": 3, "parameters": {"merge_method": "SQUASH"}},
      {"type": "update", "ruleset_id": 4, "parameters": {"update_allows_fetch_and_merge": false}},
    ])),
  );
  assert_eq!(
    read_branch_rules(&github, ".", &repository(), "master").await.unwrap(),
    BranchRules {
      dismiss_stale_reviews_on_push: false,
      requires_pull_request: false,
      requires_signatures: true,
      required_status_checks: true,
      merge_queue: true,
      restricts_updates: true,
    }
  );

  fake.answer("GET", RULES, FakeAnswer::ok(json!([{"type": "non_fast_forward"}, {"type": "creation"}])));
  assert_eq!(read_branch_rules(&github, ".", &repository(), "master").await.unwrap(), BranchRules::default());
}
