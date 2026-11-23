//! The golden corpus of the protocol (docs/design/backend-protocol.md, "Implementation Notes"),
//! `docs/design/backend-protocol/corpus/*.ndjson`, against a host on the fixtures of the e2e server: each corpus file on a
//! host of its own, over the in-process transport, each `conn` a connection of its own. The runner sends each client
//! line (its tokens filled) and expects each host line of a connection in its order: `${any}` matches any value, the
//! values of the runner (`${repo}`, `${build}`, the arguments the fixture prepares) are what it made, any other
//! `${name}` binds at its first appearance in a host line and each later appearance is that value (`${view0.version}` a
//! field of it). A frame no line expects is passed over when it is an action, an event of the app or a call event (a
//! session may send a patch of its own between two lines); a call event after the answer of its call fails the run. A
//! step line is an action of the runner on the fixture: `setPrefix` sets the prefix of the repository in its git config,
//! `deleteRepository` closes its sessions as the watch of its git directory does when the repository went away.
//! The schema of every line is checked by `src/platform/backend/contract.test.ts`.

use crate::host::{Host, HostHooks};
use crate::in_process::InProcess;
use crate::wire::Frame;
use app_api::context::AppContext;
use base64::Engine;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use test_utils::git_test_utils::git_command;

const CONNECTION: &str = include_str!("../../../docs/design/backend-protocol/corpus/connection.ndjson");
const ERRORS: &str = include_str!("../../../docs/design/backend-protocol/corpus/errors.ndjson");
const REPOSITORY_CHANNEL: &str = include_str!("../../../docs/design/backend-protocol/corpus/repository-channel.ndjson");
const COMMAND_STREAM: &str = include_str!("../../../docs/design/backend-protocol/corpus/command-stream.ndjson");
const HOST_REQUESTS: &str = include_str!("../../../docs/design/backend-protocol/corpus/host-requests.ndjson");

/// The build of the host of the corpus
const BUILD: &str = "0.0.0-corpus";

/// How long the runner waits for a frame of the host
const WAIT: Duration = Duration::from_secs(20);

/// The methods of a frame no line expects that the runner passes over
const PASSED: &[&str] = &["action", "x-branch-deck/event", "x-branch-deck/callEvent"];

/// A test repository of the e2e server and the host on its context
pub(crate) struct Fixture {
  state: Arc<test_server::state::AppState>,
  id: String,
  pub(crate) ctx: Arc<AppContext>,
  pub(crate) repo: String,
  pub(crate) host: Arc<Host>,
}

impl Fixture {
  /// A repository of the template `simple` (the virtual branch `test-branch` of two commits) and a host on its context
  pub(crate) async fn new() -> Self {
    let (_app, state) = test_server::create_route_test_app_with_state();
    let (id, repo) = repository(&state, "simple").await;
    let ctx = Arc::clone(&state.repositories.get(&id).unwrap().context);
    let host = Host::of_build(Arc::clone(&ctx), BUILD.to_owned(), HostHooks::default());
    Self { state, id, ctx, repo, host }
  }

  /// The repository synced before, as an earlier run of the app left it, and its session opened from that frame, as
  /// the start of the app opens it (the warm start): its seed
  pub(crate) async fn warm(&self) -> Value {
    let path = || axum::extract::Path(self.id.clone());
    let state = || axum::extract::State(Arc::clone(&self.state));
    let _rows = test_server::session_routes::build_warm_start(path(), state()).await.unwrap();
    let model = test_server::session_routes::warm_start(path(), state())
      .await
      .unwrap()
      .0
      .expect("the frame of the fixture seeds its session");
    serde_json::to_value(model.view).unwrap()
  }

  /// The session of the repository synced and quiet, as one that a window of an earlier start left: a subscriber of
  /// the runner keeps it open with the demand of the corpus, which the windows of the corpus ask again, so neither the
  /// sync nor a read of a fact moves its view between the lines (the clocks of the sessions stand still); the
  /// subscriber, outside the host, takes no `serverSeq`
  pub(crate) async fn settled(&self) -> repository_model::session::SubscriptionId {
    let key = self.ctx.session_key_now(&self.repo).await.unwrap();
    let (patches, mut patched) = tokio::sync::mpsc::unbounded_channel();
    let (subscription, _) = self.ctx.registry.subscribe(&key, Box::new(Probe(patches)), None).await.unwrap();
    let demand =
      serde_json::from_value(json!({ "focused": true, "online": true, "visibleBranches": [], "visibleStacks": [], "needed": [], "robotDetails": [], "watchedRobots": [] }))
        .unwrap();
    self.ctx.sessions.set_demand(subscription, &demand);
    self.ctx.sessions.sync(&key, domain_types::session::SyncReason::Manual).wait().await.unwrap();
    let deadline = tokio::time::Instant::now() + WAIT;
    while tokio::time::timeout(Duration::from_millis(500), patched.recv()).await.is_ok() {
      assert!(tokio::time::Instant::now() < deadline, "the session of the fixture never settles");
    }
    subscription
  }

  /// Another repository of the server, of `template`: its path
  async fn another(&self, template: &str) -> String {
    repository(&self.state, template).await.1
  }

  /// A client of the host, initialized as the bridge of the IDE: the host requests alone, no `showWindow` (the IDE shows
  /// its views itself)
  pub(crate) async fn bridge(&self) -> Link {
    self.client(&json!({ "hostRequests": ["openWindow"] })).await
  }

  /// A client of the host, initialized as the Tauri shell: every capability
  pub(crate) async fn shell(&self) -> Link {
    self
      .client(&json!({ "folderDialog": true, "updater": true, "restart": true, "agentCli": true, "nativeMenu": true, "hostRequests": ["openWindow", "showWindow"] }))
      .await
  }

  /// A client of the host with `capabilities`, initialized
  pub(crate) async fn client(&self, capabilities: &Value) -> Link {
    let link = Link::new(InProcess::client(&self.host));
    let answer = link
      .ask(&json!({ "jsonrpc": "2.0", "id": "init", "method": "initialize", "params": {
        "channel": "ahp-root://", "protocolVersions": ["1.0.0"], "clientId": "test", "clientInfo": { "name": "test", "build": BUILD },
        "_meta": { "x-branch-deck": { "version": "1.0.0", "capabilities": capabilities } },
      } }))
      .await;
    assert!(answer.get("result").is_some(), "{answer:#}");
    link
  }
}

async fn repository(state: &Arc<test_server::state::AppState>, template: &str) -> (String, String) {
  let request = serde_json::from_value(json!({ "template": template })).unwrap();
  let created = test_server::create_repository(axum::extract::State(Arc::clone(state)), axum::Json(request))
    .await
    .unwrap_or_else(|status| panic!("no test repository: {status}"));
  let created = created.0;
  (created.id, created.path)
}

/// The subscriber of the runner that keeps a session open: each patch it heard
struct Probe(tokio::sync::mpsc::UnboundedSender<()>);

impl repository_model::session::Subscriber for Probe {
  fn send(&self, _patch: &domain_types::session::ViewPatch) -> Result<(), repository_model::session::SubscriberGone> {
    let _ = self.0.send(());
    Ok(())
  }
}

/// The head of `repo`
pub(crate) fn head(repo: &str) -> String {
  git(repo, &["rev-parse", "HEAD"])
}

/// What git with `args` prints in `repo`, which must succeed
pub(crate) fn git(repo: &str, args: &[&str]) -> String {
  let ran = git_command().args(args).current_dir(repo).output().unwrap();
  let printed = String::from_utf8_lossy(&ran.stdout).trim().to_owned();
  assert!(ran.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&ran.stderr));
  printed
}

/// A connection of the runner: its frames as JSON
pub(crate) struct Link {
  link: InProcess,
  /// The frames the runner passed over, for the message of a failure
  passed: Vec<Value>,
}

impl Link {
  fn new(link: InProcess) -> Self {
    Self { link, passed: Vec::new() }
  }

  pub(crate) fn send(&self, message: &Value) {
    self.link.send(message);
  }

  /// The next frame of the host, as the JSON a socket carries
  pub(crate) async fn next(&self) -> Value {
    let frame = tokio::time::timeout(WAIT, self.link.next())
      .await
      .unwrap_or_else(|_| panic!("the host wrote nothing; it passed over {:#?}", self.passed))
      .expect("the connection ended");
    match frame {
      Frame::Text(text) => serde_json::from_str(&text).unwrap(),
      Frame::Bytes { id, bytes } => json!({ "jsonrpc": "2.0", "id": id, "result": { "bytes": base64::engine::general_purpose::STANDARD.encode(bytes) } }),
    }
  }

  /// The connection of the link
  pub(crate) fn connection(&self) -> &Arc<crate::connection::Connection> {
    self.link.connection()
  }

  /// The id of the connection in app-api
  pub(crate) fn id(&self) -> app_api::transport::ConnectionId {
    self.link.connection().id().expect("the connection initialized")
  }

  /// Sends the request `request` and answers its answer, passing over the frames before it
  pub(crate) async fn ask(&self, request: &Value) -> Value {
    self.send(request);
    self.until(|frame| frame["id"] == request["id"] && frame.get("method").is_none()).await.pop().unwrap()
  }

  /// Sends the request `request`: the frames up to its answer, it included
  pub(crate) async fn frames_of(&self, request: &Value) -> Vec<Value> {
    self.send(request);
    self.until(|frame| frame["id"] == request["id"] && frame.get("method").is_none()).await
  }

  /// The frames of the host up to the one that `done` takes, it included
  pub(crate) async fn until(&self, done: impl Fn(&Value) -> bool) -> Vec<Value> {
    let mut frames = Vec::new();
    loop {
      let frame = self.next().await;
      let last = done(&frame);
      frames.push(frame);
      if last {
        return frames;
      }
    }
  }
}

/// The runner of one corpus file
struct Runner<'a> {
  fixture: &'a Fixture,
  values: HashMap<String, Value>,
  bound: HashMap<String, Value>,
  links: BTreeMap<u64, Link>,
  /// The requests whose answer a connection got: a call event of one after it fails
  answered: HashSet<(u64, String)>,
}

impl<'a> Runner<'a> {
  fn new(fixture: &'a Fixture) -> Self {
    let values = HashMap::from([
      ("repo".to_owned(), Value::String(fixture.repo.clone())),
      ("build".to_owned(), Value::String(BUILD.to_owned())),
    ]);
    Self {
      fixture,
      values,
      bound: HashMap::new(),
      links: BTreeMap::new(),
      answered: HashSet::new(),
    }
  }

  async fn run(&mut self, corpus: &str) {
    for (number, line) in corpus.lines().enumerate().filter(|(_, line)| !line.trim().is_empty()) {
      let line: Value = serde_json::from_str(line).unwrap_or_else(|error| panic!("line {}: {error}", number + 1));
      let conn = line["conn"].as_u64().unwrap_or(1);
      if let Some(step) = line["step"].as_str() {
        self.step(step, &line);
      } else if let Some(raw) = line["clientRaw"].as_str() {
        self.link(conn).link.send_line(raw);
      } else if let Some(frame) = line.get("client") {
        let frame = self.fill(frame).await;
        self.link(conn).send(&frame);
      } else if let Some(expected) = line.get("host") {
        self.expect(conn, expected, number + 1).await;
      }
    }
  }

  fn link(&mut self, conn: u64) -> &Link {
    let host = &self.fixture.host;
    self.links.entry(conn).or_insert_with(|| Link::new(InProcess::client(host)))
  }

  /// An action of the runner on the fixture
  fn step(&self, step: &str, line: &Value) {
    let repo = &self.fixture.repo;
    match step {
      "setPrefix" => {
        git(repo, &["config", "branchdeck.branchPrefix", line["value"].as_str().unwrap()]);
      }
      "deleteRepository" => {
        self.fixture.ctx.sessions.close_worktree(repo);
      }
      other => panic!("the runner has no step {other}"),
    }
  }

  /// A value of the runner, made when a line first names it
  async fn value(&mut self, name: &str) -> Option<Value> {
    if let Some(value) = self.values.get(name) {
      return Some(value.clone());
    }
    let repo = self.fixture.repo.clone();
    let value = match name {
      "unprefixed" => Value::String(self.fixture.another("simple_no_prefix").await),
      "commit" => Value::String(head(&repo)),
      "settingParams" => json!({ "patch": { "kind": "value", "key": "theme", "value": "dark" } }),
      "syntaxParams" => {
        let blob = git(&repo, &["rev-parse", "HEAD:file1.txt"]);
        json!({ "repositoryPath": repo, "blob": { "id": blob, "path": null }, "filePath": "file1.txt", "fromLine": 0, "toLine": 1, "text": null })
      }
      "renameIntent" => rename(&repo, "test-branch", "corpus-renamed"),
      "renameBackIntent" => rename(&repo, "corpus-renamed", "test-branch"),
      "fingerprint" | "fingerprint2" => Value::String(String::new()),
      "pageActivity" => json!({ "repositoryPath": repo, "kind": "error", "title": "corpus", "description": null, "details": null, "operationId": null }),
      _ => return None,
    };
    // the intents are made when the line that sends them comes, of the head then
    if !name.ends_with("Intent") {
      self.values.insert(name.to_owned(), value.clone());
    }
    Some(value)
  }

  /// The value a whole-string token stands for: a value of the runner or one bound, with its field
  async fn token(&mut self, token: &str) -> Option<Value> {
    let (name, field) = token.split_once('.').map_or((token, None), |(name, field)| (name, Some(field)));
    let value = match self.value(name).await {
      Some(value) => value,
      None => self.bound.get(name)?.clone(),
    };
    match field {
      Some(field) => value.get(field).cloned(),
      None => Some(value),
    }
  }

  /// A client line with its tokens filled
  async fn fill(&mut self, frame: &Value) -> Value {
    match frame {
      Value::String(text) => match whole_token(text) {
        Some(token) => self.token(token).await.unwrap_or_else(|| panic!("the runner has no value of {text}")),
        None => Value::String(text.replace("${repo}", &self.fixture.repo)),
      },
      Value::Object(object) => {
        let mut filled = serde_json::Map::new();
        for (key, value) in object {
          filled.insert(key.clone(), Box::pin(self.fill(value)).await);
        }
        Value::Object(filled)
      }
      Value::Array(items) => {
        let mut filled = Vec::new();
        for item in items {
          filled.push(Box::pin(self.fill(item)).await);
        }
        Value::Array(filled)
      }
      other => other.clone(),
    }
  }

  /// Reads the frames of `conn` until one matches `expected`, passing over those no line expects
  async fn expect(&mut self, conn: u64, expected: &Value, line: usize) {
    loop {
      let frame = self.link(conn).next().await;
      if frame["method"] == "x-branch-deck/callEvent" {
        let call = (conn, frame["params"]["requestId"].to_string());
        assert!(!self.answered.contains(&call), "line {line}: a call event after the answer of its call: {frame:#}");
      }
      let mut bound = self.bound.clone();
      if self.matches(expected, &frame, &mut bound).await {
        self.bound = bound;
        if frame.get("id").is_some() && frame.get("method").is_none() {
          self.answered.insert((conn, frame["id"].to_string()));
        }
        return;
      }
      let method = frame["method"].as_str().unwrap_or_default();
      let passed = frame.get("id").is_none() && PASSED.contains(&method);
      let link = self.links.get_mut(&conn).unwrap();
      let versions: BTreeMap<&String, &Value> = self
        .bound
        .iter()
        .map(|(name, value)| (name, value.get("version").unwrap_or(value)))
        .filter(|(_, value)| value.get("epoch").is_some() || value.is_string())
        .collect();
      assert!(
        passed,
        "line {line}, conn {conn}: the host wrote {frame:#}\ninstead of {expected:#}\nwith the versions bound {versions:?}\nafter passing over {:#?}",
        link.passed
      );
      link.passed.push(frame);
    }
  }

  /// `expected` of a host line matches `actual`, binding the tokens of their first appearance into `bound`
  async fn matches(&mut self, expected: &Value, actual: &Value, bound: &mut HashMap<String, Value>) -> bool {
    match (expected, actual) {
      (Value::String(text), _) if text == "${any}" => true,
      (Value::String(text), _) if whole_token(text).is_some() => {
        let token = whole_token(text).unwrap();
        let (name, field) = token.split_once('.').map_or((token, None), |(name, field)| (name, Some(field)));
        if let Some(value) = self.value(name).await {
          return field.map_or(Some(&value), |field| value.get(field)) == Some(actual);
        }
        match (bound.get(name), field) {
          (Some(value), Some(field)) => value.get(field) == Some(actual),
          (Some(value), None) => value == actual,
          (None, None) => {
            bound.insert(name.to_owned(), actual.clone());
            true
          }
          (None, Some(_)) => false,
        }
      }
      (Value::String(text), Value::String(actual)) => text.replace("${repo}", &self.fixture.repo) == *actual,
      // a number of JSON is its value: 0 and 0.0 are one
      (Value::Number(expected), Value::Number(actual)) => expected.as_f64() == actual.as_f64(),
      (Value::Object(expected), Value::Object(actual)) => {
        if expected.len() != actual.len() {
          return false;
        }
        for (key, value) in expected {
          let Some(actual) = actual.get(key) else {
            return false;
          };
          if !Box::pin(self.matches(value, actual, bound)).await {
            return false;
          }
        }
        true
      }
      (Value::Array(expected), Value::Array(actual)) => {
        if expected.len() != actual.len() {
          return false;
        }
        for (expected, actual) in expected.iter().zip(actual) {
          if !Box::pin(self.matches(expected, actual, bound)).await {
            return false;
          }
        }
        true
      }
      _ => expected == actual,
    }
  }
}

/// The name of a token that is the whole string, `${name}` or `${name.field}`
fn whole_token(text: &str) -> Option<&str> {
  let token = text.strip_prefix("${")?.strip_suffix('}')?;
  (!token.is_empty() && !token.contains(['$', '{', '}'])).then_some(token)
}

/// The intent that renames the virtual branch `from` of `repo` to `to`, witnessed at its head now
fn rename(repo: &str, from: &str, to: &str) -> Value {
  let witness = json!({ "head": head(repo), "storeTips": { "issueGroups": null, "stackParents": null, "parked": null } });
  json!({ "kind": "rewrite", "rewrite": "rename", "branchName": from, "newName": to, "witness": witness })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_connection_corpus_runs() {
  let fixture = Fixture::new().await;
  Runner::new(&fixture).run(CONNECTION).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_errors_corpus_runs() {
  let fixture = Fixture::new().await;
  Runner::new(&fixture).run(ERRORS).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_repository_channel_corpus_runs() {
  let fixture = Fixture::new().await;
  let seed = fixture.warm().await;
  let _kept = fixture.settled().await;
  let mut runner = Runner::new(&fixture);
  runner.values.insert("seed".to_owned(), seed);
  runner.run(REPOSITORY_CHANNEL).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_command_stream_corpus_runs() {
  let fixture = Fixture::new().await;
  let _kept = fixture.settled().await;
  Runner::new(&fixture).run(COMMAND_STREAM).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_host_requests_corpus_runs() {
  let fixture = Fixture::new().await;
  Runner::new(&fixture).run(HOST_REQUESTS).await;
}
