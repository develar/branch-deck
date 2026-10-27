use crate::session_routes_test::{subscribe, subscription};
use crate::test_support::{Named, create_repository, post, post_as, start_test_server, sync, view_when};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;

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
    "requestedBy": "alice", "createdAt": 1, "updatedAt": 1, "requestedAt": 1, "waitingSince": 1, "backToYou": null, "additions": 1, "deletions": 0, "changedFiles": 1,
    "checksState": null, "reviewDecision": null, "reRequested": false, "reviewedCommit": null, "reviewedState": null, "headRefName": "alice/login",
    "headRefOid": head_ref_oid, "baseRefName": "master", "baseRefOid": "", "isCrossRepository": false, "baseIsDefault": true,
  })
}

/// Quiet by default, then the fixture: the requests the session of a window reads (after its sync, by the refresh
/// button), a range of the test repository without a network, its failure
#[tokio::test]
async fn review_routes_answer_the_fixture_without_a_network() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  let ids = GitCommandExecutor::new().execute_command_lines(&["rev-list", "HEAD"], &repository_path).unwrap();

  let mut messages = subscribe(&client, addr, &id, &repository_path, 51).await;
  subscription(&mut messages).await;
  sync(&client, addr, &repository_path).await;
  let view = view_when(&client, addr, &repository_path, |view| view["facts"]["reviewRequests"]["head"]["value"].is_object()).await;
  assert_eq!(view["facts"]["reviewRequests"]["head"]["value"], json!({ "status": "notGitHub" }));

  let result = json!({ "availability": { "status": "available", "owner": "acme", "name": "monorepo" }, "totalCount": 1, "requests": [request("")] });
  post(&client, addr, &format!("/repositories/{id}/review-requests"), result.clone()).await;
  let refresh = json!({ "repositoryPath": repository_path, "family": "reviewRequests", "reason": "manual" });
  post_as(&client, addr, Named::Id(&id), "/invoke/refresh_facts", refresh).await;
  let view = view_when(&client, addr, &repository_path, |view| view["facts"]["reviewRequests"]["requests"][0].is_object()).await;
  let part = &view["facts"]["reviewRequests"];
  // an empty head is answered as the tip of the branch it names, else of HEAD
  let mut requests = result["requests"].clone();
  requests[0]["headRefOid"] = json!(ids[0]);
  assert_eq!(
    (&part["head"]["value"], &part["requests"], &part["totalCount"]),
    (&result["availability"], &requests, &json!(1))
  );

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

  post_as(&client, addr, Named::Nothing, "/invoke/cancel_review_fetch", Value::Null).await;
  post(&client, addr, "/invoke/prune_review_refs", json!({ "repositoryPath": repository_path, "keep": [7] })).await;

  let failing = json!({ "availability": result["availability"], "totalCount": 1, "requests": [request("")], "fetchError": "GitHub asked for credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)" });
  post(&client, addr, &format!("/repositories/{id}/review-requests"), failing).await;
  let error = post(&client, addr, "/invoke/fetch_review_head", fetch).await;
  assert_eq!(
    error,
    json!({
      "status": "error",
      "error": {
        "code": "internal",
        "message": "GitHub asked for credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)",
        "details": null,
        "retry": "no",
        "operation_id": null
      }
    })
  );

  assert_eq!(
    fixture_calls(&client, addr, &id).await,
    json!({ "calls": 2, "reviewsCalls": 0, "fetchCalls": 2, "prunes": [[7]] })
  );
}

/// While the window shows Reviews the session reads its rest from the fixture; a link the user adds is asked of the fake
/// GitHub, kept, and comes back in the section of the links; a number the fake lacks is refused
#[tokio::test]
async fn a_link_joins_reviews_from_the_fake_github() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  let mut messages = subscribe(&client, addr, &id, &repository_path, 52).await;
  let subscribed = subscription(&mut messages).await;
  sync(&client, addr, &repository_path).await;
  let mut linkable = request("");
  linkable["number"] = json!(105);
  linkable["title"] = json!("Forgot to ask you");
  let fixture = json!({
    "availability": { "status": "available", "owner": "acme", "name": "monorepo" }, "totalCount": 0, "requests": [],
    "linkable": [linkable],
  });
  post(&client, addr, &format!("/repositories/{id}/review-requests"), fixture).await;
  let demand = json!({
    "focused": true, "online": true, "visibleBranches": [], "visibleStacks": [], "needed": [], "robotDetails": [], "watchedRobots": [],
    "reviews": true, "reviewWindows": [],
  });
  post_as(
    &client,
    addr,
    Named::Id(&id),
    "/invoke/set_demand",
    json!({ "subscription": subscribed["id"], "demand": demand }),
  )
  .await;
  let refresh = json!({ "repositoryPath": repository_path, "family": "reviewRequests", "reason": "manual" });
  post_as(&client, addr, Named::Id(&id), "/invoke/refresh_facts", refresh).await;
  let view = view_when(&client, addr, &repository_path, |view| view["reviews"]["head"]["value"]["status"] == "available").await;
  assert_eq!((&view["reviews"]["head"]["state"], &view["reviews"]["sections"]), (&json!({ "kind": "fresh" }), &json!([])));

  let answer = post(
    &client,
    addr,
    "/invoke/link_pull_request",
    json!({ "repositoryPath": repository_path, "input": "https://github.com/acme/monorepo/pull/105" }),
  )
  .await;
  assert_eq!(
    (answer["kept"].as_bool(), &answer["entry"]["request"]["title"]),
    (Some(true), &json!("Forgot to ask you")),
    "{answer}"
  );
  let view = view_when(&client, addr, &repository_path, |view| view["reviews"]["sections"][0].is_object()).await;
  assert_eq!(view["reviews"]["sections"][0]["kind"], "linked");
  assert_eq!(view["reviews"]["sections"][0]["entries"][0]["request"]["number"], 105);
  assert!(view["actions"]["pull:105"].as_array().unwrap().iter().any(|action| action["id"] == "remove-from-reviews"));

  let missing = post(&client, addr, "/invoke/link_pull_request", json!({ "repositoryPath": repository_path, "input": "#999" })).await;
  assert_eq!(missing["error"]["code"], "no_pull_request", "{missing}");

  post(&client, addr, "/invoke/unlink_pull_request", json!({ "repositoryPath": repository_path, "number": 105 })).await;
  view_when(&client, addr, &repository_path, |view| view["reviews"]["sections"] == json!([])).await;
}
