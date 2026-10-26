//! The remote of a test repository as the E2E tests use it: the app's push reaches it, a held push waits there, a
//! refused one moves nothing, and what a test does to it behind the app's back is what the next push finds

use crate::remote_host::HeldAnswer;
use crate::test_support::{create_repository, post, push_alone, start_test_server_with_state, sync};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;

/// The virtual branch of the `simple` template
const BRANCH: &str = "user-name/virtual/test-branch";

/// The app's push of the branch of the `simple` template alone, as the row saw its remote (`expected`): its outcome
async fn push_of(client: &reqwest::Client, addr: SocketAddr, path: &str, expected: Option<&str>) -> Value {
  push_alone(client, addr, path, "test-branch", expected).await
}

/// The status of the push effect of `outcome`, and the code of its error when it failed
fn push_status(outcome: &Value) -> (Value, Value) {
  let status = &outcome["effects"][0]["status"];
  assert_eq!(outcome["effects"][0]["effect"]["kind"], "push", "{outcome}");
  (status["kind"].clone(), status["error"]["code"].clone())
}

async fn remote_branches(client: &reqwest::Client, addr: SocketAddr, id: &str) -> Value {
  remote(client, addr, id).await["branches"].clone()
}

/// What `GET /repositories/{id}/remote` answers: the branches of the remote and the pushes it took from the app
async fn remote(client: &reqwest::Client, addr: SocketAddr, id: &str) -> Value {
  client.get(format!("http://{addr}/repositories/{id}/remote")).send().await.unwrap().json().await.unwrap()
}

async fn revision(client: &reqwest::Client, addr: SocketAddr, id: &str, name: &str) -> String {
  let answer: Value = client
    .get(format!("http://{addr}/repositories/{id}/revision?name={name}"))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
  answer["id"].as_str().unwrap().to_string()
}

/// A push of the app goes to the remote of the template, its `master` there as `origin/master`: the remote branch and
/// the remote-tracking ref are at the pushed commit, which the remote status names
#[tokio::test]
async fn a_push_of_the_app_reaches_the_remote_of_the_template() {
  let (addr, _) = start_test_server_with_state().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let baseline = revision(&client, addr, &id, "origin/master").await;
  assert_eq!(remote_branches(&client, addr, &id).await, json!({ "master": baseline }));
  sync(&client, addr, &path).await;

  let pushed = push_of(&client, addr, &path, None).await;
  let tip = revision(&client, addr, &id, BRANCH).await;
  assert_eq!(push_status(&pushed), (json!("done"), Value::Null), "{pushed}");
  let view = post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
  assert_eq!(view["branches"]["test-branch"]["remote"]["status"]["remoteTip"], json!(tip), "{view}");
  assert_eq!(
    remote(&client, addr, &id).await,
    json!({ "branches": { "master": baseline, BRANCH: tip }, "pushes": [{ BRANCH: tip }] })
  );
  assert_eq!(revision(&client, addr, &id, &format!("origin/{BRANCH}")).await, tip);
}

/// A push the remote refuses moves nothing, and its error says what the remote said; a held one has reached the remote
/// and moved no ref until the release, then it pushes
#[tokio::test]
async fn a_refused_push_moves_nothing_and_a_held_one_waits_at_the_remote() {
  let (addr, state) = start_test_server_with_state().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  sync(&client, addr, &path).await;
  let refusal = format!("/repositories/{id}/remote/refusal");
  post(&client, addr, &refusal, json!({ "message": "Pushes are closed for the release" })).await;
  let refused = push_of(&client, addr, &path, None).await;
  assert_eq!(push_status(&refused), (json!("failed"), json!("push_refused")), "{refused}");
  let message = refused["effects"][0]["status"]["error"]["message"].as_str().unwrap();
  assert!(
    message.contains("The remote refused the push: ") && message.contains("remote: Pushes are closed for the release"),
    "{message}"
  );
  assert_eq!(remote_branches(&client, addr, &id).await.get(BRANCH), None, "nothing was pushed");
  post(&client, addr, &refusal, json!({ "message": null })).await;
  let gates = format!("/repositories/{id}/remote-gates");
  let failed = client.post(format!("http://{addr}{gates}")).json(&json!({ "fail": ["push"] })).send().await.unwrap();
  assert_eq!(failed.status(), 400, "a push fails as the remote refuses it");

  post(&client, addr, &gates, json!({ "hold": ["push"] })).await;
  let push = tokio::spawn({
    let (client, path) = (client.clone(), path.clone());
    async move { push_of(&client, addr, &path, None).await }
  });
  let host = state.repositories.get(&id).unwrap().existing_remote().unwrap();
  host.arrival(HeldAnswer::Push).await;
  let arrived: Value = client.get(format!("http://{addr}{gates}")).send().await.unwrap().json().await.unwrap();
  assert_eq!(arrived["arrived"], json!(["push"]));
  assert_eq!(remote_branches(&client, addr, &id).await.get(BRANCH), None, "no ref moved while held");
  post(&client, addr, &gates, json!({ "release": ["push"] })).await;
  assert_eq!(push_status(&push.await.unwrap()), (json!("done"), Value::Null));
  let tip = revision(&client, addr, &id, BRANCH).await;
  assert_eq!(remote_branches(&client, addr, &id).await[BRANCH], json!(tip));
}

/// Someone else's commit on the remote branch moves only the remote: the next push of the app finds it moved, with the
/// commit it would drop, or a fetch in a terminal brings it; a push in a terminal moves the remote branch and the
/// remote-tracking ref, and an earlier build of the same commits leaves nothing to push. The remote logs only the
/// push of the app it took
#[tokio::test]
async fn a_commit_on_the_remote_is_what_the_next_push_finds() {
  let (addr, _) = start_test_server_with_state().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  sync(&client, addr, &path).await;
  push_of(&client, addr, &path, None).await;
  let pushed_tip = revision(&client, addr, &id, BRANCH).await;
  // a commit of the branch the remote lacks: the next push has work
  std::fs::write(std::path::Path::new(&path).join("file3.txt"), "Content 3").unwrap();
  for args in [&["add", "file3.txt"][..], &["commit", "-q", "-m", "(test-branch) foo 3"][..]] {
    assert!(test_utils::git_test_utils::git_command().args(args).current_dir(&path).status().unwrap().success());
  }
  sync(&client, addr, &path).await;
  let tip = revision(&client, addr, &id, BRANCH).await;

  let commit = json!({ "branch": BRANCH, "subject": "Fix the pool", "author": "Ann <ann@example.com>", "timestamp": 1_704_178_800, "files": [["pool.txt", "pool\n"]] });
  let theirs = post(&client, addr, &format!("/repositories/{id}/remote/commit"), commit.clone()).await["commitId"].clone();
  assert_eq!(remote_branches(&client, addr, &id).await[BRANCH], theirs);
  assert_eq!(revision(&client, addr, &id, &format!("origin/{BRANCH}")).await, pushed_tip, "no fetch yet");

  let moved = push_of(&client, addr, &path, Some(&pushed_tip)).await;
  assert_eq!(push_status(&moved), (json!("failed"), json!("lease_rejected")), "{moved}");
  // the session has the remote branch as the fetch after the refusal found it
  let view = post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
  let status = &view["branches"]["test-branch"]["remote"]["status"];
  assert_eq!(
    (status["remoteTip"].clone(), status["droppedCommits"][0]["subject"].clone()),
    (theirs.clone(), json!("Fix the pool")),
    "{status}"
  );

  let pushed = post(&client, addr, &format!("/repositories/{id}/remote/push"), json!({ "branch": BRANCH, "revision": BRANCH })).await;
  assert_eq!(pushed["commitId"], json!(tip));
  assert_eq!(remote_branches(&client, addr, &id).await[BRANCH], json!(tip));
  assert_eq!(revision(&client, addr, &id, &format!("origin/{BRANCH}")).await, tip);

  // an earlier build of the same commits: other ids, nothing to push by content
  let earlier = post(
    &client,
    addr,
    &format!("/repositories/{id}/remote/push"),
    json!({ "branch": BRANCH, "revision": BRANCH, "earlierBuild": true }),
  )
  .await;
  let earlier = earlier["commitId"].as_str().unwrap().to_string();
  assert_ne!(earlier, tip);
  sync(&client, addr, &path).await;
  let view = post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
  let status = &view["branches"]["test-branch"]["remote"]["status"];
  assert_eq!((status["remoteTip"].clone(), status["unpushedCommits"].clone()), (json!(earlier), json!([])), "{status}");

  let theirs = post(&client, addr, &format!("/repositories/{id}/remote/commit"), commit).await["commitId"].clone();
  post(&client, addr, &format!("/repositories/{id}/remote/fetch"), json!({})).await;
  assert_eq!(json!(revision(&client, addr, &id, &format!("origin/{BRANCH}")).await), theirs);
  assert_eq!(remote(&client, addr, &id).await["pushes"], json!([{ BRANCH: pushed_tip }]));
}
