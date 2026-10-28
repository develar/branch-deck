use crate::reads::conflict_fix::ConflictFixSearch;
use std::sync::atomic::Ordering;

/// The search of the fix is one at a time: a new one stops the one in flight, a cancel (a sync starts) the latest, and
/// a cancel with none in flight stops nothing that starts after it
#[test]
fn a_new_search_of_the_fix_stops_the_one_in_flight_and_a_cancel_the_latest() {
  let search = ConflictFixSearch::default();
  search.cancel();
  let first = search.begin();
  assert!(!first.load(Ordering::Relaxed));
  let second = search.begin();
  assert!(first.load(Ordering::Relaxed), "the new search stops the one before");
  assert!(!second.load(Ordering::Relaxed));
  search.cancel();
  assert!(second.load(Ordering::Relaxed));
}
