// Ported and modified from IntelliJ Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and
// contributors), `community/platform/util/diff/src/com/intellij/diff/comparison/iterables/`: `DiffIterable.kt`,
// `ChangeDiffIterableBase.kt`, `DiffChangeDiffIterable.kt`, `DiffFragmentsDiffIterable.kt`, `RangesDiffIterable.kt`,
// `InvertedDiffIterableWrapper.kt`, `ExpandedDiffIterable.kt`, `SubiterableDiffIterable.kt`, `FairDiffIterable.kt` and
// `FairDiffIterableWrapper.kt`. One struct holds the changed ranges; each wrapper of the source is a constructor that
// computes its ranges at once. A fair iterable is the same struct, checked by `fair`.

//! The changed ranges of two sequences, and the unchanged ranges between them.

use crate::fragments::DiffFragment;
use crate::lcs::diff::Change;
use crate::range::Range;

/// The computed differences between two sequences.
///
/// No range is empty, and the ranges do not overlap. They are not always squashed: two changed ranges can touch. In a
/// fair iterable (`DiffIterableUtil.fair`) the elements are compared one by one, so an unchanged range has the same
/// length on both sides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffIterable {
  length1: usize,
  length2: usize,
  changes: Vec<Range>,
}

impl DiffIterable {
  /// The changes given as ranges (`RangesDiffIterable`)
  pub const fn from_ranges(changes: Vec<Range>, length1: usize, length2: usize) -> Self {
    Self { length1, length2, changes }
  }

  /// The changes of an LCS (`DiffChangeDiffIterable`)
  pub fn from_changes(changes: &[Change], length1: usize, length2: usize) -> Self {
    let changes = changes
      .iter()
      .map(|change| Range::new(change.line0, change.line0 + change.deleted, change.line1, change.line1 + change.inserted))
      .collect();
    Self { length1, length2, changes }
  }

  /// The changes given as fragments (`DiffFragmentsDiffIterable`)
  pub fn from_fragments(fragments: &[DiffFragment], length1: usize, length2: usize) -> Self {
    let changes = fragments
      .iter()
      .map(|fragment| Range::new(fragment.start_offset1, fragment.end_offset1, fragment.start_offset2, fragment.end_offset2))
      .collect();
    Self { length1, length2, changes }
  }

  /// The unchanged ranges as changes and the changes as unchanged ranges (`InvertedDiffIterableWrapper`)
  #[must_use]
  pub fn inverted(&self) -> Self {
    Self {
      length1: self.length1,
      length2: self.length2,
      changes: self.unchanged().collect(),
    }
  }

  /// The changes shifted by an offset into longer sequences (`ExpandedDiffIterable`)
  #[must_use]
  pub fn expanded(&self, offset1: usize, offset2: usize, length1: usize, length2: usize) -> Self {
    let changes = self
      .changes
      .iter()
      .map(|range| Range::new(range.start1 + offset1, range.end1 + offset1, range.start2 + offset2, range.end2 + offset2))
      .collect();
    Self { length1, length2, changes }
  }

  /// The parts of `changed` inside `[start1, end1)` - `[start2, end2)`, relative to the start of it
  /// (`SubiterableDiffIterable`). `first_index` is the first range of `changed` that can reach into it, so that each
  /// window does not search from the start.
  pub fn subiterable(changed: &[Range], window: Range, first_index: usize) -> Self {
    let Range { start1, end1, start2, end2 } = window;
    let mut changes = Vec::new();
    for range in &changed[first_index..] {
      if range.end1 < start1 || range.end2 < start2 {
        continue;
      }
      if range.start1 > end1 || range.start2 > end2 {
        break;
      }

      let new_range = Range::new(
        start1.max(range.start1) - start1,
        end1.min(range.end1) - start1,
        start2.max(range.start2) - start2,
        end2.min(range.end2) - start2,
      );
      if !new_range.is_empty() {
        changes.push(new_range);
      }
    }
    Self {
      length1: end1 - start1,
      length2: end2 - start2,
      changes,
    }
  }

  /// The length of the first sequence
  pub const fn length1(&self) -> usize {
    self.length1
  }

  /// The length of the second sequence
  pub const fn length2(&self) -> usize {
    self.length2
  }

  /// The changed ranges in order
  pub fn changes(&self) -> &[Range] {
    &self.changes
  }

  /// The unchanged ranges in order: the gaps before, between and after the changes
  pub fn unchanged(&self) -> Unchanged<'_> {
    let mut iterator = Unchanged {
      changes: &self.changes,
      index: 0,
      last_index1: 0,
      last_index2: 0,
      length1: self.length1,
      length2: self.length2,
    };
    if let Some(first) = self.changes.first()
      && first.start1 == 0
      && first.start2 == 0
    {
      iterator.last_index1 = first.end1;
      iterator.last_index2 = first.end2;
      iterator.index = 1;
    }
    iterator
  }
}

/// The unchanged ranges of a `DiffIterable` (`ChangeDiffIterableBase.UnchangedIterator`)
pub struct Unchanged<'a> {
  changes: &'a [Range],
  index: usize,
  last_index1: usize,
  last_index2: usize,
  length1: usize,
  length2: usize,
}

impl Iterator for Unchanged<'_> {
  type Item = Range;

  fn next(&mut self) -> Option<Range> {
    if let Some(change) = self.changes.get(self.index) {
      debug_assert!(change.start1 >= self.last_index1 || change.start2 >= self.last_index2);
      let chunk = Range::new(self.last_index1, change.start1, self.last_index2, change.start2);

      self.last_index1 = change.end1;
      self.last_index2 = change.end2;
      self.index += 1;

      Some(chunk)
    } else if self.last_index1 != self.length1 || self.last_index2 != self.length2 {
      let chunk = Range::new(self.last_index1, self.length1, self.last_index2, self.length2);

      self.last_index1 = self.length1;
      self.last_index2 = self.length2;

      Some(chunk)
    } else {
      None
    }
  }
}
