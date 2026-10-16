use crate::device_flow::{AccessToken, DeviceCode, DeviceFlowError, SCOPES, request_device_code, wait_for_token};
use crate::fake_github::{FakeAnswer, FakeGitHub, start_fake};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};

fn code(expires_in: u64) -> DeviceCode {
  DeviceCode {
    device_code: "dev-1".to_string(),
    user_code: "WDJB-MJHT".to_string(),
    verification_uri: "https://github.com/login/device".to_string(),
    expires_in,
    interval: 0,
  }
}

fn token_answers(fake: &FakeGitHub, answers: Vec<serde_json::Value>) {
  fake.answer_in_turn("POST", "/login/oauth/access_token", answers.into_iter().map(FakeAnswer::ok).collect());
}

#[tokio::test]
async fn the_device_code_is_asked_for_the_scopes_of_the_app() {
  let (fake, client) = start_fake().await;
  fake.answer(
    "POST",
    "/login/device/code",
    FakeAnswer::ok(json!({"device_code": "dev-1", "user_code": "WDJB-MJHT", "verification_uri": "https://github.com/login/device", "expires_in": 900, "interval": 5})),
  );
  let code = request_device_code(&client, "client-1").await.unwrap();
  assert_eq!((code.user_code.as_str(), code.expires_in, code.interval), ("WDJB-MJHT", 900, 5));
  let body = fake.requests_to("POST", "/login/device/code")[0].body.clone();
  assert_eq!(body, format!("client_id=client-1&scope={}", SCOPES.replace(' ', "+").replace(':', "%3A")));

  fake.answer(
    "POST",
    "/login/device/code",
    FakeAnswer::ok(json!({"error": "device_flow_disabled", "error_description": "Device Flow must be explicitly enabled"})),
  );
  assert_eq!(request_device_code(&client, "client-1").await.unwrap_err(), DeviceFlowError::Disabled);
}

#[tokio::test]
async fn the_wait_polls_while_pending_slows_down_when_told_and_takes_the_token() {
  let (fake, client) = start_fake().await;
  token_answers(
    &fake,
    vec![
      json!({"error": "authorization_pending"}),
      json!({"error": "slow_down", "interval": 0}),
      json!({"access_token": "gho_new", "token_type": "bearer", "scope": "read:org,repo"}),
    ],
  );
  let token = wait_for_token(&client, "client-1", &code(60), || false).await.unwrap();
  assert_eq!(
    token,
    AccessToken {
      token: "gho_new".to_string(),
      scopes: "read:org,repo".to_string()
    }
  );
  let polls = fake.requests_to("POST", "/login/oauth/access_token");
  assert_eq!(polls.len(), 3);
  assert!(
    polls[0].body.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"),
    "{}",
    polls[0].body
  );
  assert!(polls[0].body.contains("device_code=dev-1"), "{}", polls[0].body);
}

#[tokio::test]
async fn a_denied_or_expired_code_and_a_cancel_end_the_wait() {
  let (fake, client) = start_fake().await;
  token_answers(&fake, vec![json!({"error": "access_denied"})]);
  assert_eq!(wait_for_token(&client, "c", &code(60), || false).await.unwrap_err(), DeviceFlowError::Denied);
  token_answers(&fake, vec![json!({"error": "expired_token"})]);
  assert_eq!(wait_for_token(&client, "c", &code(60), || false).await.unwrap_err(), DeviceFlowError::Expired);
  // past its time the code is not asked about
  let asked = fake.requests().len();
  assert_eq!(wait_for_token(&client, "c", &code(0), || false).await.unwrap_err(), DeviceFlowError::Expired);
  assert_eq!(fake.requests().len(), asked);

  let canceled = AtomicBool::new(true);
  assert_eq!(
    wait_for_token(&client, "c", &code(60), || canceled.load(Ordering::SeqCst)).await.unwrap_err(),
    DeviceFlowError::Canceled
  );
  assert_eq!(fake.requests().len(), asked);
}
