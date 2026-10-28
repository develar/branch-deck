//! The events of the app reach the pages of a test repository on the stream of each (`TestEvents` of `host.rs`), as the
//! app sends them to its windows: to every page, or to the pages of one window; the events a page emits go the same
//! way, and the streams end with the repository

use crate::invoke::WINDOW_LABEL_HEADER;
use crate::test_support::{Messages, Named, command, create_repository, delete_repository, post, push_intent, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;

/// The event stream of a page of the window `window` of the repository `id`, past its first message
async fn events_of(client: &reqwest::Client, addr: SocketAddr, id: &str, window: &str) -> Messages {
  let mut events = Messages::of(client.get(format!("http://{addr}/repositories/{id}/events?window={window}"))).await;
  assert_eq!(events.next().await, json!({ "subscribed": true }));
  events
}

/// The command `name` of the window `window` of the repository `id`: its answer
async fn invoke(client: &reqwest::Client, addr: SocketAddr, id: &str, window: &str, name: &str, params: &Value) -> Value {
  let request = command(client, addr, Named::Id(id), name, params).header(WINDOW_LABEL_HEADER, window);
  request.send().await.unwrap().json().await.unwrap()
}

/// An event of the app as a page hears it: to every window (`window` null) or to one, `source` null
fn of_app(event: &str, payload: Value, window: Option<&str>) -> Value {
  let mut message = json!({ "event": event, "window": window, "source": null });
  message["payload"] = payload;
  message
}

/// A change of the settings reaches every page, one of a key only the main window keeps the main window's alone; a write
/// of the repository config the main window's; a write of Activity every page; an event a page emits the pages it goes
/// to, its own included; and every stream ends with the repository
#[tokio::test]
async fn the_events_of_the_app_reach_the_pages_of_their_windows_and_end_with_the_repository() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let mut main = events_of(&client, addr, &id, "main").await;
  let mut settings = events_of(&client, addr, &id, "settings").await;

  let zoom = json!({ "patch": { "kind": "fields", "key": "appSettings", "fields": { "zoom": 1.1 } } });
  let change = invoke(&client, addr, &id, "settings", "set_setting", &zoom).await;
  assert_eq!(change["origin"], "settings", "{change}");
  assert_eq!(main.next().await, of_app("settings_changed", change.clone(), None));
  assert_eq!(settings.next().await, of_app("settings_changed", change, None));

  let history = json!({ "patch": { "kind": "value", "key": "robotHistory", "value": [] } });
  let change = invoke(&client, addr, &id, "main", "set_setting", &history).await;
  assert_eq!(main.next().await, of_app("settings_changed", change, Some("main")));

  let write = json!({ "repositoryPath": path, "change": { "key": "resolveByWords", "value": false } });
  invoke(&client, addr, &id, "settings", "set_repository_config", &write).await;
  let changed = json!({ "repositoryPath": path, "key": "resolveByWords" });
  assert_eq!(main.next().await, of_app("repository_config_changed", changed, Some("main")));

  let ask = json!({ "event": "sync_branches", "payload": null, "window": "main", "source": "settings" });
  post(&client, addr, &format!("/repositories/{id}/events"), ask.clone()).await;
  assert_eq!(main.next().await, ask);

  let toast = json!({ "repositoryPath": path, "kind": "error", "title": "Cannot push", "description": null, "details": null, "operationId": null });
  invoke(&client, addr, &id, "settings", "record_activity", &json!(toast)).await;
  // the page of the settings window heard nothing of the main window's events in between
  assert_eq!(main.next().await, of_app("activity-changed", Value::Null, None));
  assert_eq!(settings.next().await, of_app("activity-changed", Value::Null, None));

  let state = json!({ "event": "branch-review/state", "payload": { "items": [] }, "window": null, "source": "main" });
  post(&client, addr, &format!("/repositories/{id}/events"), state.clone()).await;
  assert_eq!(main.next().await, state);
  assert_eq!(settings.next().await, state);

  delete_repository(&client, addr, &id, &path).await;
  assert_eq!(main.next_or_end().await, None);
  assert_eq!(settings.next_or_end().await, None);
}

/// An operation that ends is journaled, and the watch of the operations registry of the repository a window shows tells
/// its pages `activity-changed`, as the app tells its windows: an operation of an agent's `bd` here
#[tokio::test]
async fn an_operation_journaled_in_the_repository_a_window_shows_tells_its_pages() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let mut page = events_of(&client, addr, &id, "main").await;
  // the window's subscription, which the watch lasts with
  let subscription = command(&client, addr, Named::Id(&id), "subscribe_view", &json!({ "repositoryPath": path }));
  let mut patches = Messages::of_channel(subscription, 1).await;
  let first = patches.next().await;
  assert!(first.get("progress").is_some() || first.get("result").is_some(), "{first}");

  let push = json!({ "kind": "push", "branches": ["test-branch"], "force": false });
  let operation = json!({ "intent": push_intent("test-branch", None), "effects": [push] });
  post(&client, addr, &format!("/repositories/{id}/agent-operation"), operation).await;
  loop {
    let event = page.next().await;
    if event["event"] == "activity-changed" {
      assert_eq!(event, of_app("activity-changed", Value::Null, None));
      break;
    }
    // the settings the subscription changed (the journal it remembers for the project)
    assert_eq!(event["event"], "settings_changed", "{event}");
  }
  drop(patches);
  delete_repository(&client, addr, &id, &path).await;
}
