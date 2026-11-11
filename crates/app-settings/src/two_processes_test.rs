//! Two writers on one app data directory, as the app and `brd mcp` of an IDE are: each a `SettingsService` of its own

use crate::service::{OTHER_PROCESS_ORIGIN, SettingsService};
use domain_types::settings::{SettingsChange, SettingsPatch};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

/// Longer than any test: a save happens only by a flush or a drop
const NEVER: Duration = Duration::from_hours(1);

/// How long a test waits for the watch to tell a change: the event under any load of the machine
const EVENT_DEADLINE: Duration = Duration::from_mins(1);

fn app_fields(value: Value) -> SettingsPatch {
  let Value::Object(fields) = value else { panic!("fields are an object") };
  SettingsPatch::Fields {
    key: "appSettings".to_owned(),
    fields,
  }
}

fn whole(key: &str, value: Value) -> SettingsPatch {
  SettingsPatch::Value { key: key.to_owned(), value }
}

fn read(path: &Path) -> Value {
  serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn writer(directory: &tempfile::TempDir, debounce: Duration) -> SettingsService {
  SettingsService::open_with_debounce(directory.path().to_path_buf(), debounce).unwrap()
}

/// The changes of another process `service` tells, in the order it tells them
fn external_changes(service: &SettingsService) -> Receiver<SettingsChange> {
  let (sender, receiver) = std::sync::mpsc::channel();
  service.on_external_change(move |change| sender.send(change.clone()).unwrap());
  receiver
}

#[test]
fn two_writers_that_change_different_keys_and_fields_keep_every_change() {
  let directory = tempfile::tempdir().unwrap();
  let app = writer(&directory, NEVER);
  let ide = writer(&directory, NEVER);

  app.set(app_fields(json!({ "zoom": 1.5 })), "main").unwrap();
  app.set(whole("recentProjects", json!(["/a"])), "main").unwrap();
  ide.set(app_fields(json!({ "autoSyncOnFocus": false })), "main").unwrap();
  ide.set(whole("diffView", json!({ "mode": "split" })), "review").unwrap();
  app.flush().unwrap();
  ide.flush().unwrap();

  let both = json!({
    "appSettings": { "zoom": 1.5, "autoSyncOnFocus": false },
    "recentProjects": ["/a"],
    "diffView": { "mode": "split" },
  });
  assert_eq!(read(&directory.path().join("settings.json")), both);
  // the save of the second took the change of the first into its memory
  let (_, snapshot) = ide.snapshot();
  assert_eq!(snapshot["appSettings"]["zoom"], json!(1.5));
  assert_eq!(snapshot["recentProjects"], json!(["/a"]));
}

#[test]
fn two_writers_that_save_at_once_lose_no_change_of_the_other() {
  let directory = tempfile::tempdir().unwrap();
  let writers = [Arc::new(writer(&directory, Duration::ZERO)), Arc::new(writer(&directory, Duration::ZERO))];
  let threads: Vec<_> = writers
    .iter()
    .enumerate()
    .map(|(index, service)| {
      let service = Arc::clone(service);
      std::thread::spawn(move || {
        for step in 0..30 {
          service.set(app_fields(json!({ format!("field{index}"): step })), "main").unwrap();
          // the repositories the syncs of each record, a field of `repositoryState` each
          let state = json!({ format!("/repo{index}/{step}"): { "lastSyncTime": step } });
          let Value::Object(fields) = state else { unreachable!() };
          service
            .set(
              SettingsPatch::Fields {
                key: "repositoryState".to_owned(),
                fields,
              },
              "main",
            )
            .unwrap();
          service.flush().unwrap();
        }
      })
    })
    .collect();
  for thread in threads {
    thread.join().unwrap();
  }

  let settings = read(&directory.path().join("settings.json"));
  assert_eq!(settings["appSettings"]["field0"], json!(29));
  assert_eq!(settings["appSettings"]["field1"], json!(29));
  let state = read(&directory.path().join("repository-state.json"));
  let recorded = state["repositoryState"].as_object().unwrap().len();
  assert_eq!(recorded, 60, "every repository each writer recorded");
}

#[test]
fn a_change_another_writer_saved_reaches_the_subscribers_with_the_next_version() {
  let directory = tempfile::tempdir().unwrap();
  let app = writer(&directory, NEVER);
  let ide = writer(&directory, NEVER);
  let heard = external_changes(&ide);
  ide.set(whole("diffView", json!({ "mode": "split" })), "main").unwrap();

  // the watch of the IDE's writer sees the save of the app: nothing of the IDE saves meanwhile
  app.set(app_fields(json!({ "zoom": 1.5 })), "main").unwrap();
  app.flush().unwrap();
  let change = heard.recv_timeout(EVENT_DEADLINE).expect("the watch told the change of the other writer");

  assert_eq!(change.key, "appSettings");
  assert_eq!(change.value["zoom"], json!(1.5));
  assert_eq!(change.origin, OTHER_PROCESS_ORIGIN);
  assert_eq!(change.version, 2, "the version after the IDE's own change");
  assert_eq!(ide.app().zoom.to_bits(), 1.5_f64.to_bits());
  assert_eq!(ide.changes_since(1), [change], "a page that started after its own change catches up with it");
  assert!(matches!(heard.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)), "one change, told once");
}

#[test]
fn a_save_takes_the_change_of_the_other_writer_and_tells_it_once_before_its_flush_returns() {
  let directory = tempfile::tempdir().unwrap();
  let app = writer(&directory, NEVER);
  let ide = writer(&directory, NEVER);
  let heard = external_changes(&ide);

  app.set(whole("recentProjects", json!(["/a", "/b"])), "main").unwrap();
  app.flush().unwrap();
  ide.set(app_fields(json!({ "zoom": 2 })), "main").unwrap();
  // the watch may have told it already; the save of the IDE takes it at the latest
  ide.flush().unwrap();

  let told: Vec<SettingsChange> = heard.try_iter().collect();
  assert_eq!(told.len(), 1, "{told:?}");
  assert_eq!((told[0].key.as_str(), &told[0].value), ("recentProjects", &json!(["/a", "/b"])));
  assert_eq!(ide.value("recentProjects"), json!(["/a", "/b"]));
}

#[test]
fn a_key_the_other_writer_removed_stays_removed_after_a_save_of_this_one() {
  let directory = tempfile::tempdir().unwrap();
  let path = directory.path().join("settings.json");
  std::fs::write(&path, r#"{"notes":{"a":1},"panel":{"a":1,"b":1}}"#).unwrap();
  let app = writer(&directory, NEVER);
  let ide = writer(&directory, NEVER);
  let heard = external_changes(&ide);

  app.set(whole("notes", Value::Null), "main").unwrap();
  app
    .set(
      SettingsPatch::Fields {
        key: "panel".to_owned(),
        fields: std::iter::once(("b".to_owned(), Value::Null)).collect(),
      },
      "main",
    )
    .unwrap();
  app.flush().unwrap();
  ide.set(app_fields(json!({ "zoom": 2 })), "main").unwrap();
  ide.flush().unwrap();

  assert_eq!(read(&path), json!({ "appSettings": { "zoom": 2 }, "panel": { "a": 1 } }));
  let mut told: Vec<(String, Value)> = heard.try_iter().map(|change| (change.key, change.value)).collect();
  told.sort_by(|a, b| a.0.cmp(&b.0));
  assert_eq!(told, [("notes".to_owned(), Value::Null), ("panel".to_owned(), json!({ "a": 1 }))]);
}

#[test]
fn an_unsaved_change_of_this_writer_stays_over_the_other_writers_save_of_the_same_field() {
  let directory = tempfile::tempdir().unwrap();
  let path = directory.path().join("settings.json");
  let app = writer(&directory, NEVER);
  let ide = writer(&directory, NEVER);

  ide.set(app_fields(json!({ "zoom": 2 })), "main").unwrap();
  app.set(app_fields(json!({ "zoom": 3, "systemTitleBar": true })), "main").unwrap();
  app.flush().unwrap();
  ide.flush().unwrap();

  // the field the IDE changed last is its own, the other field the app's
  assert_eq!(read(&path), json!({ "appSettings": { "zoom": 2, "systemTitleBar": true } }));
  assert_eq!(ide.app().zoom.to_bits(), 2.0_f64.to_bits());
  assert!(ide.app().system_title_bar);
}

#[test]
fn a_broken_file_another_program_wrote_takes_nothing_into_memory() {
  let directory = tempfile::tempdir().unwrap();
  let path = directory.path().join("settings.json");
  let ide = writer(&directory, NEVER);
  let heard = external_changes(&ide);
  ide.set(app_fields(json!({ "zoom": 2 })), "main").unwrap();
  ide.flush().unwrap();

  std::fs::write(&path, "{ cut").unwrap();
  ide.set(whole("diffView", json!({ "mode": "split" })), "main").unwrap();
  ide.flush().unwrap();

  // the save writes the file again from memory, and no change of another process came of it
  assert_eq!(read(&path), json!({ "appSettings": { "zoom": 2 }, "diffView": { "mode": "split" } }));
  assert!(matches!(heard.recv_timeout(Duration::ZERO), Err(RecvTimeoutError::Timeout)));
}
