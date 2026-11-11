use crate::test_support::{Named, channel_events, command, create_repository, create_repository_from, start_test_server};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;

async fn invoke(client: &reqwest::Client, addr: SocketAddr, name: &str, params: Value) -> Value {
  let response = command(client, addr, Named::ByPath, name, &params).send().await.unwrap();
  assert_eq!(response.status(), 200, "{name}");
  response.json().await.unwrap()
}

/// The events of a command with a channel that answers no data
async fn stream(client: &reqwest::Client, addr: SocketAddr, name: &str, params: Value) -> Vec<Value> {
  let (events, result) = channel_events(client, addr, name, &params).await;
  assert_eq!(result, json!({ "status": "ok", "data": null }), "{name}");
  events
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

/// The diffs of the commit of the `commit_marks` template through the server, as a page reads them: after the
/// contents, the marks of the files that the others say something of (the title function that moved from the ledger
/// to the report, the calls of the function the store declares and renamed), the names asked of the index; then
/// the answer of the index of the repository, its fixture (`FixtureIndex`), as one more event of their files. A
/// repository whose fixture answers no rename is asked all the same, and its names are unavailable then
#[tokio::test]
async fn the_diffs_of_a_subject_end_with_what_its_files_say_of_each_other_and_what_the_index_answers() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (_, repository_path) = create_repository_from(&client, addr, "commit_marks").await;
  let head = GitCommandExecutor::new().execute_command(&["rev-parse", "HEAD"], &repository_path).unwrap();
  let params = json!({ "repositoryPath": repository_path, "source": { "kind": "commit", "id": head }, "policy": "default", "priorityPaths": [] });

  // the events of the marks of a file, by its path: the proof of its name that is not local, and its runs confirmed
  let marks_of = |events: &[Value], path: &str| -> Vec<(Value, Value)> {
    let index = events[0]["files"].as_array().unwrap().iter().position(|file| file["path"] == path).unwrap();
    events
      .iter()
      .filter(|event| event["kind"] == "marks" && event["index"] == index)
      .map(|event| {
        let subject = &event["marks"]["subject"];
        let proof = if subject["used"][0].is_object() {
          &subject["used"][0]["proof"]
        } else {
          &subject["declared"][0]["proof"]
        };
        (proof.clone(), subject["confirmed"].clone())
      })
      .collect()
  };

  // no fixture: an index that answers nothing. The files were compared, so the marks of the subject follow their
  // contents, the names asked, and then unavailable
  let events = stream(&client, addr, "get_file_diffs", params.clone()).await;
  let unanswered = [(json!("asked"), json!([])), (json!("unavailable"), json!([]))];
  assert_eq!(marks_of(&events, "src/Report.kt"), unanswered);
  assert_eq!(marks_of(&events, "src/Store.kt"), unanswered);
  let ledger = events
    .iter()
    .find(|event| event["kind"] == "marks" && event["marks"]["subject"]["movedOut"][0].is_object())
    .unwrap();
  assert_eq!(ledger["marks"]["subject"]["movedOut"], json!([{ "start": 54, "len": 4, "file": 1, "line": 17 }]));

  // the index answers the rename of the store whole: its declaration and the two calls of the report, in both trees
  let used =
    |file: &str, line: u32, column: u32, end: u32, kind: &str| json!({ "file": file, "line": line, "column": column, "end_column": end, "kind": kind, "resolved": "import" });
  let uses = |end: u32| {
    json!({ "total": 3, "uses": [
      used("src/Report.kt", 5, 16, 12 + end, "call"),
      used("src/Report.kt", 11, 16, 12 + end, "call"),
      used("src/Store.kt", 19, 4, end, "declaration"),
    ] })
  };
  let kotlin = json!({ "language": "kotlin" });
  let renamed = json!({ "same": true, "complete": true, "old": kotlin, "new": kotlin, "base": uses(13), "head": uses(16), "conflicts": [] });
  std::fs::write(
    std::path::Path::new(&repository_path).join(".git/branch-deck-accio.json"),
    json!({ "renames": { "src/Store.kt:19:5": renamed } }).to_string(),
  )
  .unwrap();
  // the pairs are in the memory of the server now: the contents say that the index is asked, its answer follows
  let events = stream(&client, addr, "get_file_diffs", params).await;
  assert_eq!(marks_of(&events, "src/Report.kt"), [(json!("confirmed"), json!([0, 1]))]);
  assert_eq!(marks_of(&events, "src/Store.kt"), [(json!("confirmed"), json!([]))]);
  let report = events
    .iter()
    .find(|event| event["kind"] == "content" && event["content"]["marks"]["subject"]["used"][0].is_object())
    .unwrap();
  assert_eq!(report["content"]["marks"]["subject"]["used"][0]["proof"], "asked");
  assert!(matches!(events.last(), Some(event) if event["kind"] == "marks"), "the answer of the index comes last");
}
