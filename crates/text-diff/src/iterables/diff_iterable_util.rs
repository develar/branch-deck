// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/comparison/iterables/DiffIterableUtil.kt`
// of IntelliJ Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors). The checks of
// `verify` run in the tests of the crate, where the source enables them for its tests. Left out: `extractDataRanges`,
// a debug helper.

//! The comparison of two sequences, the builder of a `DiffIterable`, and the checks of its ranges.

use crate::cancellation::{Deadline, DiffTooBig};
use crate::fragments::DiffFragment;
use crate::iterables::diff_iterable::DiffIterable;
use crate::lcs::diff::{FilesTooBig, build_changes, build_changes_ints};
use crate::range::Range;
use crate::trim_util::expand;
use std::hash::Hash;

impl From<FilesTooBig> for DiffTooBig {
  fn from(_: FilesTooBig) -> Self {
    Self::TooManyChanges
  }
}

/// Compares two integer sequences
pub fn diff_ints(data1: &[u32], data2: &[u32], deadline: &Deadline) -> Result<DiffIterable, DiffTooBig> {
  deadline.check_canceled()?;
  let changes = build_changes_ints(data1, data2)?;
  Ok(fair(create_from_changes(&changes, data1.len(), data2.len())))
}

/// Compares two sequences by the `Eq` and `Hash` of their elements
pub fn diff<T: Eq + Hash>(data1: &[T], data2: &[T], deadline: &Deadline) -> Result<DiffIterable, DiffTooBig> {
  deadline.check_canceled()?;
  let changes = build_changes(data1, data2)?;
  Ok(fair(create_from_changes(&changes, data1.len(), data2.len())))
}

//
// Iterable
//

pub fn create_from_changes(changes: &[crate::lcs::diff::Change], length1: usize, length2: usize) -> DiffIterable {
  let iterable = DiffIterable::from_changes(changes, length1, length2);
  verify(&iterable);
  iterable
}

pub fn create_fragments(fragments: &[DiffFragment], length1: usize, length2: usize) -> DiffIterable {
  let iterable = DiffIterable::from_fragments(fragments, length1, length2);
  verify(&iterable);
  iterable
}

/// An iterable of the given changed ranges
pub fn create(ranges: Vec<Range>, length1: usize, length2: usize) -> DiffIterable {
  let iterable = DiffIterable::from_ranges(ranges, length1, length2);
  verify(&iterable);
  iterable
}

/// An iterable of the given unchanged ranges
pub fn create_unchanged(ranges: Vec<Range>, length1: usize, length2: usize) -> DiffIterable {
  let invert = invert(&create(ranges, length1, length2));
  verify(&invert);
  invert
}

pub fn invert(iterable: &DiffIterable) -> DiffIterable {
  let wrapper = iterable.inverted();
  verify(&wrapper);
  wrapper
}

/// Marks an iterable whose unchanged ranges have the same length on both sides
pub fn fair(iterable: DiffIterable) -> DiffIterable {
  verify_fair(&iterable);
  iterable
}

pub fn expanded_iterable(iterable: &DiffIterable, offset1: usize, offset2: usize, length1: usize, length2: usize) -> DiffIterable {
  debug_assert!(offset1 + iterable.length1() <= length1 && offset2 + iterable.length2() <= length2);
  iterable.expanded(offset1, offset2, length1, length2)
}

//
// Misc
//

/// Both the changed and the unchanged ranges in order, `true` for an unchanged one
pub fn iterate_all(iterable: &DiffIterable) -> Vec<(Range, bool)> {
  let mut result = Vec::with_capacity(iterable.changes().len() * 2 + 1);
  let next_unchanged = |start1: usize, start2: usize, next_change: Option<&Range>| {
    let range = match next_change {
      Some(change) => Range::new(start1, change.start1, start2, change.start2),
      None => Range::new(start1, iterable.length1(), start2, iterable.length2()),
    };
    (!range.is_empty()).then_some(range)
  };

  let changes = iterable.changes();
  if let Some(range) = next_unchanged(0, 0, changes.first()) {
    result.push((range, true));
  }
  for (index, change) in changes.iter().enumerate() {
    result.push((*change, false));
    if let Some(range) = next_unchanged(change.end1, change.end2, changes.get(index + 1)) {
      result.push((range, true));
    }
  }
  result
}

/// How much longer the second side of a range is than the first
#[expect(clippy::cast_possible_wrap, reason = "a text is shorter than isize::MAX")]
pub const fn get_range_delta(range: &Range) -> isize {
  let deleted = range.end1 - range.start1;
  let inserted = range.end2 - range.start2;
  inserted as isize - deleted as isize
}

//
// Verification
//

/// Checks the ranges of an iterable in the tests of the crate
pub fn verify(iterable: &DiffIterable) {
  if !cfg!(test) {
    return;
  }

  verify_ranges(iterable.changes().iter().copied());
  verify_ranges(iterable.unchanged());

  verify_full_cover(iterable);
}

/// Also checks that each unchanged range has the same length on both sides
pub fn verify_fair(iterable: &DiffIterable) {
  if !cfg!(test) {
    return;
  }

  verify(iterable);

  for range in iterable.unchanged() {
    assert_eq!(range.end1 - range.start1, range.end2 - range.start2);
  }
}

fn verify_ranges(ranges: impl Iterator<Item = Range>) {
  for range in ranges {
    assert!(range.start1 <= range.end1);
    assert!(range.start2 <= range.end2);
    assert!(range.start1 != range.end1 || range.start2 != range.end2);
  }
}

fn verify_full_cover(iterable: &DiffIterable) {
  let mut last1 = 0;
  let mut last2 = 0;
  let mut last_equals: Option<bool> = None;

  for (range, equal) in iterate_all(iterable) {
    assert_eq!(last1, range.start1);
    assert_eq!(last2, range.start2);
    assert_ne!(last_equals, Some(equal));

    last1 = range.end1;
    last2 = range.end2;
    last_equals = Some(equal);
  }

  assert_eq!(last1, iterable.length1());
  assert_eq!(last2, iterable.length2());
}

//
// Helpers
//

/// Builds an iterable from the equal ranges in order: what lies between them is changed
pub struct ChangeBuilder {
  length1: usize,
  length2: usize,
  index1: usize,
  index2: usize,
  changes: Vec<Range>,
}

impl ChangeBuilder {
  pub const fn new(length1: usize, length2: usize) -> Self {
    Self {
      length1,
      length2,
      index1: 0,
      index2: 0,
      changes: Vec::new(),
    }
  }

  /// The end of the last equal range on the first side
  pub const fn index1(&self) -> usize {
    self.index1
  }

  /// The end of the last equal range on the second side
  pub const fn index2(&self) -> usize {
    self.index2
  }

  /// Marks one element of each side as equal
  pub fn mark_equal(&mut self, index1: usize, index2: usize) {
    self.mark_equal_count(index1, index2, 1);
  }

  pub fn mark_equal_count(&mut self, index1: usize, index2: usize, count: usize) {
    self.mark_equal_range(index1, index2, index1 + count, index2 + count);
  }

  /// Marks `[index1, end1)` and `[index2, end2)` as equal
  pub fn mark_equal_range(&mut self, index1: usize, index2: usize, end1: usize, end2: usize) {
    if index1 == end1 && index2 == end2 {
      return;
    }

    debug_assert!(self.index1 <= index1);
    debug_assert!(self.index2 <= index2);
    debug_assert!(index1 <= end1);
    debug_assert!(index2 <= end2);

    if self.index1 != index1 || self.index2 != index2 {
      self.changes.push(Range::new(self.index1, index1, self.index2, index2));
    }
    self.index1 = end1;
    self.index2 = end2;
  }

  fn do_finish(&mut self) {
    debug_assert!(self.index1 <= self.length1);
    debug_assert!(self.index2 <= self.length2);

    if self.length1 != self.index1 || self.length2 != self.index2 {
      self.changes.push(Range::new(self.index1, self.length1, self.index2, self.length2));
      self.index1 = self.length1;
      self.index2 = self.length2;
    }
  }

  pub fn finish(mut self) -> DiffIterable {
    self.do_finish();
    create(self.changes, self.length1, self.length2)
  }
}

/// A `ChangeBuilder` that shrinks each change by the equal elements at its ends
pub struct ExpandChangeBuilder<'a, T> {
  builder: ChangeBuilder,
  objects1: &'a [T],
  objects2: &'a [T],
}

impl<'a, T: PartialEq> ExpandChangeBuilder<'a, T> {
  pub const fn new(objects1: &'a [T], objects2: &'a [T]) -> Self {
    Self {
      builder: ChangeBuilder::new(objects1.len(), objects2.len()),
      objects1,
      objects2,
    }
  }

  pub const fn index1(&self) -> usize {
    self.builder.index1
  }

  pub const fn index2(&self) -> usize {
    self.builder.index2
  }

  pub fn mark_equal(&mut self, index1: usize, index2: usize) {
    self.builder.mark_equal(index1, index2);
  }

  pub fn finish(mut self) -> DiffIterable {
    self.builder.do_finish();
    let changes = self
      .builder
      .changes
      .iter()
      .map(|range| expand(self.objects1, self.objects2, range.start1, range.start2, range.end1, range.end2))
      .filter(|range| !range.is_empty())
      .collect();
    create(changes, self.builder.length1, self.builder.length2)
  }
}
