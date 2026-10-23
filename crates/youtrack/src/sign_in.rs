//! The app's own sign-in, as the `youtrack` CLI signs in (`youtrack instance login`): OAuth 2.0 with PKCE through the
//! Hub of the YouTrack, by Hub's public client `localhost-auth`, the browser answering to a listener on
//! `127.0.0.1:<free port>`. With `access_type=offline` Hub adds a refresh token: the access token lasts an hour and is
//! renewed silently, the refresh token as long as the Hub keeps it (90 days on hub.jetbrains.com, 2026-10).

use crate::client::{ErrorBody, REQUEST_TIMEOUT, YouTrackClient, YouTrackError, classify_failure, request_error};
use crate::url::{api_url, check_url, url_of};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{debug, instrument, warn};

/// Hub's public client for an app on the user's machine: no secret, a loopback redirect
pub const CLIENT_ID: &str = "localhost-auth";
/// The browser sign-in may take a while: SSO, a second factor
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);
/// How often the wait looks for a cancel
const CANCEL_CHECK: Duration = Duration::from_millis(200);
/// The head of the browser's request, as far as it is read
const MAX_REQUEST_HEAD: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInError {
  /// The Hub does not accept the public client: paste a token instead
  ClientRejected,
  /// The user denied it on the Hub page, or Hub reported an error
  Denied(String),
  Expired,
  Canceled,
  Failed(String),
}

impl std::fmt::Display for SignInError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::ClientRejected => f.write_str("This YouTrack does not allow the browser sign-in of apps: use a token instead"),
      Self::Denied(message) => write!(f, "The sign-in was denied: {message}"),
      Self::Expired => f.write_str("The sign-in took too long: try again"),
      // the frontend ignores a canceled sign-in
      Self::Canceled => f.write_str("canceled"),
      Self::Failed(message) => f.write_str(message),
    }
  }
}

/// The Hub of a YouTrack: where its users sign in
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hub {
  /// e.g. `https://hub.example.com`, or `<YouTrack>/hub` for the Hub built into it
  pub url: String,
  /// The id of the YouTrack service in Hub: the scope of the token
  pub service_id: String,
}

#[derive(Deserialize)]
struct FrontendConfig {
  ring: Option<RingConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RingConfig {
  url: Option<String>,
  service_id: Option<String>,
}

/// The Hub of the YouTrack at `base`, from the configuration it gives every visitor
#[instrument(skip(client))]
pub async fn discover_hub(client: &YouTrackClient, base: &str) -> Result<Hub, SignInError> {
  let url = api_url(base, &["config"], "ring(url,serviceId)").map_err(SignInError::Failed)?;
  let config: FrontendConfig = client.get(&url, None).await.map_err(|e| SignInError::Failed(e.message()))?;
  let ring = config.ring.ok_or_else(|| SignInError::Failed("This YouTrack names no Hub to sign in with".to_string()))?;
  let (Some(hub), Some(service_id)) = (ring.url, ring.service_id) else {
    return Err(SignInError::Failed("This YouTrack names no Hub to sign in with".to_string()));
  };
  // a built-in Hub is a path of the YouTrack
  let hub = Url::parse(base)
    .and_then(|base| base.join(&hub))
    .map_err(|e| SignInError::Failed(format!("The Hub of this YouTrack is not a URL: {e}")))?;
  check_url(&hub).map_err(SignInError::Failed)?;
  Ok(Hub {
    url: hub.as_str().trim_end_matches('/').to_string(),
    service_id,
  })
}

/// A sign-in waiting for the browser: the listener the Hub page redirects to, and what the answer must match
pub struct PendingSignIn {
  pub hub: Hub,
  pub authorize_url: String,
  listener: TcpListener,
  redirect_uri: String,
  state: String,
  verifier: String,
}

/// The service of Hub itself: an access token needs it to call the Hub API (its permanent tokens)
pub const HUB_SERVICE_ID: &str = "0-0-0-0-0";

/// What a sign-in is for
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignInPurpose {
  /// The app's own sign-in: the YouTrack service, with a refresh token (`access_type=offline`)
  App,
  /// One access token that may also ask Hub itself (its service `0-0-0-0-0`), to create a permanent token for the
  /// `youtrack` CLI; no refresh token, nothing kept
  PermanentToken,
}

/// Prepares the app's sign-in (`SignInPurpose::App`)
pub async fn start_sign_in(client: &YouTrackClient, base: &str) -> Result<PendingSignIn, SignInError> {
  start_sign_in_for(client, base, SignInPurpose::App).await
}

/// Prepares the sign-in: the listener on a free loopback port and the Hub page to open. A Hub that does not know the
/// public client is told at once (`ClientRejected`), before the browser opens.
#[instrument(skip(client))]
pub async fn start_sign_in_for(client: &YouTrackClient, base: &str, purpose: SignInPurpose) -> Result<PendingSignIn, SignInError> {
  let hub = discover_hub(client, base).await?;
  let listener = TcpListener::bind("127.0.0.1:0")
    .await
    .map_err(|e| SignInError::Failed(format!("Cannot listen for the browser: {e}")))?;
  let port = listener.local_addr().map_err(|e| SignInError::Failed(e.to_string()))?.port();
  // a bare loopback URI on the literal address (RFC 8252 §7.3): the browser can't resolve "localhost" to [::1]
  let redirect_uri = format!("http://127.0.0.1:{port}");
  let verifier = random_token()?;
  let state = random_token()?;
  let mut authorize = url_of(&hub.url, &["api", "rest", "oauth2", "auth"]).map_err(SignInError::Failed)?;
  let scope = match purpose {
    SignInPurpose::App => hub.service_id.clone(),
    SignInPurpose::PermanentToken => format!("{} {HUB_SERVICE_ID}", hub.service_id),
  };
  {
    let mut query = authorize.query_pairs_mut();
    query
      .append_pair("response_type", "code")
      .append_pair("client_id", CLIENT_ID)
      .append_pair("redirect_uri", &redirect_uri)
      .append_pair("scope", &scope)
      .append_pair("state", &state);
    if purpose == SignInPurpose::App {
      query.append_pair("access_type", "offline");
    }
    query.append_pair("code_challenge", &challenge_of(&verifier)).append_pair("code_challenge_method", "S256");
  }
  preflight(client, &authorize).await?;
  Ok(PendingSignIn {
    hub,
    authorize_url: authorize.to_string(),
    listener,
    redirect_uri,
    state,
    verifier,
  })
}

/// Hub redirects a client or a redirect URI it does not accept to `/auth/oauth/error`, before any sign-in
async fn preflight(client: &YouTrackClient, authorize: &Url) -> Result<(), SignInError> {
  let response = client
    .http()
    .get(authorize.clone())
    .timeout(REQUEST_TIMEOUT)
    .send()
    .await
    .map_err(|e| SignInError::Failed(request_error(&e).message()))?;
  let location = response.headers().get(reqwest::header::LOCATION).and_then(|value| value.to_str().ok()).unwrap_or_default();
  if response.status().is_redirection() && location.contains("/auth/oauth/error") {
    return Err(SignInError::ClientRejected);
  }
  Ok(())
}

/// The tokens of a sign-in or a refresh
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenAnswer {
  pub access_token: String,
  /// Seconds
  pub expires_in: Option<u64>,
  pub refresh_token: Option<String>,
}

impl std::fmt::Debug for TokenAnswer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TokenAnswer")
      .field("expires_in", &self.expires_in)
      .field("refresh_token", &self.refresh_token.is_some())
      .finish_non_exhaustive()
  }
}

/// Waits for the browser to come back from the Hub page, then trades the code for the tokens. Ends with `Canceled`
/// once `is_canceled` turns true, `Expired` after five minutes.
#[instrument(skip_all)]
pub async fn finish_sign_in(client: &YouTrackClient, pending: PendingSignIn, is_canceled: impl Fn() -> bool) -> Result<TokenAnswer, SignInError> {
  let code = wait_for_code(&pending, is_canceled).await?;
  let token_url = url_of(&pending.hub.url, &["api", "rest", "oauth2", "token"]).map_err(SignInError::Failed)?;
  token_request(
    client,
    &token_url,
    &[
      ("grant_type", "authorization_code"),
      ("code", &code),
      ("redirect_uri", &pending.redirect_uri),
      ("code_verifier", &pending.verifier),
      ("client_id", CLIENT_ID),
    ],
  )
  .await
  .map_err(|e| match e {
    RefreshError::Expired(message) => SignInError::Denied(message),
    RefreshError::Failed(error) => SignInError::Failed(error.message()),
  })
}

async fn wait_for_code(pending: &PendingSignIn, is_canceled: impl Fn() -> bool) -> Result<String, SignInError> {
  let deadline = Instant::now() + SIGN_IN_TIMEOUT;
  loop {
    if is_canceled() {
      return Err(SignInError::Canceled);
    }
    if Instant::now() >= deadline {
      return Err(SignInError::Expired);
    }
    let Ok(accepted) = tokio::time::timeout(CANCEL_CHECK, pending.listener.accept()).await else {
      continue;
    };
    let (mut stream, _) = accepted.map_err(|e| SignInError::Failed(format!("Cannot accept the browser: {e}")))?;
    let head = read_head(&mut stream).await;
    let answer = parse_callback(&head, &pending.state);
    let (status, page) = match &answer {
      Some(Ok(_)) => ("200 OK", "Signed in to YouTrack. You can close this tab and return to Branch Deck."),
      Some(Err(_)) => ("400 Bad Request", "The sign-in to YouTrack failed. Return to Branch Deck to see why."),
      // a favicon, a prefetch: not the answer
      None => ("404 Not Found", "Not found"),
    };
    let body = format!("<!doctype html><meta charset=utf-8><title>Branch Deck</title><p>{page}</p>");
    let response = format!(
      "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
      body.len()
    );
    if let Err(e) = stream.write_all(response.as_bytes()).await {
      debug!(error = %e, "cannot answer the browser");
    }
    if let Some(answer) = answer {
      return answer;
    }
  }
}

async fn read_head(stream: &mut tokio::net::TcpStream) -> String {
  let mut head = Vec::new();
  let mut buffer = [0_u8; 2048];
  while head.len() < MAX_REQUEST_HEAD && !head.windows(4).any(|window| window == b"\r\n\r\n") {
    match tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buffer)).await {
      Ok(Ok(read)) if read > 0 => head.extend_from_slice(buffer.get(..read).unwrap_or_default()),
      _ => break,
    }
  }
  String::from_utf8_lossy(&head).into_owned()
}

/// The code of the browser's `GET /?code=…&state=…`; an error Hub sent; None for another request
pub fn parse_callback(head: &str, state: &str) -> Option<Result<String, SignInError>> {
  let target = head.lines().next()?.strip_prefix("GET ")?.split(' ').next()?;
  let url = Url::parse("http://127.0.0.1").ok()?.join(target).ok()?;
  if url.path() != "/" {
    return None;
  }
  let pairs: Vec<(String, String)> = url.query_pairs().map(|(key, value)| (key.into_owned(), value.into_owned())).collect();
  let get = |name: &str| pairs.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone());
  if let Some(error) = get("error") {
    return Some(Err(SignInError::Denied(get("error_description").unwrap_or(error))));
  }
  let code = get("code")?;
  if get("state").as_deref() != Some(state) {
    warn!("the browser answered with another state");
    return Some(Err(SignInError::Failed("The answer of the browser does not belong to this sign-in".to_string())));
  }
  Some(Ok(code))
}

/// What a refresh may end with
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshError {
  /// `invalid_grant`: the refresh token expired or was revoked; sign in again
  Expired(String),
  Failed(YouTrackError),
}

/// A new access token for the refresh token
#[instrument(skip_all)]
pub async fn refresh(client: &YouTrackClient, hub_url: &str, service_id: &str, refresh_token: &str) -> Result<TokenAnswer, RefreshError> {
  let token_url = url_of(hub_url, &["api", "rest", "oauth2", "token"]).map_err(|e| RefreshError::Failed(YouTrackError::InvalidResponse(e)))?;
  token_request(
    client,
    &token_url,
    &[
      ("grant_type", "refresh_token"),
      ("refresh_token", refresh_token),
      ("client_id", CLIENT_ID),
      ("scope", service_id),
    ],
  )
  .await
}

async fn token_request(client: &YouTrackClient, url: &Url, form: &[(&str, &str)]) -> Result<TokenAnswer, RefreshError> {
  let response = client
    .http()
    .post(url.clone())
    .header(reqwest::header::ACCEPT, "application/json")
    .form(form)
    .timeout(REQUEST_TIMEOUT)
    .send()
    .await
    .map_err(|e| RefreshError::Failed(request_error(&e)))?;
  let status = response.status();
  let body = response.bytes().await.map_err(|e| RefreshError::Failed(request_error(&e)))?;
  if status.is_success() {
    return serde_json::from_slice(&body).map_err(|e| RefreshError::Failed(YouTrackError::InvalidResponse(e.to_string())));
  }
  if let Ok(error) = serde_json::from_slice::<ErrorBody>(&body)
    && error.error.as_deref() == Some("invalid_grant")
  {
    return Err(RefreshError::Expired(error.error_description.unwrap_or_else(|| "invalid_grant".to_string())));
  }
  Err(RefreshError::Failed(classify_failure(status, &body)))
}

/// 32 random bytes, base64url: a PKCE verifier or a state
fn random_token() -> Result<String, SignInError> {
  let mut bytes = [0_u8; 32];
  getrandom::fill(&mut bytes).map_err(|e| SignInError::Failed(format!("No random numbers: {e}")))?;
  Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// The S256 challenge of a PKCE verifier
pub fn challenge_of(verifier: &str) -> String {
  URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
