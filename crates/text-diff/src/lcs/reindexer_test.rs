// Ported from IntelliJ Platform (Apache License 2.0, Copyright 2000-2020 JetBrains s.r.o. and contributors):
// `community/platform/util/testSrc/com/intellij/util/diff/ReindexerTest.java` and `ReindexerNewTest.java`, the random
// example as a property test with a fixed seed.

use crate::lcs::diff::LcsBuilder;
use crate::lcs::myers::MyersLcs;
use crate::lcs::myers_test::verify_lcs;
use crate::lcs::reindexer::Reindexer;
use crate::test_support::{insert_range, proptest_config};
use fixedbitset::FixedBitSet;
use pretty_assertions::assert_eq;
use proptest::collection::vec;
use proptest::proptest;

// ReindexerTest

#[test]
fn no_unique() {
  let mut reindexer = Reindexer::default();
  let ints1 = [1, 2, 3];
  let ints2 = [3, 2, 1, 2, 2];
  let [reindexed1, reindexed2] = reindexer.discard_unique(&ints1, &ints2);
  assert_eq!(reindexed1, ints1);
  assert_eq!(reindexed2, ints2);
  for i in 0..3 {
    assert_eq!(reindexer.restore_index(i, 0), i);
    assert_eq!(reindexer.restore_index(i, 1), i);
  }
  assert_eq!(reindexer.restore_index(3, 1), 3);
  assert_eq!(reindexer.restore_index(4, 1), 4);
}

#[test]
fn some_unique() {
  let mut reindexer = Reindexer::default();
  let [reindexed1, reindexed2] = reindexer.discard_unique(&[1, 2, 3], &[3, 4, 1, 3]);
  assert_eq!(reindexed1, [1, 3]);
  assert_eq!(reindexed2, [3, 1, 3]);
  assert_eq!(reindexer.restore_index(0, 0), 0);
  assert_eq!(reindexer.restore_index(0, 1), 0);
  assert_eq!(reindexer.restore_index(1, 0), 2);
  assert_eq!(reindexer.restore_index(1, 1), 2);
  assert_eq!(reindexer.restore_index(2, 1), 3);
}

#[test]
fn all_unique() {
  let mut reindexer = Reindexer::default();
  let [reindexed1, reindexed2] = reindexer.discard_unique(&[1, 2, 3], &[4, 5]);
  assert_eq!(reindexed1, Vec::<u32>::new());
  assert_eq!(reindexed2, Vec::<u32>::new());
}

// ReindexerNewTest

/// Marks the changes a reindex reports in two sets
#[derive(Default)]
struct BitsBuilder {
  index1: usize,
  index2: usize,
  changes: [FixedBitSet; 2],
}

impl LcsBuilder for BitsBuilder {
  fn add_equal(&mut self, length: usize) {
    self.index1 += length;
    self.index2 += length;
  }

  fn add_change(&mut self, first: usize, second: usize) {
    insert_range(&mut self.changes[0], self.index1, self.index1 + first);
    insert_range(&mut self.changes[1], self.index2, self.index2 + second);
    self.index1 += first;
    self.index2 += second;
  }
}

fn check_case(ints1: &[u32], ints2: &[u32]) {
  let mut builder = BitsBuilder::default();
  let mut reindexer = Reindexer::default();
  let [discarded1, discarded2] = reindexer.discard_unique(ints1, ints2);
  let mut lcs = MyersLcs::new(&discarded1, &discarded2);
  lcs.execute();
  reindexer.reindex(&lcs.into_changes(), &mut builder);

  let [changes1, changes2] = &builder.changes;
  verify_lcs(ints1, ints2, [0, ints1.len(), 0, ints2.len()], changes1, changes2);
}

#[test]
fn new_all_unique() {
  check_case(&[1, 2, 3], &[4, 5]);
  check_case(&[], &[4, 5]);
  check_case(&[1, 2, 3], &[]);
}

#[test]
fn new_unique_beginning() {
  check_case(&[13, 15, 1, 2, 3], &[17, 1, 3]);
}

#[test]
fn new_unique_end() {
  check_case(&[1, 2, 3, 13, 15], &[1, 3, 17]);
}

#[test]
fn new_single_unique_moved() {
  check_case(&[1, 1, 2, 2, 10], &[10, 1, 1, 2, 2]);
}

#[test]
fn new_single_function_moved() {
  check_case(
    &[1, 2, 3, 4, 5, 6, 7, 8, 7, 9, 10, 2, 11, 12, 5, 13, 7, 8, 7],
    &[10, 2, 11, 12, 5, 13, 7, 8, 7, 9, 1, 2, 3, 4, 5, 6, 7, 8, 7],
  );
}

#[test]
fn new_inner_chunks() {
  check_case(&[0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0], &[1, 2, 1, 3, 1, 4, 1, 5, 1, 6, 1, 7, 1]);
  check_case(&[0, 2, 3, 0, 4, 5, 0], &[1, 2, 1, 3, 4, 5, 1]);
  check_case(&[15, 1, 2, 3, 1, 4, 5, 1, 15], &[13, 1, 2, 1, 3, 4, 5, 1, 13]);
  check_case(&[15, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 15], &[13, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 13]);
}

proptest! {
  #![proptest_config(proptest_config(10))]

  #[test]
  fn new_random_example(ints1 in vec(0..60_u32, 100), ints2 in vec(0..60_u32, 100)) {
    check_case(&ints1, &ints2);
  }
}
