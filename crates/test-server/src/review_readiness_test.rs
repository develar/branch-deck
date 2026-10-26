use crate::review_fixtures::fixture_review_answer;
use crate::session_routes::TestFactSources;
use crate::session_routes_test::{subscribe, subscription};
use crate::state::AppState;
use crate::tauri_command_bridge::branch_prefix_of;
use crate::test_support::{Named, create_repository, post, post_as, start_test_server, start_test_server_with_state, sync, view_when};
use domain_types::patronus::PatronusReviewRequirement;
use domain_types::patronus::ReviewReadinessBranch;
use patronus::client::PreMergeCheckResponse;
use patronus::review::to_requirement;
use pretty_assertions::assert_eq;
use remote_facts::fact_sources::FactSources;
use repository_model::session::SessionKey;
use serde_json::{Value, json};
use std::sync::Arc;
use test_utils::git_test_utils::git_command;

/// The answer of a Patronus that evaluated the pull request: the code owner is asked and hasn't approved
fn awaiting_code_owner() -> Value {
  json!({
    "required": true, "mergeRequest": null, "skipped": false,
    "groups": [{"name": "IJPL Code Platform", "paths": ["file2.txt"], "branchWide": false, "pathsSource": "patronus",
      "requiredApprovals": 1, "collectedApprovals": 0, "approved": false, "selfApprovalAllowed": false,
      "reviewers": [{"fullName": "Ilya Korennoy", "username": "ikorennoy", "githubLogin": "ikorennoy", "available": true, "suggested": true, "approvalState": "reviewing"}]}],
  })
}

/// The server with its state, a repository of the `simple` template: its id, its path and the tip of `HEAD`
async fn setup() -> (std::net::SocketAddr, Arc<AppState>, reqwest::Client, String, String, String) {
  let (addr, state) = start_test_server_with_state().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  let tip = git(&repository_path, &["rev-parse", "HEAD"]);
  (addr, state, client, id, repository_path, tip)
}

fn git(repository_path: &str, args: &[&str]) -> String {
  let output = git_command().args(args).current_dir(repository_path).output().unwrap();
  assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
  String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// Each answer of a fixture, written as Patronus answers the pre-check, reads back through the real parser as the
/// fixture; one Patronus could not give (a skipped review, paths of the `OWNERSHIP` files) is refused
#[test]
fn a_fixture_answer_reads_back_through_the_real_parser() {
  let not_evaluated = json!({"required": true, "mergeRequest": null, "skipped": false,
    "groups": [{"name": "IJPL Code Platform", "paths": [], "branchWide": true, "pathsSource": "patronus",
      "reviewers": [{"fullName": "Ilya Korennoy", "username": "ikorennoy", "githubLogin": "ikorennoy", "available": true, "suggested": true}]}]});
  let not_required = json!({"required": false, "mergeRequest": null, "groups": [], "skipped": false});
  for fixture in [awaiting_code_owner(), not_evaluated, not_required] {
    let requirement: PatronusReviewRequirement = serde_json::from_value(fixture).unwrap();
    let wire = fixture_review_answer(&requirement).unwrap();
    assert_eq!(to_requirement(&serde_json::from_value::<PreMergeCheckResponse>(wire).unwrap(), None), requirement);
  }
  let skipped: PatronusReviewRequirement = serde_json::from_value(json!({"required": false, "mergeRequest": null, "groups": [], "skipped": true})).unwrap();
  fixture_review_answer(&skipped).unwrap_err();
  let mut ownership = serde_json::from_value::<PatronusReviewRequirement>(awaiting_code_owner()).unwrap();
  ownership.groups[0].paths_source = domain_types::patronus::ReviewPathsSource::Ownership;
  fixture_review_answer(&ownership).unwrap_err();
}

/// The readiness as the session of the repository at `path` reads it (`TestFactSources`), against `origin/master`
async fn readiness(state: &Arc<AppState>, path: &str, asks: Value, recheck: bool) -> Value {
  let prefix = branch_prefix_of(state, path).await.ok().unwrap();
  let key = SessionKey {
    worktree: path.to_owned(),
    prefix,
  };
  let asks: Vec<ReviewReadinessBranch> = serde_json::from_value(asks).unwrap();
  let sources = TestFactSources::new(state);
  serde_json::to_value(sources.readiness(&key, "origin/master".to_owned(), asks, recheck).await.unwrap()).unwrap()
}

/// The readiness comes through the real core from the fake Patronus: without an answer for the branch the owners of its
/// paths review (none without `OWNERSHIP` files); with one the answer of the branch whose tip the pre-check names,
/// judged with the reviewers and the start the session asks with; the pre-check names the pull request of the start
#[tokio::test]
async fn the_readiness_comes_from_the_fake_patronus_through_the_real_core() {
  let (addr, state, client, id, repository_path, tip) = setup().await;
  git(&repository_path, &["update-ref", "refs/remotes/origin/master", "HEAD~1"]);
  git(&repository_path, &["update-ref", "refs/heads/user-name/virtual/IJPL-1", "HEAD"]);
  post(
    &client,
    addr,
    &format!("/repositories/{id}/patronus"),
    json!({"availability": {"status": "available"}, "robots": [], "githubRepository": "acme/monorepo"}),
  )
  .await;
  let asks = json!([{"branchName": "IJPL-1", "tip": tip, "start": {"kind": "pullRequest", "number": 7}, "author": "user-name",
    "reviewers": [{"login": "junie-jetbrains", "state": "approved", "requested": false}, {"login": "ikorennoy", "state": "requested", "requested": true}]}]);

  let answer = readiness(&state, &repository_path, asks.clone(), false).await;
  assert_eq!((answer[0]["tip"].clone(), answer[0]["readiness"].clone()), (json!(tip), json!({"kind": "notRequired"})));

  post(
    &client,
    addr,
    &format!("/repositories/{id}/review-answers"),
    json!({"answers": {"IJPL-1": awaiting_code_owner()}}),
  )
  .await;
  let answer = readiness(&state, &repository_path, asks, true).await;
  let ilya = json!({"login": "ikorennoy", "fullName": "Ilya Korennoy"});
  assert_eq!(
    answer[0]["readiness"],
    json!({"kind": "missing", "reason": "approvalsMissing", "notCounted": [{"login": "junie-jetbrains", "fullName": null}],
      "groups": [{"name": "IJPL Code Platform", "needed": 1, "approvedBy": [], "requested": [ilya], "suggested": [ilya], "changesRequestedBy": []}]})
  );
  assert_eq!(answer[0]["requirement"], awaiting_code_owner());

  let unknown_field = client
    .post(format!("http://{addr}/repositories/{id}/review-answers"))
    .json(&json!({"unavailable": "gone"}))
    .send()
    .await
    .unwrap();
  assert_eq!(unknown_field.status(), 422);

  let log = review_log(&client, addr, &id).await;
  assert_eq!(log["calls"], json!([{"branches": ["IJPL-1"], "recheck": false}, {"branches": ["IJPL-1"], "recheck": true}]));
  let check = json!({"tip": tip, "pullRequest": {"repository": "acme/monorepo", "number": 7}});
  assert_eq!(log["checks"], json!([check, check]));
}

async fn review_log(client: &reqwest::Client, addr: std::net::SocketAddr, id: &str) -> Value {
  client
    .get(format!("http://{addr}/repositories/{id}/review-answers"))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap()
}

/// A branch the fixture doesn't answer is reviewed by the owners of its paths by the `OWNERSHIP` files of the target,
/// one group per owner with its paths, none approved
#[tokio::test]
async fn a_branch_without_an_answer_is_reviewed_by_the_owners_of_its_paths() {
  let (addr, state, client, id, repository_path, tip) = setup().await;
  let changed = git(&repository_path, &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]);
  let changed = changed.lines().next().unwrap().to_string();
  // the target has the OWNERSHIP file: the owners of the target decide; written without the working tree
  let target = owned_target(&repository_path, "owner 'IJPL Code Platform'\n");
  git(&repository_path, &["update-ref", "refs/remotes/origin/master", &target]);
  post(
    &client,
    addr,
    &format!("/repositories/{id}/patronus"),
    json!({"availability": {"status": "available"}, "robots": []}),
  )
  .await;
  let asks = json!([{"branchName": "IJPL-1", "tip": tip, "start": {"kind": "rest"}}]);
  let answer = readiness(&state, &repository_path, asks, false).await;
  assert_eq!(answer[0]["requirement"]["required"], json!(true), "{answer}");
  let group = &answer[0]["requirement"]["groups"][0];
  assert_eq!(
    (group["name"].clone(), group["paths"].clone(), group["pathsSource"].clone()),
    (json!("IJPL Code Platform"), json!([changed]), json!("ownership"))
  );
  assert_eq!(answer[0]["readiness"]["kind"], json!("missing"));
}

/// The session asks the readiness a window needs at once, through the real core against the fake Patronus, once
/// Patronus answered the list of the robots: the answer comes as a patch of the window's view, the call logged with
/// its subject
#[tokio::test]
async fn the_session_asks_the_readiness_a_window_needs_from_the_fake_patronus() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  post(
    &client,
    addr,
    &format!("/repositories/{id}/patronus"),
    json!({"availability": {"status": "available"}, "robots": [], "githubRepository": "acme/monorepo"}),
  )
  .await;
  let mut messages = subscribe(&client, addr, &id, &path, 31).await;
  let subscribed = subscription(&mut messages).await;
  sync(&client, addr, &path).await;
  let snapshot = view_when(&client, addr, &path, |view| view["facts"]["robots"]["head"]["state"]["kind"] == "fresh").await;
  let branch = snapshot["order"][0].as_str().unwrap().to_owned();
  let demand = json!({
    "focused": true, "online": true, "visibleBranches": [], "visibleStacks": [], "needed": [branch],
    "robotDetails": [], "watchedRobots": [],
  });
  post_as(
    &client,
    addr,
    Named::Id(&id),
    "/invoke/set_demand",
    json!({ "subscription": subscribed["id"], "demand": demand }),
  )
  .await;
  let view = view_when(&client, addr, &path, |view| view["facts"]["readiness"][&branch]["state"]["kind"] == "fresh").await;
  assert_eq!(view["facts"]["readiness"][&branch]["value"]["readiness"], json!({"kind": "notRequired"}));
  assert_eq!(review_log(&client, addr, &id).await["calls"], json!([{"branches": [branch], "recheck": false}]));
  post_as(&client, addr, Named::Id(&id), "/invoke/unsubscribe_view", json!({ "id": subscribed["id"] })).await;
}

/// A commit on `HEAD~1` that adds the `OWNERSHIP` file `text` at the root, built in a temporary index
fn owned_target(repository_path: &str, text: &str) -> String {
  let directory = tempfile::tempdir().unwrap();
  let (file, index) = (directory.path().join("OWNERSHIP"), directory.path().join("index"));
  std::fs::write(&file, text).unwrap();
  let blob = git(repository_path, &["hash-object", "-w", file.to_str().unwrap()]);
  let indexed = |args: &[&str]| {
    let output = git_command().args(args).env("GIT_INDEX_FILE", &index).current_dir(repository_path).output().unwrap();
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap().trim().to_string()
  };
  indexed(&["read-tree", "HEAD~1"]);
  indexed(&["update-index", "--add", "--cacheinfo", &format!("100644,{blob},OWNERSHIP")]);
  let tree = indexed(&["write-tree"]);
  git(repository_path, &["commit-tree", &tree, "-p", "HEAD~1", "-m", "Own the files"])
}
