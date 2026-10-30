use crate::test_support::{Named, channel_events, command, create_repository, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::Path;

/// The events of `get_review_plan` of `params` on its channel, in order, and its answer
async fn plan_events(client: &reqwest::Client, addr: SocketAddr, params: &Value) -> (Vec<Value>, Value) {
  channel_events(client, addr, "get_review_plan", params).await
}

/// Sets `branchdeck.reviewExperimental` of the repository at `path`, which the global config of the server turns off
fn review_plan_on(path: &str, on: bool) {
  let value = if on { "true" } else { "false" };
  let status = test_utils::git_test_utils::git_command().args(["config", "branchdeck.reviewExperimental", value]).current_dir(path).status();
  assert!(status.unwrap().success());
}

/// Commits `text` as `src/api.rs` on the checked out branch: its id
fn commit_api(path: &str, text: &str, subject: &str) -> String {
  std::fs::create_dir_all(Path::new(path).join("src")).unwrap();
  std::fs::write(Path::new(path).join("src/api.rs"), text).unwrap();
  for args in [&["add", "src/api.rs"][..], &["commit", "-q", "-m", subject][..]] {
    assert!(test_utils::git_test_utils::git_command().args(args).current_dir(path).status().unwrap().success());
  }
  rev_parse(path, "HEAD")
}

fn rev_parse(path: &str, rev: &str) -> String {
  let output = test_utils::git_test_utils::git_command().args(["rev-parse", rev]).current_dir(path).output().unwrap();
  String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// The review window's plan through the server's app-api, as the page asks it: Mark Reviewed keeps the branch's base and
/// tip, a commit after it is the only one the plan since the review reads, with the usages of its changed declaration
/// from the accio fixture of the repository (`FixtureIndex`): the one in the changed file updated, those in another
/// file left outside, which the file breaks; the plan without the usages comes first, a plan asked again from the memory
/// of the plans, `disabled` for a repository that turns the plan off; a branch that is gone and a range asked since its
/// review are the caller's errors
#[tokio::test]
async fn the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (_, path) = create_repository(&client, addr).await;
  let invoke = |name: &str, params: Value| {
    let request = command(&client, addr, Named::ByPath, name, &params);
    async move { request.send().await.unwrap().json::<Value>().await.unwrap() }
  };
  let output = test_utils::git_test_utils::git_command()
    .args(["symbolic-ref", "--short", "HEAD"])
    .current_dir(&path)
    .output()
    .unwrap();
  let branch = String::from_utf8(output.stdout).unwrap().trim().to_string();
  let base = rev_parse(&path, "HEAD");
  review_plan_on(&path, true);
  let reviewed = commit_api(&path, "pub fn sum(a: i32) -> i32 {\n  a\n}\n", "Add sum");

  let marked = invoke("mark_branch_reviewed", json!({ "repositoryPath": path, "branch": branch, "base": base })).await;
  assert_eq!(
    (&marked["branch"], &marked["base"], &marked["tip"], &marked["changed"]),
    (&json!(branch), &json!(base), &json!(reviewed), &json!(true)),
    "{marked}"
  );

  let head = commit_api(&path, "pub fn sum(a: i32, b: i32) -> i32 {\n  a + b\n}\n", "Take b");
  let usage = |file: &str, line: u32| json!({ "file": file, "line": line, "column": 3, "snippet": "sum(1, 2)", "kind": "call" });
  let usages = [usage("src/main.rs", 3), usage("src/api.rs", 1), usage("src/main.rs", 7)];
  std::fs::write(Path::new(&path).join(".git/branch-deck-accio.json"), json!({ "references": { "sum": usages } }).to_string()).unwrap();

  let subject = json!({ "kind": "branch", "name": branch, "base": base });
  let params = json!({ "repositoryPath": path, "subject": subject, "sinceReviewed": true });
  let (events, answer) = plan_events(&client, addr, &params).await;
  // the plan without the usages while the index is asked, then with them
  let pending: Vec<(&Value, &Value)> = events.iter().map(|event| (&event["kind"], &event["pending"])).collect();
  assert_eq!((pending, &answer["status"]), (vec![(&json!("plan"), &json!(true)), (&json!("plan"), &json!(false))], &json!("ok")), "{events:?}");
  assert_eq!(events[0]["plan"]["api"][0]["usages"], Value::Null, "{events:?}");
  let plan = &events[1]["plan"];
  let interdiff = &plan["interdiff"];
  assert_eq!(
    (
      &interdiff["reviewedTip"],
      &interdiff["commits"][0]["id"],
      &interdiff["commits"][0]["state"],
      interdiff["commits"].as_array().map(Vec::len)
    ),
    (&json!(reviewed), &json!(head), &json!("added"), Some(1)),
    "{plan}"
  );
  assert_eq!(interdiff["scope"], json!({ "base": reviewed, "head": head, "carried": false }), "{plan}");
  let api = &plan["api"][0];
  assert_eq!(
    (&api["name"], &api["kind"], &api["usages"], &plan["indexNote"]),
    (
      &json!("sum"),
      &json!("changed"),
      &json!({ "inside": 1, "outside": 2, "outsideSites": [{ "path": "src/main.rs", "line": 3 }, { "path": "src/main.rs", "line": 7 }], "complete": true, "updated": false }),
      &json!(null)
    ),
    "{plan}"
  );
  assert_eq!((&plan["files"][0]["path"], &plan["files"][0]["breaks"]), (&json!("src/api.rs"), &json!(2)), "{plan}");
  assert_eq!(plan["order"], json!(["src/api.rs"]), "{plan}");

  // asked again, the plan comes from the memory of the plans whole, in one event; a repository that turns the plan off
  // gets `disabled`
  let (again, _) = plan_events(&client, addr, &params).await;
  assert_eq!((again.len(), &again[0]["pending"], &again[0]["plan"]), (1, &json!(false), plan), "{again:?}");
  review_plan_on(&path, false);
  let (off, answer) = plan_events(&client, addr, &params).await;
  assert_eq!((off, &answer["status"]), (vec![json!({ "kind": "disabled" })], &json!("ok")));
  review_plan_on(&path, true);

  let gone = invoke("mark_branch_reviewed", json!({ "repositoryPath": path, "branch": "no-such-branch" })).await;
  assert_eq!(gone["error"]["code"], "unknown_branch", "{gone}");
  let range = json!({ "kind": "range", "base": base, "head": head });
  let refused = invoke("get_review_plan", json!({ "repositoryPath": path, "subject": range, "sinceReviewed": true })).await;
  assert_eq!(refused["error"]["code"], "invalid_arguments", "{refused}");
  let no_base = invoke("mark_branch_reviewed", json!({ "repositoryPath": path, "branch": branch, "base": "nope" })).await;
  assert_eq!(no_base["error"]["code"], "invalid_arguments", "{no_base}");
  let unknown = json!({ "kind": "range", "base": "nope", "head": head });
  let unknown = invoke("get_review_plan", json!({ "repositoryPath": path, "subject": unknown })).await;
  assert_eq!(unknown["error"]["code"], "invalid_arguments", "{unknown}");
}
