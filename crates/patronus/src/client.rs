//! The Patronus REST API (`/app/rest/v1`), as far as Branch Deck uses it. Every call is one HTTP request; a GET is
//! repeated at most once on a transient failure, a POST or PUT never.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use sync_utils::timestamp::format_utc_timestamp;
use tracing::{debug, info, instrument, warn};

pub const PATRONUS_URL: &str = "https://patronus.labs.jb.gg";

/// The connection is reused: polling is one small request on a kept-alive TLS connection
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Robot lists and statuses are database reads on the server
pub const LIST_TIMEOUT: Duration = Duration::from_secs(15);
/// A transient failure of a GET is asked again once, after this pause
const GET_RETRY_DELAY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatronusError {
  /// 401 or 403: the token is missing, expired or revoked
  Unauthorized,
  /// Another HTTP failure; `code` and `description` are the fields of the Patronus error body, when there is one
  Http {
    status: u16,
    code: Option<String>,
    description: Option<String>,
  },
  Timeout,
  /// No connection: offline, DNS, TLS
  Network(String),
  /// The answer is not what this version expects
  InvalidResponse(String),
}

impl PatronusError {
  /// Worth asking again later: the server or the network failed, not the request
  pub fn is_transient(&self) -> bool {
    match self {
      Self::Http { status, .. } => *status >= 500,
      Self::Timeout | Self::Network(_) => true,
      Self::Unauthorized | Self::InvalidResponse(_) => false,
    }
  }

  /// For the user: the description Patronus gave, else what failed
  pub fn message(&self) -> String {
    match self {
      Self::Unauthorized => "Patronus rejected the Space token".to_string(),
      Self::Http { status, description, .. } => description.clone().filter(|d| !d.is_empty()).unwrap_or_else(|| format!("Patronus answered HTTP {status}")),
      Self::Timeout => "Patronus did not answer in time".to_string(),
      Self::Network(message) => format!("Patronus is not reachable: {message}"),
      Self::InvalidResponse(message) => format!("Unexpected Patronus answer: {message}"),
    }
  }
}

/// `RobotListDto` of the list and `RobotDto` of `fetch-updates`: the fields both have
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotDto {
  pub id: String,
  pub name: String,
  pub status: String,
  pub push_mode: String,
  pub source_branch: Option<String>,
  pub source_revision: Option<String>,
  pub target_branch: String,
  pub start_date_time: String,
  pub finish_date_time: Option<String>,
  pub cancellation_reason: Option<String>,
}

#[derive(Deserialize)]
struct RobotsDto {
  robots: Vec<RobotDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotUpdateRequest {
  pub id: String,
  /// `RUNNING`, `CANCELING`, …: Patronus answers only the robots whose status differs
  pub last_known_status: String,
}

#[derive(Serialize)]
struct FetchUpdatesRequest<'a> {
  robots: &'a [RobotUpdateRequest],
}

#[derive(Deserialize)]
struct ErrorDto {
  error: Option<String>,
  description: Option<String>,
}

#[derive(Clone)]
pub struct PatronusClient {
  http: reqwest::Client,
  base_url: String,
}

impl PatronusClient {
  /// `base_url` without the trailing slash, e.g. `PATRONUS_URL`
  pub fn new(base_url: impl Into<String>, user_agent: &str) -> Self {
    let http = reqwest::Client::builder()
      .connect_timeout(CONNECT_TIMEOUT)
      .user_agent(user_agent)
      .build()
      .expect("the HTTP client of Patronus");
    Self { http, base_url: base_url.into() }
  }

  pub fn base_url(&self) -> &str {
    &self.base_url
  }

  /// The robot page in the web UI
  pub fn robot_url(&self, robot_id: &str) -> String {
    format!("{}/robot/{robot_id}", self.base_url)
  }

  /// The robots of the token's owner in the repository started since `start` (Unix seconds), newest first
  #[instrument(skip(self, token))]
  pub async fn list_robots(&self, token: &str, repository: &str, start: u32) -> Result<Vec<RobotDto>, PatronusError> {
    let start = format_utc_timestamp(start);
    let query = [("owner", "me"), ("repository", repository), ("start", start.as_str())];
    let url = reqwest::Url::parse_with_params(&format!("{}/app/rest/v1/robots", self.base_url), query).map_err(|e| PatronusError::InvalidResponse(e.to_string()))?;
    let started = Instant::now();
    let robots: RobotsDto = self.get_json(|| self.http.get(url.clone()).bearer_auth(token), LIST_TIMEOUT).await?;
    info!(count = robots.robots.len(), elapsed_ms = started.elapsed().as_millis(), "listed Patronus robots");
    Ok(robots.robots)
  }

  /// The robots whose status is not the known one; the others are not sent back
  #[instrument(skip(self, token, robots), fields(count = robots.len()))]
  pub async fn fetch_updates(&self, token: &str, robots: &[RobotUpdateRequest]) -> Result<Vec<RobotDto>, PatronusError> {
    let request = self
      .http
      .post(format!("{}/app/rest/v1/robots/fetch-updates", self.base_url))
      .json(&FetchUpdatesRequest { robots })
      .bearer_auth(token)
      .timeout(LIST_TIMEOUT);
    let started = Instant::now();
    let changed: RobotsDto = read_json(request.send().await).await?;
    info!(changed = changed.robots.len(), elapsed_ms = started.elapsed().as_millis(), "polled Patronus robots");
    Ok(changed.robots)
  }

  /// A GET, asked once more after a transient failure
  async fn get_json<T: DeserializeOwned>(&self, request: impl Fn() -> reqwest::RequestBuilder, timeout: Duration) -> Result<T, PatronusError> {
    match read_json(request().timeout(timeout).send().await).await {
      Err(e) if e.is_transient() => {
        debug!(error = ?e, "asking Patronus again");
        tokio::time::sleep(GET_RETRY_DELAY).await;
        read_json(request().timeout(timeout).send().await).await
      }
      result => result,
    }
  }
}

async fn read_json<T: DeserializeOwned>(response: reqwest::Result<reqwest::Response>) -> Result<T, PatronusError> {
  let response = response.map_err(|e| request_error(&e))?;
  let status = response.status();
  if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
    return Err(PatronusError::Unauthorized);
  }
  let body = response.bytes().await.map_err(|e| request_error(&e))?;
  if !status.is_success() {
    // a 4xx body may be empty
    let error = serde_json::from_slice::<ErrorDto>(&body).ok();
    warn!(status = status.as_u16(), body = %String::from_utf8_lossy(&body), "Patronus request failed");
    return Err(PatronusError::Http {
      status: status.as_u16(),
      code: error.as_ref().and_then(|e| e.error.clone()),
      description: error.and_then(|e| e.description),
    });
  }
  serde_json::from_slice(&body).map_err(|e| PatronusError::InvalidResponse(e.to_string()))
}

fn request_error(e: &reqwest::Error) -> PatronusError {
  if e.is_timeout() {
    PatronusError::Timeout
  } else if e.is_decode() {
    PatronusError::InvalidResponse(e.to_string())
  } else {
    PatronusError::Network(e.to_string())
  }
}
