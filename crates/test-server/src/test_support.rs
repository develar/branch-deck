//! The server on a free port and the repositories of its tests, for the route tests of this crate

use serde_json::{Value, json};
use std::net::SocketAddr;
use test_utils::git_test_utils::git_command;

pub(crate) async fn start_test_server() -> SocketAddr {
  start_test_server_with_state().await.0
}

/// The server on a free port with its state, for a test that checks the files of the server
pub(crate) async fn start_test_server_with_state() -> (SocketAddr, std::sync::Arc<crate::state::AppState>) {
  let (app, state) = crate::create_route_test_app_with_state().await;
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

/// POSTs `body` to `path`, which must answer 200: its JSON, null for none
pub(crate) async fn post(client: &reqwest::Client, addr: SocketAddr, path: &str, body: Value) -> Value {
  let response = client.post(format!("http://{addr}{path}")).json(&body).send().await.unwrap();
  assert_eq!(response.status(), 200, "{path}");
  response.json().await.unwrap_or(Value::Null)
}

/// POSTs `body` to the command `path`, which must fail: the `ApiError` it answers
pub(crate) async fn post_error(client: &reqwest::Client, addr: SocketAddr, path: &str, body: Value) -> Value {
  let response = client.post(format!("http://{addr}{path}")).json(&body).send().await.unwrap();
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

/// Syncs the repository at `path` as the page does (`/invoke/sync_branches`), under the prefix the server resolves (the
/// `user-name` of the template), to the end of its event stream: the server keeps the branch table of it, which the
/// checks of a push or a start read
pub(crate) async fn sync(client: &reqwest::Client, addr: SocketAddr, path: &str) {
  let response = client
    .post(format!("http://{addr}/invoke/sync_branches"))
    .json(&json!({ "repositoryPath": path }))
    .send()
    .await
    .unwrap();
  assert_eq!(response.status(), 200, "sync");
  response.text().await.unwrap();
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
