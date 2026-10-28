//! A rewrite of the page waits for a live holder of the repository lock as the app's does: the engine tells the page
//! whom for on the channel of its `execute` (the step `waitingForLock` with the holder, as SSE events before its answer),
//! and Stop Waiting (`cancel_operation` of the operation) ends the wait with nothing changed (the code `canceled`)

use crate::test_support::{Messages, Named, command, create_repository, head, post, post_as, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Parks `test-branch` through the engine on the channel `channel` while the server holds the lock of the repository:
/// its messages
async fn park_while_locked(client: &reqwest::Client, addr: std::net::SocketAddr, id: &str, path: &str, channel: u32) -> Messages {
  post(client, addr, &format!("/repositories/{id}/locks/repository"), json!({ "held": true })).await;
  let witness = json!({"head": head(path), "storeTips": {"issueGroups": null, "stackParents": null, "parked": null}});
  let park = json!({"kind": "rewrite", "rewrite": "park", "branches": [{"branchName": "test-branch", "note": "later"}], "witness": witness});
  let execute = json!({"repositoryPath": path, "intent": park, "fingerprint": ""});
  Messages::of_channel(command(client, addr, Named::ByPath, "execute", &execute), channel).await
}

/// The next step `waitingForLock` that names a holder: its event
async fn waiting_for_the_holder(messages: &mut Messages) -> Value {
  loop {
    let event = messages.next().await["progress"].clone();
    if event["step"]["kind"] == "waitingForLock" && event["step"]["holder"].is_object() {
      return event;
    }
  }
}

/// The answer of the command, after its events
async fn answer(messages: &mut Messages) -> Value {
  loop {
    let message = messages.next().await;
    if let Some(result) = message.get("result") {
      return result.clone();
    }
  }
}

#[tokio::test]
async fn a_rewrite_says_whom_it_waits_for_and_runs_once_the_holder_released_the_lock() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let mut messages = park_while_locked(&client, addr, &id, &path, 41).await;

  let waiting = waiting_for_the_holder(&mut messages).await;
  assert_eq!(waiting["step"]["holder"]["pid"], json!(std::process::id()), "{waiting}");

  post(&client, addr, &format!("/repositories/{id}/locks/repository"), json!({ "held": false })).await;
  let result = answer(&mut messages).await;
  assert_eq!(result["status"], "ok", "{result}");
}

#[tokio::test]
async fn stop_waiting_ends_the_wait_of_a_rewrite_with_nothing_changed() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let mut messages = park_while_locked(&client, addr, &id, &path, 42).await;
  let op = waiting_for_the_holder(&mut messages).await["op"].clone();

  assert_eq!(post_as(&client, addr, Named::Nothing, "/invoke/cancel_operation", json!({ "op": op })).await, json!(true));
  let result = answer(&mut messages).await;
  assert_eq!(result["status"], "error", "{result}");
  assert_eq!(result["error"]["code"], "canceled", "{result}");
  // nothing was parked, and the operation is gone
  let parked = test_utils::git_test_utils::git_command()
    .args(["-C", &path, "rev-parse", "--verify", "--quiet", "refs/branch-deck/parked"])
    .output()
    .unwrap();
  assert!(!parked.status.success());
  assert_eq!(post_as(&client, addr, Named::Nothing, "/invoke/cancel_operation", json!({ "op": op })).await, json!(false));
  post(&client, addr, &format!("/repositories/{id}/locks/repository"), json!({ "held": false })).await;
}
