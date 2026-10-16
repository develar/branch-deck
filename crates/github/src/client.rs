//! The GitHub API of github.com: GraphQL and REST, with the token the caller found. Every call is one HTTP request; a
//! read (a GET, a GraphQL query) is repeated at most once on a transient failure, a write never. A GET is conditional
//! when the same token asked the same URL before: a `304 Not Modified` does not count against the rate limit.

use moka::sync::Cache;
use reqwest::header::{ACCEPT, ETAG, HeaderMap, IF_NONE_MATCH};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;
use sync_utils::timestamp::unix_now;
use tracing::{debug, instrument, warn};

pub const GITHUB_API_URL: &str = "https://api.github.com";
/// The web host: the OAuth endpoints of the device flow, the pages a toast opens
pub const GITHUB_URL: &str = "https://github.com";
/// The REST API version the answers are read as
const API_VERSION: &str = "2022-11-28";

/// The connection is reused: a refresh is one small request on a kept-alive TLS connection
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// A search of 50 pull requests with their checks takes a few seconds; GitHub gives up on a query after 10 s
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// A transient failure of a read is asked again once, after this pause
const RETRY_DELAY: Duration = Duration::from_secs(1);
/// A secondary rate limit without `retry-after`: GitHub asks to wait at least a minute
const SECONDARY_RATE_LIMIT_WAIT: u32 = 60;
/// The answers kept for conditional GETs
const ETAG_CACHE_CAPACITY: u64 = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubError {
  /// 401: the token is missing, expired or revoked
  Unauthorized,
  /// The primary or a secondary rate limit: ask again after `retry_at` (Unix seconds)
  RateLimited {
    retry_at: u32,
  },
  /// The organization enforces SAML SSO and the token is not authorized for it. `url` authorizes it (GitHub keeps it
  /// valid for an hour); None when GitHub only left the organization's data out of an answer.
  SsoRequired {
    url: Option<String>,
  },
  /// 403 or 404: the token may not see or do this, e.g. a fine-grained token of another repository, the OAuth app
  /// access restrictions of the organization, or no write access
  NoAccess {
    status: u16,
    message: String,
  },
  /// Another HTTP failure, e.g. the 422 of a pull request that already exists, with GitHub's message
  Http {
    status: u16,
    message: String,
  },
  Timeout,
  /// No connection: offline, DNS, TLS
  Network(String),
  /// The answer is not what this version expects
  InvalidResponse(String),
}

impl GitHubError {
  /// Worth asking again right away: the server or the network failed, not the request
  pub fn is_transient(&self) -> bool {
    match self {
      Self::Http { status, .. } => *status >= 500,
      Self::Timeout | Self::Network(_) => true,
      Self::Unauthorized | Self::RateLimited { .. } | Self::SsoRequired { .. } | Self::NoAccess { .. } | Self::InvalidResponse(_) => false,
    }
  }

  /// For the user: GitHub's message when it gave one, else what failed
  pub fn message(&self) -> String {
    match self {
      Self::Unauthorized => "GitHub rejected the token".to_string(),
      Self::RateLimited { .. } => "GitHub refused the request for its rate limit: try again later".to_string(),
      Self::SsoRequired { .. } => "The organization requires SAML SSO: authorize the token for it".to_string(),
      Self::NoAccess { message, .. } | Self::Http { message, .. } => message.clone(),
      Self::Timeout => "GitHub did not answer in time".to_string(),
      Self::Network(message) => format!("GitHub is not reachable: {message}"),
      Self::InvalidResponse(message) => format!("Unexpected GitHub answer: {message}"),
    }
  }
}

/// An error of a GraphQL answer; `data` may still hold the rest
#[derive(Debug, Clone, Deserialize)]
pub struct GraphQlError {
  /// `NOT_FOUND`, `FORBIDDEN`, `INSUFFICIENT_SCOPES`, `RATE_LIMITED`, …
  #[serde(rename = "type")]
  pub kind: Option<String>,
  pub message: Option<String>,
  pub extensions: Option<serde_json::Value>,
}

impl GraphQlError {
  /// A `FORBIDDEN` of an organization with SAML SSO the token is not authorized for
  pub fn is_saml_failure(&self) -> bool {
    self
      .extensions
      .as_ref()
      .and_then(|extensions| extensions.get("saml_failure"))
      .and_then(serde_json::Value::as_bool)
      .unwrap_or(false)
  }
}

/// A GraphQL answer: partial data and errors are both normal
#[derive(Debug)]
pub struct GraphQlResponse<T> {
  pub data: Option<T>,
  pub errors: Vec<GraphQlError>,
  /// `X-GitHub-SSO: partial-results`: GitHub left out the data of organizations the token is not authorized for
  pub sso_partial: bool,
}

#[derive(Deserialize)]
struct GraphQlBody<T> {
  data: Option<T>,
  errors: Option<Vec<GraphQlError>>,
}

#[derive(Serialize)]
struct GraphQlRequest<'a> {
  query: &'a str,
  variables: &'a serde_json::Value,
}

/// The error body of the REST API: `{"message": …, "errors": [{"message": …} | {"code": …, "field": …}]}`
#[derive(Deserialize)]
struct ErrorBody {
  message: Option<String>,
  #[serde(default)]
  errors: Vec<serde_json::Value>,
}

#[derive(Clone)]
pub struct GitHubClient {
  http: reqwest::Client,
  api_url: String,
  web_url: String,
  /// By token and URL: the `ETag` and the body of the last answer
  etags: Cache<(u64, String), (String, Vec<u8>)>,
}

impl GitHubClient {
  pub fn new(user_agent: &str) -> Self {
    Self::with_urls(GITHUB_API_URL, GITHUB_URL, user_agent)
  }

  /// A client of another API and web host, without the trailing slash: the fake GitHub of the tests
  pub fn with_urls(api_url: impl Into<String>, web_url: impl Into<String>, user_agent: &str) -> Self {
    let http = reqwest::Client::builder()
      .connect_timeout(CONNECT_TIMEOUT)
      .user_agent(user_agent)
      .build()
      .expect("the HTTP client of GitHub");
    Self {
      http,
      api_url: api_url.into(),
      web_url: web_url.into(),
      etags: Cache::new(ETAG_CACHE_CAPACITY),
    }
  }

  pub fn web_url(&self) -> &str {
    &self.web_url
  }

  /// The client for the OAuth endpoints of the web host (`device_flow.rs`), with the same connection pool
  pub fn http(&self) -> &reqwest::Client {
    &self.http
  }

  /// A GraphQL query (never a mutation: the writes are REST), asked again once after a transient failure. A
  /// `RATE_LIMITED` error is an `Err`; any other error comes with the data, for the caller to judge.
  #[instrument(skip(self, token, query, variables))]
  pub async fn graphql<T: DeserializeOwned>(&self, token: &str, query: &str, variables: &serde_json::Value) -> Result<GraphQlResponse<T>, GitHubError> {
    let url = format!("{}/graphql", self.api_url);
    let response = read_with_retry(|| self.request(Method::POST, &url, token).json(&GraphQlRequest { query, variables }).send()).await?;
    let body: GraphQlBody<T> = serde_json::from_slice(&response.body).map_err(|e| GitHubError::InvalidResponse(e.to_string()))?;
    let errors = body.errors.unwrap_or_default();
    if errors.iter().any(|error| error.kind.as_deref() == Some("RATE_LIMITED")) {
      return Err(GitHubError::RateLimited {
        retry_at: rate_limit_reset(&response.headers).unwrap_or_else(|| unix_now() + SECONDARY_RATE_LIMIT_WAIT),
      });
    }
    Ok(GraphQlResponse {
      data: body.data,
      errors,
      sso_partial: is_sso_partial(&response.headers),
    })
  }

  /// A REST GET of `path` (segments, each escaped) with `query`, asked again once after a transient failure;
  /// conditional on the last answer to the same token
  #[instrument(skip(self, token))]
  pub async fn get<T: DeserializeOwned>(&self, token: &str, path: &[&str], query: &[(&str, &str)]) -> Result<T, GitHubError> {
    let url = self.rest_url(path, query)?;
    let key = (token_key(token), url.to_string());
    let response = read_with_retry(|| {
      let mut request = self.request(Method::GET, url.as_str(), token);
      if let Some((etag, _)) = self.etags.get(&key) {
        request = request.header(IF_NONE_MATCH, etag);
      }
      request.send()
    })
    .await?;
    let body = if response.status == StatusCode::NOT_MODIFIED {
      match self.etags.get(&key) {
        Some((_, body)) => body,
        None => return Err(GitHubError::InvalidResponse("304 without a cached answer".to_string())),
      }
    } else {
      if let Some(etag) = response.headers.get(ETAG).and_then(|value| value.to_str().ok()) {
        self.etags.insert(key, (etag.to_string(), response.body.clone()));
      }
      response.body
    };
    serde_json::from_slice(&body).map_err(|e| GitHubError::InvalidResponse(e.to_string()))
  }

  /// A REST POST of `body` to `path`; never repeated by itself
  #[instrument(skip(self, token, body))]
  pub async fn post<T: DeserializeOwned, B: Serialize + ?Sized>(&self, token: &str, path: &[&str], body: &B) -> Result<T, GitHubError> {
    self.write(Method::POST, token, path, body).await
  }

  /// A REST PATCH of `body` to `path`; never repeated by itself
  #[instrument(skip(self, token, body))]
  pub async fn patch<T: DeserializeOwned, B: Serialize + ?Sized>(&self, token: &str, path: &[&str], body: &B) -> Result<T, GitHubError> {
    self.write(Method::PATCH, token, path, body).await
  }

  async fn write<T: DeserializeOwned, B: Serialize + ?Sized>(&self, method: Method, token: &str, path: &[&str], body: &B) -> Result<T, GitHubError> {
    let url = self.rest_url(path, &[])?;
    let response = read_response(self.request(method, url.as_str(), token).json(body).send().await).await?;
    serde_json::from_slice(&response.body).map_err(|e| GitHubError::InvalidResponse(e.to_string()))
  }

  fn request(&self, method: Method, url: &str, token: &str) -> reqwest::RequestBuilder {
    self
      .http
      .request(method, url)
      .bearer_auth(token)
      .header(ACCEPT, "application/vnd.github+json")
      .header("X-GitHub-Api-Version", API_VERSION)
      .timeout(REQUEST_TIMEOUT)
  }

  /// `<api>/<segment>/<segment>…?<query>`: owner and repository names come from git config, so each segment is escaped
  fn rest_url(&self, path: &[&str], query: &[(&str, &str)]) -> Result<reqwest::Url, GitHubError> {
    let mut url = reqwest::Url::parse(&self.api_url).map_err(|e| GitHubError::InvalidResponse(e.to_string()))?;
    url
      .path_segments_mut()
      .map_err(|()| GitHubError::InvalidResponse(format!("{} is not a base URL", self.api_url)))?
      .pop_if_empty()
      .extend(path);
    if !query.is_empty() {
      url.query_pairs_mut().extend_pairs(query);
    }
    Ok(url)
  }
}

/// The token only as a cache key: the cache must not answer one user with another's data
fn token_key(token: &str) -> u64 {
  let mut hasher = DefaultHasher::new();
  token.hash(&mut hasher);
  hasher.finish()
}

struct ReadResponse {
  status: StatusCode,
  headers: HeaderMap,
  body: Vec<u8>,
}

/// The answer of a read, asked again once after a transient failure
async fn read_with_retry<R: Future<Output = reqwest::Result<reqwest::Response>>>(send: impl Fn() -> R) -> Result<ReadResponse, GitHubError> {
  match read_response(send().await).await {
    Err(e) if e.is_transient() => {
      debug!(error = ?e, "asking GitHub again");
      tokio::time::sleep(RETRY_DELAY).await;
      read_response(send().await).await
    }
    result => result,
  }
}

/// A successful or `304` answer; a failure classified
async fn read_response(response: reqwest::Result<reqwest::Response>) -> Result<ReadResponse, GitHubError> {
  let response = response.map_err(|e| request_error(&e))?;
  let status = response.status();
  let headers = response.headers().clone();
  let body = response.bytes().await.map_err(|e| request_error(&e))?.to_vec();
  if status.is_success() || status == StatusCode::NOT_MODIFIED {
    return Ok(ReadResponse { status, headers, body });
  }
  let error = classify_failure(status, &headers, &body, unix_now());
  warn!(status = status.as_u16(), error = ?error, "GitHub request failed");
  Err(error)
}

/// What a failed answer means. A rate limit is a 403 or a 429: the primary one says `x-ratelimit-remaining: 0`, a
/// secondary one has `retry-after` or only says so in its message.
pub fn classify_failure(status: StatusCode, headers: &HeaderMap, body: &[u8], now: u32) -> GitHubError {
  let message = error_message(status, body);
  if status == StatusCode::UNAUTHORIZED {
    return GitHubError::Unauthorized;
  }
  if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
    if header(headers, "x-ratelimit-remaining") == Some("0") {
      return GitHubError::RateLimited {
        retry_at: rate_limit_reset(headers).unwrap_or(now + SECONDARY_RATE_LIMIT_WAIT),
      };
    }
    if let Some(seconds) = header(headers, "retry-after").and_then(|value| value.parse::<u32>().ok()) {
      return GitHubError::RateLimited { retry_at: now + seconds };
    }
    if status == StatusCode::TOO_MANY_REQUESTS || message.to_ascii_lowercase().contains("rate limit") {
      return GitHubError::RateLimited {
        retry_at: now + SECONDARY_RATE_LIMIT_WAIT,
      };
    }
  }
  if status == StatusCode::FORBIDDEN
    && let Some(sso) = header(headers, "x-github-sso")
    && sso.starts_with("required")
  {
    return GitHubError::SsoRequired { url: sso_url(sso) };
  }
  if status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND {
    return GitHubError::NoAccess { status: status.as_u16(), message };
  }
  GitHubError::Http { status: status.as_u16(), message }
}

/// GitHub's message with the messages of its `errors`, e.g. "Validation Failed: A pull request already exists for
/// o:branch."
fn error_message(status: StatusCode, body: &[u8]) -> String {
  let Ok(error) = serde_json::from_slice::<ErrorBody>(body) else {
    return format!("GitHub answered HTTP {}", status.as_u16());
  };
  let details: Vec<String> = error
    .errors
    .iter()
    .filter_map(|detail| match detail {
      serde_json::Value::String(text) => Some(text.clone()),
      detail => detail.get("message").and_then(serde_json::Value::as_str).map(str::to_string),
    })
    .collect();
  let message = error
    .message
    .filter(|message| !message.is_empty())
    .unwrap_or_else(|| format!("GitHub answered HTTP {}", status.as_u16()));
  if details.is_empty() { message } else { format!("{message}: {}", details.join("; ")) }
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
  headers.get(name).and_then(|value| value.to_str().ok()).map(str::trim)
}

fn rate_limit_reset(headers: &HeaderMap) -> Option<u32> {
  header(headers, "x-ratelimit-reset").and_then(|value| value.parse().ok())
}

/// `X-GitHub-SSO: required; url=https://github.com/orgs/o/sso?authorization_request=…`
fn sso_url(value: &str) -> Option<String> {
  value.split(';').map(str::trim).find_map(|part| part.strip_prefix("url=")).map(str::to_string)
}

fn is_sso_partial(headers: &HeaderMap) -> bool {
  header(headers, "x-github-sso").is_some_and(|value| value.starts_with("partial-results"))
}

fn request_error(e: &reqwest::Error) -> GitHubError {
  if e.is_timeout() {
    GitHubError::Timeout
  } else if e.is_decode() {
    GitHubError::InvalidResponse(e.to_string())
  } else {
    GitHubError::Network(e.to_string())
  }
}
