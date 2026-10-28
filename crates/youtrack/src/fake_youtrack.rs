//! An in-process YouTrack with its built-in Hub, for the tests (the E2E test server serves one per test repository),
//! answering what the app asks as YouTrack does: the configuration that names the Hub; the Hub page of the public
//! client `localhost-auth`, which redirects back to the app's listener with a code at once (a user who approved
//! before); the code traded for an access and a refresh token with the PKCE verifier of its challenge, and the refresh
//! grant; the user of a token; issues with a state field, its values, and the command that sets it. A token is
//! accepted only when the fake issued it or a test added it (`accept_token`, a token pasted from Account Security).
//! Every request is logged; a route a test holds (`hold`) answers once its gate opens.

use crate::sign_in::{CLIENT_ID, HUB_SERVICE_ID, TokenAnswer, challenge_of};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use domain_types::youtrack::YouTrackUser;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use sync_utils::timestamp::unix_now;
use test_utils::fake_server::{FakeGate, route_matches};

/// The id of the YouTrack service in its Hub: the scope of a token
pub const FAKE_SERVICE_ID: &str = "fake-youtrack-service";
/// The state field of every issue, and its id
pub const STATE_FIELD: &str = "State";
const STATE_FIELD_ID: &str = "92-1";
/// The values of the state field in every project, each with whether it resolves the issue
pub const STATES: [(&str, bool); 5] = [("Open", false), ("In Progress", false), ("Fixed", true), ("Duplicate", true), ("Verified", true)];

/// An issue of the fake: its state is a value of [`STATES`]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FakeIssue {
  pub key: String,
  pub summary: String,
  pub state: String,
  /// Unix milliseconds; a command that sets a resolved state sets it, one that sets an open state clears it
  #[serde(default)]
  pub resolved: Option<u64>,
  /// The single-value fields after the state, in order
  #[serde(default)]
  pub fields: Vec<FakeField>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FakeField {
  pub name: String,
  pub value: String,
}

/// A request as the fake got it
#[derive(Clone, Debug)]
pub struct FakeRequest {
  pub method: String,
  pub path: String,
  pub query: Option<String>,
  /// The bearer token
  pub token: Option<String>,
  /// The body as text: JSON for the API, form fields for the Hub
  pub body: String,
}

/// A command YouTrack applied (`POST /api/commands`): its query and the keys of its issues
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FakeCommand {
  pub query: String,
  pub issues: Vec<String>,
}

/// A code of the Hub page: what its exchange must match, and what its tokens may do
struct Code {
  value: String,
  challenge: String,
  redirect_uri: String,
  used: bool,
  /// `access_type=offline`: a refresh token comes with it
  offline: bool,
  /// The scope named Hub itself: its access token may make permanent tokens
  hub: bool,
}

/// A permanent token the Hub made (`POST /hub/api/rest/users/me/permanenttokens`)
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FakePermanentToken {
  pub id: String,
  pub name: String,
  pub token: String,
  /// The ids of the services it is for
  pub scope: Vec<String>,
  pub revoked: bool,
}

struct Data {
  user: YouTrackUser,
  issues: Vec<FakeIssue>,
  access_tokens: Vec<String>,
  refresh_tokens: Vec<String>,
  /// The access tokens that may ask Hub itself
  hub_tokens: Vec<String>,
  permanent_tokens: Vec<FakePermanentToken>,
  codes: Vec<Code>,
  /// Codes and tokens issued, for their names
  issued: u32,
  commands: Vec<FakeCommand>,
  /// Unix seconds of the resolved time a command sets; None: now
  clock: Option<u32>,
}

impl Default for Data {
  fn default() -> Self {
    Self {
      user: YouTrackUser {
        login: "jane".to_string(),
        full_name: "Jane Doe".to_string(),
      },
      issues: Vec::new(),
      access_tokens: Vec::new(),
      refresh_tokens: Vec::new(),
      hub_tokens: Vec::new(),
      permanent_tokens: Vec::new(),
      codes: Vec::new(),
      issued: 0,
      commands: Vec::new(),
      clock: None,
    }
  }
}

impl Data {
  fn next_name(&mut self, kind: &str) -> String {
    self.issued += 1;
    format!("{kind}-{}", self.issued)
  }

  fn issue_tokens(&mut self) -> TokenAnswer {
    let (access, refresh) = (self.next_name("access"), self.next_name("refresh"));
    self.access_tokens.push(access.clone());
    self.refresh_tokens.push(refresh.clone());
    TokenAnswer {
      access_token: access,
      expires_in: Some(3600),
      refresh_token: Some(refresh),
    }
  }
}

#[derive(Default)]
pub struct FakeYouTrack {
  data: Mutex<Data>,
  requests: Mutex<Vec<FakeRequest>>,
  /// The routes a test holds (`method path`, a `*` segment for any one) and their gates
  holds: Mutex<Vec<(String, Arc<FakeGate>)>>,
}

impl FakeYouTrack {
  /// Holds the requests to `method path` (without the query; a `*` segment matches any one) until the gate it returns
  /// opens: the page shows what it shows before the answer
  pub fn hold(&self, method: &str, path: &str) -> Arc<FakeGate> {
    let (key, gate) = (format!("{method} {path}"), Arc::new(FakeGate::default()));
    let mut holds = self.holds.lock().unwrap();
    holds.retain(|(held, _)| *held != key);
    holds.push((key, Arc::clone(&gate)));
    gate
  }

  /// The gate of a request to `method uri`, when a test holds its route
  fn gate(&self, method: &Method, uri: &Uri) -> Option<Arc<FakeGate>> {
    let route = format!("{method} {}", uri.path());
    let holds = self.holds.lock().unwrap();
    holds.iter().find(|(key, _)| route_matches(key, &route)).map(|(_, gate)| Arc::clone(gate))
  }

  /// The user of every token from now on
  pub fn set_user(&self, user: YouTrackUser) {
    self.data.lock().unwrap().user = user;
  }

  pub fn user(&self) -> YouTrackUser {
    self.data.lock().unwrap().user.clone()
  }

  /// The issues from now on
  pub fn set_issues(&self, issues: Vec<FakeIssue>) {
    self.data.lock().unwrap().issues = issues;
  }

  pub fn issues(&self) -> Vec<FakeIssue> {
    self.data.lock().unwrap().issues.clone()
  }

  /// A token the user created in Account Security: accepted from now on
  pub fn accept_token(&self, token: &str) {
    self.data.lock().unwrap().access_tokens.push(token.to_string());
  }

  /// The tokens of a sign-in done before: an access and a refresh token the fake accepts
  pub fn issue_tokens(&self) -> TokenAnswer {
    self.data.lock().unwrap().issue_tokens()
  }

  /// The permanent tokens the Hub made, in order
  pub fn permanent_tokens(&self) -> Vec<FakePermanentToken> {
    self.data.lock().unwrap().permanent_tokens.clone()
  }

  /// The resolved time a command sets is `seconds` (Unix) from now on
  pub fn set_clock(&self, seconds: u32) {
    self.data.lock().unwrap().clock = Some(seconds);
  }

  /// The commands applied, in order
  pub fn commands(&self) -> Vec<FakeCommand> {
    self.data.lock().unwrap().commands.clone()
  }

  pub fn requests(&self) -> Vec<FakeRequest> {
    self.requests.lock().unwrap().clone()
  }

  /// The requests to `method path` (without the query), in order
  pub fn requests_to(&self, method: &str, path: &str) -> Vec<FakeRequest> {
    self.requests().into_iter().filter(|request| request.method == method && request.path == path).collect()
  }

  fn respond(&self, method: &Method, uri: &Uri, headers: &HeaderMap, body: &Bytes) -> Response {
    let authorization = headers.get("authorization").and_then(|value| value.to_str().ok());
    let request = FakeRequest {
      method: method.to_string(),
      path: uri.path().to_string(),
      query: uri.query().map(str::to_string),
      token: authorization.and_then(|value| value.strip_prefix("Bearer ")).map(str::to_string),
      body: String::from_utf8_lossy(body).into_owned(),
    };
    self.requests.lock().unwrap().push(request.clone());
    let segments: Vec<&str> = request.path.trim_start_matches('/').split('/').collect();
    match (request.method.as_str(), segments.as_slice()) {
      ("GET", ["api", "config"]) => json_answer(StatusCode::OK, &json!({"ring": {"url": "/hub", "serviceId": FAKE_SERVICE_ID}})),
      ("GET", ["hub", "api", "rest", "oauth2", "auth"]) => self.hub_page(&request),
      ("POST", ["hub", "api", "rest", "oauth2", "token"]) => self.token(&request),
      (_, ["hub", "api", "rest", "users", "me", "permanenttokens", rest @ ..]) => self.permanent_token(&request, rest),
      (_, ["api", ..]) => {
        let state = self.data.lock().unwrap();
        if !request.token.as_ref().is_some_and(|token| state.access_tokens.contains(token)) {
          return error_answer(StatusCode::UNAUTHORIZED, "Unauthorized", "You are not logged in.");
        }
        drop(state);
        self.api(&request, segments.get(1..).unwrap_or_default())
      }
      _ => error_answer(StatusCode::NOT_FOUND, "Not Found", "Not found"),
    }
  }

  /// The Hub page: a client or redirect it does not accept goes to its error page, else back to the app with a code
  fn hub_page(&self, request: &FakeRequest) -> Response {
    let get = |name: &str| field_of(request.query.as_deref().unwrap_or_default(), name);
    let redirect_uri = get("redirect_uri").unwrap_or_default();
    let scope = get("scope").unwrap_or_default();
    let hub = scope == format!("{FAKE_SERVICE_ID} {HUB_SERVICE_ID}");
    let accepted = get("client_id").as_deref() == Some(CLIENT_ID)
      && get("response_type").as_deref() == Some("code")
      && (scope == FAKE_SERVICE_ID || hub)
      && get("code_challenge_method").as_deref() == Some("S256")
      && redirect_uri.starts_with("http://127.0.0.1:");
    let (Some(challenge), Some(state_of_app), true) = (get("code_challenge"), get("state"), accepted) else {
      return redirect("/hub/auth/oauth/error?error=invalid_client");
    };
    let mut state = self.data.lock().unwrap();
    let code = state.next_name("code");
    state.codes.push(Code {
      value: code.clone(),
      challenge,
      redirect_uri: redirect_uri.clone(),
      used: false,
      offline: get("access_type").as_deref() == Some("offline"),
      hub,
    });
    let mut back = url::Url::parse(&redirect_uri).expect("a checked redirect URI");
    back.query_pairs_mut().append_pair("code", &code).append_pair("state", &state_of_app);
    redirect(back.as_str())
  }

  /// The token endpoint of the Hub: a code of its page once, with the verifier of its challenge; a refresh token it issued
  fn token(&self, request: &FakeRequest) -> Response {
    let get = |name: &str| field_of(&request.body, name);
    if get("client_id").as_deref() != Some(CLIENT_ID) {
      return error_answer(StatusCode::UNAUTHORIZED, "invalid_client", "Unknown client");
    }
    let mut state = self.data.lock().unwrap();
    match get("grant_type").as_deref() {
      Some("authorization_code") => {
        let verifier = get("code_verifier").unwrap_or_default();
        let (code, redirect_uri) = (get("code"), get("redirect_uri"));
        let Some(issued) = state
          .codes
          .iter_mut()
          .find(|it| !it.used && Some(&it.value) == code.as_ref() && Some(&it.redirect_uri) == redirect_uri.as_ref() && it.challenge == challenge_of(&verifier))
        else {
          return error_answer(StatusCode::BAD_REQUEST, "invalid_grant", "The code is not valid");
        };
        issued.used = true;
        let (offline, hub) = (issued.offline, issued.hub);
        let mut tokens = state.issue_tokens();
        if !offline {
          tokens.refresh_token = None;
        }
        if hub {
          state.hub_tokens.push(tokens.access_token.clone());
        }
        json_answer(StatusCode::OK, &serde_json::to_value(tokens).unwrap())
      }
      Some("refresh_token") if get("refresh_token").is_some_and(|token| state.refresh_tokens.contains(&token)) && get("scope").as_deref() == Some(FAKE_SERVICE_ID) => {
        let access = state.next_name("access");
        state.access_tokens.push(access.clone());
        json_answer(StatusCode::OK, &json!({"access_token": access, "expires_in": 3600}))
      }
      Some("refresh_token") => error_answer(StatusCode::BAD_REQUEST, "invalid_grant", "The refresh token is not valid"),
      _ => error_answer(StatusCode::BAD_REQUEST, "unsupported_grant_type", "Unsupported grant type"),
    }
  }
}

impl FakeYouTrack {
  /// The permanent tokens of the user, for an access token that may ask Hub: a POST makes one (YouTrack accepts it from
  /// then on), a DELETE of its id revokes it
  fn permanent_token(&self, request: &FakeRequest, rest: &[&str]) -> Response {
    let mut state = self.data.lock().unwrap();
    if !request.token.as_ref().is_some_and(|token| state.hub_tokens.contains(token)) {
      return error_answer(StatusCode::FORBIDDEN, "Forbidden", "The token may not ask Hub");
    }
    match (request.method.as_str(), rest) {
      ("POST", []) => {
        let body: Value = serde_json::from_str(&request.body).unwrap_or(Value::Null);
        let id = state.next_name("permanent");
        let token = format!("perm-{id}");
        state.access_tokens.push(token.clone());
        let scope = body["scope"]
          .as_array()
          .into_iter()
          .flatten()
          .filter_map(|it| it["id"].as_str().map(str::to_string))
          .collect();
        state.permanent_tokens.push(FakePermanentToken {
          id: id.clone(),
          name: body["name"].as_str().unwrap_or_default().to_string(),
          token: token.clone(),
          scope,
          revoked: false,
        });
        json_answer(StatusCode::OK, &json!({"id": id, "token": token}))
      }
      ("DELETE", [id]) => {
        let Some(found) = state.permanent_tokens.iter_mut().find(|it| it.id == *id && !it.revoked) else {
          return error_answer(StatusCode::NOT_FOUND, "Not Found", "No such token");
        };
        found.revoked = true;
        let token = found.token.clone();
        state.access_tokens.retain(|it| *it != token);
        StatusCode::OK.into_response()
      }
      _ => error_answer(StatusCode::NOT_FOUND, "Not Found", "Not found"),
    }
  }

  /// `/api/<segments…>` with an accepted token
  fn api(&self, request: &FakeRequest, segments: &[&str]) -> Response {
    let mut state = self.data.lock().unwrap();
    match (request.method.as_str(), segments) {
      ("GET", ["users", "me"]) => json_answer(StatusCode::OK, &json!({"login": state.user.login, "fullName": state.user.full_name})),
      ("GET", ["issues", key]) => match state.issues.iter().find(|issue| issue.key == *key) {
        Some(issue) => json_answer(StatusCode::OK, &issue_json(issue)),
        None => error_answer(StatusCode::NOT_FOUND, "Not Found", "Entity with id not found"),
      },
      ("GET", ["issues", key, "customFields", STATE_FIELD_ID]) if state.issues.iter().any(|issue| issue.key == *key) => {
        let values: Vec<Value> = STATES.iter().map(|(name, _)| json!({ "name": name })).collect();
        json_answer(StatusCode::OK, &json!({"projectCustomField": {"bundle": {"values": values}}}))
      }
      ("POST", ["commands"]) => {
        let body: Value = serde_json::from_str(&request.body).unwrap_or(Value::Null);
        let query = body["query"].as_str().unwrap_or_default().to_string();
        let keys: Vec<String> = body["issues"]
          .as_array()
          .into_iter()
          .flatten()
          .filter_map(|it| it["idReadable"].as_str().map(str::to_string))
          .collect();
        let resolved_at = u64::from(state.clock.unwrap_or_else(unix_now)) * 1000;
        let value = query
          .strip_prefix(&format!("{STATE_FIELD} "))
          .and_then(|value| STATES.iter().find(|(name, _)| *name == value));
        let Some(&(value, resolves)) = value else {
          return error_answer(StatusCode::BAD_REQUEST, "bad_request", &format!("Unknown command: {query}"));
        };
        if keys.is_empty() || !keys.iter().all(|key| state.issues.iter().any(|issue| issue.key == *key)) {
          return error_answer(StatusCode::NOT_FOUND, "Not Found", "Entity with id not found");
        }
        for issue in state.issues.iter_mut().filter(|issue| keys.contains(&issue.key)) {
          value.clone_into(&mut issue.state);
          issue.resolved = resolves.then_some(resolved_at);
        }
        state.commands.push(FakeCommand { query, issues: keys });
        json_answer(StatusCode::OK, &json!({"$type": "CommandList"}))
      }
      _ => error_answer(StatusCode::NOT_FOUND, "Not Found", "Not found"),
    }
  }
}

/// The issue as YouTrack answers `fields=idReadable,summary,resolved,customFields(…)`
fn issue_json(issue: &FakeIssue) -> Value {
  let resolves = STATES.iter().any(|(name, resolves)| *name == issue.state && *resolves);
  let mut fields = vec![json!({
    "$type": "StateIssueCustomField",
    "id": STATE_FIELD_ID,
    "name": STATE_FIELD,
    "value": {"$type": "StateBundleElement", "name": issue.state, "isResolved": resolves},
  })];
  for (index, field) in issue.fields.iter().enumerate() {
    fields.push(json!({
      "$type": "SingleEnumIssueCustomField",
      "id": format!("92-{}", index + 2),
      "name": field.name,
      "value": {"$type": "EnumBundleElement", "name": field.value},
    }));
  }
  json!({"idReadable": issue.key, "summary": issue.summary, "resolved": issue.resolved, "customFields": fields})
}

/// The decoded value of the form or query field `name`
fn field_of(text: &str, name: &str) -> Option<String> {
  url::form_urlencoded::parse(text.as_bytes())
    .find(|(key, _)| key == name)
    .map(|(_, value)| value.into_owned())
}

fn json_answer(status: StatusCode, body: &Value) -> Response {
  (status, axum::Json(body.clone())).into_response()
}

/// YouTrack's and Hub's error body: `{"error": …, "error_description": …}`
fn error_answer(status: StatusCode, error: &str, description: &str) -> Response {
  json_answer(status, &json!({"error": error, "error_description": description}))
}

fn redirect(location: &str) -> Response {
  (StatusCode::FOUND, [(axum::http::header::LOCATION, location.to_string())]).into_response()
}

/// The routes of `fake`, every path: for a server of the caller's (the E2E test server serves one per test repository)
pub fn router(fake: Arc<FakeYouTrack>) -> axum::Router {
  axum::Router::new().fallback(answer).with_state(fake)
}

async fn answer(State(fake): State<Arc<FakeYouTrack>>, method: Method, uri: Uri, headers: HeaderMap, body: Bytes) -> Response {
  if let Some(gate) = fake.gate(&method, &uri) {
    gate.pass().await;
  }
  fake.respond(&method, &uri, &headers, &body)
}

/// A fake YouTrack on a free port for the rest of the runtime, and its URL (`http://127.0.0.1:<port>`)
pub async fn start_fake() -> (Arc<FakeYouTrack>, String) {
  let fake = Arc::new(FakeYouTrack::default());
  let url = test_utils::fake_server::FakeServer::start(router(Arc::clone(&fake))).await.detach();
  (fake, url)
}
