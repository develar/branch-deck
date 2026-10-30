//! An in-process GitHub for the tests: answers each `METHOD /path` as the test says and records every request. With a
//! repository (`with_repository`, `fake_repository.rs`) the requests no answer is scripted for are answered from its
//! state, as GitHub would. The OAuth device flow (`POST /login/device/code`, `POST /login/oauth/access_token`) is
//! answered as GitHub does too: a code to poll every second, `authorization_pending` until a test authorizes the codes
//! issued so far (`authorize_device_codes`, the user entering them on GitHub), then the token the test gave.

use crate::client::GitHubClient;
use crate::fake_repository::FakeGitHubRepository;
use axum::body::Bytes;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use test_utils::fake_server::{FakeGate, route_matches};

/// One answer: a status, a JSON body and extra headers
#[derive(Clone, Debug)]
pub struct FakeAnswer {
  pub status: u16,
  pub body: serde_json::Value,
  pub headers: Vec<(String, String)>,
}

impl FakeAnswer {
  pub const fn ok(body: serde_json::Value) -> Self {
    Self::status(200, body)
  }

  pub const fn status(status: u16, body: serde_json::Value) -> Self {
    Self {
      status,
      body,
      headers: Vec::new(),
    }
  }

  #[must_use]
  pub fn header(mut self, name: &str, value: &str) -> Self {
    self.headers.push((name.to_string(), value.to_string()));
    self
  }
}

/// A request as the fake got it
#[derive(Clone, Debug)]
pub struct FakeRequest {
  pub method: String,
  pub path: String,
  pub query: Option<String>,
  pub authorization: Option<String>,
  pub if_none_match: Option<String>,
  /// `X-GitHub-Api-Version`
  pub api_version: Option<String>,
  /// The body as text: JSON for the API, form fields for the OAuth endpoints
  pub body: String,
}

impl FakeRequest {
  pub fn json(&self) -> serde_json::Value {
    serde_json::from_str(&self.body).unwrap_or(serde_json::Value::Null)
  }
}

#[derive(Default)]
pub struct FakeGitHub {
  /// By `METHOD /path`: the answers still to give; the last one is given again and again
  answers: Mutex<HashMap<String, VecDeque<FakeAnswer>>>,
  /// The answers to a `METHOD /path` whose body has a text (one GraphQL query or mutation: `POST /graphql` serves them
  /// all), before those by `METHOD /path` alone
  body_answers: Mutex<Vec<(String, String, FakeAnswer)>>,
  requests: Mutex<Vec<FakeRequest>>,
  /// Answers what no answer is scripted for
  repository: Option<Arc<FakeGitHubRepository>>,
  device_flow: Mutex<DeviceFlow>,
  /// The gates that hold the requests to a `METHOD /path` (`hold`), of a body with a text only (`hold_with_body`)
  holds: Mutex<Vec<FakeHold>>,
}

/// The requests to one `METHOD /path` a gate holds: every one, or those whose body has `body` (a GraphQL query of one
/// search: `POST /graphql` serves them all)
struct FakeHold {
  key: String,
  body: Option<String>,
  gate: Arc<FakeGate>,
}

/// The device codes the fake issued, and what the user did with each
#[derive(Default)]
struct DeviceFlow {
  codes: Vec<(String, DeviceCodeState)>,
  /// The seconds between two polls the codes ask for (`set_device_interval`); 1 unless set
  interval: Option<u64>,
}

#[derive(Default, Clone)]
enum DeviceCodeState {
  /// The user has not entered it
  #[default]
  Pending,
  /// The user entered it: the token the poll gets
  Granted(String),
  /// The user denied the app on GitHub
  Denied,
}

/// The code the user enters of every device code of the fake
pub const FAKE_USER_CODE: &str = "WDJB-MJHT";
/// Where GitHub asks for the code
pub const DEVICE_VERIFICATION_URI: &str = "https://github.com/login/device";

impl FakeGitHub {
  /// A fake that answers from `repository` what no answer is scripted for
  pub fn with_repository(repository: Arc<FakeGitHubRepository>) -> Self {
    Self {
      repository: Some(repository),
      ..Self::default()
    }
  }

  /// The repository behind the fake, if any
  pub fn repository(&self) -> Option<&FakeGitHubRepository> {
    self.repository.as_deref()
  }

  /// Answers `method path` (without the query) with `answer` from now on
  pub fn answer(&self, method: &str, path: &str, answer: FakeAnswer) {
    self.answer_in_turn(method, path, vec![answer]);
  }

  /// Answers `method path` whose body has `body` (a GraphQL query or mutation) with `answer` from now on, before an
  /// answer of `answer`
  pub fn answer_with_body(&self, method: &str, path: &str, body: &str, answer: FakeAnswer) {
    let key = format!("{method} {path}");
    let mut answers = self.body_answers.lock().unwrap();
    answers.retain(|(other, marker, _)| *other != key || marker != body);
    answers.push((key, body.to_string(), answer));
  }

  /// Answers `method path` with `answers` in turn, then with the last one
  pub fn answer_in_turn(&self, method: &str, path: &str, answers: Vec<FakeAnswer>) {
    self.answers.lock().unwrap().insert(format!("{method} {path}"), answers.into());
  }

  /// Holds the requests to `method path` (without the query; a `*` segment matches any one) until the gate it returns
  /// opens
  pub fn hold(&self, method: &str, path: &str) -> Arc<FakeGate> {
    self.hold_matching(method, path, None)
  }

  /// Holds the requests to `method path` whose body has `body` (the search of one GraphQL query) until the gate it
  /// returns opens
  pub fn hold_with_body(&self, method: &str, path: &str, body: &str) -> Arc<FakeGate> {
    self.hold_matching(method, path, Some(body.to_string()))
  }

  fn hold_matching(&self, method: &str, path: &str, body: Option<String>) -> Arc<FakeGate> {
    let gate = Arc::new(FakeGate::default());
    let key = format!("{method} {path}");
    let mut holds = self.holds.lock().unwrap();
    holds.retain(|hold| hold.key != key || hold.body != body);
    holds.push(FakeHold {
      key,
      body,
      gate: Arc::clone(&gate),
    });
    gate
  }

  fn gate(&self, method: &Method, uri: &Uri, body: &[u8]) -> Option<Arc<FakeGate>> {
    let key = format!("{method} {}", uri.path());
    let text = String::from_utf8_lossy(body);
    let holds = self.holds.lock().unwrap();
    holds
      .iter()
      .find(|hold| route_matches(&hold.key, &key) && hold.body.as_ref().is_none_or(|marker| text.contains(marker.as_str())))
      .map(|hold| Arc::clone(&hold.gate))
  }

  pub fn requests(&self) -> Vec<FakeRequest> {
    self.requests.lock().unwrap().clone()
  }

  /// The requests to `method path`, in order
  pub fn requests_to(&self, method: &str, path: &str) -> Vec<FakeRequest> {
    self.requests().into_iter().filter(|request| request.method == method && request.path == path).collect()
  }

  /// The user entered every device code issued so far on GitHub: the next poll of each gets `token`
  pub fn authorize_device_codes(&self, token: &str) {
    for (_, state) in &mut self.device_flow.lock().unwrap().codes {
      if matches!(state, DeviceCodeState::Pending) {
        *state = DeviceCodeState::Granted(token.to_string());
      }
    }
  }

  /// The seconds between two polls that the device codes issued from now on ask for: 1 unless set (GitHub's are 5). 0
  /// for a test that steps the polls with holds (`hold`), so the wait between them takes no time
  pub fn set_device_interval(&self, seconds: u64) {
    self.device_flow.lock().unwrap().interval = Some(seconds);
  }

  /// The user denied the app for every device code issued so far: the next poll of each gets `access_denied`
  pub fn deny_device_codes(&self) {
    for (_, state) in &mut self.device_flow.lock().unwrap().codes {
      if matches!(state, DeviceCodeState::Pending) {
        *state = DeviceCodeState::Denied;
      }
    }
  }

  /// The OAuth device flow: a code of `client_id`, polled for with `grant_type` device code; None for another path
  fn device_flow_answer(&self, request: &FakeRequest) -> Option<FakeAnswer> {
    let field = |name: &str| form_field(&request.body, name);
    let error = |error: &str| Some(FakeAnswer::ok(serde_json::json!({ "error": error })));
    match (request.method.as_str(), request.path.as_str()) {
      ("POST", "/login/device/code") => {
        if field("client_id").is_none_or(str::is_empty) {
          return error("incorrect_client_credentials");
        }
        let mut flow = self.device_flow.lock().unwrap();
        let device_code = format!("device-{}", flow.codes.len() + 1);
        flow.codes.push((device_code.clone(), DeviceCodeState::Pending));
        Some(FakeAnswer::ok(serde_json::json!({
          "device_code": device_code,
          "user_code": FAKE_USER_CODE,
          "verification_uri": DEVICE_VERIFICATION_URI,
          "expires_in": 900,
          "interval": flow.interval.unwrap_or(1),
        })))
      }
      ("POST", "/login/oauth/access_token") => {
        if field("client_id").is_none_or(str::is_empty) {
          return error("incorrect_client_credentials");
        }
        if field("grant_type") != Some("urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code") {
          return error("unsupported_grant_type");
        }
        let flow = self.device_flow.lock().unwrap();
        match flow.codes.iter().find(|(code, _)| Some(code.as_str()) == field("device_code")) {
          None => error("incorrect_device_code"),
          Some((_, DeviceCodeState::Pending)) => error("authorization_pending"),
          Some((_, DeviceCodeState::Denied)) => error("access_denied"),
          Some((_, DeviceCodeState::Granted(token))) => Some(FakeAnswer::ok(serde_json::json!({"access_token": token, "token_type": "bearer", "scope": "read:org,repo"}))),
        }
      }
      _ => None,
    }
  }

  fn next_answer(&self, key: &str) -> Option<FakeAnswer> {
    let mut answers = self.answers.lock().unwrap();
    let queue = answers.get_mut(key)?;
    if queue.len() > 1 { queue.pop_front() } else { queue.front().cloned() }
  }

  fn respond(&self, method: &Method, uri: &Uri, headers: &HeaderMap, body: &Bytes) -> Response {
    let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok()).map(str::to_string);
    let request = FakeRequest {
      method: method.to_string(),
      path: uri.path().to_string(),
      query: uri.query().map(str::to_string),
      authorization: header("authorization"),
      if_none_match: header("if-none-match"),
      api_version: header("x-github-api-version"),
      body: String::from_utf8_lossy(body).into_owned(),
    };
    self.requests.lock().unwrap().push(request.clone());
    let key = format!("{method} {}", uri.path());
    let by_body = self
      .body_answers
      .lock()
      .unwrap()
      .iter()
      .find(|(other, marker, _)| *other == key && request.body.contains(marker.as_str()))
      .map(|(_, _, answer)| answer.clone());
    let answer = by_body
      .or_else(|| self.next_answer(&key))
      .or_else(|| self.repository.as_ref().and_then(|repository| repository.answer(&request)))
      .or_else(|| self.device_flow_answer(&request));
    let Some(answer) = answer else {
      return (StatusCode::NOT_FOUND, axum::Json(serde_json::json!({"message": "Not Found"}))).into_response();
    };
    let status = StatusCode::from_u16(answer.status).unwrap();
    let mut response = if answer.body.is_null() {
      status.into_response()
    } else {
      (status, axum::Json(answer.body)).into_response()
    };
    for (name, value) in answer.headers {
      response
        .headers_mut()
        .insert(axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(), value.parse().unwrap());
    }
    response
  }
}

/// The value of the form field `name` of `body`, as sent (percent-encoded)
fn form_field<'a>(body: &'a str, name: &str) -> Option<&'a str> {
  body
    .split('&')
    .find_map(|pair| pair.split_once('=').filter(|(key, _)| *key == name).map(|(_, value)| value))
}

/// A fake GitHub on a free port and a client of it (the API and the web host are the same server)
pub async fn start_fake() -> (Arc<FakeGitHub>, GitHubClient) {
  let (fake, url) = start_fake_server().await;
  (fake, GitHubClient::with_urls(url.clone(), url, "Branch Deck test"))
}

/// The fake on a free port and its URL (`http://127.0.0.1:<port>`): also the fake of other HTTP services, a YouTrack
/// and its Hub
pub async fn start_fake_server() -> (Arc<FakeGitHub>, String) {
  let fake = Arc::new(FakeGitHub::default());
  let url = serve(Arc::clone(&fake)).await;
  (fake, url)
}

/// Serves `fake` on a free port for the rest of the runtime; its URL (`http://127.0.0.1:<port>`)
pub async fn serve(fake: Arc<FakeGitHub>) -> String {
  test_utils::fake_server::FakeServer::start(router(fake)).await.detach()
}

/// The routes of `fake`, every path: for a server of the caller's (the E2E test server serves one per test repository)
pub fn router(fake: Arc<FakeGitHub>) -> axum::Router {
  axum::Router::new().fallback(move |method: Method, uri: Uri, headers: HeaderMap, body: Bytes| {
    let fake = Arc::clone(&fake);
    // on the blocking pool: the branch tips of a repository (`BranchTip`) may run git
    async move {
      if let Some(gate) = fake.gate(&method, &uri, &body) {
        gate.pass().await;
      }
      tokio::task::spawn_blocking(move || fake.respond(&method, &uri, &headers, &body))
        .await
        .expect("the fake's answer")
    }
  })
}

/// A fake GitHub and a service of it that asks with `token` only (no `gh`, no git credentials of the developer)
pub async fn start_fake_service(token: &str) -> (Arc<FakeGitHub>, crate::service::GitHubService) {
  let (fake, client) = start_fake().await;
  let tokens = crate::token::TokenChain::new(crate::token::TokenSources::fixed(token), git_executor::git_command_executor::GitCommandExecutor::new());
  (fake, crate::service::GitHubService::new(client, tokens))
}

/// The poll of a device code for its token (`POST`), which a test of a sign-in holds to step it
pub const DEVICE_TOKEN_POLL: &str = "/login/oauth/access_token";

/// A fake GitHub whose repository knows `viewer` of `token` and asks for no pause between two polls of a device code
/// (a test steps them with holds), and a service on it that asks only the token file in `dir`, where the app's own
/// sign-in keeps its token: the tests of the sign-in here and in `remote_facts::sign_in`
pub async fn start_sign_in_fake(token: &str, viewer: &str, dir: &std::path::Path) -> (Arc<FakeGitHub>, crate::service::GitHubService) {
  let repository = Arc::new(FakeGitHubRepository::default());
  repository.set(crate::fake_repository::RepositoryState {
    token: token.to_owned(),
    viewer: viewer.to_owned(),
    ..crate::fake_repository::RepositoryState::default()
  });
  let fake = Arc::new(FakeGitHub::with_repository(repository));
  fake.set_device_interval(0);
  let url = serve(Arc::clone(&fake)).await;
  let sources = crate::token::TokenSources {
    token_file: Some(crate::token_store::token_file(dir)),
    git_credential: false,
    ..crate::token::TokenSources::default()
  };
  let tokens = crate::token::TokenChain::new(sources, git_executor::git_command_executor::GitCommandExecutor::new());
  (fake, crate::service::GitHubService::new(GitHubClient::with_urls(&url, &url, "Branch Deck test"), tokens))
}

/// `steps` within 10 s: a sign-in that never got to the poll its test holds fails instead of hanging
pub async fn within_sign_in<T>(steps: impl Future<Output = T>) -> T {
  tokio::time::timeout(std::time::Duration::from_secs(10), steps)
    .await
    .expect("the sign-in never got to its poll")
}
