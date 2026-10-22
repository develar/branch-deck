//! An in-process GitHub for the tests: answers each `METHOD /path` as the test says and records every request. With a
//! repository (`with_repository`, `fake_repository.rs`) the requests no answer is scripted for are answered from its
//! state, as GitHub would.

use crate::client::GitHubClient;
use crate::fake_repository::FakeGitHubRepository;
use axum::body::Bytes;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

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
  requests: Mutex<Vec<FakeRequest>>,
  /// Answers what no answer is scripted for
  repository: Option<Arc<FakeGitHubRepository>>,
}

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

  /// Answers `method path` with `answers` in turn, then with the last one
  pub fn answer_in_turn(&self, method: &str, path: &str, answers: Vec<FakeAnswer>) {
    self.answers.lock().unwrap().insert(format!("{method} {path}"), answers.into());
  }

  pub fn requests(&self) -> Vec<FakeRequest> {
    self.requests.lock().unwrap().clone()
  }

  /// The requests to `method path`, in order
  pub fn requests_to(&self, method: &str, path: &str) -> Vec<FakeRequest> {
    self.requests().into_iter().filter(|request| request.method == method && request.path == path).collect()
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
      body: String::from_utf8_lossy(body).into_owned(),
    };
    self.requests.lock().unwrap().push(request.clone());
    let answer = self
      .next_answer(&format!("{method} {}", uri.path()))
      .or_else(|| self.repository.as_ref().and_then(|repository| repository.answer(&request)));
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

/// A fake GitHub on a free port and a client of it (the API and the web host are the same server)
pub async fn start_fake() -> (Arc<FakeGitHub>, GitHubClient) {
  let (fake, url) = start_fake_server().await;
  (fake, GitHubClient::with_urls(url.clone(), url, "Branch Deck test"))
}

/// The fake on a free port and its URL (`http://127.0.0.1:<port>`): also the fake of other HTTP services, a YouTrack
/// and its Hub
pub async fn start_fake_server() -> (Arc<FakeGitHub>, String) {
  let fake = Arc::new(FakeGitHub::default());
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap();
  let app = router(Arc::clone(&fake));
  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  (fake, format!("http://{address}"))
}

/// The routes of `fake`, every path: for a server of the caller's (the E2E test server serves one per test repository)
pub fn router(fake: Arc<FakeGitHub>) -> axum::Router {
  axum::Router::new().fallback(move |method: Method, uri: Uri, headers: HeaderMap, body: Bytes| {
    let fake = Arc::clone(&fake);
    async move { fake.respond(&method, &uri, &headers, &body) }
  })
}

/// A fake GitHub and a service of it that asks with `token` only (no `gh`, no git credentials of the developer)
pub async fn start_fake_service(token: &str) -> (Arc<FakeGitHub>, crate::service::GitHubService) {
  let (fake, client) = start_fake().await;
  let tokens = crate::token::TokenChain::new(crate::token::TokenSources::fixed(token), git_executor::git_command_executor::GitCommandExecutor::new());
  (fake, crate::service::GitHubService::new(client, tokens))
}
