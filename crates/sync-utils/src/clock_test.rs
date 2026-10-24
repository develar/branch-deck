use crate::clock::{Clock, SystemClock, TestClock};
use std::time::Duration;

#[test]
fn a_test_clock_moves_only_by_advance_and_its_clones_share_the_time() {
  let clock = TestClock::new();
  let shared = clock.clone();
  assert_eq!(clock.now(), Duration::ZERO);
  clock.advance(Duration::from_secs(120));
  assert_eq!(shared.now(), Duration::from_secs(120));
  shared.advance(Duration::from_millis(5));
  assert_eq!(clock.now(), Duration::from_millis(120_005));
  clock.advance(Duration::MAX);
  assert_eq!(clock.now(), Duration::from_nanos(u64::MAX));
}

#[test]
fn the_system_clock_never_goes_back() {
  let clock = SystemClock::new();
  let first = clock.now();
  assert!(clock.now() >= first);
}
