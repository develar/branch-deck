// Ported and modified from IntelliJ Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and
// contributors): `community/platform/util/diff/src/com/intellij/util/diff/Diff.kt`, `LCSBuilder.kt`, `DiffConfig.kt`
// and `FilesTooBigForDiffException.kt`. A `Vec<Change>` replaces the linked list of `Diff.Change`. Left out: the line
// splitting, `translateLine` and `linesDiff`, which the comparison does not use, and the `USE_PATIENCE_ALG` switch,
// which is off.

//! The changes between two sequences: the common prefix and suffix cut off, the elements only one side has discarded,
//! Myers' algorithm, and patience diff when Myers gives up.

use crate::lcs::enumerator::Enumerator;
use crate::lcs::myers::MyersLcs;
use crate::lcs::patience::PatienceIntLcs;
use crate::lcs::reindexer::Reindexer;
use fixedbitset::FixedBitSet;
use std::hash::Hash;

/// The edit distance below which Myers' algorithm always runs to the end
pub const DELTA_THRESHOLD_SIZE: usize = 20000;
/// Do not try to compare lines by word after that many prior failures
pub const MAX_BAD_LINES: usize = 3;
/// The resolve of a conflict is the greedy one (`try_greedy_resolve`) instead of the simple one
pub const USE_GREEDY_MERGE_MAGIC_RESOLVE: bool = false;
/// A line with this many or fewer non-space chars cannot anchor a match of lines
pub const UNIMPORTANT_LINE_CHAR_COUNT: usize = 3;

/// The sequences differ in too many places for the time an exact answer takes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilesTooBig;

/// One edit: `deleted` elements of the first sequence at `line0` became `inserted` elements of the second at `line1`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
  /// The index of the first deleted element
  pub line0: usize,
  /// The index of the first inserted element
  pub line1: usize,
  pub deleted: usize,
  pub inserted: usize,
}

/// Receives the result of an LCS in order: runs of equal elements and changes
pub trait LcsBuilder {
  fn add_equal(&mut self, length: usize);

  fn add_change(&mut self, first: usize, second: usize);
}

/// Collects the changes an LCS reports into a list
pub struct ChangeBuilder {
  index1: usize,
  index2: usize,
  changes: Vec<Change>,
}

impl ChangeBuilder {
  pub const fn new(start_shift: usize) -> Self {
    Self {
      index1: start_shift,
      index2: start_shift,
      changes: Vec::new(),
    }
  }

  pub fn into_changes(self) -> Vec<Change> {
    self.changes
  }
}

impl LcsBuilder for ChangeBuilder {
  fn add_equal(&mut self, length: usize) {
    self.index1 += length;
    self.index2 += length;
  }

  fn add_change(&mut self, first: usize, second: usize) {
    self.changes.push(Change {
      line0: self.index1,
      line1: self.index2,
      deleted: first,
      inserted: second,
    });
    self.index1 += first;
    self.index2 += second;
  }
}

/// The changes between two sequences of objects, compared by `Eq` and `Hash`
pub fn build_changes<T: Eq + Hash>(objects1: &[T], objects2: &[T]) -> Result<Vec<Change>, FilesTooBig> {
  let start_shift = get_start_shift(objects1, objects2);
  let end_cut = get_end_cut(objects1, objects2, start_shift);

  if let Some(changes) = build_changes_fast(objects1.len(), objects2.len(), start_shift, end_cut) {
    return Ok(changes);
  }

  let trimmed_length = objects1.len() + objects2.len() - 2 * start_shift - 2 * end_cut;
  let mut enumerator = Enumerator::new(trimmed_length);
  let ints1 = enumerator.enumerate(objects1, start_shift, end_cut);
  let ints2 = enumerator.enumerate(objects2, start_shift, end_cut);
  do_build_changes(&ints1, &ints2, ChangeBuilder::new(start_shift))
}

/// The changes between two sequences of integers
pub fn build_changes_ints(array1: &[u32], array2: &[u32]) -> Result<Vec<Change>, FilesTooBig> {
  let start_shift = get_start_shift(array1, array2);
  let end_cut = get_end_cut(array1, array2, start_shift);

  if let Some(changes) = build_changes_fast(array1.len(), array2.len(), start_shift, end_cut) {
    return Ok(changes);
  }

  let ints1 = &array1[start_shift..array1.len() - end_cut];
  let ints2 = &array2[start_shift..array2.len() - end_cut];
  do_build_changes(ints1, ints2, ChangeBuilder::new(start_shift))
}

/// The answer without an LCS, when one side is empty after the common prefix and suffix are cut off
fn build_changes_fast(length1: usize, length2: usize, start_shift: usize, end_cut: usize) -> Option<Vec<Change>> {
  let trimmed_length1 = length1 - start_shift - end_cut;
  let trimmed_length2 = length2 - start_shift - end_cut;
  if trimmed_length1 != 0 && trimmed_length2 != 0 {
    return None;
  }
  if trimmed_length1 != 0 || trimmed_length2 != 0 {
    return Some(vec![Change {
      line0: start_shift,
      line1: start_shift,
      deleted: trimmed_length1,
      inserted: trimmed_length2,
    }]);
  }
  Some(Vec::new())
}

fn do_build_changes(ints1: &[u32], ints2: &[u32], mut builder: ChangeBuilder) -> Result<Vec<Change>, FilesTooBig> {
  // discard unique elements, that have no chance to be matched
  let mut reindexer = Reindexer::default();
  let [discarded1, discarded2] = reindexer.discard_unique(ints1, ints2);

  if discarded1.is_empty() && discarded2.is_empty() {
    builder.add_change(ints1.len(), ints2.len());
    return Ok(builder.into_changes());
  }

  let changes: [FixedBitSet; 2] = {
    let mut lcs = MyersLcs::new(&discarded1, &discarded2);
    if lcs.execute_with_threshold() == Ok(()) {
      lcs.into_changes()
    } else {
      let mut patience = PatienceIntLcs::new(&discarded1, &discarded2);
      patience.execute(true)?;
      patience.into_changes()
    }
  };

  reindexer.reindex(&changes, &mut builder);
  Ok(builder.into_changes())
}

fn get_start_shift<T: PartialEq>(o1: &[T], o2: &[T]) -> usize {
  o1.iter().zip(o2).take_while(|(a, b)| a == b).count()
}

fn get_end_cut<T: PartialEq>(o1: &[T], o2: &[T], start_shift: usize) -> usize {
  let size = o1.len().min(o2.len()) - start_shift;
  o1.iter().rev().zip(o2.iter().rev()).take(size).take_while(|(a, b)| a == b).count()
}
