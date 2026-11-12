//! The app link of `brd mcp` (openspec/specs/repository/ide-embedding/spec.md): the corpus of the app side,
//! `docs/design/ide-embedding/protocol/corpus/branch-deck-app.ndjson`, against `mcp-host` on the fixtures of the e2e
//! server, and the requirements of the spec the corpus does not pin. The runner plays air-ide: it sends each line of
//! `air-ide` (its `${any}` arguments of a command filled from the fixture) and checks each line of `app` in order:
//! `${any}` matches any value, `${repo}` is the path of the test repository. A line of the app that is a notification
//! takes each message after it that matches it too (a diff is one call event per step), and an event of the app or a
//! change of a mirror between two lines (the first sync of the session, a change of the settings) is no line of the
//! corpus: the session of the fixture lives while it runs.

use crate::mcp_socket::context_of_connection;
use crate::test_support::{create_repository, head, start_test_server_with_state};
use mcp_host::server::Connection;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::sync::mpsc;

const CORPUS: &str = include_str!("../../../docs/design/ide-embedding/protocol/corpus/branch-deck-app.ndjson");

/// How long the runner waits for a message of brd
const WAIT: Duration = Duration::from_secs(60);

/// The air-ide side of a connection of brd on the fixtures of a test repository
struct Ide {
  state: std::sync::Arc<crate::state::AppState>,
  to_app: Option<mpsc::UnboundedSender<String>>,
  from_app: mpsc::UnboundedReceiver<String>,
  serving: tokio::task::JoinHandle<()>,
  repo: String,
  /// The notifications of brd the runner passed over, for the message of a failure
  passed: Vec<Value>,
}

impl Ide {
  async fn start() -> Self {
    let (addr, state) = start_test_server_with_state().await;
    let (_, repo) = create_repository(&reqwest::Client::new(), addr).await;
    let id = state.path_to_id.get(&repo).map(|id| id.value().clone()).unwrap();
    let (connection, from_app) = Connection::new();
    let ctx = context_of_connection(&state, &id, &connection).unwrap();
    let (to_app, incoming) = mpsc::unbounded_channel();
    let serving = tokio::spawn(connection.serve(ctx, incoming));
    Self {
      state,
      to_app: Some(to_app),
      from_app,
      serving,
      repo,
      passed: Vec::new(),
    }
  }

  fn send(&self, message: &Value) {
    self.to_app.as_ref().unwrap().send(message.to_string()).unwrap();
  }

  async fn next(&mut self) -> Value {
    let message = tokio::time::timeout(WAIT, self.from_app.recv())
      .await
      .unwrap_or_else(|_| panic!("brd wrote nothing; it passed over {:#?}", self.passed))
      .expect("the connection ended");
    serde_json::from_str(&message).unwrap()
  }

  /// The next message of brd that matches `expected`, passing over the events and the changes of the mirrors before it
  async fn expect(&mut self, expected: &Value) -> Value {
    self.expect_after(expected, None).await
  }

  /// `expect`, passing over the repeats of the notification `repeated` too
  async fn expect_after(&mut self, expected: &Value, repeated: Option<&Value>) -> Value {
    loop {
      let message = self.next().await;
      if matches(expected, &message) {
        return message;
      }
      let method = message["method"].as_str().unwrap_or_default();
      let passed = repeated.is_some_and(|repeated| matches(repeated, &message)) || (message.get("id").is_none() && ["x-air-app/event", "x-air-app/viewChanged"].contains(&method));
      assert!(passed, "brd wrote {message:#}\ninstead of {expected:#}");
      self.passed.push(message);
    }
  }

  /// The messages of brd up to the one that matches `expected`, it included
  async fn until(&mut self, expected: &Value) -> Vec<Value> {
    let mut messages = Vec::new();
    loop {
      let message = self.next().await;
      let done = matches(expected, &message);
      messages.push(message);
      if done {
        return messages;
      }
    }
  }

  /// The stdin of brd ends: the connection ends
  async fn end(mut self) {
    self.to_app = None;
    tokio::time::timeout(WAIT, self.serving).await.expect("the connection ends with its input").unwrap();
  }
}

/// `expected` of the corpus matches `actual`: `${any}` any value, else the same value
fn matches(expected: &Value, actual: &Value) -> bool {
  match (expected, actual) {
    (Value::String(any), _) if any == "${any}" => true,
    (Value::Object(expected), Value::Object(actual)) => {
      expected.len() == actual.len() && expected.iter().all(|(key, value)| actual.get(key).is_some_and(|actual| matches(value, actual)))
    }
    (Value::Array(expected), Value::Array(actual)) => expected.len() == actual.len() && expected.iter().zip(actual).all(|(expected, actual)| matches(expected, actual)),
    _ => expected == actual,
  }
}

/// The arguments of a command of the fixture where the corpus has `${any}`
fn fixture_params(command: &str, repo: &str) -> Value {
  let commit = head(repo);
  match command {
    "get_blob_syntax" => {
      let blob = test_utils::git_test_utils::git_command()
        .args(["rev-parse", "HEAD:file1.txt"])
        .current_dir(repo)
        .output()
        .unwrap();
      let blob = String::from_utf8(blob.stdout).unwrap().trim().to_owned();
      json!({ "repositoryPath": repo, "blob": { "id": blob, "path": null }, "filePath": "file1.txt", "fromLine": 0, "toLine": 1, "text": null })
    }
    "get_file_diffs" => json!({ "repositoryPath": repo, "source": { "kind": "commit", "id": commit }, "policy": "default", "priorityPaths": [] }),
    other => panic!("the corpus names no fixture for the arguments of {other}"),
  }
}

#[tokio::test]
async fn the_app_link_corpus_runs() {
  let mut ide = Ide::start().await;
  let repo_json = serde_json::to_string(&ide.repo).unwrap();
  let repo_text = repo_json.trim_matches('"').to_owned();
  let mut last_notification: Option<Value> = None;
  for line in CORPUS.lines().filter(|line| !line.trim().is_empty()) {
    let line: Value = serde_json::from_str(&line.replace("${repo}", &repo_text)).unwrap();
    if line["link"] != "app" {
      continue;
    }
    let mut message = line["message"].clone();
    if line["from"] == "air-ide" {
      let arguments = &mut message["params"]["arguments"];
      if arguments["args"]["params"] == "${any}" {
        let command = arguments["command"].as_str().unwrap().to_owned();
        arguments["args"]["params"] = fixture_params(&command, &ide.repo);
      }
      ide.send(&message);
      continue;
    }
    ide.expect_after(&message, last_notification.as_ref()).await;
    last_notification = message.get("id").is_none().then_some(message);
  }
  ide.end().await;
}

/// The initialize of the corpus and the roots: the main view shows the first Git root
async fn initialized(ide: &mut Ide) {
  ide.send(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": { "roots": { "listChanged": true }, "experimental": { "x-air-app": { "version": "1.0.0", "ahpVersion": "1.0.0" } } }, "clientInfo": { "name": "air-ide", "version": "1.0.0" } } }));
  ide.expect(&json!({ "jsonrpc": "2.0", "id": 1, "result": "${any}" })).await;
  ide.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
  let roots = ide.expect(&json!({ "jsonrpc": "2.0", "id": "${any}", "method": "roots/list" })).await;
  ide.send(&json!({ "jsonrpc": "2.0", "id": roots["id"], "result": { "roots": [{ "uri": format!("file://{}", ide.repo), "name": "repo" }] } }));
  ide.send(&json!({ "jsonrpc": "2.0", "method": "x-air-app/viewShown", "params": { "instance": "main:1", "view": "main", "params": {} } }));
}

/// `initialized`, then the main view subscribes to the mirror of the repository (`x-air-app/subscribe`, id 2): its path
async fn subscribed(ide: &mut Ide) -> String {
  initialized(ide).await;
  let repo = ide.repo.clone();
  ide.send(&json!({ "jsonrpc": "2.0", "id": 2, "method": "x-air-app/subscribe", "params": { "instance": "main:1", "params": { "repositoryPath": repo } } }));
  ide.expect(&json!({ "jsonrpc": "2.0", "id": 2, "result": { "mirror": repo, "state": "${any}" } })).await;
  repo
}

/// A call of `bd_invoke` from the instance `instance`
fn invoke(id: u32, instance: &str, command: &str, args: &Value) -> Value {
  json!({
    "jsonrpc": "2.0", "id": id, "method": "tools/call",
    "params": { "name": "bd_invoke", "arguments": { "command": command, "args": args }, "_meta": { "x-air-app": { "origin": "click", "instance": instance } } },
  })
}

/// A rewrite answers after the change of the mirror it made and after its call events
#[tokio::test]
async fn a_rewrite_answers_after_its_changes_and_its_call_events() {
  let mut ide = Ide::start().await;
  let repo = subscribed(&mut ide).await;

  let witness = json!({ "head": head(&repo), "storeTips": { "issueGroups": null, "stackParents": null, "parked": null } });
  let park = json!({ "kind": "rewrite", "rewrite": "park", "branches": [{ "branchName": "test-branch", "note": "later" }], "witness": witness });
  ide.send(&invoke(
    3,
    "main:1",
    "execute",
    &json!({ "params": { "repositoryPath": repo, "intent": park, "fingerprint": "" } }),
  ));
  let messages = ide.until(&json!({ "jsonrpc": "2.0", "id": 3, "result": "${any}" })).await;
  let answer = messages.last().unwrap();
  assert!(answer["result"].get("isError").is_none(), "{answer:#}");
  let changed = messages
    .iter()
    .position(|message| message["method"] == "x-air-app/viewChanged" && message["params"]["mirror"] == json!(repo));
  let events = messages
    .iter()
    .filter(|message| message["method"] == "x-air-app/callEvent" && message["params"]["callId"] == 3)
    .count();
  assert!(changed.is_some(), "the park changed the mirror before its answer: {messages:#?}");
  assert!(events > 0, "the steps of the park came before its answer: {messages:#?}");
  ide.end().await;
}

/// An open of a review is `openView` of `review`, keyed by its target: an open of the same target again asks the same
/// key, and the tab it showed takes the data and the title of the new open
#[tokio::test]
async fn an_open_of_a_review_is_an_open_view_keyed_by_its_target() {
  let mut ide = Ide::start().await;
  initialized(&mut ide).await;
  let data = json!({ "kind": "branch", "branchName": "test-branch" });
  let open = |id: u32, title: &str| {
    invoke(
      id,
      "main:1",
      "open_sub_window",
      &json!({ "params": { "windowId": "review", "url": "/review", "title": title, "width": null, "height": null, "data": data.to_string(), "openedAt": 1.0 } }),
    )
  };
  ide.send(&open(2, "Review test-branch"));
  let request = ide
    .expect(&json!({ "jsonrpc": "2.0", "id": "${any}", "method": "x-air-app/openView", "params": { "view": "review", "key": "branch:test-branch", "title": "Review test-branch", "params": { "data": data }, "opener": "main:1" } }))
    .await;
  ide.send(&json!({ "jsonrpc": "2.0", "id": request["id"], "result": { "instance": "review:2" } }));
  ide
    .expect(&json!({ "jsonrpc": "2.0", "id": 2, "result": { "content": [], "structuredContent": { "value": null } } }))
    .await;
  ide.send(&json!({ "jsonrpc": "2.0", "method": "x-air-app/viewShown", "params": { "instance": "review:2", "view": "review", "params": { "data": data } } }));

  ide.send(&open(3, "Review test-branch again"));
  let request = ide
    .expect(&json!({ "jsonrpc": "2.0", "id": "${any}", "method": "x-air-app/openView", "params": "${any}" }))
    .await;
  assert_eq!(request["params"]["key"], "branch:test-branch");
  ide.send(&json!({ "jsonrpc": "2.0", "id": request["id"], "result": { "instance": "review:2" } }));
  let shown = ide
    .until(&json!({ "jsonrpc": "2.0", "id": "${any}", "method": "x-air-app/setViewTitle", "params": { "instance": "review:2", "title": "Review test-branch again" } }))
    .await;
  assert!(
    shown
      .iter()
      .any(|message| message["method"] == "x-air-app/event" && message["params"]["instance"] == "review:2" && message["params"]["event"] == "init-data-updated"),
    "{shown:#?}"
  );
  ide.end().await;
}

/// A change of a setting is one event of x-air-app to every view, without an instance; the badge of the Dock is the
/// badge of the tool window
#[tokio::test]
async fn a_change_of_a_setting_is_an_event_and_the_badge_marks_the_tool_window() {
  let mut ide = Ide::start().await;
  initialized(&mut ide).await;
  let patch = json!({ "params": { "patch": { "kind": "value", "key": "theme", "value": "dark" } } });
  ide.send(&invoke(2, "main:1", "set_setting", &patch));
  let messages = ide.until(&json!({ "jsonrpc": "2.0", "id": 2, "result": "${any}" })).await;
  let changed: Vec<_> = messages
    .iter()
    .filter(|message| message["method"] == "x-air-app/event" && message["params"]["event"] == "settings_changed")
    .collect();
  assert_eq!(changed.len(), 1, "{messages:#?}");
  assert!(changed[0]["params"].get("instance").is_none(), "{:#}", changed[0]);

  ide.send(&invoke(3, "main:1", "plugin:window|set_badge_count", &json!({ "value": 2 })));
  ide
    .expect(&json!({ "jsonrpc": "2.0", "id": "${any}", "method": "x-air-app/setBadge", "params": { "view": "main", "count": 2 } }))
    .await;
  ide.end().await;
}

/// An event the page emits goes to the views of the app: to every view, or with a target to the view of that window
#[tokio::test]
async fn an_emit_of_the_page_is_an_event_of_the_views() {
  let mut ide = Ide::start().await;
  initialized(&mut ide).await;
  ide.send(&invoke(
    2,
    "main:1",
    "plugin:event|emit",
    &json!({ "event": "review-run", "payload": { "branch": "a" }, "target": null }),
  ));
  ide
    .expect(&json!({ "jsonrpc": "2.0", "method": "x-air-app/event", "params": { "event": "review-run", "payload": { "branch": "a" } } }))
    .await;
  ide
    .expect(&json!({ "jsonrpc": "2.0", "id": 2, "result": { "content": [], "structuredContent": { "value": null } } }))
    .await;
  ide.send(&invoke(
    3,
    "main:1",
    "plugin:event|emit",
    &json!({ "event": "focus-branch", "payload": "a", "target": "main" }),
  ));
  ide
    .expect(&json!({ "jsonrpc": "2.0", "method": "x-air-app/event", "params": { "instance": "main:1", "event": "focus-branch", "payload": "a" } }))
    .await;
  ide.end().await;
}

/// A call of the tool `name` from the instance `instance` of the view `view`
fn tool_call(id: u32, name: &str, instance: &str, arguments: &Value) -> Value {
  let view = instance.split_once(':').map_or(instance, |(view, _)| view);
  json!({
    "jsonrpc": "2.0", "id": id, "method": "tools/call",
    "params": { "name": name, "arguments": arguments, "_meta": { "x-air-app": { "origin": "click", "view": view, "instance": instance } } },
  })
}

/// A cancel of a running read stops it: no answer and no call event of the call follow, while the same read uncancelled
/// gives both
#[tokio::test]
async fn a_cancelled_read_gives_no_answer_and_no_call_event() {
  let mut ide = Ide::start().await;
  initialized(&mut ide).await;
  let args = json!({ "params": fixture_params("get_file_diffs", &ide.repo) });
  // the cancel comes before the command could write anything: the server reads both before the command runs
  ide.send(&invoke(2, "main:1", "get_file_diffs", &args));
  ide.send(&json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": { "requestId": 2, "reason": "the tab closed" } }));
  ide.send(&invoke(3, "main:1", "get_file_diffs", &args));
  let messages = ide.until(&json!({ "jsonrpc": "2.0", "id": 3, "result": "${any}" })).await;
  let of_call = |id: u32| {
    messages
      .iter()
      .filter(|message| message["id"] == id || (message["method"] == "x-air-app/callEvent" && message["params"]["callId"] == id))
      .count()
  };
  assert_eq!(of_call(2), 0, "the cancelled call wrote nothing: {messages:#?}");
  assert!(of_call(3) > 1, "the same read uncancelled streams its diffs and answers: {messages:#?}");
  ide.end().await;
}

/// The session of a mirror closes (its repository went): the view of the mirror hears `viewEnded` with `sessionClosed`
#[tokio::test]
async fn the_end_of_a_session_ends_its_mirror_with_the_reason_session_closed() {
  let mut ide = Ide::start().await;
  let repo = subscribed(&mut ide).await;
  assert_eq!(ide.state.sessions.close_worktree(&repo), 1, "the session of the repository closes");
  ide
    .expect(&json!({ "jsonrpc": "2.0", "method": "x-air-app/viewEnded", "params": { "mirror": repo, "reason": "sessionClosed" } }))
    .await;
  ide.end().await;
}

/// Two review tabs call the same tool and the same command for their window: each gets the answer of its own instance,
/// the data of its own open and a change of the settings whose origin is its window
#[tokio::test]
async fn two_review_tabs_each_get_the_answer_of_their_own_instance() {
  let mut ide = Ide::start().await;
  initialized(&mut ide).await;
  for (instance, branch) in [("review:2", "a"), ("review:3", "b")] {
    let data = json!({ "kind": "branch", "branchName": branch });
    ide.send(&json!({ "jsonrpc": "2.0", "method": "x-air-app/viewShown", "params": { "instance": instance, "view": "review", "key": format!("branch:{branch}"), "params": { "data": data } } }));
  }
  ide.send(&tool_call(2, "bd_bootstrap", "review:2", &json!({ "view": "review" })));
  ide.send(&tool_call(3, "bd_bootstrap", "review:3", &json!({ "view": "review" })));
  let patch = |value: u32| json!({ "params": { "patch": { "kind": "value", "key": "reviewProbe", "value": value } } });
  ide.send(&invoke(4, "review:2", "set_setting", &patch(2)));
  ide.send(&invoke(5, "review:3", "set_setting", &patch(3)));
  let mut answers = std::collections::HashMap::new();
  while answers.len() < 4 {
    let message = ide.next().await;
    if let Some(id) = message["id"].as_u64().filter(|_| message.get("result").is_some()) {
      answers.insert(id, message["result"]["structuredContent"].clone());
    }
  }
  assert_eq!(answers[&2]["initData"]["data"]["branchName"], "a");
  assert_eq!(answers[&3]["initData"]["data"]["branchName"], "b");
  assert_eq!(answers[&4]["value"]["origin"], "review:2");
  assert_eq!(answers[&5]["value"]["origin"], "review:3");
  ide.end().await;
}
