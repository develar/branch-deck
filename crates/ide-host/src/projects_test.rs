use super::{Projects, merged};
use pretty_assertions::assert_eq;
use serde_json::json;

fn roots(paths: &[&str]) -> Vec<String> {
  paths.iter().map(|path| (*path).to_owned()).collect()
}

/// The main view shows the root picked last, else the one it shows while it is still a root, else the first
#[test]
fn the_main_view_shows_the_root_picked_last_else_the_one_it_shows_else_the_first() {
  let projects = Projects::default();
  assert!(projects.set_roots(roots(&["/a", "/b"]), None));
  assert_eq!(projects.shown().as_deref(), Some("/a"));
  assert!(!projects.set_roots(roots(&["/a", "/b"]), None), "the same roots change nothing");
  assert!(projects.pick("/b"));
  assert!(!projects.pick("/b"), "the root shown already");
  assert!(!projects.pick("/elsewhere"), "no root of the project");
  assert!(projects.set_roots(roots(&["/c", "/b"]), None));
  assert_eq!(projects.shown().as_deref(), Some("/b"), "the root shown is still a root");
  projects.set_roots(roots(&["/c", "/d"]), Some("/d".to_owned()));
  assert_eq!(projects.shown().as_deref(), Some("/d"), "the root picked last for these roots");
  projects.set_roots(Vec::new(), None);
  assert_eq!(projects.shown(), None);
}

/// The views see the roots as the recent projects, the root shown first, each with what the settings keep of it; a
/// write of the page goes into the recent projects of the desktop app in their place
#[test]
fn the_views_see_the_roots_as_the_recent_projects_and_a_write_keeps_the_order_of_the_desktop_app() {
  let projects = Projects::default();
  projects.set_roots(roots(&["/a", "/b"]), Some("/b".to_owned()));
  let recent = json!([{ "path": "/desktop" }, { "path": "/a", "parkedCollapsed": true }]);
  assert_eq!(projects.seen(&recent), json!([{ "path": "/b" }, { "path": "/a", "parkedCollapsed": true }]));

  let mut change = json!({ "version": 3, "key": "recentProjects", "value": recent, "origin": "main" });
  let mut answer = json!({ "update": null, "settings": [change.clone(), { "version": 4, "key": "theme", "value": "dark", "origin": "main" }] });
  projects.seen_change(&mut change);
  assert_eq!(change["value"], json!([{ "path": "/b" }, { "path": "/a", "parkedCollapsed": true }]));
  projects.seen_changes(&mut answer);
  assert_eq!(answer["settings"][0]["value"], change["value"]);
  assert_eq!(answer["settings"][1]["value"], "dark", "another key stays");

  let written = json!([{ "path": "/a", "parkedCollapsed": false }, { "path": "/b", "reviewRequestsCollapsed": true }]);
  assert_eq!(
    merged(&recent, &written),
    json!([{ "path": "/desktop" }, { "path": "/a", "parkedCollapsed": false }, { "path": "/b", "reviewRequestsCollapsed": true }])
  );
}
