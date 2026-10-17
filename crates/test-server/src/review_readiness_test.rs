use crate::test_support::{create_repository, post, start_test_server};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The answer of a Patronus that evaluated the pull request: the code owner is asked and hasn't approved
fn awaiting_code_owner() -> Value {
  json!({
    "required": true, "mergeRequest": null, "skipped": false,
    "groups": [{"name": "IJPL Code Platform", "paths": ["file2.txt"], "branchWide": false, "pathsSource": "patronus",
      "requiredApprovals": 1, "collectedApprovals": 0, "approved": false, "selfApprovalAllowed": false,
      "reviewers": [{"fullName": "Ilya Korennoy", "username": "ikorennoy", "githubLogin": "ikorennoy", "available": true, "suggested": true, "approvalState": "reviewing"}]}],
  })
}

/// Without a fixture no review is needed (no `OWNERSHIP` files); with one the readiness is computed as the app does,
/// from the reviewers and the start the page sends
#[tokio::test]
async fn the_readiness_comes_from_the_answer_of_the_fixture() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  let tip = GitCommandExecutor::new().execute_command(&["rev-parse", "HEAD"], &repository_path).unwrap();
  let params = json!({
    "repositoryPath": repository_path, "branchPrefix": "user-name", "baselineBranch": "HEAD~1",
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
  assert_eq!(answer[0]["requirement"]["groups"][0]["approved"], json!(false));

  post(
    &client,
    addr,
    &format!("/repositories/{id}/review-answers"),
    json!({"unavailable": "Patronus did not answer in time"}),
  )
  .await;
  let answer = post(&client, addr, "/invoke/get_branch_review_readiness", params).await;
  assert_eq!(
    (answer[0]["requirement"].clone(), answer[0]["readiness"].clone()),
    (Value::Null, json!({"kind": "unavailable", "reason": "error", "message": "Patronus did not answer in time"}))
  );

  let calls: Value = client
    .get(format!("http://{addr}/repositories/{id}/review-answers"))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
  assert_eq!(calls["calls"].as_array().map(Vec::len), Some(3));
  assert_eq!(calls["calls"][0], json!({"branches": ["IJPL-1"], "recheck": false}));
}
