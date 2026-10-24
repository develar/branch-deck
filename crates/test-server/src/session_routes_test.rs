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

/// The next sync reports what a test set for it (`sync-overrides`): the remote status of a branch with the fields given
/// and the newest of its unpushed commits kept, the issue links of a YouTrack user; the one after it leaves out a status set to null, so the branch keeps the one it
/// had; the sync after that reports the server's own status again. Overrides set for every sync stay for each run, until
/// a test sets others
#[tokio::test]
async fn the_next_sync_takes_the_overrides_of_its_repository_and_the_syncs_after_it_do_not_unless_set_for_every_sync() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let mut messages = subscribe(&client, addr, &id, &path, 9).await;
  subscription(&mut messages).await;
  let links = json!({ "links": [{ "issueRegexp": "\\b[A-Z]+-\\d+\\b", "linkRegexp": "https://youtrack.test/issue/$0" }] });
  let sync_with = |overrides: Value| {
    let (client, path, id) = (client.clone(), path.clone(), id.clone());
    async move {
      if !overrides.is_null() {
        post(&client, addr, &format!("/repositories/{id}/sync-overrides"), overrides).await;
      }
      let synced = post(&client, addr, "/invoke/sync", json!({ "repositoryPath": path, "reason": "manual" })).await;
      assert_eq!(synced["outcome"], "synced", "{synced}");
      post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await
    }
  };
  let remote_of = |view: &Value| {
    let status = &view["branches"]["test-branch"]["remote"]["status"];
    (status["remoteExists"].clone(), status["remoteTip"].clone())
  };

  let pushed = json!({ "remote": { "test-branch": { "remoteExists": true, "remoteTip": "test-branch-pushed", "keepUnpushed": 1 } }, "issueLinks": links });
  let first = sync_with(pushed).await;
  assert_eq!(remote_of(&first), (json!(true), json!("test-branch-pushed")), "{first}");
  let unpushed = first["branches"]["test-branch"]["remote"]["status"]["unpushedCommits"]
    .as_array()
    .cloned()
    .unwrap_or_default();
  let copies = first["branches"]["test-branch"]["copies"].as_array().cloned().unwrap_or_default();
  assert_eq!(unpushed.len(), 1, "the newest of the two commits the template has: {first}");
  assert_eq!(unpushed[0], copies[0]["hash"], "the copy of the newest commit");
  assert_eq!(first["header"]["issueNavigationConfig"], links);

  let second = sync_with(json!({ "remote": { "test-branch": null } })).await;
  assert_eq!(remote_of(&second), (json!(true), json!("test-branch-pushed")), "no status reported: the one before stays");

  let third = sync_with(Value::Null).await;
  assert_eq!(remote_of(&third), (json!(false), Value::Null), "the template has no remote: {third}");

  let every = json!({ "remote": { "test-branch": { "remoteExists": true, "remoteTip": "test-branch-pushed" } }, "every": true });
  let fourth = sync_with(every).await;
  let fifth = sync_with(Value::Null).await;
  assert_eq!(remote_of(&fourth), (json!(true), json!("test-branch-pushed")), "{fourth}");
  assert_eq!(remote_of(&fifth), remote_of(&fourth), "the run after it takes them too: {fifth}");
  let replaced = sync_with(json!({})).await;
  assert_eq!(remote_of(&replaced), (json!(false), Value::Null), "others set: the server's own status again");
}

/// A ref of the repository answers the commit it is at; an unknown one is not found
#[tokio::test]
async fn a_revision_of_the_repository_answers_its_commit() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, _) = create_repository(&client, addr).await;
  let revision = |name: &str| client.get(format!("http://{addr}/repositories/{id}/revision?name={name}")).send();
  let head: Value = revision("HEAD").await.unwrap().json().await.unwrap();
  assert_eq!(head["id"].as_str().map(str::len), Some(40), "{head}");
  assert_eq!(revision("no-such-branch").await.unwrap().status(), 404);
}

/// An interrupted rebase defers the syncs of the session, its table as it was; once the rebase is gone it syncs
#[tokio::test]
async fn a_rebase_in_progress_defers_the_sync_until_it_is_gone() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let mut messages = subscribe(&client, addr, &id, &path, 10).await;
  subscription(&mut messages).await;
  let rebase = format!("/repositories/{id}/rebase-in-progress");

  post(&client, addr, &rebase, json!({ "inProgress": true })).await;
  let deferred = post_error(&client, addr, "/invoke/sync", json!({ "repositoryPath": path, "reason": "manual" })).await;
  assert_eq!(deferred["code"], "operation_in_progress", "{deferred}");
  post(&client, addr, &rebase, json!({ "inProgress": false })).await;
  let synced = post(&client, addr, "/invoke/sync", json!({ "repositoryPath": path, "reason": "manual" })).await;
  assert_eq!(synced["outcome"], "synced", "{synced}");
}

/// The patches on `messages` up to `version`, each on the one before, from `from`
async fn patches_up_to(messages: &mut Messages, from: &Value, version: &Value) -> Vec<Value> {
  let mut at = from.clone();
  let mut patches = Vec::new();
  while at != *version {
    let patch = messages.next().await["progress"].clone();
    assert_eq!(patch["base"], at, "each patch follows the one before");
    at = patch["next"].clone();
    patches.push(patch);
  }
  patches
}

/// A window on the session of a repository of `template`, synced once: the client, the path, the window's stream and
/// the version of its view
async fn synced_window(template: &str, channel: u32) -> (reqwest::Client, SocketAddr, String, Messages, Value) {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository_from(&client, addr, template).await;
  let mut messages = subscribe(&client, addr, &id, &path, channel).await;
  let subscribed = subscription(&mut messages).await;
  let synced = post(&client, addr, "/invoke/sync", json!({ "repositoryPath": path, "reason": "manual" })).await;
  patches_up_to(&mut messages, &subscribed["view"]["version"], &synced["version"]).await;
  (client, addr, path, messages, synced["version"].clone())
}

/// A push reaches the session as a patch of the remote status it left, whose version its answer names; no sync runs
/// for it. A push the page answered (`pushed-in-page`) does the same with the statuses of its answer
#[tokio::test]
async fn a_push_answers_the_version_of_its_patch_and_runs_no_sync() {
  let (client, addr, path, mut messages, synced) = synced_window("simple", 11).await;
  let push = json!({"repositoryPath": path, "branchName": "test-branch", "myEmail": null, "baselineBranch": "origin/master", "expectedRemote": null});
  let pushed = post(&client, addr, "/invoke/push_branch", push).await;
  let status = pushed["outcome"]["remoteStatus"].clone();
  assert!(status.is_object(), "{pushed}");
  patches_up_to(&mut messages, &synced, &pushed["version"]).await;
  let view = post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
  assert_eq!(view["branches"]["test-branch"]["remote"]["status"], status);

  let mut in_page = status.clone();
  in_page["remoteTip"] = json!("pushed-in-page");
  let request = json!({ "repositoryPath": path, "branches": [{ "branchName": "test-branch", "expectedRemote": null }], "remoteStatuses": [in_page] });
  let version = post(&client, addr, "/sessions/pushed-in-page", request).await;
  let patches = patches_up_to(&mut messages, &pushed["version"], &version).await;
  assert_eq!(patches.len(), 1, "{patches:?}");
  let view = post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
  assert_eq!(view["branches"]["test-branch"]["remote"]["status"]["remoteTip"], "pushed-in-page");
  assert_eq!((view["sync"].clone(), view["version"].clone()), (json!({ "kind": "idle" }), version), "no sync ran: {view}");
}

/// The deletion of an archived branch reaches the session as a patch of the archived section, whose version its answer
/// names; no sync runs for it
#[tokio::test]
async fn a_deletion_of_an_archived_branch_answers_the_version_of_its_patch_and_runs_no_sync() {
  let (client, addr, path, mut messages, synced) = synced_window("archived_branches", 12).await;
  let view = post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
  let archived = view["sections"]["archived"]["names"][0].clone();
  assert!(archived.is_string(), "{view}");

  let deleted = post(&client, addr, "/invoke/delete_archived_branch", json!({ "repositoryPath": path, "branchName": archived })).await;
  let patches = patches_up_to(&mut messages, &synced, &deleted).await;
  assert_eq!(patches.len(), 1, "{patches:?}");
  let names = patches[0]["changes"]["archived"]["names"].as_array().unwrap();
  assert!(!names.contains(&archived), "{patches:?}");
  let view = post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
  assert_eq!((view["sync"].clone(), view["version"].clone()), (json!({ "kind": "idle" }), deleted), "no sync ran: {view}");
}
