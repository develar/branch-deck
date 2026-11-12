use super::{path_of_uri, tools};
use pretty_assertions::assert_eq;

/// A root is a `file://` URI: its path, decoded; a Windows root without the slash before its drive
#[test]
fn the_path_of_a_root_is_its_file_uri_decoded() {
  assert_eq!(path_of_uri("file:///Users/me/my%20repo").as_deref(), Some("/Users/me/my repo"));
  assert_eq!(path_of_uri("file:///C:/work/repo").as_deref(), Some("C:/work/repo"));
  assert_eq!(path_of_uri("https://example.com"), None);
}

/// The app file the installer writes (the example of the contract)
const APP_FILE: &str = include_str!("../../../docs/design/ide-embedding/protocol/examples/branch-deck.yaml");

/// Every tool is only for the app: the two of the page, and the tool each action of the app file runs
#[test]
fn every_tool_is_only_for_the_app_and_each_action_of_the_app_file_has_its_tool() {
  let tools = tools();
  let names: Vec<_> = tools.as_array().unwrap().iter().map(|tool| tool["name"].as_str().unwrap()).collect();
  assert_eq!(names, ["bd_invoke", "bd_bootstrap", "sync", "open_command_palette"]);
  assert!(tools.as_array().unwrap().iter().all(|tool| tool["_meta"]["ui"]["visibility"] == serde_json::json!(["app"])));
  let run: Vec<_> = APP_FILE.lines().filter_map(|line| line.trim().strip_prefix("run: { tool: ")?.strip_suffix(" }")).collect();
  assert_eq!(run, ["sync", "open_command_palette"], "the actions of the app file that run a tool");
  assert!(run.iter().all(|tool| names.contains(tool)));
}
