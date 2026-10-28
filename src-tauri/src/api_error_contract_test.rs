//! The app's side of the adapters' contract: each error of `app_api::api_error::contract_cases`, failed by a blocking
//! step of a command (`run_blocking`) and rejected the way Tauri rejects a command (`InvokeError` of the error), is the
//! JSON of `crates/app-api/fixtures/api_errors.json` with the id of the operation it failed in, the same the e2e server
//! answers (`crates/test-server/src/api_error_contract_test.rs`): both hosts run the bodies of app-api in their
//! operations, so an error of either names its operation.

use app_api::api_error::{CONTRACT_JSON, contract_cases};
use app_api::operation::run_blocking;
use pretty_assertions::assert_eq;
use serde_json::{Map, Value};
use tauri::ipc::InvokeError;

#[tokio::test]
async fn a_command_of_the_app_rejects_with_the_api_error_json_of_the_contract() {
  let expected: Map<String, Value> = serde_json::from_str(CONTRACT_JSON).expect("api_errors.json is no JSON object");
  let mut actual = Map::new();
  for (name, error) in contract_cases() {
    let mut rejected = run_blocking(move || Err::<(), _>(error)).await.unwrap_err();
    let operation_id = rejected.operation_id.take();
    assert_eq!(operation_id.as_ref().map(String::len), Some(26), "{name}: the error names the operation of its log lines");
    actual.insert(name.to_string(), InvokeError::from(rejected).0);
  }
  assert_eq!(Value::Object(actual), Value::Object(expected));
}
