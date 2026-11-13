// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/comparison/ChangeCorrector.kt` of
// IntelliJ Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors). The abstract class is
// a trait with `build` as its provided method.

//! The second step of a two-step diff: given a matching of some subsequences of two sequences, build the matching of
//! the whole sequences.

use crate::by_char;
use crate::by_char::CodePointsOffsets;
use crate::by_line::Line;
use crate::cancellation::{Deadline, DiffTooBig};
use crate::iterables::diff_iterable::DiffIterable;
use crate::iterables::diff_iterable_util::{ChangeBuilder, diff, fair};
use crate::trim_util::expand;

pub(crate) trait ChangeCorrector {
  /// The lengths of the whole sequences
  fn lengths(&self) -> (usize, usize);

  /// Matches the elements of `[start1, end1)` and `[start2, end2)`
  fn match_gap(&self, builder: &mut ChangeBuilder, start1: usize, end1: usize, start2: usize, end2: usize) -> Result<(), DiffTooBig>;

  /// The range in the first whole sequence of an element of its subsequence
  fn original_range1(&self, index: usize) -> (usize, usize);

  /// The range in the second whole sequence of an element of its subsequence
  fn original_range2(&self, index: usize) -> (usize, usize);

  /// The matching of the whole sequences, given `changes`, the matching of the subsequences
  fn build(&self, changes: &DiffIterable) -> Result<DiffIterable, DiffTooBig> {
    let (length1, length2) = self.lengths();
    let mut builder = ChangeBuilder::new(length1, length2);

    let mut last1 = 0;
    let mut last2 = 0;

    for ch in changes.unchanged() {
      let count = ch.end1 - ch.start1;
      for i in 0..count {
        let (start1, end1) = self.original_range1(ch.start1 + i);
        let (start2, end2) = self.original_range2(ch.start2 + i);

        self.match_gap(&mut builder, last1, start1, last2, start2)?;
        builder.mark_equal_range(start1, start2, end1, end2);

        last1 = end1;
        last2 = end2;
      }
    }
    self.match_gap(&mut builder, last1, length1, last2, length2)?;

    Ok(fair(builder.finish()))
  }
}

//
// Implementations
//

/// Matches the lines of the gaps between the big lines that the first step matched
pub(crate) struct SmartLineChangeCorrector<'a, 'b> {
  pub(crate) indexes1: &'a [usize],
  pub(crate) indexes2: &'a [usize],
  pub(crate) lines1: &'a [Line<'b>],
  pub(crate) lines2: &'a [Line<'b>],
  pub(crate) deadline: &'a Deadline,
}

impl ChangeCorrector for SmartLineChangeCorrector<'_, '_> {
  fn lengths(&self) -> (usize, usize) {
    (self.lines1.len(), self.lines2.len())
  }

  fn match_gap(&self, builder: &mut ChangeBuilder, start1: usize, end1: usize, start2: usize, end2: usize) -> Result<(), DiffTooBig> {
    let expand = expand(self.lines1, self.lines2, start1, start2, end1, end2);

    let inner1 = &self.lines1[expand.start1..expand.end1];
    let inner2 = &self.lines2[expand.start2..expand.end2];
    let inner_changes = diff(inner1, inner2, self.deadline)?;

    builder.mark_equal_range(start1, start2, expand.start1, expand.start2);

    for chunk in inner_changes.unchanged() {
      builder.mark_equal_count(expand.start1 + chunk.start1, expand.start2 + chunk.start2, chunk.end1 - chunk.start1);
    }

    builder.mark_equal_range(expand.end1, expand.end2, end1, end2);
    Ok(())
  }

  fn original_range1(&self, index: usize) -> (usize, usize) {
    let offset = self.indexes1[index];
    (offset, offset + 1)
  }

  fn original_range2(&self, index: usize) -> (usize, usize) {
    let offset = self.indexes2[index];
    (offset, offset + 1)
  }
}

/// Matches the chars of the gaps between the non-space chars that the first step matched
pub(crate) struct DefaultCharChangeCorrector<'a> {
  pub(crate) code_points1: &'a CodePointsOffsets,
  pub(crate) code_points2: &'a CodePointsOffsets,
  pub(crate) text1: &'a [u16],
  pub(crate) text2: &'a [u16],
  pub(crate) deadline: &'a Deadline,
}

impl ChangeCorrector for DefaultCharChangeCorrector<'_> {
  fn lengths(&self) -> (usize, usize) {
    (self.text1.len(), self.text2.len())
  }

  fn match_gap(&self, builder: &mut ChangeBuilder, start1: usize, end1: usize, start2: usize, end2: usize) -> Result<(), DiffTooBig> {
    let inner1 = &self.text1[start1..end1];
    let inner2 = &self.text2[start2..end2];
    let inner_changes = by_char::compare(inner1, inner2, self.deadline)?;

    for chunk in inner_changes.unchanged() {
      builder.mark_equal_count(start1 + chunk.start1, start2 + chunk.start2, chunk.end1 - chunk.start1);
    }
    Ok(())
  }

  fn original_range1(&self, index: usize) -> (usize, usize) {
    (self.code_points1.char_offset(index), self.code_points1.char_offset_after(index))
  }

  fn original_range2(&self, index: usize) -> (usize, usize) {
    (self.code_points2.char_offset(index), self.code_points2.char_offset_after(index))
  }
}
