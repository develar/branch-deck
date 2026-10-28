use super::arguments;
use crate::test_support::{Named, command, create_repository, create_repository_from, start_test_server};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The arguments of a command are its `params` (which the page mock sends alone), or the object of its named arguments
#[test]
fn the_arguments_of_a_command_are_its_params_or_its_named_arguments() {
  let args = arguments::<app_api::commands::get_commit_files::Command>(br#"{ "repositoryPath": "/r", "commitId": "c" }"#).unwrap();
  assert_eq!((args.params.repository_path.as_str(), args.params.commit_id.as_str()), ("/r", "c"));

  let args = arguments::<app_api::commands::get_archived_branch_commits::Command>(br#"{ "repositoryPath": "/r", "branchName": "b" }"#).unwrap();
  assert_eq!((args.repository_path.as_str(), args.branch_name.as_str()), ("/r", "b"));

  assert!(arguments::<app_api::commands::get_archived_branch_commits::Command>(br#"{ "repositoryPath": "/r" }"#).is_err());
}

/// A read of app-api runs the app's body in the context of the test repository: the identity of the test machine
/// outside a repository, the template's inside it, and the app's error for a path that is gone, with its operation
#[tokio::test]
async fn a_read_of_app_api_answers_what_the_app_answers() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;

  let invoke = |named: Named<'_>, name: &str, params: Value| {
    let request = command(&client, addr, named, name, &params);
    async move { request.send().await.unwrap().json::<Value>().await.unwrap() }
  };
  let setup = invoke(Named::Id(&id), "check_git_setup", json!({ "repositoryPath": "" })).await;
  assert_eq!(setup, json!({ "git_error": null, "identity_error": null, "suggested_branch_prefix": "test-user" }));
  let setup = invoke(Named::ByPath, "check_git_setup", json!({ "repositoryPath": path })).await;
  assert_eq!(setup["suggested_branch_prefix"], "test", "{setup}");

  let (gone_id, gone) = create_repository_from(&client, addr, "NO_REPO").await;
  let mut answer = invoke(Named::Id(&gone_id), "resolve_branch_prefix", json!({ "repositoryPath": gone })).await;
  let operation_id = answer["error"]["operation_id"].take();
  assert_eq!(operation_id.as_str().map(str::len), Some(26), "{answer}");
  assert_eq!(answer["error"]["message"], format!("Repository not accessible: {gone}"), "{answer}");
}

/// A raw command answers the bytes of the app's body, as Tauri answers a `tauri::ipc::Response`
#[tokio::test]
async fn a_raw_command_answers_its_bytes() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (_, path) = create_repository(&client, addr).await;
  let blob = GitCommandExecutor::new().execute_command(&["rev-parse", "HEAD:file1.txt"], &path).unwrap();

  let params = json!({ "repositoryPath": path, "blob": { "id": blob.trim(), "path": null }, "filePath": "file1.txt", "fromLine": 0, "toLine": 1, "text": null });
  let response = command(&client, addr, Named::ByPath, "get_blob_syntax", &params).send().await.unwrap();
  assert_eq!(response.status(), 200);
  assert_eq!(response.headers()["content-type"], "application/octet-stream");
  assert!(!response.bytes().await.unwrap().is_empty());
}
