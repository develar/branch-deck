use crate::projects::{selected_project, synced_before};
use crate::service::SettingsService;
use domain_types::settings::SettingsPatch;
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

/// What a sync recorded of a slot of a lease pool is of the lease of then (`EPOCH_FIELD`, which the writer names in each
/// entry it writes): once the slot holds another lease the entry is of an earlier one, so Rust reads no sync of it and
/// no window gets it, whichever read comes first; an entry of a path outside a pool stays, as does one of the lease now
#[test]
fn an_entry_of_another_epoch_of_its_worktree_is_forgotten_before_the_key_leaves_the_writer() {
  let (_directory, service) = service_with(&[]);
  let epochs = std::sync::Arc::new(std::sync::Mutex::new(json!({ "/pool/1/repo": "lease-a" })));
  let current = std::sync::Arc::clone(&epochs);
  service.set_worktree_epochs(move |path| current.lock().unwrap().get(path).and_then(Value::as_str).map(str::to_owned));
  let synced = |path: &str| {
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
  };
  let change = synced("/pool/1/repo");
  assert_eq!(change.value["/pool/1/repo"]["epoch"], "lease-a", "the writer names the epoch the entry was written under");
  synced("/work/checkout");
  assert!(synced_before(&service, "/pool/1/repo") && synced_before(&service, "/work/checkout"));

  // the slot was returned and leased again
  *epochs.lock().unwrap() = json!({ "/pool/1/repo": "lease-b" });
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
  synced("/pool/1/repo");
  assert!(synced_before(&service, "/pool/1/repo"));
  assert_eq!(service.value("repositoryState")["/pool/1/repo"]["epoch"], "lease-b");
}
