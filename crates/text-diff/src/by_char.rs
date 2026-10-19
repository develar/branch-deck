// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/comparison/ByCharRt.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors).

//! The comparison of two texts by code point.

use crate::by_word::trim_spaces_correct;
use crate::cancellation::{Deadline, DiffTooBig};
use crate::change_corrector::{ChangeCorrector, DefaultCharChangeCorrector};
use crate::character_utils::{char_count, code_point_at};
use crate::iterables::diff_iterable::DiffIterable;
use crate::iterables::diff_iterable_util::{ChangeBuilder, create, diff_ints, fair, iterate_all};
use crate::range::Range;
use crate::trim_util::{expand_whitespaces_forward, is_punctuation, is_white_space_code_point};

/// Compares all code points
pub fn compare(text1: &[u16], text2: &[u16], deadline: &Deadline) -> Result<DiffIterable, DiffTooBig> {
  deadline.check_canceled()?;

  let code_points1 = get_all_code_points(text1);
  let code_points2 = get_all_code_points(text2);

  let iterable = diff_ints(&code_points1, &code_points2, deadline)?;

  let mut offset1 = 0;
  let mut offset2 = 0;
  let mut builder = ChangeBuilder::new(text1.len(), text2.len());
  for (range, equals) in iterate_all(&iterable) {
    let end1 = offset1 + count_chars(&code_points1, range.start1, range.end1);
    let end2 = offset2 + count_chars(&code_points2, range.start2, range.end2);

    if equals {
      builder.mark_equal_range(offset1, offset2, end1, end2);
    }

    offset1 = end1;
    offset2 = end2;
  }
  debug_assert_eq!(offset1, text1.len());
  debug_assert_eq!(offset2, text2.len());

  Ok(fair(builder.finish()))
}

/// Compares the non-space code points first, then the gaps between the matched ones
pub fn compare_two_step(text1: &[u16], text2: &[u16], deadline: &Deadline) -> Result<DiffIterable, DiffTooBig> {
  let (code_points1, code_points2, non_space_changes) = compare_non_space_code_points(text1, text2, deadline)?;
  match_adjustment_spaces(&code_points1, &code_points2, text1, text2, &non_space_changes, deadline)
}

pub fn compare_trim_whitespaces(text1: &[u16], text2: &[u16], deadline: &Deadline) -> Result<DiffIterable, DiffTooBig> {
  let iterable = compare_two_step(text1, text2, deadline)?;
  Ok(trim_spaces_correct(&iterable, text1, text2))
}

pub fn compare_ignore_whitespaces(text1: &[u16], text2: &[u16], deadline: &Deadline) -> Result<DiffIterable, DiffTooBig> {
  let (code_points1, code_points2, changes) = compare_non_space_code_points(text1, text2, deadline)?;
  Ok(match_adjustment_spaces_iw(&code_points1, &code_points2, text1, text2, &changes))
}

/// The first step of `compareTwoStep` and `compareIgnoreWhitespaces`: the non-space code points and their changes
fn compare_non_space_code_points(text1: &[u16], text2: &[u16], deadline: &Deadline) -> Result<(CodePointsOffsets, CodePointsOffsets, DiffIterable), DiffTooBig> {
  deadline.check_canceled()?;

  let code_points1 = get_non_space_code_points(text1);
  let code_points2 = get_non_space_code_points(text2);

  let changes = diff_ints(&code_points1.code_points, &code_points2.code_points, deadline)?;
  Ok((code_points1, code_points2, changes))
}

/// Compares the punctuation chars only, all other chars are left unmatched
pub fn compare_punctuation(text1: &[u16], text2: &[u16], deadline: &Deadline) -> Result<DiffIterable, DiffTooBig> {
  deadline.check_canceled()?;

  let chars1 = get_punctuation_chars(text1);
  let chars2 = get_punctuation_chars(text2);

  let non_space_changes = diff_ints(&chars1.code_points, &chars2.code_points, deadline)?;
  Ok(transfer_punctuation(&chars1, &chars2, text1, text2, &non_space_changes))
}

//
// Impl
//

fn transfer_punctuation(chars1: &CodePointsOffsets, chars2: &CodePointsOffsets, text1: &[u16], text2: &[u16], changes: &DiffIterable) -> DiffIterable {
  let mut builder = ChangeBuilder::new(text1.len(), text2.len());

  for range in changes.unchanged() {
    let count = range.end1 - range.start1;
    for i in 0..count {
      // Punctuation code points are always 1 char
      let offset1 = chars1.offsets[range.start1 + i];
      let offset2 = chars2.offsets[range.start2 + i];
      builder.mark_equal(offset1, offset2);
    }
  }

  fair(builder.finish())
}

/// Given a `DiffIterable` on non-space characters, converts it into a `DiffIterable` on the original texts.
///
/// Idea: run a fair diff on all gaps between matched characters (inside these pairs could met non-space characters,
/// but they will be unique and can't be matched)
fn match_adjustment_spaces(
  code_points1: &CodePointsOffsets,
  code_points2: &CodePointsOffsets,
  text1: &[u16],
  text2: &[u16],
  changes: &DiffIterable,
  deadline: &Deadline,
) -> Result<DiffIterable, DiffTooBig> {
  DefaultCharChangeCorrector {
    code_points1,
    code_points2,
    text1,
    text2,
    deadline,
  }
  .build(changes)
}

/// Given a `DiffIterable` on non-whitespace characters, converts it into a `DiffIterable` on the original texts.
///
/// matched characters: matched non-space characters + all adjustment whitespaces
fn match_adjustment_spaces_iw(code_points1: &CodePointsOffsets, code_points2: &CodePointsOffsets, text1: &[u16], text2: &[u16], changes: &DiffIterable) -> DiffIterable {
  let mut ranges = Vec::new();

  for ch in changes.changes() {
    let (start_offset1, end_offset1) = if ch.start1 == ch.end1 {
      let end_offset1 = expand_forward_w(code_points1, code_points2, text1, text2, ch, true);
      (end_offset1, end_offset1)
    } else {
      (code_points1.char_offset(ch.start1), code_points1.char_offset_after(ch.end1 - 1))
    };

    let (start_offset2, end_offset2) = if ch.start2 == ch.end2 {
      let end_offset2 = expand_forward_w(code_points1, code_points2, text1, text2, ch, false);
      (end_offset2, end_offset2)
    } else {
      (code_points2.char_offset(ch.start2), code_points2.char_offset_after(ch.end2 - 1))
    };

    ranges.push(Range::new(start_offset1, end_offset1, start_offset2, end_offset2));
  }
  create(ranges, text1.len(), text2.len())
}

/// We need it to correct place of insertion/deletion: we want to match whitespaces, if we can to.
///
/// sample: "x y" -> "x zy", space should be matched instead of being ignored.
fn expand_forward_w(code_points1: &CodePointsOffsets, code_points2: &CodePointsOffsets, text1: &[u16], text2: &[u16], ch: &Range, left: bool) -> usize {
  let offset1 = if ch.start1 == 0 { 0 } else { code_points1.char_offset_after(ch.start1 - 1) };
  let offset2 = if ch.start2 == 0 { 0 } else { code_points2.char_offset_after(ch.start2 - 1) };

  let start = if left { offset1 } else { offset2 };

  start + expand_whitespaces_forward(text1, text2, offset1, offset2, text1.len(), text2.len())
}

//
// Misc
//

fn get_all_code_points(text: &[u16]) -> Vec<u32> {
  let mut list = Vec::with_capacity(text.len());

  let mut offset = 0;
  while offset < text.len() {
    let ch = code_point_at(text, offset);
    list.push(ch);
    offset += char_count(ch);
  }

  list
}

fn get_non_space_code_points(text: &[u16]) -> CodePointsOffsets {
  let mut code_points = Vec::with_capacity(text.len());
  let mut offsets = Vec::with_capacity(text.len());

  let mut offset = 0;
  while offset < text.len() {
    let ch = code_point_at(text, offset);

    if !is_white_space_code_point(ch) {
      code_points.push(ch);
      offsets.push(offset);
    }

    offset += char_count(ch);
  }

  CodePointsOffsets { code_points, offsets }
}

fn get_punctuation_chars(text: &[u16]) -> CodePointsOffsets {
  let mut code_points = Vec::with_capacity(text.len());
  let mut offsets = Vec::with_capacity(text.len());

  for (i, &c) in text.iter().enumerate() {
    if is_punctuation(u32::from(c)) {
      code_points.push(u32::from(c));
      offsets.push(i);
    }
  }

  CodePointsOffsets { code_points, offsets }
}

fn count_chars(code_points: &[u32], start: usize, end: usize) -> usize {
  code_points[start..end].iter().map(|&code_point| char_count(code_point)).sum()
}

/// Some code points of a text with their offsets
pub(crate) struct CodePointsOffsets {
  pub(crate) code_points: Vec<u32>,
  pub(crate) offsets: Vec<usize>,
}

impl CodePointsOffsets {
  pub(crate) fn char_offset(&self, index: usize) -> usize {
    self.offsets[index]
  }

  pub(crate) fn char_offset_after(&self, index: usize) -> usize {
    self.offsets[index] + char_count(self.code_points[index])
  }
}
