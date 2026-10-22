use crate::remote_fixtures::fixture_pulls;
use crate::test_support::{create_repository, post, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use sync_types::pull_request::{ChecksState, ChecksSummary, PullRequest, PullRequestAvailability, PullRequestState, PullRequestsResult, ReviewDecision, Reviewer, ReviewerState};
use test_utils::git_test_utils::git_command;

fn pull_request(branch: &str, number: u32) -> PullRequest {
  PullRequest {
    number,
    title: format!("Pull request of {branch}"),
    url: format!("https://github.com/acme/monorepo/pull/{number}"),
    state: PullRequestState::Open,
    is_draft: false,
    review_decision: Some(ReviewDecision::ReviewRequired),
    head_ref_name: format!("user-name/virtual/{branch}"),
    base_ref_name: "master".to_string(),
    head_ref_oid: String::new(),
    checks: None,
    updated_at: 1_704_139_200,
    reviewers: Vec::new(),
    author: Some("user-name".to_string()),
  }
}

fn reviewer(login: &str, state: ReviewerState, requested: bool) -> Reviewer {
  Reviewer {
    login: login.to_string(),
    state,
    requested,
  }
}

fn available(pull_requests: BTreeMap<String, PullRequest>, stack_pull_requests: BTreeMap<String, PullRequest>) -> PullRequestsResult {
  PullRequestsResult {
    availability: PullRequestAvailability::Available {
      owner: "acme".to_string(),
      name: "monorepo".to_string(),
    },
    pull_requests,
    stack_pull_requests,
  }
}

/// Each fixture pull request, written as the fake answers the search of the user's pull requests, reads back through
/// the real parser as the fixture; one GitHub could not answer (a requested reviewer before one who reviewed, another
/// author) is refused
#[test]
fn a_fixture_pull_request_reads_back_through_the_real_parser() {
  let approved = PullRequest {
    review_decision: Some(ReviewDecision::Approved),
    checks: Some(ChecksSummary {
      state: ChecksState::Failing,
      passed: 9,
      failed: 2,
      pending: 1,
      failed_names: vec!["tests".to_string()],
    }),
    reviewers: vec![
      reviewer("alice", ReviewerState::Approved, true),
      reviewer("bob", ReviewerState::ChangesRequested, false),
      reviewer("ij-platform", ReviewerState::Requested, true),
    ],
    ..pull_request("auth", 7)
  };
  let merged = PullRequest {
    state: PullRequestState::Merged,
    is_draft: true,
    review_decision: None,
    head_ref_oid: "0123456789abcdef0123456789abcdef01234567".to_string(),
    base_ref_name: "user-name/virtual/auth".to_string(),
    ..pull_request("login", 8)
  };
  let stack = PullRequest {
    head_ref_name: "user-name/stack/auth".to_string(),
    ..pull_request("stack", 9)
  };
  let result = available(
    BTreeMap::from([("auth".to_string(), approved.clone()), ("login".to_string(), merged)]),
    BTreeMap::from([("auth".to_string(), stack)]),
  );
  assert_eq!(fixture_pulls(&result, "acme", "user-name").unwrap().len(), 3);

  let requested_first = PullRequest {
    reviewers: vec![reviewer("ij-platform", ReviewerState::Requested, true), reviewer("alice", ReviewerState::Approved, false)],
    ..approved.clone()
  };
  let another_author = PullRequest {
    author: Some("carol".to_string()),
    ..approved
  };
  for refused in [requested_first, another_author] {
    let result = available(BTreeMap::from([("auth".to_string(), refused)]), BTreeMap::new());
    fixture_pulls(&result, "acme", "user-name").unwrap_err();
  }
}

async fn get(client: &reqwest::Client, addr: std::net::SocketAddr, path: &str) -> Value {
  client.get(format!("http://{addr}{path}")).send().await.unwrap().json().await.unwrap()
}

/// The pull requests of the fixture come through the real service from the fake GitHub of the repository, with the
/// head of an empty `headRefOid` resolved; a creation, a retarget and a close run the app's cores against the fake,
/// which lists what they changed and logs each request (`GET …/pull-requests`)
#[tokio::test]
async fn pull_requests_are_listed_created_retargeted_and_closed_through_the_real_service() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  for branch in ["auth", "login"] {
    let created = git_command()
      .args(["branch", &format!("user-name/virtual/{branch}"), "HEAD"])
      .current_dir(&repository_path)
      .output()
      .unwrap();
    assert!(created.status.success());
  }
  let tip = git_command().args(["rev-parse", "HEAD"]).current_dir(&repository_path).output().unwrap();
  let tip = String::from_utf8(tip.stdout).unwrap().trim().to_string();
  let list = json!({"repositoryPath": repository_path, "branchPrefix": "user-name"});
  assert_eq!(
    post(&client, addr, "/invoke/get_pull_requests", list.clone()).await["availability"],
    json!({"status": "notGitHub"})
  );

  let fixture = available(BTreeMap::from([("login".to_string(), pull_request("login", 7))]), BTreeMap::new());
  let mut fixture = serde_json::to_value(fixture).unwrap();
  fixture["users"] = json!([{"login": "bob", "name": "Bob"}]);
  post(&client, addr, &format!("/repositories/{id}/pull-requests"), fixture).await;
  let listed = post(&client, addr, "/invoke/get_pull_requests", list.clone()).await;
  assert_eq!(listed["availability"], json!({"status": "available", "owner": "acme", "name": "monorepo"}));
  assert_eq!(listed["pullRequests"]["login"]["headRefOid"], tip.as_str());

  let create = json!({"repositoryPath": repository_path, "branchPrefix": "user-name", "branchName": "auth", "head": "user-name/virtual/auth",
    "base": "master", "title": "Add auth", "body": "b", "reviewers": ["bob", "user-name"], "assignSelf": true});
  let created = post(&client, addr, "/invoke/create_pull_request", create).await;
  assert_eq!((created["created"].clone(), created["assignees"].clone()), (json!(true), json!(["user-name"])));
  assert_eq!(created["pullRequest"]["number"], 8);
  assert_eq!(created["pullRequest"]["reviewers"], json!([{"login": "bob", "state": "requested", "requested": true}]));
  let retarget = json!({"repositoryPath": repository_path, "pullRequestNumber": 7, "base": "user-name/virtual/auth"});
  post(&client, addr, "/invoke/retarget_pull_request", retarget).await;
  let close = json!({"repositoryPath": repository_path, "pullRequestNumber": 8, "comment": "Landed with login"});
  post(&client, addr, "/invoke/close_pull_request", close).await;

  let listed = post(&client, addr, "/invoke/get_pull_requests", list).await;
  assert_eq!(listed["pullRequests"]["login"]["baseRefName"], "user-name/virtual/auth");
  assert_eq!(listed["pullRequests"]["auth"]["state"], "closed");
  let log = get(&client, addr, &format!("/repositories/{id}/pull-requests")).await;
  let writes: Vec<Value> = log["requests"]
    .as_array()
    .unwrap()
    .iter()
    .filter(|request| request["method"] != "GET" && request["path"] != "/graphql")
    .map(|request| json!([request["method"], request["path"], request["body"]]))
    .collect();
  assert_eq!(
    writes,
    vec![
      json!(["POST", "/repos/acme/monorepo/pulls", {"title": "Add auth", "body": "b", "head": "user-name/virtual/auth", "base": "master"}]),
      json!(["POST", "/repos/acme/monorepo/issues/8/assignees", {"assignees": ["user-name"]}]),
      json!(["POST", "/repos/acme/monorepo/pulls/8/requested_reviewers", {"reviewers": ["bob"]}]),
      json!(["PATCH", "/repos/acme/monorepo/pulls/7", {"base": "user-name/virtual/auth"}]),
      json!(["POST", "/repos/acme/monorepo/issues/8/comments", {"body": "Landed with login"}]),
      json!(["PATCH", "/repos/acme/monorepo/pulls/8", {"state": "closed"}]),
    ]
  );
  assert_eq!(log["calls"], 3);
}

/// Signed out is no token file, which the status of the sign-in names too; a failure of the fixture is what the real
/// service makes of GitHub's answer
#[tokio::test]
async fn signed_out_and_no_access_are_what_the_real_service_makes_of_the_fake() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  let list = json!({"repositoryPath": repository_path, "branchPrefix": "user-name"});
  let status = || async {
    let response = client
      .post(format!("http://{addr}/invoke/get_github_auth_status"))
      .header(crate::REPOSITORY_ID_HEADER, &id)
      .json(&json!({}))
      .send()
      .await
      .unwrap();
    response.json::<Value>().await.unwrap()
  };
  assert_eq!(
    status().await,
    json!({"signInAvailable": false, "signedInAs": "user-name", "tokenSource": "branchDeck", "manageUrl": null})
  );

  let fixtures = [
    json!({"availability": {"status": "signedOut"}, "pullRequests": {}, "repository": "acme/monorepo"}),
    json!({"availability": {"status": "noAccess", "source": "branchDeck", "message": "no repository acme/monorepo"}, "pullRequests": {}, "repository": "acme/monorepo"}),
  ];
  let answers = [
    json!({"status": "signedOut"}),
    json!({"status": "noAccess", "source": "branchDeck", "message": "no repository acme/monorepo"}),
  ];
  let signed_in_as = [Value::Null, json!("user-name")];
  for ((fixture, answer), login) in fixtures.into_iter().zip(answers).zip(signed_in_as) {
    post(&client, addr, &format!("/repositories/{id}/pull-requests"), fixture).await;
    assert_eq!(post(&client, addr, "/invoke/get_pull_requests", list.clone()).await["availability"], answer);
    assert_eq!(status().await["signedInAs"], login);
  }
}
