use crate::hosts::{Instances, key_of, label_of, view_of, view_of_url};
use pretty_assertions::assert_eq;
use serde_json::json;

/// The window of an instance is the instance, but the main window, which the main view is
#[test]
fn the_window_of_an_instance_is_the_instance_but_the_main_window() {
  assert_eq!(view_of("review:3"), "review");
  assert_eq!(label_of("review:3"), "review:3");
  assert_eq!(label_of("main:1"), "main");

  let instances = Instances::default();
  assert_eq!(instances.instance_of("main"), None, "no main view shown yet");
  instances.shown("main:1", json!({}));
  instances.shown("review:2", json!({ "data": { "kind": "branch", "branchName": "a" } }));
  assert_eq!(instances.instance_of("main").as_deref(), Some("main:1"));
  assert_eq!(instances.instance_of("review:2").as_deref(), Some("review:2"));
  assert_eq!(instances.params("review:2").unwrap()["data"]["branchName"], "a");
  instances.closed("review:2");
  assert_eq!(instances.instance_of("review:2"), None);
}

/// A review is keyed by its target, so one target is one tab; another view by its window
#[test]
fn a_review_is_keyed_by_its_target() {
  assert_eq!(view_of_url("/review"), "review");
  assert_eq!(view_of_url("/settings?tab=tools"), "settings");
  assert_eq!(key_of("review", "review", &json!({ "kind": "branch", "branchName": "feature" })), "branch:feature");
  assert_eq!(key_of("review", "review", &json!({ "kind": "pullRequest", "number": 12 })), "pullRequest:12");
  assert_eq!(key_of("review", "review", &json!({})), "review");
  assert_eq!(key_of("settings", "settings", &json!({ "branchName": "x" })), "settings");
}
