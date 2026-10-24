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

fn repository_state_path(directory: &tempfile::TempDir) -> PathBuf {
  directory.path().join("repository-state.json")
}

/// The save errors `service` reports, each by the path it names and its text
type SaveErrors = Arc<std::sync::Mutex<Vec<(PathBuf, String)>>>;

fn save_errors(service: &SettingsService) -> SaveErrors {
  let reports = SaveErrors::default();
  service.on_save_error({
    let reports = Arc::clone(&reports);
    move |error| reports.lock().unwrap().push((error.path.clone(), error.to_string()))
  });
  reports
}

/// An app data directory in `directory` that cannot be created: a file is in its place, which every platform refuses
/// as the directory of a file; with the path of its settings file
fn blocked_app_data(directory: &tempfile::TempDir) -> (PathBuf, PathBuf) {
  let blocked = directory.path().join("app-data");
  std::fs::write(&blocked, "").unwrap();
  let path = blocked.join("settings.json");
  (blocked, path)
}

/// A service on a new empty app data directory, which lives as long as the directory it answers with
fn opened(debounce: Duration) -> (tempfile::TempDir, SettingsService) {
  let directory = tempfile::tempdir().unwrap();
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), debounce).unwrap();
  (directory, service)
}

#[test]
fn the_changes_are_saved_in_the_order_they_arrive_so_the_last_one_is_on_disk() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  let service = Arc::new(SettingsService::open_with_debounce(directory.path().to_path_buf(), Duration::from_millis(1)).unwrap());
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
  let versions: Vec<u32> = changes.iter().map(|change| change.version).collect();
  assert_eq!(versions, (1..=200).collect::<Vec<u32>>(), "every patch gets the next version");
  let last = changes.last().unwrap();
  assert_eq!(read(&path)["appSettings"]["zoom"], last.value["zoom"]);
  assert_eq!(service.snapshot(), (200, json!({ "appSettings": last.value })));
}

#[test]
fn two_origins_that_patch_two_fields_of_one_key_at_once_keep_both() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  let service = Arc::new(SettingsService::open_with_debounce(directory.path().to_path_buf(), Duration::from_millis(1)).unwrap());
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
  let (_directory, service) = opened(NEVER);
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
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), NEVER).unwrap();
  service.set(app_fields(json!({ "zoom": 1.5 })), "main").unwrap();
  assert!(!path.exists(), "the save waits for the debounce");
  let started = Instant::now();
  service.flush().unwrap();
  // the debounce is an hour: a flush that waited for it would not end; a minute is the save under any load of the machine
  assert!(started.elapsed() < Duration::from_mins(1), "the flush took {:?}", started.elapsed());
  assert_eq!(read(&path), json!({ "appSettings": { "zoom": 1.5 } }));
}

#[test]
fn a_patch_is_saved_after_the_debounce_without_a_flush() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), Duration::from_millis(20)).unwrap();
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
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), NEVER).unwrap();
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
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), Duration::ZERO).unwrap();
  // `bd` reads the prefix beside a sync, which records the repository at every step: a step in three changes the
  // settings too
  let writer = std::thread::spawn(move || {
    for step in 0..300 {
      service
        .set(fields("repositoryState", json!({ "/repo": { "lastSyncTime": step, "lastBranchCount": 50 } })), "main")
        .unwrap();
      if step % 3 == 0 {
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
      }
      service.flush().unwrap();
    }
  });
  let state_path = repository_state_path(&directory);
  let mut reads = 0;
  while !writer.is_finished() {
    let text = std::fs::read_to_string(&path).unwrap();
    let settings = SettingsFile::parse(&text).unwrap_or_else(|error| panic!("a cut file ({error}): {text}"));
    let prefix = settings.app.global_user_branch_prefix;
    assert!(matches!(prefix.as_deref(), Some("jane" | "john")), "read {prefix:?}");
    if let Ok(text) = std::fs::read_to_string(&state_path) {
      let state = SettingsFile::parse(&text).unwrap_or_else(|error| panic!("a cut repository state ({error}): {text}"));
      assert!(state.other["repositoryState"]["/repo"]["lastSyncTime"].is_u64(), "{text}");
    }
    reads += 1;
  }
  writer.join().unwrap();
  assert!(reads > 0);
}

#[test]
fn an_unwritable_directory_reports_the_error_once_a_streak_and_memory_keeps_the_value() {
  let directory = tempfile::tempdir().unwrap();
  let (blocked, path) = blocked_app_data(&directory);
  let service = SettingsService::open_with_debounce(blocked.clone(), NEVER).unwrap();
  let reports = save_errors(&service);

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
fn a_failed_save_is_retried_by_itself_once_the_directory_is_writable_again() {
  let directory = tempfile::tempdir().unwrap();
  let (blocked, path) = blocked_app_data(&directory);
  let service = SettingsService::open_with_debounce(blocked.clone(), Duration::from_millis(20)).unwrap();
  let reports = save_errors(&service);

  service.set(app_fields(json!({ "zoom": 2 })), "main").unwrap();
  let deadline = Instant::now() + Duration::from_mins(1);
  while reports.lock().unwrap().is_empty() {
    assert!(Instant::now() < deadline, "the debounced save never failed");
    std::thread::sleep(Duration::from_millis(5));
  }

  // the directory comes back, and nothing changes or flushes: the writer saves what memory kept by itself
  std::fs::remove_file(&blocked).unwrap();
  std::fs::create_dir_all(&blocked).unwrap();
  while !path.exists() {
    assert!(Instant::now() < deadline, "the failed save was never tried again");
    std::thread::sleep(Duration::from_millis(5));
  }
  assert_eq!(read(&path), json!({ "appSettings": { "zoom": 2 } }));
  assert_eq!(reports.lock().unwrap().len(), 1, "the retries of one streak report nothing more");
}

#[test]
fn an_invalid_value_of_a_field_rust_knows_is_refused_and_changes_nothing() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), NEVER).unwrap();
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
  let (_directory, service) = opened(NEVER);
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
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), NEVER).unwrap();
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
  let (_directory, service) = opened(NEVER);
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
  let (_directory, service) = opened(NEVER);
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

#[test]
fn a_field_patch_that_removes_the_last_field_removes_the_key() {
  let (_directory, service) = opened(NEVER);
  service.set(fields("diffView", json!({ "mode": "unified" })), "main").unwrap();
  let change = service.set(fields("diffView", json!({ "mode": null })), "review").unwrap();
  assert_eq!(change.value, Value::Null);
  assert_eq!(service.snapshot().1.get("diffView"), None);
}

#[test]
fn the_snapshot_of_a_cut_file_is_a_json_object_of_the_defaults() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  for cut in ["", r#"{"appSettings": {"globalUserBra"#, "[1, 2]"] {
    std::fs::write(&path, cut).unwrap();
    let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), NEVER).unwrap();
    let (version, snapshot) = service.snapshot();
    assert_eq!(version, 0);
    // the text the init script of a window embeds parses back as the same object
    let script_value: Value = serde_json::from_str(&snapshot.to_string()).unwrap();
    assert_eq!(
      script_value,
      json!({ "appSettings": { "patronusNewApi": false, "autoSyncOnFocus": true, "systemTitleBar": false, "zoom": 1 } }),
      "{cut:?}"
    );
  }
}

#[test]
fn a_patch_of_the_repository_state_leaves_the_settings_file_untouched() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  // not the form a save writes, so any save of the file would change its bytes
  let text = r#"{"appSettings":{"zoom":1.5},"recentProjects":[{"path":"/repo"}]}"#;
  std::fs::write(&path, text).unwrap();
  let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), Duration::ZERO).unwrap();

  for step in 1..=3 {
    let change = service
      .set(
        fields("repositoryState", json!({ "/repo": { "lastSyncTime": step, "updateTimings": { "sync": [step] } } })),
        "main",
      )
      .unwrap();
    assert_eq!(change.value, json!({ "/repo": { "lastSyncTime": step, "updateTimings": { "sync": [step] } } }));
    service.flush().unwrap();
  }

  assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
  assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), modified);
  assert_eq!(
    read(&repository_state_path(&directory)),
    json!({ "repositoryState": { "/repo": { "lastSyncTime": 3, "updateTimings": { "sync": [3] } } } })
  );
  assert_eq!(service.snapshot().1["repositoryState"]["/repo"]["lastSyncTime"], 3);

  // a change of a setting writes the settings, and the repository state stays in its own file
  service.set(app_fields(json!({ "zoom": 2 })), "main").unwrap();
  service.flush().unwrap();
  assert_eq!(read(&path), json!({ "appSettings": { "zoom": 2 }, "recentProjects": [{ "path": "/repo" }] }));
}

#[test]
fn a_settings_file_of_before_moves_the_fields_of_a_sync_to_the_repository_state_and_drops_them_on_its_next_save() {
  let directory = tempfile::tempdir().unwrap();
  let path = settings_path(&directory);
  let text = r#"{"recentProjects":[{"path":"/a","cachedBranchPrefix":{"prefix":"jane","source":"app_setting"},"lastSyncTime":10,"lastBranchCount":2,"updateTimings":{"sync":[5]},"lastFetchTime":7},{"path":"/b","lastSyncTime":20},{"path":"/c"}],"unknownKey":{"kept":true}}"#;
  std::fs::write(&path, text).unwrap();
  // the repository state of a later run of the app has /b already: it wins
  std::fs::write(repository_state_path(&directory), r#"{"repositoryState":{"/b":{"lastSyncTime":30}}}"#).unwrap();
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), NEVER).unwrap();

  let (version, snapshot) = service.snapshot();
  assert_eq!(version, 0);
  let projects = json!([
    { "path": "/a", "cachedBranchPrefix": { "prefix": "jane", "source": "app_setting" } },
    { "path": "/b" },
    { "path": "/c" },
  ]);
  assert_eq!(snapshot["recentProjects"], projects);
  let state = json!({
    "/a": { "lastSyncTime": 10, "lastBranchCount": 2, "updateTimings": { "sync": [5] }, "lastFetchTime": 7 },
    "/b": { "lastSyncTime": 30 },
  });
  assert_eq!(snapshot["repositoryState"], state);

  // the moved values are saved; the settings file keeps them until its next save
  service.flush().unwrap();
  assert_eq!(read(&repository_state_path(&directory)), json!({ "repositoryState": state }));
  assert_eq!(std::fs::read_to_string(&path).unwrap(), text);

  service.set(app_fields(json!({ "zoom": 2 })), "main").unwrap();
  service.flush().unwrap();
  assert_eq!(
    read(&path),
    json!({ "appSettings": { "zoom": 2 }, "recentProjects": projects, "unknownKey": { "kept": true } })
  );
}

#[test]
fn the_moved_values_of_a_settings_file_of_before_are_saved_without_a_change() {
  let directory = tempfile::tempdir().unwrap();
  std::fs::write(settings_path(&directory), r#"{"recentProjects":[{"path":"/a","lastSyncTime":10}]}"#).unwrap();
  drop(SettingsService::open_with_debounce(directory.path().to_path_buf(), Duration::ZERO).unwrap());
  assert_eq!(read(&repository_state_path(&directory)), json!({ "repositoryState": { "/a": { "lastSyncTime": 10 } } }));
}
