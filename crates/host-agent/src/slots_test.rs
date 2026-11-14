use super::*;
use crate::home::Home;
use lease_pool::host_slots::{fill, watches_started};
use pretty_assertions::assert_eq;
use std::sync::atomic::{AtomicU32, Ordering};
use test_utils::git_test_utils::{TestRepo, append_config};

const AFFINITY: &str = "checkout-0123456789ab";

/// A wait for a host slot keeps one watch of the lend table for all of its looks of 2 s, where a watch per look started a
/// stream of file events of the system each time: the wait here runs two looks before its stop
#[test]
fn a_wait_for_a_slot_keeps_one_watch_of_the_table_for_all_its_looks() {
  let home = tempfile::tempdir().expect("a temporary home");
  let repo = TestRepo::new();
  repo.create_commit("first", "a.txt", "a\n");
  // the minimum free space of the user's config is no matter of this test
  append_config(repo.path(), &[("branchdeck.hostSlots", "1"), ("branchdeck.leaseMinFree", "0")]);
  let opened = Repository::open(&Home::new(home.path().to_path_buf()), repo.path().to_path_buf()).expect("the repository");
  assert_eq!(fill(&opened.git, &opened.params(), None).expect("the fill").len(), 1);
  let mut quiet = |_: &str| {};
  let never = || false;
  let first = lend(&opened, "run-1", AFFINITY, None, &never, &mut quiet).expect("a lend");
  assert!(matches!(first, Some(LendAnswer::Lent(_))), "{first:?}");

  let looks = AtomicU32::new(0);
  // the wait asks `stop` once before each look
  let stop = || looks.fetch_add(1, Ordering::SeqCst) >= 2;
  let before = watches_started();
  let waited = lend(&opened, "run-2", AFFINITY, Some(Duration::from_hours(1)), &stop, &mut quiet).expect("a wait");
  assert_eq!(waited, None, "the stop ended the wait");
  assert_eq!(looks.load(Ordering::SeqCst), 3, "two looks, then the stop");
  assert_eq!(watches_started() - before, 1, "one watch for the whole wait");
  give_back(&opened, "run-2").expect("run-2 leaves the queue");
  give_back(&opened, "run-1").expect("the give-back of run-1");
}
