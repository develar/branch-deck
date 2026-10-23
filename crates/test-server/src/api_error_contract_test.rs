//! The e2e server's side of the adapters' contract: each error of `app_api::api_error::contract_cases`, failed by a
//! command of the server (`run_command`), answers 200 with `{"status": "error", "error": <ApiError>}`, the error the JSON
//! of `crates/app-api/fixtures/api_errors.json`, the same the app rejects with (`src-tauri/src/api_error_contract_test.rs`).

use crate::tauri_command_bridge::run_command;
use app_api::api_error::{CONTRACT_JSON, contract_cases};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use pretty_assertions::assert_eq;
use serde_json::{Map, Value};

#[tokio::test]
async fn a_command_of_the_server_answers_the_api_error_json_of_the_contract() {
  let expected: Map<String, Value> = serde_json::from_str(CONTRACT_JSON).expect("api_errors.json is no JSON object");
  let mut actual = Map::new();
  for (name, error) in contract_cases() {
    let response = run_command("answer the contract case", move || Err::<(), _>(error)).await.unwrap_err().into_response();
    assert_eq!(response.status(), StatusCode::OK, "{name}: an error is an answer, not a failure of the server");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let mut answer: Map<String, Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(answer.remove("status"), Some(Value::from("error")), "{name}");
    actual.insert(name.to_string(), answer.remove("error").unwrap_or_default());
    assert!(answer.is_empty(), "{name}: {answer:?}");
  }
  assert_eq!(Value::Object(actual), Value::Object(expected));
}

#[tokio::test]
async fn a_panicked_command_of_the_server_fails_the_request() {
  let response = run_command("panic", || -> Result<(), String> { panic!("the command panicked") })
    .await
    .unwrap_err()
    .into_response();
  assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
