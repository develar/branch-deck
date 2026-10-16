//! The app's own sign-in to GitHub: the OAuth device flow of the Branch Deck OAuth app. The app shows a code, the user
//! enters it at github.com/login/device, and the app polls for the token. An OAuth app, not a GitHub App: a GitHub App
//! works only in the repositories it is installed in, and its user tokens expire after 8 hours, so the app and `bd`
//! would race to refresh one token. The client id is public; the device flow needs no secret.

use crate::client::GitHubClient;
use serde::Deserialize;
use std::time::{Duration, Instant};
use tracing::{info, instrument, warn};

/// The client id of the Branch Deck OAuth app, given at build time (`BRANCH_DECK_GITHUB_CLIENT_ID`); without it the app
/// offers no sign-in of its own and takes the token of `gh`, `GH_TOKEN` or git
pub const GITHUB_OAUTH_CLIENT_ID: Option<&str> = option_env!("BRANCH_DECK_GITHUB_CLIENT_ID");

/// `repo`: private repositories, the writes to pull requests; `read:org`: the slug of a team asked to review
pub const SCOPES: &str = "repo read:org";

const GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// How often the wait looks whether it was canceled
const CANCEL_CHECK: Duration = Duration::from_millis(200);
/// `slow_down` without an interval: GitHub adds 5 s
const SLOW_DOWN_STEP: u64 = 5;

/// The answer of `login/device/code`
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCode {
  pub device_code: String,
  /// The code the user enters, e.g. `WDJB-MJHT`
  pub user_code: String,
  /// `https://github.com/login/device`
  pub verification_uri: String,
  /// Seconds the code is valid, 900 by default
  pub expires_in: u64,
  /// Seconds between polls
  pub interval: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessToken {
  pub token: String,
  /// The granted scopes, comma-separated
  pub scopes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceFlowError {
  /// The code expired before the user entered it
  Expired,
  /// The user denied the access
  Denied,
  /// The wait was canceled
  Canceled,
  /// The OAuth app does not allow the device flow
  Disabled,
  Failed(String),
}

impl std::fmt::Display for DeviceFlowError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Expired => f.write_str("the code expired: sign in again"),
      Self::Denied => f.write_str("the access was denied on GitHub"),
      Self::Canceled => f.write_str("canceled"),
      Self::Disabled => f.write_str("the Branch Deck OAuth app does not allow the device flow"),
      Self::Failed(message) => f.write_str(message),
    }
  }
}

/// The answer of `login/oauth/access_token`: a token, or an error in a 200
#[derive(Deserialize)]
struct TokenAnswer {
  access_token: Option<String>,
  #[serde(default)]
  scope: String,
  error: Option<String>,
  error_description: Option<String>,
  interval: Option<u64>,
}

/// Asks GitHub for a device code
#[instrument(skip(client))]
pub async fn request_device_code(client: &GitHubClient, client_id: &str) -> Result<DeviceCode, DeviceFlowError> {
  let response = client
    .http()
    .post(format!("{}/login/device/code", client.web_url()))
    .header(reqwest::header::ACCEPT, "application/json")
    .form(&[("client_id", client_id), ("scope", SCOPES)])
    .timeout(crate::client::REQUEST_TIMEOUT)
    .send()
    .await
    .map_err(|e| DeviceFlowError::Failed(format!("GitHub is not reachable: {e}")))?;
  let status = response.status();
  let body = response.bytes().await.map_err(|e| DeviceFlowError::Failed(e.to_string()))?;
  if let Ok(answer) = serde_json::from_slice::<TokenAnswer>(&body)
    && let Some(error) = answer.error
  {
    return Err(error_of(&error, answer.error_description));
  }
  if !status.is_success() {
    return Err(DeviceFlowError::Failed(format!("GitHub answered HTTP {}", status.as_u16())));
  }
  serde_json::from_slice(&body).map_err(|e| DeviceFlowError::Failed(format!("Unexpected GitHub answer: {e}")))
}

/// Polls for the token of `code` until the user entered it, it expired, or `is_canceled` turned true
#[instrument(skip(client, code, is_canceled))]
pub async fn wait_for_token(client: &GitHubClient, client_id: &str, code: &DeviceCode, is_canceled: impl Fn() -> bool) -> Result<AccessToken, DeviceFlowError> {
  let deadline = Instant::now() + Duration::from_secs(code.expires_in);
  let mut interval = Duration::from_secs(code.interval);
  loop {
    let next = Instant::now() + interval;
    while Instant::now() < next {
      if is_canceled() {
        return Err(DeviceFlowError::Canceled);
      }
      tokio::time::sleep(CANCEL_CHECK.min(next.saturating_duration_since(Instant::now()))).await;
    }
    if is_canceled() {
      return Err(DeviceFlowError::Canceled);
    }
    if Instant::now() >= deadline {
      return Err(DeviceFlowError::Expired);
    }
    let answer = client
      .http()
      .post(format!("{}/login/oauth/access_token", client.web_url()))
      .header(reqwest::header::ACCEPT, "application/json")
      .form(&[("client_id", client_id), ("device_code", code.device_code.as_str()), ("grant_type", GRANT_TYPE)])
      .timeout(crate::client::REQUEST_TIMEOUT)
      .send()
      .await;
    let answer: TokenAnswer = match answer {
      Ok(response) => match response.json().await {
        Ok(answer) => answer,
        Err(e) => return Err(DeviceFlowError::Failed(format!("Unexpected GitHub answer: {e}"))),
      },
      // a network failure while the user is on GitHub: ask again at the next interval
      Err(e) => {
        warn!(error = %e, "cannot ask GitHub for the token");
        continue;
      }
    };
    match (answer.access_token, answer.error.as_deref()) {
      (Some(token), None) => {
        info!(scopes = %answer.scope, "signed in to GitHub");
        return Ok(AccessToken { token, scopes: answer.scope });
      }
      (_, Some("authorization_pending")) => {}
      (_, Some("slow_down")) => {
        interval = answer.interval.map_or(interval + Duration::from_secs(SLOW_DOWN_STEP), Duration::from_secs);
      }
      (_, Some(error)) => return Err(error_of(error, answer.error_description)),
      (None, None) => return Err(DeviceFlowError::Failed("GitHub answered neither a token nor an error".to_string())),
    }
  }
}

fn error_of(error: &str, description: Option<String>) -> DeviceFlowError {
  match error {
    "expired_token" => DeviceFlowError::Expired,
    "access_denied" => DeviceFlowError::Denied,
    "device_flow_disabled" => DeviceFlowError::Disabled,
    // `incorrect_client_credentials`, `incorrect_device_code`, `unsupported_grant_type`: a bug of the app
    _ => DeviceFlowError::Failed(description.unwrap_or_else(|| error.to_string())),
  }
}
