use crate::test_support::{create_repository, delete_repository, start_test_server_with_state};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;

/// A config command of the page mock for the repository `id`: its answer
async fn invoke(client: &reqwest::Client, addr: SocketAddr, id: &str, command: &str, params: Value) -> Value {
  let response = client
    .post(format!("http://{addr}/invoke/{command}"))
    .header(crate::REPOSITORY_ID_HEADER, id)
    .json(&params)
    .send()
    .await
    .unwrap();
  assert_eq!(response.status(), 200, "{command}");
  response.json().await.unwrap()
}

/// A global write of a test goes to the global config of its repository, which the reads of that repository show and
/// no other repository reads; a local value still wins, and the file goes with the repository
#[tokio::test]
async fn a_global_write_goes_to_the_own_file_of_the_repository() {
  let (addr, state) = start_test_server_with_state().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let (other_id, other_path) = create_repository(&client, addr).await;

  let answer = invoke(
    &client,
    addr,
    &id,
    "set_repository_config",
    json!({ "repositoryPath": path, "change": { "key": "resolveByWords", "value": false }, "scope": "global" }),
  )
  .await;
  assert_eq!(answer["resolveByWords"]["value"], false, "{answer}");
  assert_eq!(answer["resolveByWords"]["scope"], "global", "{answer}");
  let global_config = state.global_config_of(&id);
  let text = std::fs::read_to_string(&global_config).unwrap();
  assert!(text.contains("resolveByWords = false"), "{text}");

  let other = invoke(&client, addr, &other_id, "get_repository_config", json!({ "repositoryPath": other_path })).await;
  assert_eq!(other["resolveByWords"]["value"], true, "{other}");
  assert_eq!(other["resolveByWords"]["scope"], Value::Null, "{other}");

  let answer = invoke(
    &client,
    addr,
    &id,
    "set_repository_config",
    json!({ "repositoryPath": path, "change": { "key": "resolveByWords", "value": true } }),
  )
  .await;
  assert_eq!(answer["resolveByWords"]["value"], true, "{answer}");
  assert_eq!(answer["resolveByWords"]["scope"], "local", "{answer}");
  assert_eq!(answer["resolveByWords"]["fallbackScope"], "global", "{answer}");

  delete_repository(&client, addr, &id, &path).await;
  assert!(!global_config.exists());
}
