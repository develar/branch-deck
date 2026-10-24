//! The session commands of the server as the page mock calls them: a subscription is an event stream that answers
//! first and then carries the patches, each on the version before it, up to the version a sync answers; it ends with
//! `unsubscribe_view`; a repository without a prefix is refused with the `ApiError` of the app, on the stream too

use crate::test_support::{Messages, Named, command, create_repository, create_repository_from, post, post_as, post_error, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;

/// `subscribe_view` of the repository at `path` on the channel `channel`, for the repository `id`, as the page mock asks
pub(crate) async fn subscribe(client: &reqwest::Client, addr: SocketAddr, id: &str, path: &str, channel: u32) -> Messages {
  let request = command(client, addr, Named::Id(id), "subscribe_view", &json!({ "repositoryPath": path }));
  Messages::of_channel(request, channel).await
}

/// The answer of a subscription, which comes before its first patch: its `data`
pub(crate) async fn subscription(messages: &mut Messages) -> Value {
  let first = messages.next().await;
  assert_eq!(first["result"]["status"], "ok", "{first}");
  first["result"]["data"].clone()
}

#[tokio::test]
async fn a_subscription_answers_its_view_then_the_patches_up_to_the_version_a_sync_answers() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let opened = post(&client, addr, "/invoke/open_session", json!({ "repositoryPath": path })).await;
  assert_eq!(opened["version"]["seq"], 0, "{opened}");

  let mut messages = subscribe(&client, addr, &id, &path, 7).await;
  let subscribed = subscription(&mut messages).await;
  assert_eq!(subscribed["view"]["version"], opened["version"], "the session the window opened");

  let synced = post(&client, addr, "/invoke/sync", json!({ "repositoryPath": path, "reason": "manual" })).await;
  assert_eq!(synced["outcome"], "synced", "{synced}");
  let mut version = subscribed["view"]["version"].clone();
  while version != synced["version"] {
    let patch = messages.next().await["progress"].clone();
    assert_eq!(patch["base"], version, "each patch follows the one before");
    version = patch["next"].clone();
  }

  let snapshot = post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
  assert_eq!(snapshot["version"], version);
  assert_eq!(snapshot["sync"], json!({ "kind": "idle" }));
  assert!(!snapshot["order"].as_array().unwrap().is_empty(), "the branches of the sync: {snapshot}");
  // nothing a sync reads changed: a focus runs nothing
  let focused = post(&client, addr, "/invoke/set_window_focus", json!({ "repositoryPath": path, "focused": true })).await;
  assert_eq!(focused["outcome"], "unchanged", "{focused}");

  post_as(&client, addr, Named::Id(&id), "/invoke/unsubscribe_view", json!({ "id": subscribed["id"] })).await;
  assert_eq!(messages.next_or_end().await, None, "the stream ends with the subscription");
}

#[tokio::test]
async fn a_repository_without_a_prefix_is_refused_with_the_api_error_of_the_app() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository_from(&client, addr, "simple_no_prefix").await;

  let mut messages = subscribe(&client, addr, &id, &path, 8).await;
  let refused = messages.next().await;
  assert_eq!(refused["result"]["status"], "error", "{refused}");
  assert_eq!(refused["result"]["error"]["code"], "no_branch_prefix", "{refused}");
  assert_eq!(messages.next_or_end().await, None);

  for command in ["open_session", "get_snapshot", "sync"] {
    let error = post_error(&client, addr, &format!("/invoke/{command}"), json!({ "repositoryPath": path, "reason": "open" })).await;
    assert_eq!(error["code"], "no_branch_prefix", "{command}: {error}");
  }
}
