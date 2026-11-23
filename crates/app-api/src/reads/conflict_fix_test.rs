use crate::reads::conflict_fix::ConflictFixSearch;
use crate::transport::{ConnectionId, WindowId};
use std::sync::atomic::Ordering;

/// The search of the fix is one at a time per window: a new one of the window stops the one in flight, a cancel (a sync
/// starts) the latest, and a cancel with none in flight stops nothing that starts after it
#[test]
fn a_new_search_of_the_fix_stops_the_one_in_flight_and_a_cancel_the_latest() {
  let (search, main) = (ConflictFixSearch::default(), WindowId::main());
  search.cancel(&main);
  let first = search.begin(&main);
  assert!(!first.load(Ordering::Relaxed));
  let second = search.begin(&main);
  assert!(first.load(Ordering::Relaxed), "the new search stops the one before");
  assert!(!second.load(Ordering::Relaxed));
  search.cancel(&main);
  assert!(second.load(Ordering::Relaxed));
}

/// The main windows of two connections search apart: a search, a cancel and the close of one never stop the search of
/// the other
#[test]
fn the_search_of_a_window_of_another_connection_goes_on() {
  let search = ConflictFixSearch::default();
  let (app, ide) = (WindowId::main(), WindowId::main_of(ConnectionId(1)));
  let of_app = search.begin(&app);
  let of_ide = search.begin(&ide);
  search.cancel(&ide);
  assert!(of_ide.load(Ordering::Relaxed));
  assert!(!of_app.load(Ordering::Relaxed), "the cancel of the IDE stops only its own search");
  let again = search.begin(&ide);
  search.forget(&ide);
  assert!(again.load(Ordering::Relaxed), "a close stops the search of the window");
  assert!(!of_app.load(Ordering::Relaxed));
}
