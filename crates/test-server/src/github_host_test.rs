use crate::remote_fixtures::fixture_pulls;
use crate::session_routes::TestFactSources;
use crate::session_routes_test::{subscribe, subscription};
use crate::tauri_command_bridge::review_requests_of;
use crate::test_support::{
  Messages, Named, command, create_repository, delete_repository, head, post, post_as, push_intent, start_test_server, start_test_server_with_state, sync,
};
use domain_types::pull_request::{ChecksState, ChecksSummary, PullRequest, PullRequestAvailability, PullRequestState, PullRequestsResult, ReviewDecision, Reviewer, ReviewerState};
use pretty_assertions::assert_eq;
use remote_facts::fact_sources::FactSources;
use repository_model::session::SessionKey;
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

/// A window on the session of the repository `id` at `path`: the session lists the pull requests while one watches it
async fn window(client: &reqwest::Client, addr: std::net::SocketAddr, id: &str, path: &str) -> Messages {
  let mut messages = subscribe(client, addr, id, path, 1).await;
  subscription(&mut messages).await;
  messages
}

/// The refresh button of the pull requests of the session at `path`, which a window watches
fn refresh(client: &reqwest::Client, addr: std::net::SocketAddr, id: &str, path: &str) -> reqwest::RequestBuilder {
  command(
    client,
    addr,
    Named::Id(id),
    "refresh_facts",
    &json!({"repositoryPath": path, "family": "pullRequests", "reason": "manual"}),
  )
}

/// The pull requests of the session at `path` after the refresh button, as a listing names them
/// `intent` prepared in the session of `path` with `effects`, then run on its plan: its outcome, its first effect done
async fn planned_and_done(client: &reqwest::Client, addr: std::net::SocketAddr, path: &str, intent: Value, effects: Value) -> Value {
  let plan = post(client, addr, "/invoke/prepare", json!({"repositoryPath": path, "intent": intent})).await;
  assert_eq!(plan["effects"], effects, "{plan}");
  let execute = json!({"repositoryPath": path, "intent": intent, "fingerprint": plan["fingerprint"], "accepted": []});
  let outcome = post(client, addr, "/invoke/execute", execute).await;
  assert_eq!(outcome["effects"][0]["status"]["kind"], "done", "{outcome}");
  outcome
}

async fn listed(client: &reqwest::Client, addr: std::net::SocketAddr, path: &str) -> Value {
  post(
    client,
    addr,
    "/invoke/refresh_facts",
    json!({"repositoryPath": path, "family": "pullRequests", "reason": "manual"}),
  )
  .await;
  let view = post(client, addr, "/invoke/get_snapshot", json!({"repositoryPath": path})).await;
  of_view(&view["facts"]["pullRequests"])
}

/// The part `facts.pullRequests` of a view as a listing names it
fn of_view(part: &Value) -> Value {
  json!({"availability": part["head"]["value"], "pullRequests": part["pullRequests"], "stackPullRequests": part["stackPullRequests"]})
}

/// Waits until the session of the repository `id` listed the pull requests `calls` times
async fn until_listings(client: &reqwest::Client, addr: std::net::SocketAddr, id: &str, calls: u64) {
  while get(client, addr, &format!("/repositories/{id}/pull-requests")).await["calls"].as_u64() < Some(calls) {
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
  }
}

/// The pull requests of the fixture come through the real service from the fake GitHub of the repository, with the
/// head of an empty `headRefOid` resolved; a creation and a retarget run the engine (`prepare` and `execute` of their
/// intents), a close the app's core, against the fake, which lists what they changed and logs each request
/// (`GET …/pull-requests`)
#[tokio::test]
async fn pull_requests_are_listed_created_retargeted_and_closed_through_the_real_service() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  branch_at_head(&repository_path, "user-name/virtual/auth");
  let tip = branch_at_head(&repository_path, "user-name/virtual/login");
  let _window = window(&client, addr, &id, &repository_path).await;
  assert_eq!(listed(&client, addr, &repository_path).await["availability"], json!({"status": "notGitHub"}));

  let mut fixture = serde_json::to_value(login_into_parent()).unwrap();
  fixture["users"] = json!([{"login": "bob", "name": "Bob"}]);
  post(&client, addr, &format!("/repositories/{id}/pull-requests"), fixture).await;
  let pull_requests = listed(&client, addr, &repository_path).await;
  assert_eq!(pull_requests["availability"], json!({"status": "available", "owner": "acme", "name": "monorepo"}));
  assert_eq!(pull_requests["pullRequests"]["login"]["headRefOid"], tip.as_str());

  // each write is in the session at once; a creation and a retarget list the pull requests again, a close does not.
  // The creation and the retarget are intents of the engine, planned over the table of the sync: test-branch is not on
  // the remote, so the creation pushes it first
  sync(&client, addr, &repository_path).await;
  let intent = json!({"kind": "createPullRequest", "subject": {"kind": "branch", "name": "test-branch"}, "title": "Add auth", "body": "b",
    "assignSelf": true, "reviewers": ["bob", "user-name"]});
  let plan = post(&client, addr, "/invoke/prepare", json!({"repositoryPath": repository_path, "intent": intent})).await;
  assert_eq!(
    plan["effects"],
    json!([{"kind": "push", "branches": ["test-branch"], "force": false}, {"kind": "createPullRequest"}]),
    "{plan}"
  );
  let execute = json!({"repositoryPath": repository_path, "intent": intent, "fingerprint": plan["fingerprint"], "accepted": []});
  let created = post(&client, addr, "/invoke/execute", execute).await;
  assert!(created["versionAfter"].is_object(), "{created}");
  let created = &created["result"];
  assert_eq!(
    (created["kind"].clone(), created["created"].clone(), created["assignees"].clone()),
    (json!("pullRequest"), json!(true), json!(["user-name"]))
  );
  assert_eq!(created["pullRequest"]["number"], 8);
  assert_eq!(created["pullRequest"]["reviewers"], json!([{"login": "bob", "state": "requested", "requested": true}]));
  until_listings(&client, addr, &id, 3).await;
  // the retarget moves the pull request into the target the sync names
  let intent = json!({"kind": "retarget", "number": 7});
  let effects = json!([{"kind": "retarget", "branch": "login", "number": 7, "from": "user-name/virtual/auth", "to": "master"}]);
  let outcome = planned_and_done(&client, addr, &repository_path, intent, effects).await;
  assert!(outcome["versionAfter"].is_object(), "{outcome}");
  until_listings(&client, addr, &id, 4).await;
  // the close of the pull request goes through the engine too, into the session at once
  let intent = json!({"kind": "closePullRequest", "number": 8, "comment": "Landed with login"});
  let effects = json!([{"kind": "closePullRequest", "branch": "test-branch", "number": 8, "comment": "Landed with login"}]);
  planned_and_done(&client, addr, &repository_path, intent, effects).await;
  let view = post(&client, addr, "/invoke/get_snapshot", json!({"repositoryPath": repository_path})).await;
  assert_eq!(
    view["facts"]["pullRequests"]["pullRequests"]["test-branch"]["state"], "closed",
    "the close is in the session at once"
  );

  let pull_requests = listed(&client, addr, &repository_path).await;
  assert_eq!(pull_requests["pullRequests"]["login"]["baseRefName"], "master");
  assert_eq!(pull_requests["pullRequests"]["test-branch"]["state"], "closed");
  let log = get(&client, addr, &format!("/repositories/{id}/pull-requests")).await;
  assert_eq!(
    writes_of(&log),
    vec![
      json!(["POST", "/repos/acme/monorepo/pulls", {"title": "Add auth", "body": "b", "head": "user-name/virtual/test-branch", "base": "master"}]),
      json!(["POST", "/repos/acme/monorepo/issues/8/assignees", {"assignees": ["user-name"]}]),
      json!(["POST", "/repos/acme/monorepo/pulls/8/requested_reviewers", {"reviewers": ["bob"]}]),
      json!(["PATCH", "/repos/acme/monorepo/pulls/7", {"base": "master"}]),
      json!(["POST", "/repos/acme/monorepo/issues/8/comments", {"body": "Landed with login"}]),
      json!(["PATCH", "/repos/acme/monorepo/pulls/8", {"state": "closed"}]),
    ]
  );
  // the push the creation makes first lists the pull requests again too
  assert_eq!(log["calls"], 6);
}

/// The writes GitHub got, in their order: method, path and body of every request but the reads and the searches
fn writes_of(log: &Value) -> Vec<Value> {
  log["requests"]
    .as_array()
    .unwrap()
    .iter()
    .filter(|request| request["method"] != "GET" && request["path"] != "/graphql")
    .map(|request| json!([request["method"], request["path"], request["body"]]))
    .collect()
}

/// Signed out is no token file, which the status of the sign-in names too; a failure of the fixture is what the real
/// service makes of GitHub's answer
#[tokio::test]
async fn signed_out_and_no_access_are_what_the_real_service_makes_of_the_fake() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  let _window = window(&client, addr, &id, &repository_path).await;
  let status = || post_as(&client, addr, Named::Id(&id), "/invoke/get_github_auth_status", json!({}));
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
    assert_eq!(listed(&client, addr, &repository_path).await["availability"], answer);
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
  let invoke = |name: &'static str| {
    let request = command(&client, addr, Named::Id(&id), name, &json!({}));
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

/// The listing of the pull request #7 of login, which goes into the branch of its parent, from before
fn login_into_parent() -> PullRequestsResult {
  let into_parent = PullRequest {
    base_ref_name: "user-name/virtual/auth".to_string(),
    ..pull_request("login", 7)
  };
  available(BTreeMap::from([("login".to_string(), into_parent)]), BTreeMap::new())
}

/// A server with a repository of the simple template whose GitHub lists the pull request #7 of login: the server, its
/// client, the repository's id and path
async fn listing_login() -> (std::net::SocketAddr, reqwest::Client, String, String) {
  listing_login_at(start_test_server().await).await
}

/// `listing_login` on the server at `addr`
async fn listing_login_at(addr: std::net::SocketAddr) -> (std::net::SocketAddr, reqwest::Client, String, String) {
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  post(
    &client,
    addr,
    &format!("/repositories/{id}/pull-requests"),
    serde_json::to_value(login_into_parent()).unwrap(),
  )
  .await;
  (addr, client, id, repository_path)
}

/// Creates the branch `name` at HEAD of the repository at `path`: its tip, which the fake GitHub answers as the head
/// commit of a pull request of the branch, from the files of the repository
fn branch_at_head(path: &str, name: &str) -> String {
  let created = git_command().args(["branch", name, "HEAD"]).current_dir(path).output().unwrap();
  assert!(created.status.success(), "{}", String::from_utf8_lossy(&created.stderr));
  head(path)
}

/// Starts the command `name` of the repository `id` at `repository_path` in a task of its own: its answer, once given.
/// It names the repository as the page does, so the deletion of the repository waits for it before it removes the
/// files
/// The refresh button of the review requests of the session at `path`, in flight
fn refresh_review_requests(client: &reqwest::Client, addr: std::net::SocketAddr, id: &str, path: &str) -> tokio::task::JoinHandle<reqwest::Response> {
  let request = command(
    client,
    addr,
    Named::Id(id),
    "refresh_facts",
    &json!({"repositoryPath": path, "family": "reviewRequests", "reason": "manual"}),
  );
  tokio::spawn(async move { request.send().await.unwrap() })
}

/// A held answer (`/remote-gates`): the session's request reaches the fake and waits there, the fixture names it as
/// arrived, and a release lets it through with what GitHub holds; the review requests, which the session reads from
/// the fixture, are held there
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

  let _window = window(&client, addr, &id, &repository_path).await;
  let listing = tokio::spawn(refresh(&client, addr, &id, &repository_path).send());
  until_arrived(&client, addr, &id, "pullRequests").await;
  let requests = refresh_review_requests(&client, addr, &id, &repository_path);
  until_arrived(&client, addr, &id, "reviewRequests").await;
  assert!(!listing.is_finished() && !requests.is_finished(), "a held answer came");

  post(&client, addr, &format!("/repositories/{id}/remote-gates"), json!({"release": ["pullRequests"]})).await;
  assert_eq!(listing.await.unwrap().unwrap().status(), 200);
  let view = post(&client, addr, "/invoke/get_snapshot", json!({"repositoryPath": repository_path})).await;
  assert_eq!(view["facts"]["pullRequests"]["pullRequests"]["login"]["number"], 7);
  assert!(!requests.is_finished(), "the review requests are still held");
  post(&client, addr, &format!("/repositories/{id}/remote-gates"), json!({"release": ["reviewRequests"]})).await;
  assert_eq!(requests.await.unwrap().status(), 200);
  let view = post(&client, addr, "/invoke/get_snapshot", json!({"repositoryPath": repository_path})).await;
  assert!(view["facts"]["reviewRequests"]["head"]["value"].is_object(), "{view}");
  assert_eq!(get(&client, addr, &format!("/repositories/{id}/remote-gates")).await, json!({"arrived": []}));
  delete_repository(&client, addr, &id, &repository_path).await;
}

/// A deleted repository releases what its test held: a read of its session or a command waiting at a fake ends instead
/// of holding the deletion, and the deletion removes the files only once it ended, so the git of the fake's answer still
/// reads them (on Windows that git, its current directory in them, failed the removal under it)
#[tokio::test]
async fn a_deleted_repository_releases_the_answers_its_test_held() {
  let (addr, state) = start_test_server_with_state().await;
  let (addr, client, id, repository_path) = listing_login_at(addr).await;
  let tip = branch_at_head(&repository_path, "user-name/virtual/login");
  post(
    &client,
    addr,
    &format!("/repositories/{id}/remote-gates"),
    json!({"hold": ["pullRequests", "reviewRequests"]}),
  )
  .await;
  // the session's listings held: the pull requests at the fake, the review requests at the fixture
  let key = SessionKey {
    worktree: repository_path.clone(),
    prefix: "user-name".to_owned(),
  };
  let listing = tokio::spawn({
    let (state, key) = (std::sync::Arc::clone(&state), key.clone());
    async move { TestFactSources::new(&state).pull_requests(&key).await }
  });
  until_arrived(&client, addr, &id, "pullRequests").await;
  let requests = tokio::spawn(async move { review_requests_of(&state, &key, false).await });
  until_arrived(&client, addr, &id, "reviewRequests").await;
  delete_repository(&client, addr, &id, &repository_path).await;
  let listed = tokio::time::timeout(std::time::Duration::from_mins(1), listing)
    .await
    .expect("the held listing ended")
    .unwrap();
  assert_eq!(listed.pull_requests["login"].head_ref_oid, tip);
  assert!(
    tokio::time::timeout(std::time::Duration::from_mins(1), requests).await.is_ok(),
    "the held review requests ended"
  );
}

/// The push of a branch whose open pull request goes into a virtual branch retargets it into the target first (D3a),
/// then pushes, as one plan of the engine: GitHub gets the PATCH of the base before the remote gets the branch
#[tokio::test]
async fn a_push_retargets_the_pull_request_of_its_branch_out_of_a_virtual_branch_first() {
  let (addr, client, id, repository_path) = listing_login().await;
  let _window = window(&client, addr, &id, &repository_path).await;
  let into_virtual = PullRequest {
    base_ref_name: "user-name/virtual/auth".to_string(),
    ..pull_request("test-branch", 9)
  };
  let fixture = available(BTreeMap::from([("test-branch".to_string(), into_virtual)]), BTreeMap::new());
  post(&client, addr, &format!("/repositories/{id}/pull-requests"), serde_json::to_value(fixture).unwrap()).await;
  listed(&client, addr, &repository_path).await;
  sync(&client, addr, &repository_path).await;
  let effects = json!([
    {"kind": "retarget", "branch": "test-branch", "number": 9, "from": "user-name/virtual/auth", "to": "master"},
    {"kind": "push", "branches": ["test-branch"], "force": false},
  ]);
  let outcome = planned_and_done(&client, addr, &repository_path, push_intent("test-branch", None), effects).await;
  assert_eq!(outcome["effects"][1]["status"]["kind"], "done", "{outcome}");
  let log = get(&client, addr, &format!("/repositories/{id}/pull-requests")).await;
  assert_eq!(writes_of(&log), vec![json!(["PATCH", "/repos/acme/monorepo/pulls/9", {"base": "master"}])]);
}
