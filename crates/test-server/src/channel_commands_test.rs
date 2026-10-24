//! A rewrite of the page waits for a live holder of the repository lock as the app's does: the server tells the page
//! whom for on the channel of the command, as SSE events before its answer, and Stop Waiting (`cancel_lock_wait` of the
//! channel) ends the wait with nothing changed (the code `canceled`)

use crate::test_support::{Named, command, create_repository, post, post_as, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The JSON messages of an SSE answer, one by one
struct Messages {
  response: reqwest::Response,
  buffer: String,
}

impl Messages {
  async fn next(&mut self) -> Value {
    loop {
      if let Some(end) = self.buffer.find("\n\n") {
        let event: String = self.buffer.drain(..end + 2).collect();
        if let Some(data) = event.lines().find_map(|line| line.strip_prefix("data: ")) {
          return serde_json::from_str(data).unwrap();
        }
        continue;
      }
      let chunk = tokio::time::timeout(std::time::Duration::from_mins(1), self.response.chunk())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
      self.buffer.push_str(&String::from_utf8_lossy(&chunk));
    }
  }
}

/// Parks `test-branch` on the channel `channel` while the server holds the lock of the repository: its messages
async fn park_while_locked(client: &reqwest::Client, addr: std::net::SocketAddr, id: &str, path: &str, channel: u32) -> Messages {
  post(client, addr, &format!("/repositories/{id}/repository-lock"), json!({ "held": true })).await;
  let park = json!({ "repositoryPath": path, "branches": [{ "branchName": "test-branch", "note": "later" }] });
  let response = command(client, addr, Named::ByPath, "park_branches", &park)
    .header("X-Progress-Channel", channel.to_string())
    .send()
    .await
    .unwrap();
  assert_eq!(response.status(), 200);
  assert!(response.headers()["content-type"].to_str().unwrap().starts_with("text/event-stream"));
  Messages { response, buffer: String::new() }
}

#[tokio::test]
async fn a_rewrite_says_whom_it_waits_for_and_runs_once_the_holder_released_the_lock() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let mut messages = park_while_locked(&client, addr, &id, &path, 41).await;

  let waiting = messages.next().await;
  let text = waiting["progress"]["waitingFor"].as_str().unwrap();
  assert!(text.starts_with("Waiting for `"), "{waiting}");
  assert!(text.contains(&format!("(pid {}, for ", std::process::id())), "{waiting}");

  post(&client, addr, &format!("/repositories/{id}/repository-lock"), json!({ "held": false })).await;
  let result = loop {
    let message = messages.next().await;
    if let Some(result) = message.get("result") {
      break result.clone();
    }
  };
  assert_eq!(result["status"], "ok", "{result}");
}

#[tokio::test]
async fn stop_waiting_ends_the_wait_of_a_rewrite_with_nothing_changed() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let mut messages = park_while_locked(&client, addr, &id, &path, 42).await;
  assert!(messages.next().await["progress"]["waitingFor"].is_string());

  assert_eq!(
    post_as(&client, addr, Named::Nothing, "/invoke/cancel_lock_wait", json!({ "channel": 42 })).await,
    json!(true)
  );
  let result = loop {
    let message = messages.next().await;
    if let Some(result) = message.get("result") {
      break result.clone();
    }
  };
  assert_eq!(result["status"], "error", "{result}");
  assert_eq!(result["error"]["code"], "canceled", "{result}");
  // nothing was parked, and the wait is gone
  let parked = test_utils::git_test_utils::git_command()
    .args(["-C", &path, "rev-parse", "--verify", "--quiet", "refs/branch-deck/parked"])
    .output()
    .unwrap();
  assert!(!parked.status.success());
  assert_eq!(
    post_as(&client, addr, Named::Nothing, "/invoke/cancel_lock_wait", json!({ "channel": 42 })).await,
    json!(false)
  );
  post(&client, addr, &format!("/repositories/{id}/repository-lock"), json!({ "held": false })).await;
}
