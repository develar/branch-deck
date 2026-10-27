use crate::reword_test::commit_on_top;
use crate::test_support::{assert_stale_undo, create_repository, head, rewrite, rewrite_error, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::json;
use test_utils::git_test_utils::git_command;

/// The undo of a squash (and of squash fixups) whose branch moved on since is stale for good, as every undo of the app:
/// the rewrite `undo` of its `squash` token answers `undo_stale` with `retry: no` (`ErrorCode::in_undo` of the typed
/// `HeadMoved`), never `internal`, in words that say it can't be undone, not that a retry may help, and moves no branch
#[tokio::test]
async fn the_undo_of_a_squash_after_a_new_commit_answers_undo_stale_never_retried() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (_, path) = create_repository(&client, addr).await;
  let output = git_command().args(["rev-parse", "HEAD~1"]).current_dir(&path).output().unwrap();
  let parent = String::from_utf8(output.stdout).unwrap().trim().to_string();
  let previous = head(&path);
  let squash = json!({ "rewrite": "squash", "commitIds": [parent, previous], "message": "(test-branch) foo, squashed" });
  let squashed = rewrite(&client, addr, &path, squash).await;
  let new_head = head(&path);
  assert_eq!(squashed["result"]["answer"]["newHead"].as_str(), Some(new_head.as_str()), "{squashed}");
  assert_eq!(squashed["undo"], json!({"kind": "squash", "token": format!("{previous}..{new_head}")}));
  let moved = commit_on_top(&path, "(test-branch) after the squash");

  let error = rewrite_error(&client, addr, &path, json!({"rewrite": "undo", "token": squashed["undo"]})).await;
  assert_stale_undo(&error, &path, &moved);
}
