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

/// The SSE events of a route that stands for a command with a channel
async fn stream(client: &reqwest::Client, addr: SocketAddr, command: &str, params: Value) -> Vec<Value> {
  let response = client.post(format!("http://{addr}/invoke/{command}")).json(&params).send().await.unwrap();
  assert_eq!(response.status(), 200, "{command}");
  let body = response.text().await.unwrap();
  body
    .lines()
    .filter_map(|line| line.strip_prefix("data: "))
    .map(|data| serde_json::from_str(data).unwrap())
    .collect()
}

/// The routes take the params of the Tauri commands and answer in their shape, on a fixture repository
#[tokio::test]
async fn commit_files_routes_read_the_fixture_repository() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (_, repository_path) = create_repository(&client, addr).await;
  let ids = GitCommandExecutor::new().execute_command_lines(&["rev-list", "HEAD"], &repository_path).unwrap();
  assert_eq!(ids.len(), 2);

  let git = GitCommandExecutor::new();
  let blob = |commit: &str, path: &str| git.execute_command(&["rev-parse", &format!("{commit}:{path}")], &repository_path).unwrap();
  let (blob2, blob1) = (blob(&ids[0], "file2.txt"), blob(&ids[1], "file1.txt"));
  let all = invoke(&client, addr, "get_commits_files", json!({ "repositoryPath": repository_path, "commitIds": ids })).await;
  // newest first, as asked; the root commit adds its file too
  assert_eq!(
    all,
    json!([
      { "commitId": ids[0], "files": [{ "path": "file2.txt", "status": "added", "oldPath": null, "additions": 1, "deletions": 0, "binary": false, "oldId": null, "newId": blob2, "submodule": false }], "additions": 1, "deletions": 0 },
      { "commitId": ids[1], "files": [{ "path": "file1.txt", "status": "added", "oldPath": null, "additions": 1, "deletions": 0, "binary": false, "oldId": null, "newId": blob1, "submodule": false }], "additions": 1, "deletions": 0 },
    ])
  );

  let one = invoke(&client, addr, "get_commit_files", json!({ "repositoryPath": repository_path, "commitId": ids[1] })).await;
  assert_eq!(one, all[1]);

  // the typed diffs, and the texts by blob id
  let source = json!({ "kind": "commit", "id": ids[1] });
  let params = json!({ "repositoryPath": repository_path, "source": source, "policy": "default", "priorityPaths": [] });
  let events = stream(&client, addr, "get_file_diffs", params).await;
  assert_eq!(events[0]["kind"], "files");
  assert_eq!(
    (events[0]["files"][0]["path"].as_str(), events[0]["files"][0]["newId"].as_str()),
    (Some("file1.txt"), Some(blob1.as_str()))
  );
  assert_eq!((events[1]["kind"].as_str(), events[1]["index"].as_u64()), (Some("content"), Some(0)));
  assert_eq!((events[1]["content"]["kind"].as_str(), events[1]["content"]["additions"].as_u64()), (Some("text"), Some(1)));

  // the merges of a cherry-pick: this one merges cleanly, so it lists no file
  let source = json!({ "kind": "cherryPick", "parent": ids[1], "target": ids[0], "commit": ids[0] });
  let params = json!({ "repositoryPath": repository_path, "source": source, "policy": "default", "priorityPaths": [] });
  let events = stream(&client, addr, "get_file_merges", params).await;
  assert_eq!(events, [json!({ "kind": "files", "files": [] })]);
  let texts = invoke(
    &client,
    addr,
    "get_blob_texts",
    json!({ "repositoryPath": repository_path, "blobs": [{ "id": blob1, "path": null }] }),
  )
  .await;
  assert_eq!(texts, json!([{ "kind": "text", "id": blob1, "text": "Content 1" }]));
}
