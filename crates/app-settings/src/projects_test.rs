use crate::projects::{selected_project, synced_before};
use crate::service::SettingsService;
use domain_types::settings::SettingsPatch;
use domain_types::worktree_epoch::WorktreeEpoch;
use serde_json::{Value, json};
use std::time::Duration;

fn service_with(values: &[(&str, Value)]) -> (tempfile::TempDir, SettingsService) {
  let directory = tempfile::tempdir().unwrap();
  // no save within the test: the values are the writer's
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), Duration::from_hours(1)).unwrap();
  for (key, value) in values {
    service
      .set(
        SettingsPatch::Value {
          key: (*key).to_owned(),
          value: value.clone(),
        },
        "test",
      )
      .unwrap();
  }
  (directory, service)
}

#[test]
fn the_selected_project_is_the_first_recent_one() {
  let (_directory, service) = service_with(&[("recentProjects", json!([{ "path": "/work/a" }, { "path": "/work/b" }]))]);
  assert_eq!(selected_project(&service), Some("/work/a".to_owned()));

  let (_directory, none) = service_with(&[("recentProjects", json!([]))]);
  assert_eq!(selected_project(&none), None);
  let (_directory, unset) = service_with(&[]);
  assert_eq!(selected_project(&unset), None);
}

#[test]
fn a_project_was_synced_before_when_its_state_has_the_time_of_a_sync() {
  let state = json!({ "/work/a": { "lastSyncTime": 1_700_000_000_000_u64, "lastBranchCount": 3 }, "/work/b": { "lastBranchCount": 3 } });
  let (_directory, service) = service_with(&[("repositoryState", state)]);
  assert!(synced_before(&service, "/work/a"));
  assert!(!synced_before(&service, "/work/b"));
  assert!(!synced_before(&service, "/work/c"));
}

/// The epochs of the worktrees as a test sets them, by path: a string is the epoch of a slot, `null` a path outside a
/// pool, a path not named an epoch the files could not tell
fn epochs_of(service: &SettingsService) -> std::sync::Arc<std::sync::Mutex<Value>> {
  let epochs = std::sync::Arc::new(std::sync::Mutex::new(json!({})));
  let current = std::sync::Arc::clone(&epochs);
  service.set_worktree_epochs(move |path| match current.lock().unwrap().get(path) {
    Some(Value::String(epoch)) => WorktreeEpoch::Lease(epoch.clone()),
    Some(_) => WorktreeEpoch::NotASlot,
    None => WorktreeEpoch::Unknown,
  });
  epochs
}

/// The window of the repository at `path` records a sync of it, as `recordSynced` writes it
fn record_sync(service: &SettingsService, path: &str) -> domain_types::settings::SettingsChange {
  let entry = json!({ path: { "lastSyncTime": 1_700_000_000_000_u64, "lastBranchCount": 3 } });
  service
    .set(
      SettingsPatch::Fields {
        key: "repositoryState".to_owned(),
        fields: entry.as_object().unwrap().clone(),
      },
      "main",
    )
    .unwrap()
}

/// What a sync recorded of a slot of a lease pool is of the lease of then (`EPOCH_FIELD`, which the writer names in each
/// entry it writes): once the slot holds another lease the entry is of an earlier one, so Rust reads no sync of it and
/// no window gets it, whichever read comes first; an entry of a path outside a pool stays, as does one of the lease now
#[test]
fn an_entry_of_another_epoch_of_its_worktree_is_forgotten_before_the_key_leaves_the_writer() {
  let (_directory, service) = service_with(&[]);
  let epochs = epochs_of(&service);
  *epochs.lock().unwrap() = json!({ "/pool/1/repo": "lease-a", "/work/checkout": null });
  let change = record_sync(&service, "/pool/1/repo");
  assert_eq!(change.value["/pool/1/repo"]["epoch"], "lease-a", "the writer names the epoch the entry was written under");
  record_sync(&service, "/work/checkout");
  assert!(synced_before(&service, "/pool/1/repo") && synced_before(&service, "/work/checkout"));

  // the slot was returned and leased again
  *epochs.lock().unwrap() = json!({ "/pool/1/repo": "lease-b", "/work/checkout": null });
  let (version, snapshot) = service.snapshot();
  assert_eq!(snapshot["repositoryState"].as_object().unwrap().keys().collect::<Vec<_>>(), ["/work/checkout"]);
  let forgotten = service.changes_since(version - 1);
  assert_eq!(
    forgotten.iter().map(|change| (change.key.as_str(), change.origin.as_str())).collect::<Vec<_>>(),
    [("repositoryState", "epoch")]
  );
  assert!(!synced_before(&service, "/pool/1/repo"));
  assert!(synced_before(&service, "/work/checkout"));

  // a sync of the new lease is of it
  record_sync(&service, "/pool/1/repo");
  assert!(synced_before(&service, "/pool/1/repo"));
  assert_eq!(service.value("repositoryState")["/pool/1/repo"]["epoch"], "lease-b");
}

/// An epoch the files could not tell (a state of the pool this build cannot parse, a read during a rename) decides
/// nothing: the entry of the slot stays with the epoch it names, in memory and on disk after a save, a write meanwhile
/// keeps that epoch rather than dropping it, and once the read works again the entry is of the lease it was recorded
/// under, as before
#[test]
fn an_entry_whose_epoch_cannot_be_read_stays_as_it_is_and_on_disk() {
  let directory = tempfile::tempdir().unwrap();
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), Duration::from_hours(1)).unwrap();
  let epochs = epochs_of(&service);
  *epochs.lock().unwrap() = json!({ "/pool/1/repo": "lease-a" });
  record_sync(&service, "/pool/1/repo");

  // the read of the pool fails
  *epochs.lock().unwrap() = json!({});
  let (_, snapshot) = service.snapshot();
  assert_eq!(snapshot["repositoryState"]["/pool/1/repo"]["epoch"], "lease-a");
  assert!(synced_before(&service, "/pool/1/repo"));
  let change = record_sync(&service, "/pool/1/repo");
  assert_eq!(change.value["/pool/1/repo"]["epoch"], "lease-a", "a write keeps the epoch the entry names");
  service.flush().unwrap();
  let saved: Value = serde_json::from_str(&std::fs::read_to_string(directory.path().join("repository-state.json")).unwrap()).unwrap();
  assert_eq!(saved["repositoryState"]["/pool/1/repo"]["epoch"], "lease-a", "{saved}");

  // the read works again: the same lease
  *epochs.lock().unwrap() = json!({ "/pool/1/repo": "lease-a" });
  assert!(synced_before(&service, "/pool/1/repo"));
}

/// The epochs are read without the mutex of the writer, between two holds of it: a write that comes between (the slot
/// leased again, a window of the new lease that records a sync) is of the lease now, so the epoch read before it, of the
/// lease before, forgets nothing of it
#[test]
fn an_entry_written_while_its_epoch_was_read_stays() {
  let directory = tempfile::tempdir().unwrap();
  let service = std::sync::Arc::new(SettingsService::open_with_debounce(directory.path().to_path_buf(), Duration::from_hours(1)).unwrap());
  let lease = std::sync::Arc::new(std::sync::Mutex::new("lease-a"));
  let between = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
  let (writer, current, write_between) = (std::sync::Arc::downgrade(&service), std::sync::Arc::clone(&lease), std::sync::Arc::clone(&between));
  service.set_worktree_epochs(move |_path| {
    let read = *current.lock().unwrap();
    if write_between.swap(false, std::sync::atomic::Ordering::SeqCst) {
      // as the read of the lease before ends, the slot holds another lease and its window records a sync
      *current.lock().unwrap() = "lease-b";
      record_sync(&writer.upgrade().unwrap(), "/pool/1/repo");
    }
    WorktreeEpoch::Lease(read.to_owned())
  });
  record_sync(&service, "/pool/1/repo");
  assert_eq!(service.value("repositoryState")["/pool/1/repo"]["epoch"], "lease-a");

  *lease.lock().unwrap() = "lease-a";
  between.store(true, std::sync::atomic::Ordering::SeqCst);
  let (_, snapshot) = service.snapshot();
  assert_eq!(snapshot["repositoryState"]["/pool/1/repo"]["epoch"], "lease-b", "{snapshot}");
  assert!(synced_before(&service, "/pool/1/repo"));
}
