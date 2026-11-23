//! The root commands on a host of the fixtures (`call.rs`; openspec/specs/repository/backend-protocol/spec.md,
//! "Commands", "Events")

use crate::call::PROTOCOL_METHODS;
use crate::corpus_test::Fixture;
use app_api::commands::COMMANDS;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use test_utils::git_test_utils::git_command;

fn command(id: u32, name: &str, window: Option<&str>, args: &Value) -> Value {
  let mut params = json!({ "channel": "ahp-root://", "args": args });
  if let Some(window) = window {
    params["window"] = json!(window);
  }
  json!({ "jsonrpc": "2.0", "id": id, "method": format!("x-branch-deck/{name}"), "params": params })
}

fn head(repo: &str) -> String {
  let output = git_command().args(["rev-parse", "HEAD"]).current_dir(repo).output().unwrap();
  String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// A read answers its value as the command of the app answers it
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_read_answers_its_value() {
  let fixture = Fixture::new().await;
  let link = fixture.bridge().await;
  let args = json!({ "params": { "repositoryPath": fixture.repo, "commitId": head(&fixture.repo) } });
  let answer = link.ask(&command(2, "get_commit_files", None, &args)).await;
  let files = &answer["result"]["value"]["files"];
  assert_eq!(files[0]["path"], "file2.txt", "{answer:#}");
}

/// A sync of a repository with a rebase in progress answers the refusal as its result, never a JSON-RPC error
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_sync_answers_its_error() {
  let fixture = Fixture::new().await;
  let link = fixture.bridge().await;
  let git_dir = std::path::Path::new(&fixture.repo).join(".git");
  std::fs::create_dir_all(git_dir.join("rebase-merge")).unwrap();
  std::fs::write(git_dir.join("rebase-merge").join("head-name"), "refs/heads/master\n").unwrap();
  let args = json!({ "params": { "repositoryPath": fixture.repo, "reason": "manual" } });
  let answer = link.ask(&command(2, "sync", Some("main"), &args)).await;
  assert!(answer.get("error").is_none(), "{answer:#}");
  assert_eq!(answer["result"]["error"]["code"], "operation_in_progress", "{answer:#}");
}

/// A method of the protocol of one word could equal a command added later: no name of the registry is one (Key decision
/// 4 of docs/design/backend-protocol.md)
#[test]
fn no_command_takes_a_method_of_the_protocol() {
  let taken: Vec<&str> = COMMANDS.iter().map(|command| command.name).filter(|name| PROTOCOL_METHODS.contains(name)).collect();
  assert_eq!(taken, Vec::<&str>::new());
}

/// A setting changed in a window of the IDE reaches every window of the app as `settings_changed`
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_setting_changed_in_the_ide_reaches_every_window_of_the_app() {
  let fixture = Fixture::new().await;
  let (app, ide) = (fixture.shell().await, fixture.bridge().await);
  let args = json!({ "params": { "patch": { "kind": "value", "key": "theme", "value": "dark" } } });
  let answer = ide.ask(&command(2, "set_setting", Some("main"), &args)).await;
  assert!(answer["result"].get("value").is_some(), "{answer:#}");
  let event = app
    .until(|frame| frame["method"] == "x-branch-deck/event" && frame["params"]["event"] == "settings_changed")
    .await
    .pop()
    .unwrap();
  assert!(event["params"].get("window").is_none(), "to every window of the app: {event:#}");
  assert_eq!(event["params"]["payload"]["key"], "theme");
}
