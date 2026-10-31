use crate::client::{GitHubError, RestAnswer, STACKS_API_VERSION, WriteMethod, classify_failure};
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
  // a pull request of a branch GitHub doesn't have (yet): the error names the field only
  assert_eq!(
    classify(
      422,
      &[],
      &json!({"message": "Validation Failed", "errors": [{"resource": "PullRequest", "field": "head", "code": "invalid"}]})
    )
    .message(),
    "Validation Failed: head invalid"
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

/// A client of another API version sends it with every request, the client it came from the default one; the cache of
/// the conditional GETs keeps the answers of each version apart
#[tokio::test]
async fn a_client_of_another_api_version_names_it_and_the_default_stays() {
  let (fake, client) = start_fake().await;
  fake.answer_in_turn(
    "GET",
    "/repos/o/r/pulls/7",
    vec![
      FakeAnswer::ok(json!({"number": 7})).header("etag", "\"v1\""),
      FakeAnswer::ok(json!({"number": 7, "stack": null})),
    ],
  );
  let stacks = client.with_api_version(STACKS_API_VERSION);
  let _: serde_json::Value = client.get("t", &["repos", "o", "r", "pulls", "7"], &[]).await.unwrap();
  let newer: serde_json::Value = stacks.get("t", &["repos", "o", "r", "pulls", "7"], &[]).await.unwrap();
  assert_eq!(newer, json!({"number": 7, "stack": null}));
  let requests = fake.requests_to("GET", "/repos/o/r/pulls/7");
  let versions: Vec<Option<&str>> = requests.iter().map(|request| request.api_version.as_deref()).collect();
  assert_eq!(versions, [Some("2022-11-28"), Some("2026-03-10")]);
  assert_eq!(requests[1].if_none_match, None, "the answer of the default version is not the newer one's");

  fake.answer("POST", "/repos/o/r/stacks", FakeAnswer::status(201, json!({"number": 9})));
  let _: serde_json::Value = stacks.post("t", &["repos", "o", "r", "stacks"], &json!({"pull_requests": [7, 8]})).await.unwrap();
  let _ = client.viewer_login("t").await;
  let posted = &fake.requests_to("POST", "/repos/o/r/stacks")[0];
  assert_eq!(posted.api_version.as_deref(), Some(STACKS_API_VERSION));
  assert_eq!(fake.requests_to("GET", "/user")[0].api_version.as_deref(), Some("2022-11-28"));
}

/// A write read by its status: a 204 without a body, a 202, and a refusal of a kept status with the body that names
/// the request; a refusal of another status is classified
#[tokio::test]
async fn a_write_by_its_status_keeps_the_body_of_the_statuses_asked_for() {
  let (fake, client) = start_fake().await;
  let path = ["repos", "o", "r", "stacks", "9", "unstack"];
  fake.answer("POST", "/repos/o/r/stacks/9/unstack", FakeAnswer::status(204, json!(null)));
  let dissolved = client.send(WriteMethod::Post, "t", &path, None, &[]).await.unwrap();
  assert_eq!(dissolved, RestAnswer { status: 204, body: Vec::new() });
  assert_eq!(fake.requests_to("POST", "/repos/o/r/stacks/9/unstack")[0].body, "");

  let merge = ["repos", "o", "r", "pulls", "7", "merge-async"];
  let body = json!({"sha": "a"});
  fake.answer(
    "PUT",
    "/repos/o/r/pulls/7/merge-async",
    FakeAnswer::status(409, json!({"status": "pending", "details": {"uuid": "u1"}})),
  );
  let pending = client.send(WriteMethod::Put, "t", &merge, Some(&body), &[409]).await.unwrap();
  assert_eq!(pending.status, 409);
  assert_eq!(pending.json::<serde_json::Value>().unwrap()["details"]["uuid"], "u1");
  assert_eq!(fake.requests_to("PUT", "/repos/o/r/pulls/7/merge-async")[0].json(), body);
  let refused = client.send(WriteMethod::Put, "t", &merge, Some(&body), &[]).await.unwrap_err();
  assert!(matches!(refused, GitHubError::Http { status: 409, .. }), "{refused:?}");
  assert_eq!(fake.requests_to("PUT", "/repos/o/r/pulls/7/merge-async").len(), 2, "a write is never asked again");
}

/// A read that timed out is not asked again: it already waited all the time a read has (the clock of the test runs
/// ahead while the answer is held)
#[tokio::test(start_paused = true)]
async fn a_get_that_timed_out_is_not_asked_again() {
  let (fake, client) = start_fake().await;
  fake.answer("GET", "/repos/o/r", FakeAnswer::ok(json!({"id": 1})));
  let held = fake.hold("GET", "/repos/o/r");
  let error = client.get::<serde_json::Value>("t", &["repos", "o", "r"], &[]).await.unwrap_err();
  assert!(matches!(error, GitHubError::Timeout), "{error:?}");
  assert_eq!(held.arrivals(), 1);
}
