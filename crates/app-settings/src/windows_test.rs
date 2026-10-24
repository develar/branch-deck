use crate::service::SettingsService;
use crate::windows::{window_changes_since, window_snapshot};
use domain_types::settings::SettingsPatch;
use serde_json::json;
use std::time::Duration;

fn set(service: &SettingsService, key: &str, value: serde_json::Value) {
  service.set(SettingsPatch::Value { key: key.to_owned(), value }, "main").unwrap();
}

#[test]
fn a_sub_window_gets_no_key_only_the_main_window_keeps() {
  let directory = tempfile::tempdir().unwrap();
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), Duration::from_hours(1)).unwrap();
  set(&service, "activity", json!([{ "id": "1" }]));
  set(&service, "repositoryState", json!({ "/repo": { "lastSyncTime": 1 } }));
  set(&service, "diffView", json!({ "mode": "unified" }));

  let (_, main) = window_snapshot(&service, "main");
  assert!(
    main.get("activity").is_some() && main.get("repositoryState").is_some() && main.get("diffView").is_some(),
    "{main}"
  );
  let (version, review) = window_snapshot(&service, "commit-review");
  assert_eq!((review.get("activity"), review.get("repositoryState")), (None, None));
  assert_eq!(review["diffView"], json!({ "mode": "unified" }));

  set(&service, "robotHistory", json!([1]));
  set(&service, "repositoryState", json!({ "/repo": { "lastSyncTime": 2 } }));
  set(&service, "diffView", json!({ "mode": "split" }));
  let keys = |label: &str| window_changes_since(&service, label, version).into_iter().map(|change| change.key).collect::<Vec<_>>();
  assert_eq!(keys("main"), ["robotHistory", "repositoryState", "diffView"]);
  assert_eq!(keys("commit-review"), ["diffView"]);
}
