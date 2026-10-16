use crate::client::{GitHubError, classify_failure};
use crate::fake_github::{FakeAnswer, start_fake};
use pretty_assertions::assert_eq;
use reqwest::StatusCode;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::json;

const NOW: u32 = 1_800_000_000;

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
  pairs
    .iter()
    .map(|(name, value)| (HeaderName::from_bytes(name.as_bytes()).unwrap(), HeaderValue::from_str(value).unwrap()))
    .collect()
}

fn classify(status: u16, pairs: &[(&str, &str)], body: &serde_json::Value) -> GitHubError {
  classify_failure(StatusCode::from_u16(status).unwrap(), &headers(pairs), body.to_string().as_bytes(), NOW)
}

#[test]
fn a_401_is_a_rejected_token() {
  assert_eq!(classify(401, &[], &json!({"message": "Bad credentials"})), GitHubError::Unauthorized);
}

#[test]
fn the_primary_rate_limit_waits_for_its_reset() {
  let error = classify(
    403,
    &[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "1800000900")],
    &json!({"message": "API rate limit exceeded"}),
  );
  assert_eq!(error, GitHubError::RateLimited { retry_at: 1_800_000_900 });
}

#[test]
fn a_secondary_rate_limit_waits_for_retry_after_or_a_minute() {
  assert_eq!(classify(429, &[("retry-after", "120")], &json!({})), GitHubError::RateLimited { retry_at: NOW + 120 });
  assert_eq!(
    classify(403, &[], &json!({"message": "You have exceeded a secondary rate limit."})),
    GitHubError::RateLimited { retry_at: NOW + 60 }
  );
  assert_eq!(classify(429, &[], &json!({})), GitHubError::RateLimited { retry_at: NOW + 60 });
}

#[test]
fn a_403_of_saml_sso_carries_the_authorization_url() {
  let error = classify(
    403,
    &[("x-github-sso", "required; url=https://github.com/orgs/acme/sso?authorization_request=abc")],
    &json!({"message": "Resource protected by organization SAML enforcement."}),
  );
  assert_eq!(
    error,
    GitHubError::SsoRequired {
      url: Some("https://github.com/orgs/acme/sso?authorization_request=abc".to_string())
    }
  );
}

#[test]
fn a_403_or_404_is_no_access_with_the_message() {
  let restricted = "Although you appear to have the correct authorization credentials, the `acme` organization has enabled OAuth App access restrictions";
  assert_eq!(
    classify(403, &[], &json!({"message": restricted})),
    GitHubError::NoAccess {
      status: 403,
      message: restricted.to_string()
    }
  );
  assert_eq!(
    classify(404, &[], &json!({"message": "Not Found"})),
    GitHubError::NoAccess {
      status: 404,
      message: "Not Found".to_string()
    }
  );
}

#[test]
fn a_validation_failure_names_its_errors() {
  let error = classify(
    422,
    &[],
    &json!({"message": "Validation Failed", "errors": [{"resource": "PullRequest", "code": "custom", "message": "A pull request already exists for acme:me/virtual/auth."}]}),
  );
  assert_eq!(
    error,
    GitHubError::Http {
      status: 422,
      message: "Validation Failed: A pull request already exists for acme:me/virtual/auth.".to_string()
    }
  );
  assert_eq!(classify(502, &[], &json!(null)).message(), "GitHub answered HTTP 502");
}

#[derive(Debug, Deserialize, PartialEq)]
struct Viewer {
  viewer: Login,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Login {
  login: String,
}

#[tokio::test]
async fn a_graphql_query_sends_the_token_and_returns_data_with_errors() {
  let (fake, client) = start_fake().await;
  fake.answer(
    "POST",
    "/graphql",
    FakeAnswer::ok(json!({"data": {"viewer": {"login": "me"}}, "errors": [{"type": "NOT_FOUND", "message": "Could not resolve"}]})),
  );
  let response = client.graphql::<Viewer>("t0ken", "query { viewer { login } }", &json!({"q": 1})).await.unwrap();
  assert_eq!(
    response.data,
    Some(Viewer {
      viewer: Login { login: "me".to_string() }
    })
  );
  assert_eq!(response.errors[0].kind.as_deref(), Some("NOT_FOUND"));
  assert!(!response.sso_partial);
  let request = &fake.requests_to("POST", "/graphql")[0];
  assert_eq!(request.authorization.as_deref(), Some("Bearer t0ken"));
  assert_eq!(request.json(), json!({"query": "query { viewer { login } }", "variables": {"q": 1}}));
}

#[tokio::test]
async fn a_graphql_rate_limit_is_an_error_and_a_partial_sso_answer_is_flagged() {
  let (fake, client) = start_fake().await;
  fake.answer(
    "POST",
    "/graphql",
    FakeAnswer::ok(json!({"data": null, "errors": [{"type": "RATE_LIMITED", "message": "API rate limit exceeded"}]})).header("x-ratelimit-reset", "1800000900"),
  );
  let error = client.graphql::<Viewer>("t", "query", &json!({})).await.unwrap_err();
  assert_eq!(error, GitHubError::RateLimited { retry_at: 1_800_000_900 });

  fake.answer(
    "POST",
    "/graphql",
    FakeAnswer::ok(json!({"data": {"viewer": {"login": "me"}}})).header("x-github-sso", "partial-results; organizations=21955855"),
  );
  assert!(client.graphql::<Viewer>("t", "query", &json!({})).await.unwrap().sso_partial);
}

#[tokio::test]
async fn a_get_escapes_its_segments_and_asks_conditionally_the_second_time() {
  let (fake, client) = start_fake().await;
  fake.answer_in_turn(
    "GET",
    "/repos/acme/a%20b/issues/comments/7",
    vec![FakeAnswer::ok(json!({"id": 7})).header("etag", "\"v1\""), FakeAnswer::status(304, json!(null))],
  );
  let first: serde_json::Value = client.get("t", &["repos", "acme", "a b", "issues", "comments", "7"], &[]).await.unwrap();
  let second: serde_json::Value = client.get("t", &["repos", "acme", "a b", "issues", "comments", "7"], &[]).await.unwrap();
  assert_eq!(first, json!({"id": 7}));
  assert_eq!(second, json!({"id": 7}));
  let requests = fake.requests_to("GET", "/repos/acme/a%20b/issues/comments/7");
  assert_eq!(requests[0].if_none_match, None);
  assert_eq!(requests[1].if_none_match.as_deref(), Some("\"v1\""));

  // another token is not answered from the cache of the first
  let _ = client.get::<serde_json::Value>("other", &["repos", "acme", "a b", "issues", "comments", "7"], &[]).await;
  assert_eq!(fake.requests_to("GET", "/repos/acme/a%20b/issues/comments/7")[2].if_none_match, None);
}

#[tokio::test]
async fn a_get_is_asked_again_once_after_a_server_error_and_a_post_never() {
  let (fake, client) = start_fake().await;
  fake.answer_in_turn("GET", "/repos/o/r", vec![FakeAnswer::status(502, json!({})), FakeAnswer::ok(json!({"id": 1}))]);
  let repository: serde_json::Value = client.get("t", &["repos", "o", "r"], &[("per_page", "100")]).await.unwrap();
  assert_eq!(repository, json!({"id": 1}));
  assert_eq!(fake.requests_to("GET", "/repos/o/r").len(), 2);
  assert_eq!(fake.requests_to("GET", "/repos/o/r")[0].query.as_deref(), Some("per_page=100"));

  fake.answer_in_turn(
    "POST",
    "/repos/o/r/issues/1/comments",
    vec![FakeAnswer::status(502, json!({})), FakeAnswer::ok(json!({"id": 2}))],
  );
  let error = client
    .post::<serde_json::Value, _>("t", &["repos", "o", "r", "issues", "1", "comments"], &json!({"body": "x"}))
    .await
    .unwrap_err();
  assert!(error.is_transient(), "{error:?}");
  assert_eq!(fake.requests_to("POST", "/repos/o/r/issues/1/comments").len(), 1);
}
