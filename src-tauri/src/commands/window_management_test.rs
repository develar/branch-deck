use crate::commands::window_management::{Reopen, SubWindowLoaded, SubWindowOpen, SubWindowState, SubWindowUpdate};
use app_settings::service::SettingsService;
use domain_types::settings::SettingsPatch;
use serde_json::json;
use std::time::Duration;

/// A writer that never saves within a test
fn settings(dir: &tempfile::TempDir) -> SettingsService {
  SettingsService::open_with_debounce(dir.path().join("settings.json"), Duration::from_hours(1)).unwrap()
}

/// A change of one field of `appSettings` from the main window
fn set_app_field(settings: &SettingsService, field: &str, value: serde_json::Value) -> u32 {
  let patch = SettingsPatch::Fields {
    key: "appSettings".to_owned(),
    fields: serde_json::Map::from_iter([(field.to_owned(), value)]),
  };
  settings.set(patch, "main").unwrap().version
}

/// The open that came while the page loaded, as its load answers it
fn loaded_update(state: &SubWindowState, label: &str) -> Option<SubWindowUpdate> {
  let dir = tempfile::tempdir().unwrap();
  state.loaded(label, &settings(&dir)).0.update
}

fn update(state: &SubWindowState, data: &str) -> SubWindowUpdate {
  SubWindowUpdate {
    data: data.to_string(),
    open: state.next_open(0.0),
  }
}

fn build(state: &SubWindowState, label: &str) -> SubWindowOpen {
  let open = state.next_open(0.0);
  state.built(label, open, 0);
  open
}

#[test]
fn an_open_while_the_page_loads_waits_for_the_page() {
  let state = SubWindowState::default();
  let first = build(&state, "commit-review");
  // two quick opens: the page still loads, the later one wins
  let second = update(&state, "second");
  assert_eq!(state.reopened("commit-review", second.clone()), Reopen::Pending);
  let third = update(&state, "third");
  assert_eq!(state.reopened("commit-review", third.clone()), Reopen::Pending);
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(
    state.loaded("commit-review", &settings(&dir)),
    (
      SubWindowLoaded {
        update: Some(third.clone()),
        settings: Vec::new()
      },
      Some(first.id)
    )
  );
  // taken once
  assert_eq!(loaded_update(&state, "commit-review"), None);
  // the content of the build or of an earlier open does not show the window, the latest one does, once
  assert!(!state.ready("commit-review", first.id));
  assert!(!state.ready("commit-review", second.open.id));
  assert!(state.ready("commit-review", third.open.id));
  assert!(!state.ready("commit-review", third.open.id));
}

#[test]
fn a_loaded_hidden_page_takes_an_open_and_shows_for_it() {
  let state = SubWindowState::default();
  let first = build(&state, "branch-review");
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(state.loaded("branch-review", &settings(&dir)).1, Some(first.id));
  let next = update(&state, "next");
  assert_eq!(state.reopened("branch-review", next.clone()), Reopen::Hidden);
  assert!(!state.ready("branch-review", first.id));
  assert!(state.ready("branch-review", next.open.id));
  // on screen: a later open comes to the front at once
  assert_eq!(state.reopened("branch-review", update(&state, "later")), Reopen::Shown);
}

#[test]
fn a_budget_shows_the_window_once_and_only_its_own_build() {
  let state = SubWindowState::default();
  let first = build(&state, "pull-request-review");
  state.forget("pull-request-review");
  let second = build(&state, "pull-request-review");
  // the fallback timer of the closed window leaves the new one hidden
  assert!(!state.budget_passed("pull-request-review", first.id));
  assert!(state.budget_passed("pull-request-review", second.id));
  assert!(!state.budget_passed("pull-request-review", second.id));
  // shown by the budget: the page's later ready does nothing
  assert!(!state.ready("pull-request-review", second.id));
}

#[test]
fn a_window_built_again_after_its_close_waits_for_its_page_again() {
  let state = SubWindowState::default();
  build(&state, "pull-request-review");
  loaded_update(&state, "pull-request-review");
  state.forget("pull-request-review");
  build(&state, "pull-request-review");
  let again = update(&state, "again");
  assert_eq!(state.reopened("pull-request-review", again.clone()), Reopen::Pending);
  assert_eq!(loaded_update(&state, "pull-request-review"), Some(again));
}

#[test]
fn a_page_that_loads_takes_the_changes_after_the_snapshot_of_its_build() {
  let dir = tempfile::tempdir().unwrap();
  let settings = settings(&dir);
  let state = SubWindowState::default();
  let before = set_app_field(&settings, "zoom", json!(1.1));
  // built with the snapshot at `before`; Actual Size and the Activity panel of the main window while its page loads
  state.built("commit-review", state.next_open(0.0), settings.snapshot().0);
  set_app_field(&settings, "zoom", json!(null));
  let last = set_app_field(&settings, "activityPanel", json!(true));
  assert!(last > before);

  let (loaded, _) = state.loaded("commit-review", &settings);
  assert_eq!(loaded.update, None);
  // the last change of the key, with its value now: the page applies it by its version
  assert_eq!(loaded.settings.len(), 1);
  assert_eq!(loaded.settings[0].version, last);
  assert_eq!(loaded.settings[0].value["zoom"], json!(1));
  assert_eq!(loaded.settings[0].value["activityPanel"], json!(true));
  // a reload of the page starts from the same snapshot, so it takes them again
  assert_eq!(state.loaded("commit-review", &settings).0.settings, loaded.settings);
}

#[test]
fn a_page_that_loads_takes_no_change_older_than_its_snapshot_nor_a_key_only_the_main_window_keeps() {
  let dir = tempfile::tempdir().unwrap();
  let settings = settings(&dir);
  let state = SubWindowState::default();
  set_app_field(&settings, "zoom", json!(1.25));
  state.built("branch-review", state.next_open(0.0), settings.snapshot().0);
  let activity = SettingsPatch::Value {
    key: "activity".to_owned(),
    value: json!([{ "id": "1" }]),
  };
  settings.set(activity, "main").unwrap();
  assert_eq!(state.loaded("branch-review", &settings).0.settings, Vec::new());
}
