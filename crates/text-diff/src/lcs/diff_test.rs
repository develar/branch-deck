// Ported from `community/platform/util/testSrc/com/intellij/util/diff/DiffTest.java` of IntelliJ Platform (Apache
// License 2.0, Copyright 2000-2020 JetBrains s.r.o. and contributors).

use crate::lcs::diff::{Change, build_changes};
use pretty_assertions::assert_eq;

const DATA_123: [&str; 3] = ["1", "2", "3"];
const DATA_AB: [&str; 2] = ["a", "b"];
const DATA_12AB23: [&str; 6] = ["1", "2", "a", "b", "2", "3"];
const DATA_123_: [&str; 8] = ["x", "y", "z", "1", "2", "3", "alpha", "beta"];
const DATA_12AB23_: [&str; 11] = ["x", "y", "z", "1", "2", "a", "b", "2", "3", "alpha", "beta"];
const DATA_12NULLABNULL23: [Option<&str>; 8] = [Some("1"), Some("2"), None, Some("a"), Some("b"), None, Some("2"), Some("3")];
const DATA_NULL12AB23: [Option<&str>; 7] = [None, Some("1"), Some("2"), Some("a"), Some("b"), Some("2"), Some("3")];

/// A change in the argument order of `IntLCSTest.checkChange`
pub(crate) const fn change(line0: usize, line1: usize, inserted: usize, deleted: usize) -> Change {
  Change { line0, line1, deleted, inserted }
}

fn changes<T: Eq + std::hash::Hash>(objects1: &[T], objects2: &[T]) -> Vec<Change> {
  build_changes(objects1, objects2).expect("small inputs")
}

#[test]
fn equal() {
  assert_eq!(changes(&DATA_123, &DATA_123), vec![]);
}

#[test]
fn completely_different() {
  assert_eq!(changes(&DATA_AB, &DATA_123), vec![change(0, 0, 3, 2)]);
}

#[test]
fn same_middle() {
  assert_eq!(changes(&DATA_123, &["a", "2", "b"]), vec![change(0, 0, 1, 1), change(2, 2, 1, 1)]);
}

#[test]
fn overlap() {
  // inserted: AB2
  assert_eq!(changes(&DATA_123, &DATA_12AB23), vec![change(2, 2, 3, 0)]);
}

#[test]
fn trim() {
  // inserted: AB2
  assert_eq!(changes(&DATA_123_, &DATA_12AB23_), vec![change(5, 5, 3, 0)]);
}

#[test]
fn equal_up_to_one_end() {
  assert_eq!(changes(&DATA_AB, &["a", "b", "1"]), vec![change(2, 2, 1, 0)]);
}

#[test]
fn empty_against_something() {
  let empty: [&str; 0] = [];
  assert_eq!(changes(&empty, &empty), vec![]);
  assert_eq!(changes(&DATA_AB, &empty), vec![change(0, 0, 0, 2)]);
  assert_eq!(changes(&empty, &DATA_123), vec![change(0, 0, 3, 0)]);
}

#[test]
fn nulls() {
  assert_eq!(
    changes(&DATA_12NULLABNULL23, &DATA_NULL12AB23),
    vec![change(0, 0, 1, 0), change(2, 3, 0, 1), change(5, 5, 0, 1)]
  );
}
