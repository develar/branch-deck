use crate::test_support::{create_repository, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// A request of the page mock for the repository `id`, which holds it while the command runs
fn command(client: &reqwest::Client, addr: SocketAddr, id: &str, command: &str, params: &Value) -> reqwest::RequestBuilder {
  client.post(format!("http://{addr}/invoke/{command}")).header(crate::REPOSITORY_ID_HEADER, id).json(params)
}

/// A failed command answers with 200 and its error, as Tauri rejects a command with a `CommandError`, so neither the
/// server nor the page logs a failed request for the expected answer of an error-path test
#[tokio::test]
async fn a_failed_command_answers_with_its_command_error() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;

  let response = command(
    &client,
    addr,
    &id,
    "get_archived_branch_commits",
    &json!({ "repositoryPath": path, "branchName": "user-name/archived/2025-01-01/no-such-branch" }),
  )
  .send()
  .await
  .unwrap();
  assert_eq!(response.status(), 200);
  let answer: Value = response.json().await.unwrap();
  assert_eq!(answer["status"], "error");
  assert!(!answer["error"]["message"].as_str().unwrap().is_empty(), "{answer}");

  let answer: Value = command(
    &client,
    addr,
    &id,
    "get_commit_files",
    &json!({ "repositoryPath": "/no/such/repository", "commitId": "0".repeat(40) }),
  )
  .send()
  .await
  .unwrap()
  .json()
  .await
  .unwrap();
  assert_eq!(
    answer,
    json!({ "status": "error", "error": { "message": "Repository not accessible: /no/such/repository" } })
  );
}

/// The deletion at the end of a test waits for the command still running for the repository, also when the page has
/// closed and the browser aborted the request, and refuses a request that comes after it
#[tokio::test]
async fn deleting_a_repository_waits_for_its_running_command() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let delay_ms: u64 = 600;
  let delay = Duration::from_millis(delay_ms);
  client
    .post(format!("http://{addr}/repositories/{id}/branch-name-suggestions"))
    .json(&json!({ "delayMs": delay_ms }))
    .send()
    .await
    .unwrap();

  let started = Instant::now();
  // the page closes while the command runs: the client gives up on the request
  let aborted = tokio::time::timeout(
    Duration::from_millis(100),
    command(&client, addr, &id, "suggest_branch_names", &json!({ "repositoryPath": path, "commits": [] })).send(),
  )
  .await;
  assert!(aborted.is_err(), "the command answered before its delay");

  let deleted = client.delete(format!("http://{addr}/repositories/{id}")).send().await.unwrap();
  assert_eq!(deleted.status(), 204);
  assert!(started.elapsed() >= delay, "deleted after {:?}, before the command ended", started.elapsed());
  assert!(!std::path::Path::new(&path).exists());

  let late = command(&client, addr, &id, "get_commit_files", &json!({ "repositoryPath": path, "commitId": "0".repeat(40) }))
    .send()
    .await
    .unwrap();
  assert_eq!(late.status(), 410);
}
