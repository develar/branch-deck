use crate::test_support::{create_repository, start_test_server};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;

async fn invoke(client: &reqwest::Client, addr: SocketAddr, command: &str, params: Value) -> Value {
  let response = client.post(format!("http://{addr}/invoke/{command}")).json(&params).send().await.unwrap();
  assert_eq!(response.status(), 200, "{command}");
  response.json().await.unwrap()
}

/// The routes take the params of the Tauri commands and answer in their shape, on a fixture repository
#[tokio::test]
async fn commit_files_routes_read_the_fixture_repository() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (_, repository_path) = create_repository(&client, addr).await;
  let ids = GitCommandExecutor::new().execute_command_lines(&["rev-list", "HEAD"], &repository_path).unwrap();
  assert_eq!(ids.len(), 2);

  let all = invoke(&client, addr, "get_commits_files", json!({ "repositoryPath": repository_path, "commitIds": ids })).await;
  // newest first, as asked; the root commit adds its file too
  assert_eq!(
    all,
    json!([
      { "commitId": ids[0], "files": [{ "path": "file2.txt", "status": "added", "oldPath": null, "additions": 1, "deletions": 0, "binary": false }], "additions": 1, "deletions": 0 },
      { "commitId": ids[1], "files": [{ "path": "file1.txt", "status": "added", "oldPath": null, "additions": 1, "deletions": 0, "binary": false }], "additions": 1, "deletions": 0 },
    ])
  );

  let one = invoke(&client, addr, "get_commit_files", json!({ "repositoryPath": repository_path, "commitId": ids[1] })).await;
  assert_eq!(one, all[1]);

  let patch = invoke(&client, addr, "get_commit_patch", json!({ "repositoryPath": repository_path, "commitId": ids[1] })).await;
  assert_eq!(patch["commitId"], ids[1]);
  let section = patch["files"][0]["patch"].as_str().unwrap();
  assert!(section.starts_with("diff --git a/file1.txt b/file1.txt\n"), "{section}");
  assert!(section.contains("+Content 1"), "{section}");
  assert_eq!(patch["files"][0]["omittedLines"], Value::Null);

  // the blob the patch names on its index line
  let blob_id = section
    .lines()
    .find_map(|line| line.strip_prefix("index "))
    .and_then(|range| range.split("..").nth(1))
    .unwrap();
  let blob_id = blob_id.split(' ').next().unwrap();
  let blobs = invoke(&client, addr, "get_blob_contents", json!({ "repositoryPath": repository_path, "objectIds": [blob_id] })).await;
  assert_eq!(blobs, json!([{ "kind": "text", "objectId": blob_id, "content": "Content 1" }]));
}
