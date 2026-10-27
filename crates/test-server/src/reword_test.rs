use crate::test_support::{assert_stale_undo, create_repository, head, rewrite, rewrite_error, start_test_server};
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

/// The rewrite `reword` of `commit` to `message`, as the page sends it
fn reword_of(commit: &str, message: &str) -> serde_json::Value {
  json!({ "rewrite": "reword", "rewords": [{ "commit": commit, "text": { "message": message } }] })
}

/// The page's reword is the rewrite `reword` of the engine, run by the app's core: its outcome carries what the core
/// answered and the undo by the regroup's token, and the rewrite `undo` with it brings the old HEAD back; a message equal
/// after the cleanup rewrites nothing and has no undo
#[tokio::test]
async fn a_reword_answers_the_undo_that_brings_the_old_head_back() {
  let (addr, client, path, previous) = repository().await;

  let unchanged = rewrite(&client, addr, &path, reword_of(&previous, "(test-branch) foo 2\n")).await;
  assert_eq!(unchanged["result"]["answer"]["newCommitIds"][&previous], previous, "{unchanged}");
  assert!(unchanged["undo"].is_null(), "{unchanged}");
  assert_eq!(head(&path), previous);

  let answer = rewrite(&client, addr, &path, reword_of(&previous, "(test-branch) foo 2, reworded")).await;
  let new_head = head(&path);
  assert_ne!(new_head, previous);
  let reworded = &answer["result"]["answer"];
  assert_eq!(reworded["newCommitIds"][&previous], new_head, "{answer}");
  assert_eq!(
    (reworded["previousHead"].as_str(), reworded["newHead"].as_str()),
    (Some(previous.as_str()), Some(new_head.as_str()))
  );
  assert_eq!(answer["undo"], json!({"kind": "regroup", "token": format!("{previous}..{new_head}")}));

  let undone = rewrite(&client, addr, &path, json!({"rewrite": "undo", "token": answer["undo"]})).await;
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
  let answer = rewrite(&client, addr, &path, reword_of(&previous, "(test-branch) foo 2, reworded")).await;
  let moved = commit_on_top(&path, "(test-branch) after the reword");

  let error = rewrite_error(&client, addr, &path, json!({"rewrite": "undo", "token": answer["undo"]})).await;
  assert_stale_undo(&error, &path, &moved);
}
