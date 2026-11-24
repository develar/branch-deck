//! The pages of the app over the in-process connection, on a host of the fixtures (`page.rs`;
//! openspec/specs/repository/backend-protocol/spec.md, "Pages: …")

use crate::corpus_test::{Fixture, head};
use crate::page::{Answer, Failed, PageChannel, Pages, Reply};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use test_utils::git_test_utils::git_command;
use tokio::sync::mpsc;

const WAIT: Duration = Duration::from_secs(20);

/// The channel of a page: each message it got, as JSON
struct Messages(mpsc::UnboundedSender<Value>);

impl PageChannel for Messages {
  fn send(&self, json: String) -> bool {
    self.0.send(serde_json::from_str(&json).expect("a message of a page is JSON")).is_ok()
  }
}

fn channel() -> (Arc<dyn PageChannel>, mpsc::UnboundedReceiver<Value>) {
  let (sender, receiver) = mpsc::unbounded_channel();
  (Arc::new(Messages(sender)), receiver)
}

/// The JSON of a reply that answered JSON
fn json_of(reply: Reply) -> Value {
  match reply {
    Ok(Answer::Json(json)) => serde_json::from_str(&json).unwrap(),
    other => panic!("not a JSON answer: {other:?}"),
  }
}

/// The `ApiError` of a reply that failed
fn error_of(reply: Reply) -> Value {
  match reply {
    Err(Failed::Error(json)) => serde_json::from_str(&json).unwrap(),
    other => panic!("not an error: {other:?}"),
  }
}

async fn next(messages: &mut mpsc::UnboundedReceiver<Value>) -> Value {
  tokio::time::timeout(WAIT, messages.recv()).await.expect("no message came").expect("the channel ended")
}

/// The patches of `messages` until one brings the view to `version`
async fn patched_to(messages: &mut mpsc::UnboundedReceiver<Value>, version: &Value) -> Vec<Value> {
  let mut patches = Vec::new();
  loop {
    let patch = next(messages).await;
    let reached = patch["next"] == *version;
    patches.push(patch);
    if reached {
      return patches;
    }
  }
}

fn sync_args(repo: &str) -> Value {
  json!({ "params": { "repositoryPath": repo, "reason": "manual" } })
}

/// A command of a page answers the JSON of its value, and a failed one its `ApiError`, as Tauri resolves and rejects the
/// `invoke`
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_command_of_a_page_answers_its_value_or_its_error() {
  let fixture = Fixture::new().await;
  let pages = Pages::new(&fixture.host);
  let args = json!({ "params": { "repositoryPath": fixture.repo, "commitId": head(&fixture.repo) } });
  let files = json_of(pages.invoke("main", "get_commit_files", &args, None).await);
  assert_eq!(files["files"][0]["path"], "file2.txt", "{files:#}");

  let missing = error_of(pages.invoke("main", "get_commit_files", &json!({}), None).await);
  assert_eq!(missing["code"], "internal", "{missing:#}");

  let git_dir = std::path::Path::new(&fixture.repo).join(".git");
  std::fs::create_dir_all(git_dir.join("rebase-merge")).unwrap();
  std::fs::write(git_dir.join("rebase-merge").join("head-name"), "refs/heads/master\n").unwrap();
  let refused = error_of(pages.invoke("main", "sync", &sync_args(&fixture.repo), None).await);
  assert_eq!(refused["code"], "operation_in_progress", "{refused:#}");
}

/// A raw command answers its bytes as they are
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_raw_command_of_a_page_answers_its_bytes() {
  let fixture = Fixture::new().await;
  let pages = Pages::new(&fixture.host);
  let output = git_command().args(["rev-parse", "HEAD:file1.txt"]).current_dir(&fixture.repo).output().unwrap();
  let blob = String::from_utf8(output.stdout).unwrap().trim().to_owned();
  let args = json!({ "params": { "repositoryPath": fixture.repo, "blob": { "id": blob, "path": null }, "filePath": "file1.txt", "fromLine": 0, "toLine": 1, "text": null } });
  match pages.invoke("main", "get_blob_syntax", &args, None).await {
    Ok(Answer::Bytes(bytes)) => assert_ne!(bytes, Vec::<u8>::new()),
    other => panic!("not bytes: {other:?}"),
  }
}

/// The events of a command reach the channel the page gave it, each before the answer
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_events_of_a_command_reach_its_channel_before_its_answer() {
  let fixture = Fixture::new().await;
  let pages = Pages::new(&fixture.host);
  let (events, mut received) = channel();
  let commit = head(&fixture.repo);
  let args = json!({ "params": { "repositoryPath": fixture.repo, "source": { "kind": "commit", "id": commit }, "policy": "default", "priorityPaths": [] } });
  let answer = pages.invoke("review-1", "get_file_diffs", &args, Some(events)).await;
  assert!(answer.is_ok(), "{answer:?}");
  let first = received.try_recv().expect("the events came before the answer");
  assert_eq!(first["kind"], "files", "{first:#}");
}

/// `subscribe_view` answers the view of the session with the id of the subscription, the patches of a sync reach its
/// channel, and `unsubscribe_view` ends it
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_subscription_of_a_page_answers_its_view_and_its_patches_reach_its_channel() {
  let fixture = Fixture::new().await;
  let pages = Pages::new(&fixture.host);
  let (patches, mut received) = channel();
  let args = json!({ "params": { "repositoryPath": fixture.repo } });
  let subscription = json_of(pages.invoke("main", "subscribe_view", &args, Some(patches)).await);
  assert!(subscription["view"]["version"].is_object(), "{subscription:#}");
  assert_eq!(subscription["catchUp"], Value::Null);

  let synced = json_of(pages.invoke("main", "sync", &sync_args(&fixture.repo), None).await);
  // the patches up to the version of the answer came before it (the fence); a patch of a read of a fact may come after
  // it, so the sync's is any of them, not the last
  let mut came = Vec::new();
  while let Ok(patch) = received.try_recv() {
    came.push(patch);
  }
  assert!(came.iter().any(|patch| patch["next"] == synced["version"]), "no patch to {}: {came:#?}", synced["version"]);

  let demand = json!({ "focused": true, "online": true, "visibleBranches": [], "visibleStacks": [], "needed": [], "robotDetails": [], "watchedRobots": [] });
  let set = pages.invoke("main", "set_demand", &json!({ "params": { "subscription": subscription["id"], "demand": demand } }), None);
  assert_eq!(json_of(set.await), Value::Null);
  let ended = pages.invoke("main", "unsubscribe_view", &json!({ "params": { "id": subscription["id"] } }), None);
  assert_eq!(json_of(ended.await), Value::Null);
  // a patch of a read of a fact may still come; none of the sync after the end, which a new commit moves
  let committed = git_command()
    .args(["commit", "--allow-empty", "-m", "(test-branch) after the end"])
    .current_dir(&fixture.repo)
    .output()
    .unwrap();
  assert!(committed.status.success());
  let after = json_of(pages.invoke("main", "sync", &sync_args(&fixture.repo), None).await);
  let mut late = Vec::new();
  while let Ok(patch) = received.try_recv() {
    late.push(patch);
  }
  assert_ne!(after["version"], synced["version"], "the commit moved the view");
  assert!(late.iter().all(|patch| patch["next"] != after["version"]), "a patch of the sync after the end: {late:#?}");
}

/// A page subscribes again before it ends the subscription it replaces: the window holds the new one, which answers
/// from the version the page holds, and the unsubscribe of the replaced id ends nothing
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_subscription_again_of_a_window_replaces_the_one_it_holds() {
  let fixture = Fixture::new().await;
  let _kept = fixture.settled().await;
  let pages = Pages::new(&fixture.host);
  let (first, _first_patches) = channel();
  let args = json!({ "params": { "repositoryPath": fixture.repo } });
  let replaced = json_of(pages.invoke("main", "subscribe_view", &args, Some(first)).await);
  let version = replaced["view"]["version"].clone();

  let (second, mut patches) = channel();
  let again = json!({ "params": { "repositoryPath": fixture.repo, "have": version } });
  let subscription = json_of(pages.invoke("main", "subscribe_view", &again, Some(second)).await);
  assert_ne!(subscription["id"], replaced["id"]);
  assert_eq!(subscription["view"], Value::Null, "the page holds the view of the session: {subscription:#}");
  let ended = pages.invoke("main", "unsubscribe_view", &json!({ "params": { "id": replaced["id"] } }), None);
  assert_eq!(json_of(ended.await), Value::Null);

  let synced = json_of(pages.invoke("main", "sync", &sync_args(&fixture.repo), None).await);
  let came = patched_to(&mut patches, &synced["version"]).await;
  assert_eq!(came[0]["base"], version, "{came:#?}");
}

/// The subscription of a window that subscribed with the seed of the warm start answers no view, and the catch-up from
/// the seed reaches its channel
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_seed_of_a_page_gets_its_catch_up_on_the_channel() {
  let fixture = Fixture::new().await;
  let seed = fixture.warm().await;
  let pages = Pages::new(&fixture.host);
  let (patches, mut received) = channel();
  let args = json!({ "params": { "repositoryPath": fixture.repo, "have": seed["version"] } });
  let subscription = json_of(pages.invoke("main", "subscribe_view", &args, Some(patches)).await);
  assert_eq!(subscription["view"], Value::Null, "{subscription:#}");
  let synced = json_of(pages.invoke("main", "sync", &sync_args(&fixture.repo), None).await);
  let came = patched_to(&mut received, &synced["version"]).await;
  assert_eq!(came[0]["base"], seed["version"], "the first patch is the one from the seed: {came:#?}");
}

/// The channel of a window moves to the session of another prefix when another client subscribes after the prefix
/// changed: the page gets a patch from and to the version of the new view, which no view applies, so it reads the
/// snapshot of the session
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_resync_reaches_the_page_as_a_patch_no_view_applies() {
  let fixture = Fixture::new().await;
  let pages = Pages::new(&fixture.host);
  let (patches, mut received) = channel();
  let subscription = json_of(
    pages
      .invoke("main", "subscribe_view", &json!({ "params": { "repositoryPath": fixture.repo } }), Some(patches))
      .await,
  );
  let before = subscription["view"]["version"].clone();
  let output = git_command()
    .args(["config", "branchdeck.branchPrefix", "other"])
    .current_dir(&fixture.repo)
    .output()
    .unwrap();
  assert!(output.status.success());

  let ide = fixture.bridge().await;
  let uri = ide
    .ask(&json!({ "jsonrpc": "2.0", "id": 1, "method": "x-branch-deck/repositoryChannel", "params": { "channel": "ahp-root://", "repositoryPath": fixture.repo } }))
    .await["result"]["channel"]
    .clone();
  let subscribed = ide
    .ask(&json!({ "jsonrpc": "2.0", "id": 2, "method": "subscribe", "params": { "channel": uri, "_meta": { "x-branch-deck": { "window": "main" } } } }))
    .await;
  let other = subscribed["result"]["snapshot"]["state"]["version"].clone();
  assert_ne!(other["epoch"], before["epoch"], "{subscribed:#}");

  let resync = loop {
    let patch = next(&mut received).await;
    if patch["next"] == other {
      break patch;
    }
  };
  assert_eq!(resync, json!({ "base": other, "next": other, "changes": {} }));
}

/// A page subscribes its window again while its first subscribe still joins: the second holds the window, the first,
/// once joined, leaves the session, and the patches of a sync reach the second alone
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_subscription_again_while_the_first_joins_holds_the_window() {
  let fixture = Fixture::new().await;
  let pages = Pages::new(&fixture.host);
  let args = json!({ "params": { "repositoryPath": fixture.repo } });
  let (first, mut first_patches) = channel();
  let (second, mut second_patches) = channel();
  let joining = pages.invoke("main", "subscribe_view", &args, Some(first));
  let again = pages.invoke("main", "subscribe_view", &args, Some(second));
  let (replaced, subscription) = tokio::join!(joining, again);
  let (replaced, subscription) = (json_of(replaced), json_of(subscription));
  assert_ne!(replaced["id"], subscription["id"]);

  let synced = json_of(pages.invoke("main", "sync", &sync_args(&fixture.repo), None).await);
  let came = patched_to(&mut second_patches, &synced["version"]).await;
  assert_ne!(came, Vec::<Value>::new());
  let mut first_got = Vec::new();
  while let Ok(patch) = first_patches.try_recv() {
    first_got.push(patch);
  }
  assert!(
    first_got.iter().all(|patch| patch["next"] != synced["version"]),
    "the replaced subscription heard the sync: {first_got:#?}"
  );
}
