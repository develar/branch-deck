use crate::commands::window_management::{SubWindowState, SubWindowUpdate};

fn update(data: &str) -> SubWindowUpdate {
  SubWindowUpdate {
    data: data.to_string(),
    store_cache: "{}".to_string(),
  }
}

#[test]
fn an_open_while_the_page_loads_waits_for_the_page() {
  let state = SubWindowState::default();
  state.built("commit-review");
  // two quick opens: the page still loads, the later one wins
  assert!(!state.reopened("commit-review", update("first")));
  assert!(!state.reopened("commit-review", update("second")));
  assert_eq!(state.ready("commit-review"), Some(update("second")));
  // taken once
  assert_eq!(state.ready("commit-review"), None);
}

#[test]
fn a_loaded_page_takes_an_open_at_once() {
  let state = SubWindowState::default();
  state.built("branch-review");
  assert_eq!(state.ready("branch-review"), None);
  assert!(state.reopened("branch-review", update("next")));
  assert_eq!(state.ready("branch-review"), None);
}

#[test]
fn a_window_built_again_after_its_close_waits_for_its_page_again() {
  let state = SubWindowState::default();
  state.built("pull-request-review");
  state.ready("pull-request-review");
  state.forget("pull-request-review");
  state.built("pull-request-review");
  assert!(!state.reopened("pull-request-review", update("again")));
  assert_eq!(state.ready("pull-request-review"), Some(update("again")));
}
