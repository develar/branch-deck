use crate::file::SettingsFile;
use crate::service::{SettingsError, SettingsService};
use domain_types::settings::{SettingsChange, SettingsPatch};
use pretty_assertions::assert_eq;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Longer than any test: a save happens only by a flush or a drop
const NEVER: Duration = Duration::from_hours(1);

fn fields(key: &str, value: Value) -> SettingsPatch {
  let Value::Object(fields) = value else { panic!("fields are an object") };
  SettingsPatch::Fields { key: key.to_owned(), fields }
}

fn app_fields(value: Value) -> SettingsPatch {
  fields("appSettings", value)
}

fn read(path: &Path) -> Value {
  serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn settings_path(directory: &tempfile::TempDir) -> PathBuf {
  directory.path().join("settings.json")
}

#[test]
fn the_changes_are_saved_in_the_order_they_arrive_so_the_last_one_is_on_disk() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  let service = Arc::new(SettingsService::open_with_debounce(path.clone(), Duration::from_millis(1)).unwrap());
  #[expect(clippy::needless_collect, reason = "every thread starts before the first is joined")]
  let threads: Vec<_> = (0..4)
    .map(|thread| {
      let service = Arc::clone(&service);
      std::thread::spawn(move || {
        (1..=50)
          .map(|step| service.set(app_fields(json!({ "zoom": f64::from(thread * 100 + step) / 10.0 })), "main").unwrap())
          .collect::<Vec<SettingsChange>>()
      })
    })
    .collect();
  let mut changes: Vec<SettingsChange> = threads.into_iter().flat_map(|thread| thread.join().unwrap()).collect();
  service.flush().unwrap();

  changes.sort_by_key(|change| change.version);
  let versions: Vec<u64> = changes.iter().map(|change| change.version).collect();
  assert_eq!(versions, (1..=200).collect::<Vec<u64>>(), "every patch gets the next version");
  let last = changes.last().unwrap();
  assert_eq!(read(&path)["appSettings"]["zoom"], last.value["zoom"]);
  assert_eq!(service.snapshot(), (200, json!({ "appSettings": last.value })));
}

#[test]
fn two_origins_that_patch_two_fields_of_one_key_at_once_keep_both() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  let service = Arc::new(SettingsService::open_with_debounce(path.clone(), Duration::from_millis(1)).unwrap());
  let patch_in_a_loop = |origin: &'static str, field: &'static str| {
    let service = Arc::clone(&service);
    std::thread::spawn(move || {
      for step in 0..200 {
        service.set(app_fields(json!({ field: { "step": step } })), origin).unwrap();
      }
    })
  };
  let main = patch_in_a_loop("main", "activityPanel");
  let review = patch_in_a_loop("review", "reviewSidebar");
  main.join().unwrap();
  review.join().unwrap();
  service.flush().unwrap();

  let (_, snapshot) = service.snapshot();
  assert_eq!(snapshot["appSettings"]["activityPanel"], json!({ "step": 199 }));
  assert_eq!(snapshot["appSettings"]["reviewSidebar"], json!({ "step": 199 }));
  let stored = read(&path);
  assert_eq!(stored["appSettings"]["activityPanel"], json!({ "step": 199 }));
  assert_eq!(stored["appSettings"]["reviewSidebar"], json!({ "step": 199 }));
}

#[test]
fn a_change_older_than_the_applied_version_is_an_echo() {
  let service = SettingsService::open_with_debounce(tempfile::tempdir().unwrap().path().join("settings.json"), NEVER).unwrap();
  // the main window sets the zoom twice in a row; the broadcast of the first change reaches it after the second
  let first = service.set(app_fields(json!({ "zoom": 1.1 })), "main").unwrap();
  let second = service.set(app_fields(json!({ "zoom": 1.2 })), "main").unwrap();
  let from_review = service.set(app_fields(json!({ "reviewSidebar": false })), "review").unwrap();
  // (the version the window has applied, the change it gets, whether it applies it)
  let table = [
    (0, &first, true),
    (second.version, &first, false),
    (second.version, &second, false),
    (first.version, &second, true),
    (second.version, &from_review, true),
    (from_review.version, &from_review, false),
  ];
  for (applied, change, newer) in table {
    assert_eq!(change.is_newer_than(applied), newer, "a window at {applied} gets {change:?}");
  }
  // the newest change carries the zoom of the second patch: applying it never rolls the zoom back
  assert_eq!(from_review.value["zoom"], json!(1.2));
}

#[test]
fn a_flush_soon_after_a_patch_saves_it_without_waiting_for_the_debounce() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  let service = SettingsService::open_with_debounce(path.clone(), NEVER).unwrap();
  service.set(app_fields(json!({ "zoom": 1.5 })), "main").unwrap();
  assert!(!path.exists(), "the save waits for the debounce");
  let started = Instant::now();
  service.flush().unwrap();
  assert!(started.elapsed() < Duration::from_millis(100), "the flush took {:?}", started.elapsed());
  assert_eq!(read(&path), json!({ "appSettings": { "zoom": 1.5 } }));
}

#[test]
fn a_patch_is_saved_after_the_debounce_without_a_flush() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  let service = SettingsService::open_with_debounce(path.clone(), Duration::from_millis(20)).unwrap();
  service.set(app_fields(json!({ "systemTitleBar": true })), "rust").unwrap();
  let deadline = Instant::now() + Duration::from_secs(10);
  while !path.exists() {
    assert!(Instant::now() < deadline, "the debounced save never came");
    std::thread::sleep(Duration::from_millis(5));
  }
  assert_eq!(read(&path), json!({ "appSettings": { "systemTitleBar": true } }));
}

#[test]
fn the_service_saves_what_is_unsaved_when_it_is_dropped() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  let service = SettingsService::open_with_debounce(path.clone(), NEVER).unwrap();
  service
    .set(
      SettingsPatch::Value {
        key: "diffViewSettings".to_owned(),
        value: json!({ "mode": "unified" }),
      },
      "main",
    )
    .unwrap();
  drop(service);
  assert_eq!(read(&path), json!({ "diffViewSettings": { "mode": "unified" } }));
}

#[test]
fn a_reader_of_the_file_never_sees_a_missing_prefix_while_the_service_writes_it() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  std::fs::write(&path, r#"{"appSettings":{"globalUserBranchPrefix":"jane"}}"#).unwrap();
  let service = SettingsService::open_with_debounce(path.clone(), Duration::ZERO).unwrap();
  let writer = std::thread::spawn(move || {
    for step in 0..300 {
      // a big file, so a write that is not atomic is caught cut
      let notes: Map<String, Value> = (0..50).map(|note| (format!("note{note}"), json!(format!("{step}-{note}")))).collect();
      service
        .set(
          SettingsPatch::Value {
            key: "notes".to_owned(),
            value: Value::Object(notes),
          },
          "main",
        )
        .unwrap();
      service
        .set(app_fields(json!({ "globalUserBranchPrefix": if step % 2 == 0 { "john" } else { "jane" } })), "main")
        .unwrap();
      service.flush().unwrap();
    }
  });
  let mut reads = 0;
  while !writer.is_finished() {
    let text = std::fs::read_to_string(&path).unwrap();
    let settings = SettingsFile::parse(&text).unwrap_or_else(|error| panic!("a cut file ({error}): {text}"));
    let prefix = settings.app.global_user_branch_prefix;
    assert!(matches!(prefix.as_deref(), Some("jane" | "john")), "read {prefix:?}");
    reads += 1;
  }
  writer.join().unwrap();
  assert!(reads > 0);
}

#[test]
fn an_unwritable_directory_reports_the_error_once_a_streak_and_memory_keeps_the_value() {
  let directory = tempfile::tempdir().unwrap();
  // the directory of the file is a file: every platform refuses to create it
  let blocked = directory.path().join("app-data");
  std::fs::write(&blocked, "").unwrap();
  let path = blocked.join("settings.json");
  let service = SettingsService::open_with_debounce(path.clone(), NEVER).unwrap();
  let reports = Arc::new(std::sync::Mutex::new(Vec::new()));
  service.on_save_error({
    let reports = Arc::clone(&reports);
    move |error| reports.lock().unwrap().push((error.path.clone(), error.to_string()))
  });

  service.set(app_fields(json!({ "zoom": 2 })), "main").unwrap();
  assert!(matches!(service.flush(), Err(SettingsError::Save(error)) if error.path == path));
  service.set(app_fields(json!({ "zoom": 3 })), "main").unwrap();
  assert!(matches!(service.flush(), Err(SettingsError::Save(_))));
  assert_eq!(service.app().zoom.to_bits(), 3.0_f64.to_bits(), "memory keeps the value");
  {
    let reports = reports.lock().unwrap();
    assert_eq!(reports.len(), 1, "one report for a streak of failures: {reports:?}");
    assert_eq!(reports[0].0, path);
    assert!(reports[0].1.contains("settings.json"), "the error names the file: {}", reports[0].1);
  }

  // the directory comes back: the next flush retries and saves what memory kept
  std::fs::remove_file(&blocked).unwrap();
  std::fs::create_dir_all(&blocked).unwrap();
  service.flush().unwrap();
  assert_eq!(read(&path), json!({ "appSettings": { "zoom": 3 } }));

  // a failure after a success is a new streak
  std::fs::remove_dir_all(&blocked).unwrap();
  std::fs::write(&blocked, "").unwrap();
  service.set(app_fields(json!({ "zoom": 4 })), "main").unwrap();
  assert!(service.flush().is_err());
  assert_eq!(reports.lock().unwrap().len(), 2);
}

#[test]
fn an_invalid_value_of_a_field_rust_knows_is_refused_and_changes_nothing() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  let service = SettingsService::open_with_debounce(path.clone(), NEVER).unwrap();
  let before = service.snapshot();
  let table = [
    ("zoom", json!(0)),
    ("zoom", json!(-1.5)),
    ("zoom", json!("big")),
    ("patronusNewApi", json!("yes")),
    ("autoSyncOnFocus", json!(1)),
    ("systemTitleBar", json!([])),
    ("globalUserBranchPrefix", json!(5)),
  ];
  for (field, value) in table {
    let refused = service.set(app_fields(json!({ field: value, "reviewSidebar": false })), "main");
    assert!(
      matches!(&refused, Err(SettingsError::Invalid { key, field: refused_field, .. }) if key == "appSettings" && refused_field == field),
      "{field} = {value}: {refused:?}"
    );
  }
  for value in [json!(1), json!("zoom"), json!([1])] {
    let refused = service.set(
      SettingsPatch::Value {
        key: "appSettings".to_owned(),
        value,
      },
      "main",
    );
    assert!(matches!(refused, Err(SettingsError::NotAnObject { .. })), "{refused:?}");
  }
  assert_eq!(service.snapshot(), before);
  assert_eq!(service.changes_since(0), Vec::new());
  service.flush().unwrap();
  assert!(!path.exists(), "nothing to save");
}

#[test]
fn the_prefix_is_trimmed_and_a_blank_one_is_no_prefix() {
  let service = SettingsService::open_with_debounce(tempfile::tempdir().unwrap().path().join("settings.json"), NEVER).unwrap();
  let change = service.set(app_fields(json!({ "globalUserBranchPrefix": "  jane \n" })), "main").unwrap();
  assert_eq!(change.value["globalUserBranchPrefix"], json!("jane"));
  assert_eq!(service.app().global_user_branch_prefix.as_deref(), Some("jane"));
  let change = service.set(app_fields(json!({ "globalUserBranchPrefix": "   " })), "main").unwrap();
  assert_eq!(change.value.get("globalUserBranchPrefix"), None);
  assert_eq!(service.app().global_user_branch_prefix, None);
}

#[test]
fn the_snapshot_has_the_defaults_of_the_fields_rust_knows_and_the_file_keeps_only_what_differs() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  let old = json!({
    "appSettings": { "zoom": 1.25, "notifyApprovals": false },
    "recentProjects": { "items": [{ "path": "/work/repo" }] },
  });
  std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
  let service = SettingsService::open_with_debounce(path.clone(), NEVER).unwrap();
  let full_app = json!({
    "zoom": 1.25,
    "patronusNewApi": false,
    "autoSyncOnFocus": true,
    "systemTitleBar": false,
    "notifyApprovals": false,
  });
  assert_eq!(service.snapshot(), (0, json!({ "appSettings": full_app, "recentProjects": old["recentProjects"] })));

  // the zoom back at its default leaves the file
  let change = service.set(app_fields(json!({ "zoom": 1 })), "main").unwrap();
  assert_eq!(change.value["zoom"], json!(1));
  service.flush().unwrap();
  assert_eq!(read(&path), json!({ "appSettings": { "notifyApprovals": false }, "recentProjects": old["recentProjects"] }));
}

#[test]
fn the_changes_since_a_snapshot_name_each_key_changed_after_it_once_with_its_value_now() {
  let service = SettingsService::open_with_debounce(tempfile::tempdir().unwrap().path().join("settings.json"), NEVER).unwrap();
  let (snapshot_version, _) = service.snapshot();
  service.set(app_fields(json!({ "zoom": 1.5 })), "main").unwrap();
  service.set(fields("diffViewSettings", json!({ "mode": "unified" })), "review").unwrap();
  let zoom = service.set(app_fields(json!({ "zoom": 2 })), "main").unwrap();
  let removed = service
    .set(
      SettingsPatch::Value {
        key: "diffViewSettings".to_owned(),
        value: Value::Null,
      },
      "review",
    )
    .unwrap();
  assert_eq!(removed.value, Value::Null);

  assert_eq!(service.changes_since(snapshot_version), vec![zoom.clone(), removed.clone()]);
  assert_eq!(service.changes_since(zoom.version), vec![removed.clone()]);
  assert_eq!(service.changes_since(removed.version), Vec::new());
  assert_eq!(service.snapshot().1.get("diffViewSettings"), None);
}

#[test]
fn a_field_patch_merges_into_the_key_and_a_null_field_is_removed() {
  let service = SettingsService::open_with_debounce(tempfile::tempdir().unwrap().path().join("settings.json"), NEVER).unwrap();
  service.set(fields("diffViewSettings", json!({ "mode": "unified", "wrap": true })), "main").unwrap();
  let change = service.set(fields("diffViewSettings", json!({ "wrap": null, "context": 3 })), "review").unwrap();
  assert_eq!(change.value, json!({ "mode": "unified", "context": 3 }));

  // the zoom removed is its default again
  service.set(app_fields(json!({ "zoom": 1.5 })), "main").unwrap();
  let change = service.set(app_fields(json!({ "zoom": null })), "main").unwrap();
  assert_eq!(change.value["zoom"], json!(1));

  service
    .set(
      SettingsPatch::Value {
        key: "recentProjects".to_owned(),
        value: json!([1, 2]),
      },
      "main",
    )
    .unwrap();
  let refused = service.set(fields("recentProjects", json!({ "items": [] })), "main");
  assert!(matches!(&refused, Err(SettingsError::NotAnObject { key }) if key == "recentProjects"), "{refused:?}");
  assert_eq!(service.snapshot().1["recentProjects"], json!([1, 2]));
}
