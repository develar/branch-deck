//! The server on a free port and the repositories of its tests, for the route tests of this crate

use serde_json::{Value, json};
use std::net::SocketAddr;
use test_utils::git_test_utils::git_command;

pub(crate) async fn start_test_server() -> SocketAddr {
  start_test_server_with_state().await.0
}

/// The server on a free port with its state, for a test that checks the files of the server
pub(crate) async fn start_test_server_with_state() -> (SocketAddr, std::sync::Arc<crate::state::AppState>) {
  let (app, state) = crate::create_route_test_app_with_state();
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("Failed to bind to port");
  let addr = listener.local_addr().unwrap();
  tokio::spawn(async move {
    axum::serve(listener, app).await.unwrap();
  });
  (addr, state)
}

/// A repository from the `simple` template: its id and path
pub(crate) async fn create_repository(client: &reqwest::Client, addr: SocketAddr) -> (String, String) {
  create_repository_from(client, addr, "simple").await
}

/// A repository from the template `template` (`RepositoryTemplate`): its id and path
pub(crate) async fn create_repository_from(client: &reqwest::Client, addr: SocketAddr, template: &str) -> (String, String) {
  let created: Value = client
    .post(format!("http://{addr}/repositories"))
    .json(&json!({ "template": template }))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
  (created["id"].as_str().unwrap().to_string(), created["path"].as_str().unwrap().to_string())
}

/// Deletes the repository `id` at `path` as the cleanup of a test does, which answers at once, and waits until the server
/// removed its files in the background, once the commands still running for it ended
pub(crate) async fn delete_repository(client: &reqwest::Client, addr: SocketAddr, id: &str, path: &str) {
  let deleted = client.delete(format!("http://{addr}/repositories/{id}")).send().await.unwrap();
  assert_eq!(deleted.status(), 202, "delete");
  let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
  while std::path::Path::new(path).exists() {
    assert!(std::time::Instant::now() < deadline, "{path} is still there");
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
  }
}

/// The repository a command of a route test names, as a page names its repository on every request
/// (`X-Repository-Id`): the server holds the repository while the command runs, and its deletion waits for the command
/// (`hold_repository`) instead of removing the files under it
#[derive(Clone, Copy)]
pub(crate) enum Named<'a> {
  /// The repository at the `repositoryPath` of the params, which must be one of the server
  ByPath,
  /// The repository `id`: for a command whose params name none (a sign-in of the requesting repository, the cancel of an
  /// operation) or name a path that is no repository of the server
  Id(&'a str),
}

/// The request of the command `name` (`/invoke/<name>`) with `params`, naming the repository as `named` says: the one
/// way a route test sends a command (`every_command_of_a_route_test_names_its_repository`)
pub(crate) fn command(client: &reqwest::Client, addr: SocketAddr, named: Named<'_>, name: &str, params: &Value) -> reqwest::RequestBuilder {
  let request = client.post(format!("http://{addr}/invoke/{name}")).json(params);
  let id = match named {
    Named::ByPath => {
      let path = params["repositoryPath"].as_str().unwrap_or_default();
      repository_id_of_path(path).unwrap_or_else(|| panic!("{name}: {path:?} is no repository of the server; Named::Id says what it runs for"))
    }
    Named::Id(id) => id,
  };
  request.header(crate::REPOSITORY_ID_HEADER, id)
}

/// The id of the repository of the server at `path`: its directory is named by it, and the path of a `NO_REPO`
/// repository is its id after `NO_REPO_` (`create_repository`)
fn repository_id_of_path(path: &str) -> Option<&str> {
  let name = std::path::Path::new(path).file_name()?.to_str()?;
  let id = name.strip_prefix(crate::NO_REPO_PREFIX).unwrap_or(name);
  let digits = id.strip_prefix(crate::REPOSITORY_ID_PREFIX)?;
  (digits.len() == crate::REPOSITORY_ID_DIGITS && digits.bytes().all(|byte| byte.is_ascii_digit())).then_some(id)
}

/// POSTs `body` to `path`, which must answer 200: its JSON, null for none. A command (`/invoke/<name>`) names the
/// repository at its `repositoryPath` (`Named::ByPath`)
pub(crate) async fn post(client: &reqwest::Client, addr: SocketAddr, path: &str, body: Value) -> Value {
  post_as(client, addr, Named::ByPath, path, body).await
}

/// `post`, a command naming the repository as `named` says
pub(crate) async fn post_as(client: &reqwest::Client, addr: SocketAddr, named: Named<'_>, path: &str, body: Value) -> Value {
  let request = match path.strip_prefix("/invoke/") {
    Some(name) => command(client, addr, named, name, &body),
    None => client.post(format!("http://{addr}{path}")).json(&body),
  };
  let response = request.send().await.unwrap();
  assert_eq!(response.status(), 200, "{path}");
  response.json().await.unwrap_or(Value::Null)
}

/// POSTs `body` to the command `path` (`/invoke/<name>`) of the repository at its `repositoryPath`, which must fail:
/// the `ApiError` it answers
pub(crate) async fn post_error(client: &reqwest::Client, addr: SocketAddr, path: &str, body: Value) -> Value {
  let name = path.strip_prefix("/invoke/").unwrap_or_else(|| panic!("{path} is no command"));
  let response = command(client, addr, Named::ByPath, name, &body).send().await.unwrap();
  let answer: Value = response.json().await.unwrap();
  assert_eq!(answer["status"], "error", "{path}: {answer}");
  answer["error"].clone()
}

/// Asserts that `error` is the answer of an undo whose branch moved on to `moved`: `undo_stale` with `retry: no`, in
/// words that say it can't be undone, not that a retry may help, and the branch of the repository at `path` still at
/// `moved`
pub(crate) fn assert_stale_undo(error: &Value, path: &str, moved: &str) {
  assert_eq!((&error["code"], &error["retry"]), (&json!("undo_stale"), &json!("no")), "{error}");
  let message = error["message"].as_str().unwrap();
  assert!(message.ends_with("; it can't be undone any more") && !message.contains("retry"), "{message}");
  assert_eq!(head(path), moved);
}

/// The HEAD of the repository at `path`
pub(crate) fn head(path: &str) -> String {
  let output = git_command().args(["rev-parse", "HEAD"]).current_dir(path).output().unwrap();
  String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// The view of the session of `path` once `done` says it is, asked again until then
pub(crate) async fn view_when(client: &reqwest::Client, addr: SocketAddr, path: &str, done: impl Fn(&Value) -> bool) -> Value {
  for _ in 0..2000 {
    let view = post(client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
    if done(&view) {
      return view;
    }
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
  }
  panic!("the view of {path} never came")
}

/// Syncs the repository at `path` as the page does (`/invoke/sync` of its session), under the prefix the server resolves
/// (the `user-name` of the template), to the end of its run: the server keeps the branch table of it, which the checks
/// of a push or a start read
pub(crate) async fn sync(client: &reqwest::Client, addr: SocketAddr, path: &str) {
  let response = command(client, addr, Named::ByPath, "sync", &json!({ "repositoryPath": path, "reason": "manual" }))
    .send()
    .await
    .unwrap();
  assert_eq!(response.status(), 200, "sync");
  response.text().await.unwrap();
}

/// The intent of the push of `branch` alone, as its push button asks it: over the remote head the row showed
/// (`expected`, `None`: no remote branch)
pub(crate) fn push_intent(branch: &str, expected: Option<&str>) -> Value {
  serde_json::json!({
    "kind": "push",
    "subject": {"kind": "branch", "name": branch},
    "scope": {"kind": "alone"},
    "expectRemote": expected,
    "force": false,
    "restartRunning": false,
  })
}

/// The `execute` of `intent` in the session of `path` on the plan `prepare` answers now, every confirmation of it
/// accepted, as the push button runs it once the user confirmed
pub(crate) async fn execute_request(client: &reqwest::Client, addr: SocketAddr, path: &str, intent: &Value) -> Value {
  let plan = post(client, addr, "/invoke/prepare", serde_json::json!({"repositoryPath": path, "intent": intent})).await;
  assert!(plan.is_object(), "no plan of {intent}: {plan}");
  serde_json::json!({"repositoryPath": path, "intent": plan["intent"], "fingerprint": plan["fingerprint"], "accepted": plan["confirmations"]})
}

/// The push of `branch` alone through the engine (`push_intent`, `execute_request`): its outcome
pub(crate) async fn push_alone(client: &reqwest::Client, addr: SocketAddr, path: &str, branch: &str, expected: Option<&str>) -> Value {
  let execute = execute_request(client, addr, path, &push_intent(branch, expected)).await;
  post(client, addr, "/invoke/execute", execute).await
}

/// The rewrite `rewrite` (`{"rewrite": "park", "branches": […]}`) of the repository at `path` through the engine, as
/// the page runs it: named on the table at HEAD as it is now (an undo names none), its outcome
pub(crate) async fn rewrite(client: &reqwest::Client, addr: SocketAddr, path: &str, rewrite: Value) -> Value {
  post(client, addr, "/invoke/execute", rewrite_request(path, rewrite)).await
}

/// `rewrite` for a rewrite that fails: its error
pub(crate) async fn rewrite_error(client: &reqwest::Client, addr: SocketAddr, path: &str, rewrite: Value) -> Value {
  post_error(client, addr, "/invoke/execute", rewrite_request(path, rewrite)).await
}

/// The `execute` of `rewrite` in the repository at `path`, named on the table at HEAD as it is now (an undo names none)
fn rewrite_request(path: &str, mut rewrite: Value) -> Value {
  if rewrite["rewrite"] != "undo" {
    rewrite["witness"] = json!({"head": head(path), "storeTips": {"issueGroups": null, "stackParents": null, "parked": null}});
  }
  rewrite["kind"] = json!("rewrite");
  json!({"repositoryPath": path, "intent": rewrite, "fingerprint": ""})
}

/// What the server logs from now on at warning level and above, for a test that asserts it logs none: the global
/// subscriber, which sees the threads of the server too, sound because nextest runs each test in a process of its own
pub(crate) fn capture_warnings() -> std::sync::Arc<std::sync::Mutex<Vec<u8>>> {
  let captured = std::sync::Arc::<std::sync::Mutex<Vec<u8>>>::default();
  let writer = std::sync::Arc::clone(&captured);
  tracing_subscriber::fmt()
    .with_max_level(tracing::Level::WARN)
    .with_ansi(false)
    .with_writer(move || CapturedWriter(std::sync::Arc::clone(&writer)))
    .init();
  captured
}

/// A writer of `capture_warnings`
struct CapturedWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for CapturedWriter {
  fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
    self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).extend_from_slice(bytes);
    Ok(bytes.len())
  }

  fn flush(&mut self) -> std::io::Result<()> {
    Ok(())
  }
}

/// The JSON messages of an SSE answer (`invoke::answer_with_progress`), one by one
pub(crate) struct Messages {
  response: reqwest::Response,
  buffer: String,
}

impl Messages {
  /// The messages of `request` sent on the channel `channel` (`X-Progress-Channel`), which must answer an event stream
  pub(crate) async fn of_channel(request: reqwest::RequestBuilder, channel: u32) -> Self {
    Self::of(request.header(crate::invoke::PROGRESS_CHANNEL_HEADER, channel.to_string())).await
  }

  /// The messages of `request`, which must answer an event stream (a command's channel, the events of a page)
  pub(crate) async fn of(request: reqwest::RequestBuilder) -> Self {
    let response = request.send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.headers()["content-type"].to_str().unwrap().starts_with("text/event-stream"));
    Self { response, buffer: String::new() }
  }

  /// The next message; the stream must have one within a minute
  pub(crate) async fn next(&mut self) -> Value {
    self.next_or_end().await.expect("the stream ended before its next message")
  }

  /// The next message, `None` once the stream ended
  pub(crate) async fn next_or_end(&mut self) -> Option<Value> {
    loop {
      if let Some(end) = self.buffer.find("\n\n") {
        let event: String = self.buffer.drain(..end + 2).collect();
        if let Some(data) = event.lines().find_map(|line| line.strip_prefix("data: ")) {
          return Some(serde_json::from_str(data).unwrap());
        }
        continue;
      }
      let chunk = tokio::time::timeout(std::time::Duration::from_mins(1), self.response.chunk())
        .await
        .expect("no message within a minute")
        .unwrap()?;
      self.buffer.push_str(&String::from_utf8_lossy(&chunk));
    }
  }
}
