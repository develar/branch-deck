use crate::review_fixtures::fixture_review_answer;
use crate::test_support::{create_repository, post, start_test_server};
use patronus::client::PreMergeCheckResponse;
use patronus::review::to_requirement;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sync_types::patronus::PatronusReviewRequirement;
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

/// The server, a repository of the `simple` template: its id, its path and the tip of `HEAD`
async fn setup() -> (std::net::SocketAddr, reqwest::Client, String, String, String) {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  let tip = git(&repository_path, &["rev-parse", "HEAD"]);
  (addr, client, id, repository_path, tip)
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
  ownership.groups[0].paths_source = sync_types::patronus::ReviewPathsSource::Ownership;
  fixture_review_answer(&ownership).unwrap_err();
}

/// The readiness comes through the real core from the fake Patronus: without an answer for the branch the owners of its
/// paths review (none without `OWNERSHIP` files); with one the answer of the branch whose tip the pre-check names,
/// judged with the reviewers and the start the page sends; the pre-check names the pull request of the start
#[tokio::test]
async fn the_readiness_comes_from_the_fake_patronus_through_the_real_core() {
  let (addr, client, id, repository_path, tip) = setup().await;
  git(&repository_path, &["update-ref", "refs/remotes/origin/master", "HEAD~1"]);
  git(&repository_path, &["update-ref", "refs/heads/user-name/virtual/IJPL-1", "HEAD"]);
  post(
    &client,
    addr,
    &format!("/repositories/{id}/patronus"),
    json!({"availability": {"status": "available"}, "robots": [], "githubRepository": "acme/monorepo"}),
  )
  .await;
  let params = json!({
    "repositoryPath": repository_path, "branchPrefix": "user-name", "baselineBranch": "origin/master",
    "branches": [{"branchName": "IJPL-1", "tip": tip, "start": {"kind": "pullRequest", "number": 7}, "author": "user-name",
      "reviewers": [{"login": "junie-jetbrains", "state": "approved", "requested": false}, {"login": "ikorennoy", "state": "requested", "requested": true}]}],
  });

  let answer = post(&client, addr, "/invoke/get_branch_review_readiness", params.clone()).await;
  assert_eq!((answer[0]["tip"].clone(), answer[0]["readiness"].clone()), (json!(tip), json!({"kind": "notRequired"})));

  post(
    &client,
    addr,
    &format!("/repositories/{id}/review-answers"),
    json!({"answers": {"IJPL-1": awaiting_code_owner()}}),
  )
  .await;
  let answer = post(&client, addr, "/invoke/get_branch_review_readiness", params.clone()).await;
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

  let log: Value = client
    .get(format!("http://{addr}/repositories/{id}/review-answers"))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
  assert_eq!(
    log["calls"],
    json!([{"branches": ["IJPL-1"], "recheck": false}, {"branches": ["IJPL-1"], "recheck": false}])
  );
  let check = json!({"tip": tip, "pullRequest": {"repository": "acme/monorepo", "number": 7}});
  assert_eq!(log["checks"], json!([check, check]));
}

/// A branch the fixture doesn't answer is reviewed by the owners of its paths by the `OWNERSHIP` files of the target,
/// one group per owner with its paths, none approved
#[tokio::test]
async fn a_branch_without_an_answer_is_reviewed_by_the_owners_of_its_paths() {
  let (addr, client, id, repository_path, tip) = setup().await;
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
  let params = json!({
    "repositoryPath": repository_path, "branchPrefix": "user-name", "baselineBranch": "origin/master",
    "branches": [{"branchName": "IJPL-1", "tip": tip, "start": {"kind": "rest"}}],
  });
  let answer = post(&client, addr, "/invoke/get_branch_review_readiness", params).await;
  assert_eq!(answer[0]["requirement"]["required"], json!(true), "{answer}");
  let group = &answer[0]["requirement"]["groups"][0];
  assert_eq!(
    (group["name"].clone(), group["paths"].clone(), group["pathsSource"].clone()),
    (json!("IJPL Code Platform"), json!([changed]), json!("ownership"))
  );
  assert_eq!(answer[0]["readiness"]["kind"], json!("missing"));
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
