// Ported from `community/platform/util/testSrc/com/intellij/util/diff/UniqueLCSTest.java` of IntelliJ Platform (Apache
// License 2.0, Copyright 2000-2014 JetBrains s.r.o. and contributors).

use crate::lcs::unique_lcs::UniqueLcs;
use pretty_assertions::assert_eq;

fn build_change(first: &[u32], second: &[u32]) -> Option<[Vec<usize>; 2]> {
  UniqueLcs::new(first, second).execute()
}

fn check_change(change: Option<[Vec<usize>; 2]>, expected1: &[usize], expected2: &[usize]) {
  let [change1, change2] = change.expect("a matching");
  assert_eq!(&change1[..expected1.len()], expected1);
  assert_eq!(&change2[..expected2.len()], expected2);
}

fn check_max_sequence(sequence: &[usize], expected: &[usize]) {
  let max = sequence.iter().map(|value| value + 1).max().unwrap_or(0);

  let mut first = vec![0; sequence.len()];
  let mut second = vec![0; max + 2];

  for (i, &value) in sequence.iter().enumerate() {
    assert_eq!(second[value], 0, "Elements in sequence should be unique");
    let number = u32::try_from(i + 1).expect("a small value");
    first[i] = number;
    second[value] = number;
  }

  let [result1, result2] = build_change(&first, &second).expect("a matching");

  assert_eq!(result1.len(), result2.len());
  assert_eq!(result1.len(), expected.len());
  for (i, &expected) in expected.iter().enumerate() {
    assert_eq!(sequence[result1[i]], expected);
  }
}

#[test]
fn no_unique() {
  assert_eq!(build_change(&[1, 2, 3], &[4, 5, 6]), None);
  assert_eq!(build_change(&[1, 2, 1], &[1, 3, 1]), None);
  assert_eq!(build_change(&[1, 2, 3, 3, 2, 1], &[1, 2, 3]), None);
  assert_eq!(build_change(&[1, 2, 3], &[1, 2, 3, 3, 2, 1]), None);
  assert_eq!(build_change(&[1, 2, 1], &[2, 1, 2]), None);
  assert_eq!(build_change(&[], &[2, 1, 2]), None);
  assert_eq!(build_change(&[1, 2, 2], &[]), None);
  assert_eq!(build_change(&[], &[]), None);
}

#[test]
fn single_unique() {
  check_change(build_change(&[1], &[1]), &[0], &[0]);
  check_change(build_change(&[5, 1], &[1]), &[1], &[0]);
  check_change(build_change(&[1], &[2, 1]), &[0], &[1]);
  check_change(build_change(&[1], &[1, 4]), &[0], &[0]);
  check_change(build_change(&[1, 3], &[1]), &[0], &[0]);
  check_change(build_change(&[5, 1, 3], &[2, 1, 4]), &[1], &[1]);
}

#[test]
fn single_sequence() {
  check_change(
    build_change(&[2, 4, 6, 1, 8, 3, 8, 2, 6, 5, 2, 4, 7, 11, 13], &[1, 10, 2, 3, 5, 7, 11, 13, 12]),
    &[3, 5, 9, 12, 13, 14],
    &[0, 3, 4, 5, 6, 7],
  );
}

#[test]
fn largest_lcs1() {
  check_max_sequence(&[1, 2, 3], &[1, 2, 3]);
}

#[test]
fn largest_lcs2() {
  check_max_sequence(&[5, 6, 1, 2, 3], &[1, 2, 3]);
}

#[test]
fn largest_lcs3() {
  check_max_sequence(&[0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 7, 15], &[0, 2, 6, 9, 13, 15]);
}

#[test]
fn largest_lcs4() {
  check_max_sequence(&[1, 9, 3, 8, 11, 4, 5, 6, 19, 7], &[1, 3, 4, 5, 6, 7]);
}
