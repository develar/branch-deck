//! The search for the first conflicting commit, without git: a replay of commits is the range of their indices, and
//! the commit at `conflict` conflicts.

use crate::replay::{RunsReplay, first_conflict, first_conflict_in_runs};
use pretty_assertions::assert_eq;
use std::cell::RefCell;

/// A replay of the commits `first..=last` onto `tip`: the tip of the replayed range (`<tip>+<first>..<last>`), `None`
/// when it holds `conflict`; each probe recorded as `(first, last)`
fn replay_with(conflict: usize, probes: &RefCell<Vec<(usize, usize)>>) -> impl FnMut(usize, &str, usize) -> anyhow::Result<Option<String>> + '_ {
  move |first, tip, last| {
    probes.borrow_mut().push((first, last));
    Ok((!(first..=last).contains(&conflict)).then(|| format!("{tip}+{first}..{last}")))
  }
}

/// Each probe starts after the longest clean prefix it knows, so the probes replay each commit about once
#[test]
fn each_probe_starts_from_the_longest_clean_prefix() {
  let probes = RefCell::new(Vec::new());
  let (index, tip) = first_conflict(0, 9, "U".to_string(), replay_with(7, &probes)).unwrap();
  assert_eq!((index, tip.as_str()), (7, "U+0..4+5..6"));
  // the prefix up to the last commit conflicts already: no probe of it
  assert_eq!(probes.into_inner(), [(0, 4), (5, 7), (5, 6)]);
}

/// The runs between commits left out replay one by one; the first that conflicts is searched
#[test]
fn the_runs_replay_until_the_first_that_conflicts() {
  let probes = RefCell::new(Vec::new());
  let outcome = first_conflict_in_runs(&[3, 6], 0, 9, "U".to_string(), false, replay_with(7, &probes)).unwrap();
  assert_eq!(
    outcome,
    RunsReplay::Conflict {
      index: 7,
      tip: "U+0..2+3..5+6..6".to_string()
    }
  );
  assert_eq!(probes.into_inner(), [(0, 2), (3, 5), (6, 8), (6, 7), (6, 6)]);

  // a known conflict of the first run is searched at once; the commits before `end` replay
  let probes = RefCell::new(Vec::new());
  let outcome = first_conflict_in_runs(&[], 0, 9, "U".to_string(), true, replay_with(0, &probes)).unwrap();
  assert_eq!(outcome, RunsReplay::Conflict { index: 0, tip: "U".to_string() });
  assert_eq!(probes.into_inner(), [(0, 4), (0, 2), (0, 1), (0, 0)]);
  let probes = RefCell::new(Vec::new());
  let outcome = first_conflict_in_runs(&[3], 0, 5, "U".to_string(), false, replay_with(7, &probes)).unwrap();
  assert_eq!(outcome, RunsReplay::Clean("U+0..2+3..4".to_string()));
  assert_eq!(probes.into_inner(), [(0, 2), (3, 4)]);
}
