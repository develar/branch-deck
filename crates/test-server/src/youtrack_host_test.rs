use crate::test_support::{Named, create_repository, post, post_as, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;

/// Invokes the command `name` for the repository `id`, as a page of it does; the answer
async fn invoke(client: &reqwest::Client, addr: SocketAddr, id: &str, name: &str, params: Value) -> Value {
  post_as(client, addr, Named::Id(id), &format!("/invoke/{name}"), params).await
}

/// A test repository whose fake YouTrack has the open issue ABC-1 with the `fields` of `fixture` (and its `signedIn`):
/// its id, its path, the fixture endpoint of its YouTrack and the URL of the fake
async fn with_issue(client: &reqwest::Client, addr: SocketAddr, fixture: Value) -> (String, String, String, Value) {
  let (id, path) = create_repository(client, addr).await;
  let endpoint = format!("/repositories/{id}/youtrack");
  let issue = json!({"key": "ABC-1", "summary": "The settings page loses the timeout", "state": "Open", "fields": fixture["fields"]});
  let mut body = json!({"issues": [issue]});
  if let Some(signed_in) = fixture.get("signedIn") {
    body["signedIn"] = signed_in.clone();
  }
  post(client, addr, &endpoint, body).await;
  let url = client.get(format!("http://{addr}{endpoint}")).send().await.unwrap().json::<Value>().await.unwrap()["url"].clone();
  (id, path, endpoint, url)
}

/// A browser sign-in, a Mark Fixed and an expired sign-in run the real session and service against the fake YouTrack
/// of the repository; the fixture endpoint reads back what reached it
#[tokio::test]
async fn the_youtrack_commands_run_the_real_session_and_service_against_the_fake() {
  let (addr, client) = (start_test_server().await, reqwest::Client::new());
  let fields = json!([{"name": "Priority", "value": "Major"}]);
  let (id, path, fixture, url) = with_issue(&client, addr, json!({"fields": fields})).await;

  let started = invoke(
    &client,
    addr,
    &id,
    "start_youtrack_sign_in",
    json!({"url": format!("{}/issue/ABC-1", url.as_str().unwrap())}),
  )
  .await;
  let authorize_url = started["authorizeUrl"].as_str().unwrap().to_string();
  let waiting = tokio::spawn({
    let (client, id) = (client.clone(), id.clone());
    async move { invoke(&client, addr, &id, "wait_youtrack_sign_in", json!({})).await }
  });
  // the browser: the Hub page redirects back to the listener of the sign-in
  assert_eq!(reqwest::get(&authorize_url).await.unwrap().status(), 200);
  assert_eq!(waiting.await.unwrap(), json!({"login": "jane", "fullName": "Jane Doe"}));
  let status = invoke(&client, addr, &id, "get_youtrack_auth_status", json!({})).await;
  assert_eq!((&status["url"], &status["source"]), (&url, &json!("signIn")));

  // Mark Fixed is an intent of the session of the repository, with its YouTrack
  let intent = json!({"kind": "setIssueState", "key": "ABC-1", "state": "Fixed", "expected": null});
  let plan = post(&client, addr, "/invoke/prepare", json!({"repositoryPath": path, "intent": intent})).await;
  let execute = json!({"repositoryPath": path, "intent": intent, "fingerprint": plan["fingerprint"], "accepted": []});
  let outcome = post(&client, addr, "/invoke/execute", execute).await;
  assert_eq!(outcome["result"], json!({"kind": "issue", "field": "State", "previous": "Open"}), "{outcome}");
  let log: Value = client.get(format!("http://{addr}{fixture}")).send().await.unwrap().json().await.unwrap();
  assert_eq!(log["commands"], json!([{"query": "State Fixed", "issues": ["ABC-1"]}]));
  assert_eq!(log["lookups"], json!(["ABC-1"]));
  assert_eq!(log["pending"], json!(false));

  post(&client, addr, &fixture, json!({"signedIn": "expired"})).await;
  let answer = invoke(&client, addr, &id, "get_youtrack_issue", json!({"key": "ABC-1"})).await;
  assert_eq!(answer, json!({"status": "signInProblem", "problem": "expired"}));
}

/// The title of a new pull request carries the summary of its issue from the YouTrack of the repository, as the
/// app's command reads it with its service: the issue links name the fake, and the app is signed in to it
#[tokio::test]
async fn a_new_pull_request_reads_its_issue_summaries_from_the_youtrack_of_the_repository() {
  let (addr, client) = (start_test_server().await, reqwest::Client::new());
  let (id, path, fixture, url) = with_issue(&client, addr, json!({"fields": [], "signedIn": "signIn"})).await;
  let url = url.as_str().unwrap().to_string();
  let vcs_xml = format!(
    r#"<?xml version="1.0" encoding="UTF-8"?>
<project version="4"><component name="IssueNavigationConfiguration"><option name="links"><list><IssueNavigationLink>
<option name="issueRegexp" value="\b[A-Z]+-\d+\b" /><option name="linkRegexp" value="{url}/issue/$0" />
</IssueNavigationLink></list></option></component></project>"#
  );
  std::fs::create_dir_all(format!("{path}/.idea")).unwrap();
  std::fs::write(format!("{path}/.idea/vcs.xml"), vcs_xml).unwrap();
  let git = |args: &[&str]| {
    let run = test_utils::git_test_utils::git_command().args(args).current_dir(&path).output().unwrap();
    assert!(run.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&run.stderr));
  };
  git(&["checkout", "-q", "-b", "user-name/virtual/ABC-1"]);
  std::fs::write(format!("{path}/parser.js"), "export function parse() {}").unwrap();
  git(&["add", "parser.js"]);
  git(&["commit", "-q", "-m", "ABC-1: Add parser"]);

  let params = json!({"repositoryPath": path, "head": "user-name/virtual/ABC-1", "base": "master"});
  let pull_request = invoke(&client, addr, &id, "get_new_pull_request", params).await;
  assert_eq!(pull_request["title"], json!("ABC-1 The settings page loses the timeout"));
  assert_eq!(
    pull_request["body"],
    json!(format!("[ABC-1]({url}/issue/ABC-1)\n\n1. **(test-branch) foo 2**\n\n2. **Add parser**"))
  );
  let log: Value = client.get(format!("http://{addr}{fixture}")).send().await.unwrap().json().await.unwrap();
  assert_eq!(log["lookups"], json!(["ABC-1"]));
}
