// Ported from IntelliJ Platform (Apache License 2.0, Copyright 2000-2014 JetBrains s.r.o. and contributors):
// `community/platform/util/testSrc/com/intellij/util/diff/PatienceIntLCSTest.java` and the patience half of
// `IntLCSAutoTest.java`, as a property test with a fixed seed instead of an unseeded generator.

use crate::lcs::myers_test::{check_bits, sequences_with_windows, verify_lcs};
use crate::lcs::patience::PatienceIntLcs;
use crate::lcs::window::Window;
use crate::test_support::proptest_config;
use fixedbitset::FixedBitSet;
use proptest::proptest;

fn build_change(first: &[u32], second: &[u32]) -> [FixedBitSet; 2] {
  let mut lcs = PatienceIntLcs::new(first, second);
  lcs.execute(false).expect("no threshold");
  lcs.into_changes()
}

fn check_aborted(should_be_aborted: bool, first: &[u32], second: &[u32]) {
  let mut lcs = PatienceIntLcs::new(first, second);
  assert_eq!(lcs.execute(true).is_err(), should_be_aborted);
}

#[test]
fn equal() {
  check_bits(&build_change(&[1, 2, 3], &[1, 2, 3]), &[0, 0, 0], &[0, 0, 0]);
}

#[test]
fn one_at_beginning() {
  check_bits(&build_change(&[1, 2], &[1, 3]), &[0, 1], &[0, 1]);
}

#[test]
fn one_at_end() {
  check_bits(&build_change(&[1, 3], &[2, 3]), &[1, 0], &[1, 0]);
}

#[test]
fn one_over_at_end() {
  check_bits(&build_change(&[1, 2], &[1, 2, 3]), &[0, 0], &[0, 0, 1]);
}

#[test]
fn one_over_at_beginning() {
  check_bits(&build_change(&[1, 2, 3], &[2, 3]), &[1, 0, 0], &[0, 0]);
}

#[test]
fn single_middle() {
  check_bits(&build_change(&[1, 2, 3], &[4, 2, 5]), &[1, 0, 1], &[1, 0, 1]);
}

#[test]
fn absolutely_different() {
  check_bits(&build_change(&[1, 2], &[3, 4]), &[1, 1], &[1, 1]);
  check_bits(&build_change(&[1, 2, 3], &[4, 5, 6]), &[1, 1, 1], &[1, 1, 1]);
}

#[test]
fn single_unique_moved() {
  check_bits(&build_change(&[1, 1, 2, 2, 10], &[10, 1, 1, 2, 2]), &[1, 1, 1, 1, 0], &[0, 1, 1, 1, 1]);
}

#[test]
fn single_function_moved() {
  check_bits(
    &build_change(
      &[1, 2, 3, 4, 5, 6, 7, 8, 7, 9, 10, 2, 11, 12, 5, 13, 7, 8, 7],
      &[10, 2, 11, 12, 5, 13, 7, 8, 7, 9, 1, 2, 3, 4, 5, 6, 7, 8, 7],
    ),
    &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0],
    &[0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
  );
}

#[test]
fn bug1() {
  check_bits(
    &build_change(&[2, 3, 4, 6, 7, 8, 9, 11, 12, 4, 6, 11, 15], &[2, 3, 6, 7, 8, 9, 6, 11, 12, 4, 11, 15, 6]),
    &[0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    &[0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0],
  );
}

#[test]
fn inner_chunks1() {
  check_bits(
    &build_change(&[0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0], &[1, 2, 1, 3, 1, 4, 1, 5, 1, 6, 1, 7, 1]),
    &[1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1],
    &[1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1],
  );
}

#[test]
fn inner_chunks2() {
  check_bits(
    &build_change(&[0, 2, 3, 0, 4, 5, 0], &[1, 2, 1, 3, 4, 5, 1]),
    &[1, 0, 0, 1, 0, 0, 1],
    &[1, 0, 1, 0, 0, 0, 1],
  );
}

#[test]
fn inner_chunks3() {
  check_bits(
    &build_change(&[15, 1, 2, 3, 1, 4, 5, 1, 15], &[13, 1, 2, 1, 3, 4, 5, 1, 13]),
    &[1, 0, 0, 0, 1, 0, 0, 0, 1],
    &[1, 0, 0, 1, 0, 0, 0, 0, 1],
  );
}

#[test]
fn inner_chunks4() {
  check_bits(
    &build_change(&[15, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 15], &[13, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 13]),
    &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
  );
}

#[test]
fn not_aborted_equal() {
  check_aborted(false, &[1, 2, 3], &[1, 2, 3]);
}

#[test]
fn not_aborted_simple() {
  check_aborted(false, &[100, 1, 2, 3, 4, 5, 100], &[1, 2, 3, 4]);
}

#[test]
fn aborted_no_matches() {
  check_aborted(true, &[4, 1, 2, 3], &[500, 100, 600]);
}

#[test]
fn aborted_simple() {
  check_aborted(true, &[4, 1, 2, 3, 100], &[5, 1, 6, 8, 9, 10]);
}

#[test]
fn not_aborted_second_step() {
  check_aborted(
    false,
    &[100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 100],
    &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0],
  );
}

#[test]
fn not_aborted_third_step() {
  check_aborted(
    false,
    &[100, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 200, 1, 0, 100],
    &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 2],
  );
}

#[test]
fn aborted_forth_step() {
  check_aborted(
    true,
    &[100, 3, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 200, 1, 0, 100],
    &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 1, 0, 2],
  );
}

proptest! {
  #![proptest_config(proptest_config(1000))]

  #[test]
  fn auto_patience_lcs((sequence1, sequence2, window) in sequences_with_windows()) {
    let [start1, count1, start2, count2] = window;
    let lcs_window = Window::new(&sequence1, &sequence2, start1, count1, start2, count2);
    let mut lcs = PatienceIntLcs::with_changes(lcs_window, lcs_window.empty_changes());
    lcs.execute(false).expect("no threshold");
    let [changes1, changes2] = lcs.into_changes();
    verify_lcs(&sequence1, &sequence2, window, &changes1, &changes2);
  }
}
