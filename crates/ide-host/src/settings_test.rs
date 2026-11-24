use crate::settings::SettingsMirror;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn a_key_keeps_the_value_of_its_newest_version_whatever_the_order_the_bridge_hears_them_in() {
  let settings = SettingsMirror::default();
  settings.changed(&json!({ "key": "recentProjects", "value": ["/b"], "version": 8, "origin": "main" }));
  // a snapshot taken before that change keeps its value of the key, and gives the others
  settings.snapshot(&json!({ "recentProjects": ["/a"], "notifyPatronusFailures": false }), 7);
  assert_eq!(settings.value("recentProjects"), json!(["/b"]));
  assert_eq!(settings.value("notifyPatronusFailures"), json!(false));
  settings.changed(&json!({ "key": "notifyPatronusFailures", "value": true, "version": 9, "origin": "main" }));
  settings.changed(&json!({ "key": "notifyPatronusFailures", "value": false, "version": 6, "origin": "main" }));
  assert_eq!(settings.value("notifyPatronusFailures"), json!(true));
  assert_eq!(settings.value("zoom"), json!(null));
}
