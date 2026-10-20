use crate::test_support::{create_repository, post, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::json;
use test_utils::git_test_utils::git_command;

fn head(path: &str) -> String {
  let output = git_command().args(["rev-parse", "HEAD"]).current_dir(path).output().unwrap();
  String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// The page's `reword` reaches the app's core: the answer carries the undo token, and `undo_regroup` with it brings the
/// old HEAD back; a message equal after the cleanup answers no token
#[tokio::test]
async fn a_reword_answers_the_undo_token_that_undo_regroup_takes() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (_, path) = create_repository(&client, addr).await;
  let previous = head(&path);

  let reword = |message: &str| json!({ "repositoryPath": path, "rewords": [{ "commit": previous, "text": { "message": message } }] });
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
