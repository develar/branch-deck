//! The REST API of a YouTrack and the OAuth endpoints of its Hub. Every call is one HTTP request; a read is repeated at
//! most once on a transient failure, a write never. Redirects are not followed: a token or a code goes only to the URL
//! it was meant for.

use reqwest::header::ACCEPT;
use reqwest::{Method, StatusCode, Url};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::time::Duration;
use tracing::{debug, instrument, warn};

/// The connection is reused: a popover's lookup is one small request on a kept-alive TLS connection
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// A transient failure of a read is asked again once, after this pause
const RETRY_DELAY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YouTrackError {
  /// 401: the token is missing, expired or revoked
  Unauthorized,
  /// 404: no such issue, or one the user may not see
  NotFound,
  /// Another HTTP failure, with YouTrack's `error_description`, e.g. the 400 of a command a workflow refused
  Http {
    status: u16,
    message: String,
  },
  Timeout,
  /// No connection: offline, DNS, TLS
  Network(String),
  /// The answer is not what this version expects, or the URL is not allowed
  InvalidResponse(String),
}

impl YouTrackError {
  /// Worth asking again right away: the server or the network failed, not the request
  pub const fn is_transient(&self) -> bool {
    matches!(self, Self::Timeout | Self::Network(_) | Self::Http { status: 500.., .. })
  }

  /// A read is asked once more after it: the server failed, or the connection did (refused, reset); never after a
  /// timeout, which already waited all the time a read has
  pub const fn is_asked_again(&self) -> bool {
    self.is_transient() && !matches!(self, Self::Timeout)
  }

  /// For the user: YouTrack's message when it gave one, else what failed
  pub fn message(&self) -> String {
    match self {
      Self::Unauthorized => "YouTrack rejected the token".to_string(),
      Self::NotFound => "Not found, or no access".to_string(),
      Self::Http { message, .. } => message.clone(),
      Self::Timeout => "YouTrack did not answer in time".to_string(),
      Self::Network(message) => format!("YouTrack is not reachable: {message}"),
      Self::InvalidResponse(message) => format!("Unexpected YouTrack answer: {message}"),
    }
  }
}

/// The error body of YouTrack and Hub: `{"error": "invalid_grant", "error_description": "…"}`
#[derive(Deserialize)]
pub struct ErrorBody {
  pub error: Option<String>,
  pub error_description: Option<String>,
}

#[derive(Clone)]
pub struct YouTrackClient {
  http: reqwest::Client,
}

impl YouTrackClient {
  pub fn new(user_agent: &str) -> Self {
    let http = reqwest::Client::builder()
      .connect_timeout(CONNECT_TIMEOUT)
      .user_agent(user_agent)
      .redirect(reqwest::redirect::Policy::none())
      .build()
      .expect("the HTTP client of YouTrack");
    Self { http }
  }

  pub const fn http(&self) -> &reqwest::Client {
    &self.http
  }

  /// A GET with the token (None: an endpoint every visitor may ask, `/api/config`), asked again once after a failure
  /// of the server or the connection (`is_asked_again`)
  #[instrument(skip(self, token), fields(url = %url.path()))]
  pub async fn get<T: DeserializeOwned>(&self, url: &Url, token: Option<&str>) -> Result<T, YouTrackError> {
    let body = match self.send(Method::GET, url, token, None).await {
      Err(e) if e.is_asked_again() => {
        debug!(error = ?e, "asking YouTrack again");
        tokio::time::sleep(RETRY_DELAY).await;
        self.send(Method::GET, url, token, None).await?
      }
      result => result?,
    };
    parse(&body)
  }

  /// A POST of JSON; never repeated by itself
  #[instrument(skip(self, token, body), fields(url = %url.path()))]
  pub async fn post<T: DeserializeOwned>(&self, url: &Url, token: &str, body: &serde_json::Value) -> Result<T, YouTrackError> {
    parse(&self.send(Method::POST, url, Some(token), Some(body)).await?)
  }

  /// A DELETE; never repeated by itself
  #[instrument(skip(self, token), fields(url = %url.path()))]
  pub async fn delete(&self, url: &Url, token: &str) -> Result<(), YouTrackError> {
    self.send(Method::DELETE, url, Some(token), None).await.map(drop)
  }

  async fn send(&self, method: Method, url: &Url, token: Option<&str>, body: Option<&serde_json::Value>) -> Result<Vec<u8>, YouTrackError> {
    // the cost of the operation it is sent for, if any
    sync_utils::operation_tally::http_request();
    let mut request = self.http.request(method, url.clone()).header(ACCEPT, "application/json").timeout(REQUEST_TIMEOUT);
    if let Some(token) = token {
      request = request.bearer_auth(token);
    }
    if let Some(body) = body {
      request = request.json(body);
    }
    let response = request.send().await.map_err(|e| request_error(&e))?;
    let status = response.status();
    let bytes = response.bytes().await.map_err(|e| request_error(&e))?.to_vec();
    if status.is_success() {
      return Ok(bytes);
    }
    let error = classify_failure(status, &bytes);
    warn!(status = status.as_u16(), error = ?error, "YouTrack request failed");
    Err(error)
  }
}

fn parse<T: DeserializeOwned>(body: &[u8]) -> Result<T, YouTrackError> {
  serde_json::from_slice(body).map_err(|e| YouTrackError::InvalidResponse(e.to_string()))
}

/// What a failed answer means; a redirect is a failure too (the URL is not the YouTrack, or it moved)
pub fn classify_failure(status: StatusCode, body: &[u8]) -> YouTrackError {
  match status {
    StatusCode::UNAUTHORIZED => YouTrackError::Unauthorized,
    StatusCode::NOT_FOUND => YouTrackError::NotFound,
    _ => {
      let described = serde_json::from_slice::<ErrorBody>(body).ok().and_then(|body| body.error_description.or(body.error));
      let message = described.unwrap_or_else(|| {
        if status.is_redirection() {
          format!("YouTrack answered with a redirect (HTTP {}): check the URL", status.as_u16())
        } else {
          format!("YouTrack answered HTTP {}", status.as_u16())
        }
      });
      YouTrackError::Http { status: status.as_u16(), message }
    }
  }
}

/// A failed request classified: a connect that failed or timed out is no connection, a read that timed out a timeout
pub fn request_error(e: &reqwest::Error) -> YouTrackError {
  if e.is_timeout() && !e.is_connect() {
    YouTrackError::Timeout
  } else {
    YouTrackError::Network(e.to_string())
  }
}
