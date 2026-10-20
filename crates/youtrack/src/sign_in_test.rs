use crate::client::YouTrackClient;
use crate::sign_in::{CLIENT_ID, Hub, SignInError, challenge_of, discover_hub, finish_sign_in, parse_callback, start_sign_in};
use github::fake_github::{FakeAnswer, FakeGitHub, start_fake_server};
use pretty_assertions::assert_eq;
use reqwest::Url;
use serde_json::json;
use std::sync::Arc;

fn client() -> YouTrackClient {
  YouTrackClient::new("Branch Deck test")
}

/// A YouTrack with a built-in Hub at `<url>/hub`
async fn youtrack() -> (Arc<FakeGitHub>, String) {
  let (fake, url) = start_fake_server().await;
  fake.answer("GET", "/api/config", FakeAnswer::ok(json!({"ring": {"url": "/hub", "serviceId": "service-1"}})));
  (fake, url)
}

fn query_of(url: &str, name: &str) -> String {
  Url::parse(url).unwrap().query_pairs().find(|(key, _)| key == name).unwrap().1.into_owned()
}

#[tokio::test]
async fn the_hub_is_the_one_the_youtrack_names() {
  let (_fake, url) = youtrack().await;
  assert_eq!(
    discover_hub(&client(), &url).await.unwrap(),
    Hub {
      url: format!("{url}/hub"),
      service_id: "service-1".to_string()
    }
  );
  let (fake, url) = start_fake_server().await;
  fake.answer("GET", "/api/config", FakeAnswer::ok(json!({"ring": {"url": "http://hub.example.com", "serviceId": "s"}})));
  assert!(discover_hub(&client(), &url).await.is_err(), "http to a host that is not loopback");
}

#[tokio::test]
async fn the_browser_answer_is_traded_for_tokens_with_the_verifier_of_the_challenge() {
  let (fake, url) = youtrack().await;
  fake.answer(
    "POST",
    "/hub/api/rest/oauth2/token",
    FakeAnswer::ok(json!({"access_token": "access-1", "expires_in": 3600, "refresh_token": "refresh-1", "token_type": "Bearer"})),
  );
  let pending = start_sign_in(&client(), &url).await.unwrap();
  let authorize = pending.authorize_url.clone();
  assert!(authorize.starts_with(&format!("{url}/hub/api/rest/oauth2/auth?")));
  assert_eq!(
    ["response_type", "client_id", "scope", "access_type", "code_challenge_method"].map(|name| query_of(&authorize, name)),
    ["code", CLIENT_ID, "service-1", "offline", "S256"].map(str::to_string)
  );
  let redirect = query_of(&authorize, "redirect_uri");
  assert!(redirect.starts_with("http://127.0.0.1:"), "{redirect}");
  // the browser: first a favicon, then the answer
  let state = query_of(&authorize, "state");
  let browser = tokio::spawn(async move {
    let http = reqwest::Client::new();
    assert_eq!(http.get(format!("{redirect}/favicon.ico")).send().await.unwrap().status(), 404);
    let page = http.get(format!("{redirect}/?code=code-1&state={state}")).send().await.unwrap();
    (page.status().as_u16(), page.text().await.unwrap())
  });
  let tokens = finish_sign_in(&client(), pending, || false).await.unwrap();
  let (status, page) = browser.await.unwrap();
  assert_eq!(status, 200);
  assert!(page.contains("Signed in to YouTrack"), "{page}");
  assert_eq!(
    (tokens.access_token.as_str(), tokens.expires_in, tokens.refresh_token.as_deref()),
    ("access-1", Some(3600), Some("refresh-1"))
  );

  let form: Vec<(String, String)> = url::form_urlencoded::parse(fake.requests_to("POST", "/hub/api/rest/oauth2/token")[0].body.as_bytes())
    .map(|(key, value)| (key.into_owned(), value.into_owned()))
    .collect();
  let field = |name: &str| form.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone()).unwrap();
  assert_eq!(
    (field("grant_type"), field("code"), field("client_id")),
    ("authorization_code".to_string(), "code-1".to_string(), CLIENT_ID.to_string())
  );
  assert_eq!(challenge_of(&field("code_verifier")), query_of(&authorize, "code_challenge"));
}

#[tokio::test]
async fn a_hub_that_does_not_know_the_client_is_told_before_the_browser_opens() {
  let (fake, url) = youtrack().await;
  fake.answer(
    "GET",
    "/hub/api/rest/oauth2/auth",
    FakeAnswer::status(303, serde_json::Value::Null).header("location", "/hub/auth/oauth/error?error=invalid_client"),
  );
  assert_eq!(start_sign_in(&client(), &url).await.err(), Some(SignInError::ClientRejected));
}

#[tokio::test]
async fn a_canceled_sign_in_stops_waiting() {
  let (_fake, url) = youtrack().await;
  let pending = start_sign_in(&client(), &url).await.unwrap();
  assert_eq!(finish_sign_in(&client(), pending, || true).await.unwrap_err(), SignInError::Canceled);
}

#[test]
fn only_the_answer_of_this_sign_in_is_taken() {
  let head = |target: &str| format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
  assert_eq!(parse_callback(&head("/?code=c&state=s"), "s"), Some(Ok("c".to_string())));
  assert!(matches!(parse_callback(&head("/?code=c&state=other"), "s"), Some(Err(SignInError::Failed(_)))));
  assert_eq!(
    parse_callback(&head("/?error=access_denied&error_description=Denied%20by%20user"), "s"),
    Some(Err(SignInError::Denied("Denied by user".to_string())))
  );
  assert_eq!(parse_callback(&head("/favicon.ico"), "s"), None);
  assert_eq!(parse_callback("POST / HTTP/1.1\r\n\r\n", "s"), None);
}
