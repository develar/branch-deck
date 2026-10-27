use crate::test_support::{assert_stale_undo, create_repository, head, post, post_error, rewrite, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::net::SocketAddr;
use test_utils::git_test_utils::git_command;

/// A server with a repository: its address, a client, the repository's path and its HEAD
async fn repository() -> (SocketAddr, reqwest::Client, String, String) {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (_, path) = create_repository(&client, addr).await;
  let previous = head(&path);
  (addr, client, path, previous)
}

/// An empty commit with `message` on top of the branch at `path`, made as a terminal makes it: its id. Here, not in
/// `test_support`, which the worktree safety scan reads as production code
pub(crate) fn commit_on_top(path: &str, message: &str) -> String {
  let commit = git_command().args(["commit", "--allow-empty", "-m", message]).current_dir(path).output().unwrap();
  assert!(commit.status.success(), "{}", String::from_utf8_lossy(&commit.stderr));
  head(path)
}

/// The rewrite `Undo` of the regroup token `token`
fn undo_of(token: &str) -> serde_json::Value {
  json!({"rewrite": "undo", "token": {"kind": "regroup", "token": token}})
}

/// The body of a reword of `commit` to `message`
fn reword_of(path: &str, commit: &str, message: &str) -> serde_json::Value {
  json!({ "repositoryPath": path, "rewords": [{ "commit": commit, "text": { "message": message } }] })
}

/// The page's `reword` reaches the app's core: the answer carries the undo token, and the rewrite `Undo` of the engine
/// with it brings the old HEAD back; a message equal after the cleanup answers no token
#[tokio::test]
async fn a_reword_answers_the_undo_token_that_the_rewrite_undo_takes() {
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

  let undone = rewrite(&client, addr, &path, undo_of(token)).await;
  assert_eq!(
    undone["result"],
    json!({"kind": "rewrite", "rewrite": "undone", "answer": {"alreadyUndone": false}}),
    "{undone}"
  );
  assert_eq!(head(&path), previous);
}

/// An undo whose branch moved on since the rewrite is stale for good: the rewrite `Undo` answers `undo_stale` with
/// `retry: no`, as `bd undo` does (`ErrorCode::in_undo`), never `head_moved`, which tells the page that a retry may
/// succeed; its words say it can't be undone, not that a retry may help
#[tokio::test]
async fn the_undo_of_a_regroup_after_a_new_commit_answers_undo_stale_never_retried() {
  let (addr, client, path, previous) = repository().await;
  let answer = post(&client, addr, "/invoke/reword", reword_of(&path, &previous, "(test-branch) foo 2, reworded")).await;
  let token = answer["undoToken"].as_str().unwrap().to_string();
  let moved = commit_on_top(&path, "(test-branch) after the reword");

  let mut intent = undo_of(&token);
  intent["kind"] = json!("rewrite");
  let error = post_error(&client, addr, "/invoke/execute", json!({ "repositoryPath": path, "intent": intent, "fingerprint": "" })).await;
  assert_stale_undo(&error, &path, &moved);
}
