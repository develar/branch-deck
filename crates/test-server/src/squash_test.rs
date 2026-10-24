use crate::reword_test::commit_on_top;
use crate::test_support::{create_repository, head, post, post_error, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::json;
use test_utils::git_test_utils::git_command;

/// The undo of a squash (and of squash fixups) whose branch moved on since is stale for good, as every undo of the app:
/// `restore_branch_head` answers `undo_stale` with `retry: no` (`ErrorCode::in_undo` of the typed `HeadMoved`), never
/// `internal`, and moves no branch
#[tokio::test]
async fn restore_branch_head_after_a_new_commit_answers_undo_stale_never_retried() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (_, path) = create_repository(&client, addr).await;
  let output = git_command().args(["rev-parse", "HEAD~1"]).current_dir(&path).output().unwrap();
  let parent = String::from_utf8(output.stdout).unwrap().trim().to_string();
  let commit_ids = json!([parent, head(&path)]);
  let squashed = post(
    &client,
    addr,
    "/invoke/squash_commits",
    json!({ "repositoryPath": path, "commitIds": commit_ids, "message": "(test-branch) foo, squashed" }),
  )
  .await;
  assert_eq!(squashed["newHead"].as_str(), Some(head(&path).as_str()), "{squashed}");
  let moved = commit_on_top(&path, "(test-branch) after the squash");

  let undo = json!({ "repositoryPath": path, "expectedHead": squashed["newHead"], "previousHead": squashed["previousHead"] });
  let error = post_error(&client, addr, "/invoke/restore_branch_head", undo).await;
  assert_eq!((&error["code"], &error["retry"]), (&json!("undo_stale"), &json!("no")), "{error}");
  assert_eq!(head(&path), moved);
}
