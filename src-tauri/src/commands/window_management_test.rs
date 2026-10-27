use crate::commands::window_management::{Awaited, BuiltPage, Deadline, Reopen, SubWindowLoaded, SubWindowOpen, SubWindowState, SubWindowUpdate, wait_for_deadline};
use app_settings::service::SettingsService;
use domain_types::settings::SettingsPatch;
use serde_json::json;
use std::time::Duration;
use tokio::time::{Instant, sleep};

/// A writer of the app data directory `dir` that never saves within a test
fn settings(dir: &tempfile::TempDir) -> SettingsService {
  SettingsService::open_with_debounce(dir.path().to_path_buf(), Duration::from_hours(1)).unwrap()
}

/// A change of one field of `appSettings` from the main window
fn set_app_field(settings: &SettingsService, field: &str, value: serde_json::Value) -> u32 {
  let patch = SettingsPatch::Fields {
    key: "appSettings".to_owned(),
    fields: serde_json::Map::from_iter([(field.to_owned(), value)]),
  };
  settings.set(patch, "main").unwrap().version
}

/// The page of `label` loaded: the open that came while it loaded, and the deadline of its shell
fn load(state: &SubWindowState, label: &str) -> (Option<SubWindowUpdate>, Deadline) {
  let dir = tempfile::tempdir().unwrap();
  let (loaded, deadline) = state.loaded(label, &settings(&dir));
  (loaded.update, deadline.expect("a window this state built"))
}

/// The open that came while the page loaded, as its load answers it
fn loaded_update(state: &SubWindowState, label: &str) -> Option<SubWindowUpdate> {
  load(state, label).0
}

fn update(state: &SubWindowState, data: &str) -> SubWindowUpdate {
  SubWindowUpdate {
    data: data.to_string(),
    open: state.next_open(0.0),
  }
}

fn build(state: &SubWindowState, label: &str) -> SubWindowOpen {
  build_with_deadline(state, label).0
}

fn build_with_deadline(state: &SubWindowState, label: &str) -> (SubWindowOpen, Deadline) {
  let open = state.next_open(0.0);
  let deadline = state.built(label, open, BuiltPage::Sub { settings_version: 0 });
  (open, deadline)
}

/// What `deadline` waits for and how long
const fn awaits(deadline: &Deadline) -> (Awaited, Duration) {
  (deadline.awaited, deadline.after)
}

#[test]
fn an_open_while_the_page_loads_waits_for_the_page() {
  let state = SubWindowState::default();
  let (first, built) = build_with_deadline(&state, "commit-review");
  // a page that never loads still shows, 3 s after the build
  assert_eq!(awaits(&built), (Awaited::Load, Duration::from_secs(3)));
  // two quick opens: the page still loads, the later one wins
  let second = update(&state, "second");
  assert_eq!(state.reopened("commit-review", second.clone()), Reopen::Pending);
  let third = update(&state, "third");
  assert_eq!(state.reopened("commit-review", third.clone()), Reopen::Pending);
  let dir = tempfile::tempdir().unwrap();
  let (loaded, shell) = state.loaded("commit-review", &settings(&dir));
  assert_eq!(
    loaded,
    SubWindowLoaded {
      update: Some(third.clone()),
      settings: Vec::new()
    }
  );
  // a loaded page that draws no shell (its script failed) still shows, 3 s after its load
  assert_eq!(shell.map(|it| awaits(&it)), Some((Awaited::Shell, Duration::from_secs(3))));
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
  load(&state, "branch-review");
  let next = update(&state, "next");
  assert_eq!(state.reopened("branch-review", next.clone()), Reopen::Hidden);
  assert!(!state.ready("branch-review", first.id));
  assert!(state.ready("branch-review", next.open.id));
  // on screen: a later open comes to the front at once
  assert_eq!(state.reopened("branch-review", update(&state, "later")), Reopen::Shown);
}

#[test]
fn a_deadline_shows_the_window_once_and_only_its_own_build() {
  let state = SubWindowState::default();
  let (_, closed) = build_with_deadline(&state, "pull-request-review");
  state.forget("pull-request-review");
  let (second, built) = build_with_deadline(&state, "pull-request-review");
  // the fallback timer of the closed window leaves the new one hidden
  assert!(!state.deadline_passed("pull-request-review", &closed));
  assert!(state.deadline_passed("pull-request-review", &built));
  assert!(!state.deadline_passed("pull-request-review", &built));
  // shown by the deadline: the page's later ready does nothing
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
fn a_shell_counts_once_per_load_and_only_after_it() {
  let state = SubWindowState::default();
  build(&state, "settings");
  // the page says its shell after its load (`markWindowShellDrawn`): a shell said before it is not its window's
  assert_eq!(state.drawn("settings"), None);
  load(&state, "settings");
  let shell = state.drawn("settings").expect("the shell of the loaded page");
  assert_eq!(awaits(&shell), (Awaited::Content, Duration::from_millis(300)));
  // a second title bar or a remount sets no second budget
  assert_eq!(state.drawn("settings"), None);
  // a window this state never built
  assert_eq!(state.drawn("activity"), None);
}

#[test]
fn a_page_that_loads_takes_the_changes_after_the_snapshot_of_its_build() {
  let dir = tempfile::tempdir().unwrap();
  let settings = settings(&dir);
  let state = SubWindowState::default();
  let before = set_app_field(&settings, "zoom", json!(1.1));
  // built with the snapshot at `before`; Actual Size and the Activity panel of the main window while its page loads
  state.built(
    "commit-review",
    state.next_open(0.0),
    BuiltPage::Sub {
      settings_version: settings.snapshot().0,
    },
  );
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
  state.built(
    "branch-review",
    state.next_open(0.0),
    BuiltPage::Sub {
      settings_version: settings.snapshot().0,
    },
  );
  let history = SettingsPatch::Value {
    key: "robotHistory".to_owned(),
    value: json!([{ "id": "1" }]),
  };
  settings.set(history, "main").unwrap();
  assert_eq!(state.loaded("branch-review", &settings).0.settings, Vec::new());
}

#[test]
fn the_main_window_waits_for_its_first_rows_longer_and_takes_no_settings_with_its_load() {
  let dir = tempfile::tempdir().unwrap();
  let settings = settings(&dir);
  let state = SubWindowState::default();
  let open = state.next_open(0.0);
  state.built("main", open, BuiltPage::Main);
  // a change while the page loads: the main window's page asks for it itself (`get_settings_changes`)
  set_app_field(&settings, "zoom", json!(1.1));
  let (loaded, shell) = state.loaded("main", &settings);
  assert_eq!(
    loaded,
    SubWindowLoaded {
      update: None,
      settings: Vec::new()
    }
  );
  assert_eq!(shell.map(|it| awaits(&it)), Some((Awaited::Shell, Duration::from_secs(3))));
  // the title row and the loading table: the first rows may take 1 s from them
  let content = state.drawn("main").unwrap();
  assert_eq!(awaits(&content), (Awaited::Content, Duration::from_secs(1)));
  // its first rows show it, once
  assert!(state.ready("main", open.id));
  assert!(!state.ready("main", open.id));
  // a reload of its page: on screen, its deadlines and its ready do nothing
  let (_, reloaded) = state.loaded("main", &settings);
  assert!(!state.deadline_passed("main", &reloaded.unwrap()));
  assert!(!state.deadline_passed("main", &state.drawn("main").unwrap()));
  assert!(!state.ready("main", open.id));
}

/// A main window built hidden: its open and the deadline of its load
fn built_main(state: &SubWindowState) -> (SubWindowOpen, Deadline) {
  let open = state.next_open(0.0);
  let deadline = state.built("main", open, BuiltPage::Main);
  (open, deadline)
}

/// Waits for `deadline` of the main window: when it showed the window, from `start`, if it did
async fn shown_by(state: &SubWindowState, deadline: Deadline, start: Instant) -> Option<Duration> {
  wait_for_deadline(state, "main", deadline).await.then(|| start.elapsed())
}

#[tokio::test(start_paused = true)]
async fn a_page_that_loaded_is_never_shown_by_the_fallback_of_its_load() {
  // the release app on the IntelliJ monorepo at load 90: the page loaded 2.26 s after the build, and the fallback of
  // the load showed it 0.74 s later as a page that did not load, before its own budget, on no content
  let state = SubWindowState::default();
  let start = Instant::now();
  let (open, built) = built_main(&state);
  let (by_load, by_shell, by_content, ready) = tokio::join!(
    shown_by(&state, built, start),
    async {
      sleep(Duration::from_millis(2260)).await;
      let (_, shell) = load(&state, "main");
      shown_by(&state, shell, start).await
    },
    async {
      sleep(Duration::from_millis(2500)).await;
      let content = state.drawn("main").unwrap();
      shown_by(&state, content, start).await
    },
    async {
      sleep(Duration::from_millis(4000)).await;
      state.ready("main", open.id)
    }
  );
  assert_eq!(by_load, None);
  assert_eq!(by_shell, None);
  // the budget of the content after the shell showed it: the title row and the loading table
  assert_eq!(by_content, Some(Duration::from_millis(3500)));
  // the rows that came later show nothing again
  assert!(!ready);
}

#[tokio::test(start_paused = true)]
async fn the_budget_of_the_content_counts_from_the_shell_not_from_the_load() {
  // the page loaded and took 0.8 s to commit its shell (a busy machine): 1 s after the load would show a bare canvas
  let state = SubWindowState::default();
  built_main(&state);
  let start = Instant::now();
  let (_, shell) = load(&state, "main");
  let (by_shell, by_content) = tokio::join!(shown_by(&state, shell, start), async {
    sleep(Duration::from_millis(800)).await;
    let content = state.drawn("main").unwrap();
    shown_by(&state, content, start).await
  });
  assert_eq!(by_shell, None);
  assert_eq!(by_content, Some(Duration::from_millis(1800)));
}

#[tokio::test(start_paused = true)]
async fn a_page_that_loaded_and_draws_no_shell_shows_once_the_fallback_of_its_shell_passed() {
  let state = SubWindowState::default();
  built_main(&state);
  let start = Instant::now();
  let (_, shell) = load(&state, "main");
  assert_eq!(shown_by(&state, shell, start).await, Some(Duration::from_secs(3)));
}

#[tokio::test(start_paused = true)]
async fn a_page_that_never_loads_shows_once_the_fallback_of_its_build_passed() {
  let state = SubWindowState::default();
  let start = Instant::now();
  let (open, built) = built_main(&state);
  assert_eq!(shown_by(&state, built, start).await, Some(Duration::from_secs(3)));
  assert!(!state.ready("main", open.id));
}

#[tokio::test(start_paused = true)]
async fn the_backend_shows_a_main_window_whose_page_says_nothing_after_its_shell_once_its_budget_passed() {
  let state = SubWindowState::default();
  let (open, _) = built_main(&state);
  load(&state, "main");
  // the first sync is slow and the page says nothing after its shell: the timers of a hidden page are throttled, the
  // backend's clock is not, and the budget shows the title row and the loading table 1 s after the shell
  let start = Instant::now();
  let content = state.drawn("main").unwrap();
  assert_eq!(shown_by(&state, content, start).await, Some(Duration::from_secs(1)));
  // the rows that come later show nothing again
  assert!(!state.ready("main", open.id));
}

#[tokio::test(start_paused = true)]
async fn the_first_rows_within_the_budget_show_the_main_window_and_the_budget_then_does_nothing() {
  let state = SubWindowState::default();
  let (open, _) = built_main(&state);
  load(&state, "main");
  let content = state.drawn("main").unwrap();
  let start = Instant::now();
  let (by_budget, by_page) = tokio::join!(shown_by(&state, content, start), async {
    sleep(Duration::from_millis(600)).await;
    state.ready("main", open.id)
  });
  assert!(by_page);
  assert_eq!(by_budget, None);
}

#[tokio::test(start_paused = true)]
async fn a_reopen_while_the_page_loads_shows_the_window_on_the_content_of_the_reopen() {
  let state = SubWindowState::default();
  let start = Instant::now();
  let (first, built) = build_with_deadline(&state, "branch-review");
  let reopen = update(&state, "the branch the user picked next");
  let (by_load, by_content, shown_for) = tokio::join!(
    wait_for_deadline(&state, "branch-review", built),
    async {
      sleep(Duration::from_millis(100)).await;
      assert_eq!(state.reopened("branch-review", reopen.clone()), Reopen::Pending);
      sleep(Duration::from_millis(400)).await;
      // the load hands the reopen over; the shell then starts the budget of its content
      assert_eq!(load(&state, "branch-review").0, Some(reopen.clone()));
      let content = state.drawn("branch-review").unwrap();
      wait_for_deadline(&state, "branch-review", content).await
    },
    async {
      sleep(Duration::from_millis(600)).await;
      // the content of the build settles first: the window waits for the reopen's
      let by_build = state.ready("branch-review", first.id);
      sleep(Duration::from_millis(100)).await;
      (by_build, state.ready("branch-review", reopen.open.id), start.elapsed())
    }
  );
  assert!(!by_load);
  assert!(!by_content);
  assert_eq!(shown_for, (false, true, Duration::from_millis(700)));
}
