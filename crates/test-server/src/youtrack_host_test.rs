use crate::test_support::{create_repository, post, start_test_server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;

/// Invokes the command `name` for the repository `id`, as a page of it does; the answer
async fn invoke(client: &reqwest::Client, addr: SocketAddr, id: &str, name: &str, params: Value) -> Value {
  let response = client
    .post(format!("http://{addr}/invoke/{name}"))
    .header(crate::REPOSITORY_ID_HEADER, id)
    .json(&params)
    .send()
    .await
    .unwrap();
  assert_eq!(response.status(), 200, "{name}");
  response.json().await.unwrap()
}

/// A browser sign-in, a Mark Fixed and an expired sign-in run the real session and service against the fake YouTrack
/// of the repository; the fixture endpoint reads back what reached it
#[tokio::test]
async fn the_youtrack_commands_run_the_real_session_and_service_against_the_fake() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, _) = create_repository(&client, addr).await;
  let fixture = format!("/repositories/{id}/youtrack");
  let issue = json!({"key": "ABC-1", "summary": "The settings page loses the timeout", "state": "Open", "fields": [{"name": "Priority", "value": "Major"}]});
  post(&client, addr, &fixture, json!({"issues": [issue]})).await;
  let url = client.get(format!("http://{addr}{fixture}")).send().await.unwrap().json::<Value>().await.unwrap()["url"].clone();

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

  let change = invoke(&client, addr, &id, "set_youtrack_issue_state", json!({"key": "ABC-1", "state": "Fixed", "expected": null})).await;
  assert_eq!(change, json!({"field": "State", "previous": "Open"}));
  let log: Value = client.get(format!("http://{addr}{fixture}")).send().await.unwrap().json().await.unwrap();
  assert_eq!(log["commands"], json!([{"query": "State Fixed", "issues": ["ABC-1"]}]));
  assert_eq!(log["lookups"], json!(["ABC-1"]));
  assert_eq!(log["pending"], json!(false));

  post(&client, addr, &fixture, json!({"signedIn": "expired"})).await;
  let answer = invoke(&client, addr, &id, "get_youtrack_issue", json!({"key": "ABC-1"})).await;
  assert_eq!(answer, json!({"status": "signInProblem", "problem": "expired"}));
}
