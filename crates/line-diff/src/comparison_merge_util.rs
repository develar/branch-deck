// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/comparison/ComparisonMergeUtil.kt` of
// IntelliJ Platform (Apache License 2.0, Copyright 2000-2024 JetBrains s.r.o. and contributors).

//! The three-way merge of two two-way comparisons that share their first side, the base.

use crate::cancellation::Deadline;
use crate::iterables::diff_iterable::DiffIterable;
use crate::lcs::diff::USE_GREEDY_MERGE_MAGIC_RESOLVE;
use crate::merge_range::MergeRange;
use crate::merge_resolve_util::{try_greedy_resolve, try_resolve};
use crate::range::Range;
use crate::side::Side;

/// The changes of a three-way merge from the comparisons base-left (`fragments1`) and base-right (`fragments2`): a
/// change wherever a side differs from the base
pub fn build_simple(fragments1: &DiffIterable, fragments2: &DiffIterable) -> Vec<MergeRange> {
  assert_eq!(fragments1.length1(), fragments2.length1());
  FairMergeBuilder::new(None).execute(fragments1, fragments2)
}

/// As `build_simple`, but a run of matched lines that `true_equality` (left, base, right index) does not hold equal
/// is a change of its own, outside the changes around it: a change the policy ignores
pub(crate) fn build_merge(fragments1: &DiffIterable, fragments2: &DiffIterable, true_equality: &dyn Fn(usize, usize, usize) -> bool) -> Vec<MergeRange> {
  assert_eq!(fragments1.length1(), fragments2.length1());
  FairMergeBuilder::new(Some(true_equality)).execute(fragments1, fragments2)
}

/// The merged text of a conflict when the changes of its sides do not overlap at the word level, else `None`
pub fn try_resolve_conflict(left_text: &[u16], base_text: &[u16], right_text: &[u16]) -> Option<Vec<u16>> {
  if USE_GREEDY_MERGE_MAGIC_RESOLVE {
    try_greedy_resolve(left_text, base_text, right_text, &Deadline::NONE)
  } else {
    try_resolve(left_text, base_text, right_text, &Deadline::NONE)
  }
}

struct FairMergeBuilder<'a> {
  changes: ChangeBuilder<'a>,
}

impl<'a> FairMergeBuilder<'a> {
  fn new(true_equality: Option<&'a dyn Fn(usize, usize, usize) -> bool>) -> Self {
    Self {
      changes: ChangeBuilder {
        changes: Vec::new(),
        index1: 0,
        index2: 0,
        index3: 0,
        true_equality,
      },
    }
  }

  fn execute(mut self, fragments1: &DiffIterable, fragments2: &DiffIterable) -> Vec<MergeRange> {
    let mut unchanged1 = fragments1.unchanged().peekable();
    let mut unchanged2 = fragments2.unchanged().peekable();

    while let (Some(&range1), Some(&range2)) = (unchanged1.peek(), unchanged2.peek()) {
      match self.add(range1, range2) {
        Side::Left => unchanged1.next(),
        Side::Right => unchanged2.next(),
      };
    }

    self.changes.finish(fragments1.length2(), fragments1.length1(), fragments2.length2())
  }

  fn add(&mut self, range1: Range, range2: Range) -> Side {
    let (start1, end1) = (range1.start1, range1.end1);
    let (start2, end2) = (range2.start1, range2.end1);

    if end1 <= start2 {
      return Side::Left;
    }
    if end2 <= start1 {
      return Side::Right;
    }

    let start_base = start1.max(start2);
    let end_base = end1.min(end2);
    let count = end_base - start_base;

    let start_shift1 = start_base - start1;
    let start_shift2 = start_base - start2;

    let start_left = range1.start2 + start_shift1;
    let end_left = start_left + count;
    let start_right = range2.start2 + start_shift2;
    let end_right = start_right + count;

    self.changes.mark_equal(start_left, start_base, start_right, end_left, end_base, end_right);

    Side::from_left(end1 <= end2)
  }
}

/// The changes between the equal ranges marked in order (`ChangeBuilder`); with a `true_equality`, the runs of the
/// equal ranges that it does not hold equal become changes too (`IgnoringChangeBuilder`)
struct ChangeBuilder<'a> {
  changes: Vec<MergeRange>,
  index1: usize,
  index2: usize,
  index3: usize,
  true_equality: Option<&'a dyn Fn(usize, usize, usize) -> bool>,
}

impl ChangeBuilder<'_> {
  fn add_change(&mut self, start1: usize, start2: usize, start3: usize, end1: usize, end2: usize, end3: usize) {
    if start1 == end1 && start2 == end2 && start3 == end3 {
      return;
    }
    self.changes.push(MergeRange::new(start1, end1, start2, end2, start3, end3));
  }

  fn mark_equal(&mut self, start1: usize, start2: usize, start3: usize, end1: usize, end2: usize, end3: usize) {
    assert!(self.index1 <= start1 && self.index2 <= start2 && self.index3 <= start3);
    assert!(start1 <= end1 && start2 <= end2 && start3 <= end3);

    self.process_change(self.index1, self.index2, self.index3, start1, start2, start3);

    self.index1 = end1;
    self.index2 = end2;
    self.index3 = end3;
  }

  fn finish(mut self, length1: usize, length2: usize, length3: usize) -> Vec<MergeRange> {
    assert!(self.index1 <= length1 && self.index2 <= length2 && self.index3 <= length3);

    self.process_change(self.index1, self.index2, self.index3, length1, length2, length3);

    self.changes
  }

  fn process_change(&mut self, start1: usize, start2: usize, start3: usize, end1: usize, end2: usize, end3: usize) {
    if let Some(true_equality) = self.true_equality {
      let last_change = self.changes.last();
      let unchanged_start1 = last_change.map_or(0, |change| change.end1);
      let unchanged_start2 = last_change.map_or(0, |change| change.end2);
      let unchanged_start3 = last_change.map_or(0, |change| change.end3);
      self.add_ignored_changes(true_equality, unchanged_start1, unchanged_start2, unchanged_start3, start1, start2, start3);
    }
    self.add_change(start1, start2, start3, end1, end2, end3);
  }

  #[expect(clippy::too_many_arguments, reason = "the six bounds of the source")]
  fn add_ignored_changes(&mut self, true_equality: &dyn Fn(usize, usize, usize) -> bool, start1: usize, start2: usize, start3: usize, end1: usize, end2: usize, end3: usize) {
    let count = end2 - start2;
    assert_eq!(end1 - start1, count);
    assert_eq!(end3 - start3, count);

    let mut first_ignored: Option<usize> = None;
    for i in 0..count {
      let is_ignored = !true_equality(start1 + i, start2 + i, start3 + i);
      match first_ignored {
        None if is_ignored => first_ignored = Some(i),
        Some(first) if !is_ignored => {
          self.add_change(start1 + first, start2 + first, start3 + first, start1 + i, start2 + i, start3 + i);
          first_ignored = None;
        }
        _ => {}
      }
    }

    if let Some(first) = first_ignored {
      self.add_change(start1 + first, start2 + first, start3 + first, start1 + count, start2 + count, start3 + count);
    }
  }
}
