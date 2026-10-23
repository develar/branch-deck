use crate::commands::window_management::{Reopen, SubWindowOpen, SubWindowState, SubWindowUpdate};

fn update(state: &SubWindowState, data: &str) -> SubWindowUpdate {
  SubWindowUpdate {
    data: data.to_string(),
    open: state.next_open(0.0),
  }
}

fn build(state: &SubWindowState, label: &str) -> SubWindowOpen {
  let open = state.next_open(0.0);
  state.built(label, open);
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
  assert_eq!(state.loaded("commit-review"), (Some(third.clone()), Some(first.id)));
  // taken once
  assert_eq!(state.loaded("commit-review").0, None);
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
  assert_eq!(state.loaded("branch-review"), (None, Some(first.id)));
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
  state.loaded("pull-request-review");
  state.forget("pull-request-review");
  build(&state, "pull-request-review");
  let again = update(&state, "again");
  assert_eq!(state.reopened("pull-request-review", again.clone()), Reopen::Pending);
  assert_eq!(state.loaded("pull-request-review").0, Some(again));
}
