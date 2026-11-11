use crate::file::SettingsFile;
use crate::merge::{Unsaved, changed_keys};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn file(value: &Value) -> SettingsFile {
  SettingsFile::parse(&value.to_string()).unwrap()
}

fn stored(file: &SettingsFile) -> Value {
  Value::Object(file.to_stored())
}

/// A change of the fields `names` of a key, as `Unsaved::record` takes it
type Change = (&'static str, Option<Vec<String>>);

fn fields(names: &[&str]) -> Vec<String> {
  names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn a_save_puts_only_the_keys_and_fields_this_process_changed_on_top_of_the_file_of_the_other() {
  // (what the other process left in the file, this process's copy, what it changed: key, fields or the whole value,
  // what the save writes)
  let table: [(Value, Value, Vec<Change>, Value); 7] = [
    // another key of the other stays
    (
      json!({ "diffView": { "mode": "split" } }),
      json!({ "appSettings": { "zoom": 1.5 } }),
      vec![("appSettings", Some(fields(&["zoom"])))],
      json!({ "appSettings": { "zoom": 1.5 }, "diffView": { "mode": "split" } }),
    ),
    // another field of the same key stays, in appSettings and in a key only the page reads
    (
      json!({ "appSettings": { "autoSyncOnFocus": false }, "panel": { "a": 1, "b": 1 } }),
      json!({ "appSettings": { "zoom": 1.5 }, "panel": { "a": 2 } }),
      vec![("appSettings", Some(fields(&["zoom"]))), ("panel", Some(fields(&["a"])))],
      json!({ "appSettings": { "autoSyncOnFocus": false, "zoom": 1.5 }, "panel": { "a": 2, "b": 1 } }),
    ),
    // a field this process set back at its default goes, the other's field stays
    (
      json!({ "appSettings": { "zoom": 2, "systemTitleBar": true } }),
      json!({}),
      vec![("appSettings", Some(fields(&["zoom"])))],
      json!({ "appSettings": { "systemTitleBar": true } }),
    ),
    // a field this process removed goes, and the key with its last field
    (json!({ "panel": { "a": 1 } }), json!({}), vec![("panel", Some(fields(&["a"])))], json!({})),
    // a whole value replaces the other's
    (
      json!({ "recentProjects": ["/b", "/a"] }),
      json!({ "recentProjects": ["/c", "/a"] }),
      vec![("recentProjects", None)],
      json!({ "recentProjects": ["/c", "/a"] }),
    ),
    // a key this process removed goes
    (json!({ "notes": { "a": 1 }, "kept": true }), json!({}), vec![("notes", None)], json!({ "kept": true })),
    // what this process did not change is the other's, also where its copy is older
    (
      json!({ "appSettings": { "zoom": 3 }, "panel": { "a": 3 } }),
      json!({ "appSettings": { "zoom": 1.5 }, "panel": { "a": 1 }, "gone": true }),
      vec![],
      json!({ "appSettings": { "zoom": 3 }, "panel": { "a": 3 } }),
    ),
  ];
  for (theirs, mine, changes, expected) in table {
    let mut unsaved = Unsaved::default();
    for (version, (key, fields)) in (1..).zip(&changes) {
      unsaved.record(key, fields.clone(), version);
    }
    let merged = unsaved.overlay(file(&theirs), &file(&mine));
    assert_eq!(stored(&merged), expected, "{mine} on top of {theirs} by {changes:?}");
  }
}

#[test]
fn a_save_forgets_the_changes_it_wrote_and_keeps_the_ones_made_after_it_took_its_copy() {
  let mut unsaved = Unsaved::default();
  unsaved.record("appSettings", Some(fields(&["zoom", "autoSyncOnFocus"])), 1);
  unsaved.record("recentProjects", None, 2);
  // a change made while the save of version 2 ran
  unsaved.record("appSettings", Some(fields(&["zoom"])), 3);
  unsaved.record("panel", Some(fields(&["a"])), 3);
  // a field patch after a whole value keeps the whole value its own
  unsaved.record("recentProjects", Some(fields(&["x"])), 4);
  unsaved.forget_saved(2);

  let mut expected = Unsaved::default();
  expected.record("appSettings", Some(fields(&["zoom"])), 3);
  expected.record("panel", Some(fields(&["a"])), 3);
  expected.record("recentProjects", None, 4);
  assert_eq!(unsaved, expected);

  unsaved.forget_saved(4);
  assert_eq!(unsaved, Unsaved::default());
}

#[test]
fn the_keys_that_differ_are_the_changes_of_the_other_process() {
  let before = file(&json!({ "appSettings": { "zoom": 1.5 }, "a": 1, "b": 1, "gone": true }));
  let after = file(&json!({ "appSettings": { "zoom": 1.5 }, "a": 2, "b": 1, "new": true }));
  assert_eq!(changed_keys(&before, &after), ["a", "gone", "new"]);
  let zoomed = file(&json!({ "appSettings": { "zoom": 2 }, "a": 1, "b": 1, "gone": true }));
  assert_eq!(changed_keys(&before, &zoomed), ["appSettings"]);
  assert_eq!(changed_keys(&before, &before.clone()), Vec::<String>::new());
}
