// Ported from IntelliJ Platform (Apache License 2.0, Copyright 2000-2020 JetBrains s.r.o. and contributors):
// `community/platform/util/testSrc/com/intellij/util/diff/IntLCSTest.java`, `IntLCSNewTest.java` and the Myers half
// of `IntLCSAutoTest.java`, as a property test with a fixed seed instead of an unseeded generator.

use crate::lcs::diff::{Change, ChangeBuilder};
use crate::lcs::diff_test::change;
use crate::lcs::myers::MyersLcs;
use crate::lcs::reindexer::Reindexer;
use crate::lcs::window::Window;
use crate::test_support::proptest_config;
use fixedbitset::FixedBitSet;
use pretty_assertions::assert_eq;
use proptest::collection::vec;
use proptest::prelude::{Just, Strategy};
use proptest::proptest;

fn build_change(first: &[u32], second: &[u32], expected_non_diags: usize) -> Vec<Change> {
  assert_eq!(count_changes(first, second), expected_non_diags);
  let mut lcs = MyersLcs::new(first, second);
  lcs.execute();
  let mut reindexer = Reindexer::default();
  reindexer.id_init(first.len(), second.len());
  let mut builder = ChangeBuilder::new(0);
  reindexer.reindex(&lcs.into_changes(), &mut builder);
  builder.into_changes()
}

fn count_changes(first: &[u32], second: &[u32]) -> usize {
  let mut lcs = MyersLcs::new(first, second);
  lcs.execute();
  let [changes1, changes2] = lcs.changes();
  changes1.count_ones(..) + changes2.count_ones(..)
}

fn build_bits(first: &[u32], second: &[u32]) -> [FixedBitSet; 2] {
  let mut lcs = MyersLcs::new(first, second);
  lcs.execute();
  lcs.into_changes()
}

/// Checks the first bits of both sets against `1` (changed) and `0` (unchanged)
pub(crate) fn check_bits(changes: &[FixedBitSet; 2], expected1: &[u8], expected2: &[u8]) {
  for (i, &expected) in expected1.iter().enumerate() {
    assert_eq!(changes[0].contains(i), expected == 1, "first, index {i}");
  }
  for (i, &expected) in expected2.iter().enumerate() {
    assert_eq!(changes[1].contains(i), expected == 1, "second, index {i}");
  }
}

/// The unchanged elements of both windows pair up and are equal, the changed ones are outside the LCS
pub(crate) fn verify_lcs(sequence1: &[u32], sequence2: &[u32], window: [usize; 4], changes1: &FixedBitSet, changes2: &FixedBitSet) {
  let [start1, count1, start2, count2] = window;
  let mut index1 = next_clear_bit(changes1, start1);
  let mut index2 = next_clear_bit(changes2, start2);

  while index1 < start1 + count1 || index2 < start2 + count2 {
    assert!(index1 < start1 + count1);
    assert!(index2 < start2 + count2);

    assert_eq!(sequence1[index1], sequence2[index2]);

    index1 = next_clear_bit(changes1, index1 + 1);
    index2 = next_clear_bit(changes2, index2 + 1);
  }
  assert!(index1 >= start1 + count1 && index2 >= start2 + count2);
}

/// The first index at or after `from` whose bit is not set (`BitSet.nextClearBit`)
fn next_clear_bit(set: &FixedBitSet, from: usize) -> usize {
  // the bits at and past the length are clear
  (from..=set.len().max(from)).find(|&index| !set.contains(index)).expect("a clear bit")
}

// IntLCSTest

#[test]
fn diag() {
  assert_eq!(build_change(&[1, 2, 3], &[1, 2, 3], 0), vec![]);
}

#[test]
fn one_at_beginning() {
  assert_eq!(build_change(&[1, 2], &[1, 3], 2), vec![change(1, 1, 1, 1)]);
}

#[test]
fn one_at_end() {
  assert_eq!(build_change(&[1, 3], &[2, 3], 2), vec![change(0, 0, 1, 1)]);
}

#[test]
fn one_over_at_end() {
  assert_eq!(build_change(&[1, 2], &[1, 2, 3], 1), vec![change(2, 2, 1, 0)]);
}

#[test]
fn one_over_at_beginning() {
  assert_eq!(build_change(&[1, 2, 3], &[2, 3], 1), vec![change(0, 0, 0, 1)]);
}

#[test]
fn one_tail() {
  assert_eq!(count_changes(&[1, 2], &[1, 2, 3]), 1);
}

#[test]
fn single_middle() {
  assert_eq!(build_change(&[1, 2, 3], &[4, 2, 5], 4), vec![change(0, 0, 1, 1), change(2, 2, 1, 1)]);
}

#[test]
fn absolutely_different() {
  assert_eq!(count_changes(&[1, 2], &[3, 4]), 4);
  assert_eq!(count_changes(&[1, 2, 3], &[4, 5, 6]), 6);
}

// IntLCSNewTest

#[test]
fn bits_equal() {
  check_bits(&build_bits(&[1, 2, 3], &[1, 2, 3]), &[0, 0, 0], &[0, 0, 0]);
}

#[test]
fn bits_one_at_beginning() {
  check_bits(&build_bits(&[1, 2], &[1, 3]), &[0, 1], &[0, 1]);
}

#[test]
fn bits_one_at_end() {
  check_bits(&build_bits(&[1, 3], &[2, 3]), &[1, 0], &[1, 0]);
}

#[test]
fn bits_one_over_at_end() {
  check_bits(&build_bits(&[1, 2], &[1, 2, 3]), &[0, 0], &[0, 0, 1]);
}

#[test]
fn bits_one_over_at_beginning() {
  check_bits(&build_bits(&[1, 2, 3], &[2, 3]), &[1, 0, 0], &[0, 0]);
}

#[test]
fn bits_single_middle() {
  check_bits(&build_bits(&[1, 2, 3], &[4, 2, 5]), &[1, 0, 1], &[1, 0, 1]);
}

#[test]
fn bits_absolutely_different() {
  check_bits(&build_bits(&[1, 2], &[3, 4]), &[1, 1], &[1, 1]);
  check_bits(&build_bits(&[1, 2, 3], &[4, 5, 6]), &[1, 1, 1], &[1, 1, 1]);
}

#[test]
fn bits_single_unique_moved() {
  check_bits(&build_bits(&[1, 1, 2, 2, 10], &[10, 1, 1, 2, 2]), &[0, 0, 0, 0, 1], &[1, 0, 0, 0, 0]);
}

#[test]
fn bits_single_function_moved() {
  check_bits(
    &build_bits(
      &[1, 2, 3, 4, 5, 6, 7, 8, 7, 9, 10, 2, 11, 12, 5, 13, 7, 8, 7],
      &[10, 2, 11, 12, 5, 13, 7, 8, 7, 9, 1, 2, 3, 4, 5, 6, 7, 8, 7],
    ),
    &[1, 0, 1, 1, 0, 1, 0, 0, 0, 0, 1, 0, 1, 1, 0, 1],
    &[1, 0, 1, 1, 0, 1, 0, 0, 0, 0, 1, 0, 1, 1, 0, 1],
  );
}

// IntLCSAutoTest

const CASES: u32 = 1000;

/// Two sequences of 150 to 299 values below 20 and a random window of each, as `IntLCSAutoTest.doTestLCS`
pub(crate) fn sequences_with_windows() -> impl Strategy<Value = (Vec<u32>, Vec<u32>, [usize; 4])> {
  (vec(0..20_u32, 150..300), vec(0..20_u32, 150..300)).prop_flat_map(|(sequence1, sequence2)| {
    let window = |length: usize| (0..length).prop_flat_map(move |start| (Just(start), 0..length - start));
    let windows = (window(sequence1.len()), window(sequence2.len()));
    (
      Just(sequence1),
      Just(sequence2),
      windows.prop_map(|((start1, count1), (start2, count2))| [start1, count1, start2, count2]),
    )
  })
}

proptest! {
  #![proptest_config(proptest_config(CASES))]

  #[test]
  fn auto_myers_lcs((sequence1, sequence2, window) in sequences_with_windows()) {
    let [start1, count1, start2, count2] = window;
    let lcs_window = Window::new(&sequence1, &sequence2, start1, count1, start2, count2);
    let mut lcs = MyersLcs::with_changes(lcs_window, lcs_window.empty_changes());
    lcs.execute();
    let [changes1, changes2] = lcs.into_changes();
    verify_lcs(&sequence1, &sequence2, window, &changes1, &changes2);
  }
}
