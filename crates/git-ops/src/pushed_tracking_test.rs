use super::pushed_tracking::{PushedTracking, catch_up_pushed_tracking};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use test_utils::git_test_utils::TestRepo;

const TRACKING: &str = "refs/remotes/origin/p/virtual/auth";

/// The remote-tracking ref of `repo`, `None` when it does not exist
fn tracking(repo: &TestRepo) -> Option<String> {
  repo.rev_parse(TRACKING).ok()
}

/// A ref git's push left at the head it replaced moves to the pushed head; one a fetch moved meanwhile (the tips read
/// after the push and the ref now differ: the compare-and-swap refuses) and one at the pushed head stay, with no process
/// for the latter
#[test]
fn a_tracking_ref_left_at_the_replaced_head_moves_to_the_pushed_one_and_a_fetched_one_stays() {
  let repo = TestRepo::new();
  let replaced = repo.create_commit("Replaced", "a.txt", "a");
  let pushed = repo.create_commit("Pushed", "b.txt", "b");
  let fetched = repo.create_commit("Fetched", "c.txt", "c");
  let (git, path) = (repo.git_executor(), repo.path_str());
  let push = [PushedTracking {
    tracking_ref: TRACKING,
    replaced: Some(&replaced),
    pushed: &pushed,
  }];
  let read = |tip: &str| BTreeMap::from([(TRACKING.to_string(), Some(tip.to_string()))]);

  repo.git(&["update-ref", TRACKING, &replaced]);
  assert_eq!(catch_up_pushed_tracking(git, path, &push, &read(&replaced)), [TRACKING]);
  assert_eq!(tracking(&repo), Some(pushed.clone()));

  assert_eq!(catch_up_pushed_tracking(git, path, &push, &read(&pushed)), Vec::<String>::new(), "at the pushed head");

  // read at the replaced head after the push, moved by a fetch before the transaction
  repo.git(&["update-ref", TRACKING, &fetched]);
  assert_eq!(catch_up_pushed_tracking(git, path, &push, &read(&replaced)), Vec::<String>::new());
  assert_eq!(tracking(&repo), Some(fetched), "the fetch's head stays");
}
