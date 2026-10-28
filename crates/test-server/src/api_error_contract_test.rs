//! The e2e server's side of the adapters' contract: each error of `app_api::api_error::contract_cases`, failed by a
//! command of the server (`caught`, which runs every command of `invoke.rs`), answers 200 with `{"status": "error",
//! "error": <ApiError>}`, the error the JSON of `crates/app-api/fixtures/api_errors.json`, the same the app rejects with
//! (`src-tauri/src/api_error_contract_test.rs`). A real route of a command (`/invoke/<name>`, `invoke::run`) answers
//! that JSON with the id of the operation the body failed in, as the app does, on the stream of a channel too.

use crate::invoke::{answer_with_progress, caught, erase, failed};
use crate::test_support::{Messages, Named, command, create_repository_from, start_test_server};
use app_api::api_error::{CONTRACT_JSON, api_error, contract_cases, no_branch_prefix};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use pretty_assertions::assert_eq;
use serde_json::{Map, Value, json};

/// `error` as the contract has it, with the id of the operation `answered` names, which must be one: the JSON of the
/// `ApiError` the app rejects the same failure with
fn with_its_operation(mut error: domain_types::error::ApiError, answered: &Value) -> Value {
  let id = answered["operation_id"].as_str().unwrap_or_else(|| panic!("the error names no operation: {answered}"));
  assert_eq!(id.len(), 26, "{answered}");
  error.operation_id = Some(id.to_owned());
  serde_json::to_value(error).unwrap()
}

/// The route of a command answers the error of its body, with the operation it failed in: a command of a session
/// refused by its frame for a repository without a prefix, answered alone and on the stream of its channel
#[tokio::test]
async fn a_route_of_a_command_answers_the_api_error_json_with_its_operation() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository_from(&client, addr, "simple_no_prefix").await;

  let response = command(&client, addr, Named::ByPath, "sync", &json!({ "repositoryPath": path, "reason": "manual" }))
    .send()
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::OK, "an error is an answer, not a failure of the server");
  let answer: Value = response.json().await.unwrap();
  assert_eq!(answer, json!({ "status": "error", "error": with_its_operation(no_branch_prefix(), &answer["error"]) }));

  let request = command(&client, addr, Named::Id(&id), "subscribe_view", &json!({ "repositoryPath": path }));
  let mut messages = Messages::of_channel(request, 3).await;
  let message = messages.next().await;
  let result = &message["result"];
  assert_eq!(
    message,
    json!({ "result": { "status": "error", "error": with_its_operation(no_branch_prefix(), &result["error"]) } })
  );
  assert_eq!(messages.next_or_end().await, None, "the stream ends with the answer");
}

#[tokio::test]
async fn a_command_of_the_server_answers_the_api_error_json_of_the_contract() {
  let expected: Map<String, Value> = serde_json::from_str(CONTRACT_JSON).expect("api_errors.json is no JSON object");
  let mut actual = Map::new();
  for (name, error) in contract_cases() {
    let response = caught(erase(async move { Err::<(), _>(api_error(error)) })).await.unwrap_err().into_response();
    assert_eq!(response.status(), StatusCode::OK, "{name}: an error is an answer, not a failure of the server");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let mut answer: Map<String, Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(answer.remove("status"), Some(Value::from("error")), "{name}");
    actual.insert(name.to_string(), answer.remove("error").unwrap_or_default());
    assert!(answer.is_empty(), "{name}: {answer:?}");
  }
  assert_eq!(Value::Object(actual), Value::Object(expected));
}

/// A command that panicked has no answer: the request fails (500), and on the stream of its channel the answer says so,
/// which the page mock throws as a bug of the server
#[tokio::test]
async fn a_panicked_command_of_the_server_fails_the_request() {
  let response = caught(erase::<()>(async { panic!("the command panicked") })).await.unwrap_err().into_response();
  assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

  let response = answer_with_progress(Some(1), |_progress| async {
    caught(erase::<()>(async { panic!("the command panicked") })).await.map(|_| axum::Json(()))
  })
  .await;
  let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
  let body = String::from_utf8(body.to_vec()).unwrap();
  assert!(body.contains(r#"data: {"result":{"panicked":true}}"#), "{body}");
}

/// A command with a channel (`invoke.rs`) fails on its event stream with the same JSON:
/// `{"result": {"status": "error", "error": <ApiError>}}`, as the page mock hands it to the bindings (`subscribe_view`)
#[tokio::test]
async fn a_command_with_a_channel_answers_the_api_error_json_of_the_contract_on_its_stream() {
  let expected: Map<String, Value> = serde_json::from_str(CONTRACT_JSON).expect("api_errors.json is no JSON object");
  let mut actual = Map::new();
  for (name, error) in contract_cases() {
    let response = answer_with_progress(Some(1), move |_progress| async move { Err::<axum::Json<()>, _>(failed("answer the contract case", error)) }).await;
    assert_eq!(response.status(), StatusCode::OK, "{name}");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    let messages: Vec<Value> = body
      .lines()
      .filter_map(|line| line.strip_prefix("data: "))
      .map(|data| serde_json::from_str(data).unwrap())
      .collect();
    let [message] = messages.as_slice() else {
      panic!("{name}: one message, the answer: {body}");
    };
    let mut result: Map<String, Value> = serde_json::from_value(message["result"].clone()).unwrap();
    assert_eq!(result.remove("status"), Some(Value::from("error")), "{name}");
    actual.insert(name.to_string(), result.remove("error").unwrap_or_default());
    assert!(result.is_empty(), "{name}: {result:?}");
  }
  assert_eq!(Value::Object(actual), Value::Object(expected));
}
