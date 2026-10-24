use crate::remote_fixtures::fixture_pulls;
use crate::test_support::{create_repository, delete_repository, head, post, start_test_server};
use domain_types::pull_request::{ChecksState, ChecksSummary, PullRequest, PullRequestAvailability, PullRequestState, PullRequestsResult, ReviewDecision, Reviewer, ReviewerState};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::collections::BTreeMap;
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
  branch_at_head(&repository_path, "user-name/virtual/auth");
  let tip = branch_at_head(&repository_path, "user-name/virtual/login");
  let list = json!({"repositoryPath": repository_path});
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

  let create = json!({"repositoryPath": repository_path, "branchName": "auth", "head": "user-name/virtual/auth",
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
  let list = json!({"repositoryPath": repository_path});
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

/// The app's sign-in runs the real session against the device flow of the fake: pending until a fixture authorizes
/// the code, then the token of the host's user in the token file; a sign-out removes it, a fake `gh` gives its token,
/// and a deleted repository ends the wait of a sign-in its page left
#[tokio::test]
async fn the_sign_in_runs_the_device_flow_of_the_fake_and_ends_with_its_repository() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let invoke = |command: &'static str| {
    let request = client
      .post(format!("http://{addr}/invoke/{command}"))
      .header(crate::REPOSITORY_ID_HEADER, &id)
      .json(&json!({}));
    async move { request.send().await.unwrap().json::<Value>().await.unwrap() }
  };
  let sign_in_log = || async {
    let response = client.get(format!("http://{addr}/repositories/{id}/github-sign-in")).send().await.unwrap();
    response.json::<Value>().await.unwrap()
  };
  post(
    &client,
    addr,
    &format!("/repositories/{id}/github-sign-in"),
    json!({"clientId": "client-1", "signedIn": false}),
  )
  .await;
  assert_eq!(invoke("get_github_auth_status").await["signedInAs"], Value::Null);

  let code = invoke("start_github_sign_in").await;
  assert_eq!(
    code,
    json!({"userCode": "WDJB-MJHT", "verificationUri": "https://github.com/login/device", "expiresIn": 900})
  );
  let wait = tokio::spawn(invoke("wait_github_sign_in"));
  while sign_in_log().await["polls"] == 0 {
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
  }
  assert_eq!(sign_in_log().await, json!({"pending": true, "codes": 1, "polls": 1}));
  post(&client, addr, &format!("/repositories/{id}/github-sign-in"), json!({"authorize": true})).await;
  assert_eq!(wait.await.unwrap(), json!("user-name"));
  assert_eq!(
    invoke("get_github_auth_status").await,
    json!({"signInAvailable": true, "signedInAs": "user-name", "tokenSource": "branchDeck", "manageUrl": "https://github.com/settings/connections/applications/client-1"})
  );

  assert_eq!(invoke("sign_out_of_github").await, Value::Null);
  assert_eq!(invoke("get_github_auth_status").await["tokenSource"], Value::Null);
  post(&client, addr, &format!("/repositories/{id}/github-sign-in"), json!({"ghToken": "gho_from_gh"})).await;
  assert_eq!(invoke("get_github_auth_status").await["tokenSource"], json!("gh"));

  invoke("start_github_sign_in").await;
  let wait = tokio::spawn(invoke("wait_github_sign_in"));
  while sign_in_log().await["polls"].as_u64() < Some(2) {
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
  }
  let started = std::time::Instant::now();
  delete_repository(&client, addr, &id, &path).await;
  assert!(started.elapsed() < std::time::Duration::from_secs(5), "the deletion waited {:?}", started.elapsed());
  assert_eq!(
    wait.await.unwrap(),
    json!({"status": "error", "error": {"code": "canceled", "message": "the sign-in was canceled", "details": null, "retry": "no", "operation_id": null}})
  );
}

/// Waits until a request of the page reached the held answer `answer` of the repository `id` (`/remote-gates`)
async fn until_arrived(client: &reqwest::Client, addr: std::net::SocketAddr, id: &str, answer: &str) {
  while !get(client, addr, &format!("/repositories/{id}/remote-gates")).await["arrived"]
    .as_array()
    .unwrap()
    .contains(&json!(answer))
  {
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
  }
}

/// A server with a repository of the simple template whose GitHub lists the pull request #7 of login: the server, its
/// client, the repository's id and path
async fn listing_login() -> (std::net::SocketAddr, reqwest::Client, String, String) {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  let fixture = available(BTreeMap::from([("login".to_string(), pull_request("login", 7))]), BTreeMap::new());
  post(&client, addr, &format!("/repositories/{id}/pull-requests"), serde_json::to_value(fixture).unwrap()).await;
  (addr, client, id, repository_path)
}

/// Creates the branch `name` at HEAD of the repository at `path`: its tip, which the fake GitHub answers as the head
/// commit of a pull request of the branch, from the files of the repository
fn branch_at_head(path: &str, name: &str) -> String {
  let created = git_command().args(["branch", name, "HEAD"]).current_dir(path).output().unwrap();
  assert!(created.status.success(), "{}", String::from_utf8_lossy(&created.stderr));
  head(path)
}

/// Starts `command` of the repository `id` at `repository_path` in a task of its own: its answer, once given. It names
/// the repository as the page does (`X-Repository-Id`), so the deletion of the repository waits for it before it
/// removes the files
fn invoke(client: &reqwest::Client, addr: std::net::SocketAddr, id: &str, command: &'static str, repository_path: &str) -> tokio::task::JoinHandle<Value> {
  let request = client
    .post(format!("http://{addr}/invoke/{command}"))
    .header(crate::REPOSITORY_ID_HEADER, id)
    .json(&json!({"repositoryPath": repository_path}));
  tokio::spawn(async move {
    let response = request.send().await.unwrap();
    assert_eq!(response.status(), 200, "{command}");
    response.json().await.unwrap()
  })
}

/// A held answer (`/remote-gates`): the page's request reaches the fake and waits there, the fixture names it as
/// arrived, and a release lets it through with what GitHub holds; the review requests, which the bridge answers from
/// its fixture, are held there
#[tokio::test]
async fn a_held_answer_waits_at_the_fake_until_it_is_released() {
  let (addr, client, id, repository_path) = listing_login().await;
  post(
    &client,
    addr,
    &format!("/repositories/{id}/remote-gates"),
    json!({"hold": ["pullRequests", "reviewRequests"]}),
  )
  .await;
  assert_eq!(get(&client, addr, &format!("/repositories/{id}/remote-gates")).await, json!({"arrived": []}));

  let listed = invoke(&client, addr, &id, "get_pull_requests", &repository_path);
  until_arrived(&client, addr, &id, "pullRequests").await;
  let requests = invoke(&client, addr, &id, "get_review_requests", &repository_path);
  until_arrived(&client, addr, &id, "reviewRequests").await;
  assert!(!listed.is_finished() && !requests.is_finished(), "a held answer came");

  post(&client, addr, &format!("/repositories/{id}/remote-gates"), json!({"release": ["pullRequests"]})).await;
  assert_eq!(listed.await.unwrap()["pullRequests"]["login"]["number"], 7);
  assert!(!requests.is_finished(), "the review requests are still held");
  post(&client, addr, &format!("/repositories/{id}/remote-gates"), json!({"release": ["reviewRequests"]})).await;
  assert!(requests.await.unwrap().get("availability").is_some());
  assert_eq!(get(&client, addr, &format!("/repositories/{id}/remote-gates")).await, json!({"arrived": []}));
  delete_repository(&client, addr, &id, &repository_path).await;
}

/// A deleted repository releases what its test held: a command waiting at a fake ends instead of holding the deletion,
/// and the deletion removes the files only once it ended, so the git of the fake's answer still reads them (on Windows
/// that git, its current directory in them, failed the removal under it)
#[tokio::test]
async fn a_deleted_repository_releases_the_answers_its_test_held() {
  let (addr, client, id, repository_path) = listing_login().await;
  let tip = branch_at_head(&repository_path, "user-name/virtual/login");
  post(&client, addr, &format!("/repositories/{id}/remote-gates"), json!({"hold": ["pullRequests"]})).await;
  let listed = invoke(&client, addr, &id, "get_pull_requests", &repository_path);
  until_arrived(&client, addr, &id, "pullRequests").await;
  delete_repository(&client, addr, &id, &repository_path).await;
  let listed = tokio::time::timeout(std::time::Duration::from_mins(1), listed).await.expect("the held listing ended");
  assert_eq!(listed.unwrap()["pullRequests"]["login"]["headRefOid"], tip.as_str());
}
