use super::{path_of_uri, tools};
use pretty_assertions::assert_eq;

/// A root is a `file://` URI: its path, decoded; a Windows root without the slash before its drive
#[test]
fn the_path_of_a_root_is_its_file_uri_decoded() {
  assert_eq!(path_of_uri("file:///Users/me/my%20repo").as_deref(), Some("/Users/me/my repo"));
  assert_eq!(path_of_uri("file:///C:/work/repo").as_deref(), Some("C:/work/repo"));
  assert_eq!(path_of_uri("https://example.com"), None);
}

/// The two tools are only for the app
#[test]
fn both_tools_are_only_for_the_app() {
  let tools = tools();
  let names: Vec<_> = tools.as_array().unwrap().iter().map(|tool| tool["name"].as_str().unwrap()).collect();
  assert_eq!(names, ["bd_invoke", "bd_bootstrap"]);
  assert!(tools.as_array().unwrap().iter().all(|tool| tool["_meta"]["ui"]["visibility"] == serde_json::json!(["app"])));
}
