use crate::test_support::{create_repository, post, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::net::SocketAddr;
use test_utils::git_test_utils::git_command;

fn head(path: &str) -> String {
  let output = git_command().args(["rev-parse", "HEAD"]).current_dir(path).output().unwrap();
  String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// A server with a repository: its address, a client, the repository's path and its HEAD
async fn repository() -> (SocketAddr, reqwest::Client, String, String) {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (_, path) = create_repository(&client, addr).await;
  let previous = head(&path);
  (addr, client, path, previous)
}

/// The body of a reword of `commit` to `message`
fn reword_of(path: &str, commit: &str, message: &str) -> serde_json::Value {
  json!({ "repositoryPath": path, "rewords": [{ "commit": commit, "text": { "message": message } }] })
}

/// The page's `reword` reaches the app's core: the answer carries the undo token, and `undo_regroup` with it brings the
/// old HEAD back; a message equal after the cleanup answers no token
#[tokio::test]
async fn a_reword_answers_the_undo_token_that_undo_regroup_takes() {
  let (addr, client, path, previous) = repository().await;

  let reword = |message: &str| reword_of(&path, &previous, message);
  let unchanged = post(&client, addr, "/invoke/reword", reword("(test-branch) foo 2\n")).await;
  assert_eq!(unchanged["newCommitIds"][&previous], previous, "{unchanged}");
  assert!(unchanged["undoToken"].is_null(), "{unchanged}");

  let answer = post(&client, addr, "/invoke/reword", reword("(test-branch) foo 2, reworded")).await;
  let new_head = head(&path);
  assert_ne!(new_head, previous);
  assert_eq!(answer["newCommitIds"][&previous], new_head, "{answer}");
  assert_eq!(
    (answer["previousHead"].as_str(), answer["newHead"].as_str()),
    (Some(previous.as_str()), Some(new_head.as_str()))
  );
  let token = answer["undoToken"].as_str().unwrap();
  assert_eq!(token, format!("{previous}..{new_head}"));

  post(&client, addr, "/invoke/undo_regroup", json!({ "repositoryPath": path, "token": token })).await;
  assert_eq!(head(&path), previous);
}

/// An undo whose branch moved on since the rewrite is stale for good: `undo_regroup` answers `undo_stale` with `retry:
/// no`, as `bd undo` does (`ErrorCode::in_undo`), never `head_moved`, which tells the page that a retry may succeed
#[tokio::test]
async fn undo_regroup_after_a_new_commit_answers_undo_stale_never_retried() {
  let (addr, client, path, previous) = repository().await;
  let answer = post(&client, addr, "/invoke/reword", reword_of(&path, &previous, "(test-branch) foo 2, reworded")).await;
  let token = answer["undoToken"].as_str().unwrap().to_string();
  let commit = git_command()
    .args(["commit", "--allow-empty", "-m", "(test-branch) after the reword"])
    .current_dir(&path)
    .output()
    .unwrap();
  assert!(commit.status.success(), "{}", String::from_utf8_lossy(&commit.stderr));
  let moved = head(&path);

  let response = client
    .post(format!("http://{addr}/invoke/undo_regroup"))
    .json(&json!({ "repositoryPath": path, "token": token }))
    .send()
    .await
    .unwrap();
  let answer: serde_json::Value = response.json().await.unwrap();
  assert_eq!(answer["status"], "error", "{answer}");
  let error = &answer["error"];
  assert_eq!((&error["code"], &error["retry"]), (&json!("undo_stale"), &json!("no")), "{error}");
  assert_eq!(head(&path), moved);
}
