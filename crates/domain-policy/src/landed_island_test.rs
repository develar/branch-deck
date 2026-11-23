use crate::landed_island::{MIN_SEEN, Seen, hidden_by_sync, seen, stays_seen};
use pretty_assertions::assert_eq;
use std::time::Duration;

/// A case of the hide: its name, the sightings with whether their windows have the focus, and what the sync hides
type HideCase = (&'static str, Vec<(Seen, bool)>, Option<u64>);

const fn at(seconds: u64, landed_at: u64) -> Seen {
  Seen {
    at: Duration::from_secs(seconds),
    landed_at,
  }
}

/// The same `landedAt` is seen since it was first; a newer one, more landed, is seen from now; a first sighting is now
#[test]
fn a_sighting_of_the_same_report_keeps_its_time_and_a_newer_one_starts_again() {
  let now = Duration::from_secs(30);
  let cases = [
    ("the first sighting", None, 100, at(30, 100)),
    ("the same report again", Some(at(5, 100)), 100, at(5, 100)),
    ("more landed", Some(at(5, 100)), 200, at(30, 200)),
  ];
  for (case, before, landed_at, expected) in cases {
    assert_eq!(seen(before, landed_at, now), expected, "{case}");
  }
}

/// A successful sync hides up to the newest `landedAt` seen at least `MIN_SEEN` ago by a window that has the focus as
/// it ends; a sighting younger than that, or of a window without the focus, hides nothing
#[test]
fn a_sync_hides_what_a_focused_window_saw_at_least_ten_seconds_ago() {
  let now = Duration::from_secs(100);
  let ten_ago = 100 - MIN_SEEN.as_secs();
  let cases: [HideCase; 7] = [
    ("nothing seen", vec![], None),
    ("seen just now", vec![(at(100, 7), true)], None),
    ("seen less than 10 s ago", vec![(at(ten_ago + 1, 7), true)], None),
    ("seen 10 s ago", vec![(at(ten_ago, 7), true)], Some(7)),
    ("seen long ago by a window without the focus", vec![(at(0, 7), false)], None),
    ("the newest of two windows that saw it long enough", vec![(at(0, 7), true), (at(ten_ago, 9), true)], Some(9)),
    (
      "a newer report seen too briefly leaves the older one seen long enough",
      vec![(at(0, 7), true), (at(95, 9), true)],
      Some(7),
    ),
  ];
  for (case, sightings, expected) in cases {
    assert_eq!(hidden_by_sync(sightings, now), expected, "{case}");
  }
}

/// A hide spends the sightings of what it hid; one of what landed later stays seen
#[test]
fn a_hide_spends_the_sightings_of_what_it_hid() {
  assert!(!stays_seen(at(0, 7), 7), "what it hid");
  assert!(!stays_seen(at(0, 5), 7), "what landed before");
  assert!(stays_seen(at(95, 9), 7), "what landed later");
}
