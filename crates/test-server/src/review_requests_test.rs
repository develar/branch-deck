use crate::test_support::{create_repository, start_test_server};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;

async fn post(client: &reqwest::Client, addr: SocketAddr, path: &str, body: Value) -> Value {
  let response = client.post(format!("http://{addr}{path}")).json(&body).send().await.unwrap();
  assert_eq!(response.status(), 200, "{path}");
  response.json().await.unwrap_or(Value::Null)
}

async fn fixture_calls(client: &reqwest::Client, addr: SocketAddr, id: &str) -> Value {
  client
    .get(format!("http://{addr}/repositories/{id}/review-requests"))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap()
}

fn request(head_ref_oid: &str) -> Value {
  json!({
    "number": 7, "title": "Add login", "url": "https://github.com/acme/monorepo/pull/7", "bodyText": "", "author": "alice",
    "requestedBy": "alice", "createdAt": 1, "updatedAt": 1, "waitingSince": 1, "additions": 1, "deletions": 0, "changedFiles": 1,
    "checksState": null, "reviewDecision": null, "reRequested": false, "reviewedCommit": null, "headRefName": "alice/login",
    "headRefOid": head_ref_oid, "baseRefName": "master", "baseRefOid": "", "isCrossRepository": false, "baseIsDefault": true,
  })
}

/// Quiet by default, then the fixture: the requests, a range of the test repository without a network, its failure
#[tokio::test]
async fn review_routes_answer_the_fixture_without_a_network() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  let ids = GitCommandExecutor::new().execute_command_lines(&["rev-list", "HEAD"], &repository_path).unwrap();

  let answer = post(
    &client,
    addr,
    "/invoke/get_review_requests",
    json!({ "repositoryPath": repository_path, "githubRepository": null }),
  )
  .await;
  assert_eq!(answer, json!({ "availability": { "status": "notGitHub" }, "totalCount": 0, "requests": [] }));

  let result = json!({ "availability": { "status": "available", "owner": "acme", "name": "monorepo" }, "totalCount": 1, "requests": [request("")] });
  post(&client, addr, &format!("/repositories/{id}/review-requests"), result.clone()).await;
  let answer = post(
    &client,
    addr,
    "/invoke/get_review_requests",
    json!({ "repositoryPath": repository_path, "githubRepository": null }),
  )
  .await;
  assert_eq!(answer, result);

  // an empty head is HEAD, an empty base the baseline, else the commit before: both local, nothing fetched
  let fetch = json!({
    "repositoryPath": repository_path, "number": 7, "headRefName": "alice/login", "headRefOid": "", "baseRefName": "master",
    "baseRefOid": "", "isCrossRepository": false,
  });
  let range = post(&client, addr, "/invoke/fetch_review_head", fetch.clone()).await;
  assert_eq!(range["base"], ids[1]);
  assert_eq!(range["head"], ids[0]);
  assert_eq!((range["headState"].as_str(), range["source"].as_str()), (Some("same"), Some("local")));
  assert_eq!(range["commits"].as_array().unwrap().len(), 1);

  let files = post(
    &client,
    addr,
    "/invoke/get_range_files",
    json!({ "repositoryPath": repository_path, "base": ids[1], "head": ids[0] }),
  )
  .await;
  assert_eq!(files["commitId"], format!("{}..{}", ids[1], ids[0]));
  assert_eq!(files["files"][0]["path"], "file2.txt");
  let patch = post(
    &client,
    addr,
    "/invoke/get_range_patch",
    json!({ "repositoryPath": repository_path, "base": ids[1], "head": ids[0] }),
  )
  .await;
  assert!(patch["files"][0]["patch"].as_str().unwrap().contains("+Content 2"), "{patch}");

  post(&client, addr, "/invoke/cancel_review_fetch", Value::Null).await;
  post(&client, addr, "/invoke/prune_review_refs", json!({ "repositoryPath": repository_path, "keep": [7] })).await;

  let failing = json!({ "availability": result["availability"], "totalCount": 1, "requests": [request("")], "fetchError": "GitHub asked for credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)" });
  post(&client, addr, &format!("/repositories/{id}/review-requests"), failing).await;
  let error = post(&client, addr, "/invoke/fetch_review_head", fetch).await;
  assert_eq!(
    error,
    json!({ "status": "error", "error": { "message": "GitHub asked for credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)" } })
  );

  assert_eq!(fixture_calls(&client, addr, &id).await, json!({ "calls": 2, "fetchCalls": 2, "prunes": [[7]] }));
}
