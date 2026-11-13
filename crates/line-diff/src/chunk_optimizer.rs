// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/comparison/ChunkOptimizer.kt` of
// IntelliJ Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors). The abstract class is
// a function that takes `getShift` as a closure.

//! Moves a change over equal elements to a better place, when the LCS left a choice: before an empty line, between
//! two words.

use crate::by_line::Line;
use crate::by_word::InlineChunk;
use crate::comparison_util::unimportant_line_char_count;
use crate::iterables::diff_iterable::DiffIterable;
use crate::iterables::diff_iterable_util::{create_unchanged, fair};
use crate::range::Range;
use crate::side::Side;
use crate::trim_util::{expand_backward, expand_forward, is_space_enter_or_tab};

/// The shift of the change between two unchanged ranges: 0 keeps it, a positive value moves it forward, a negative
/// one backward
pub(crate) type GetShift<'a> = dyn Fn(Side, usize, usize, Range, Range) -> isize + 'a;

/// Squashes and moves the changes of `iterable`, choosing each move by `get_shift`
pub(crate) fn optimize<T: PartialEq>(data1: &[T], data2: &[T], iterable: &DiffIterable, get_shift: &GetShift<'_>) -> DiffIterable {
  let mut ranges: Vec<Range> = Vec::new();
  for range in iterable.unchanged() {
    ranges.push(range);
    process_last_ranges(data1, data2, &mut ranges, get_shift);
  }

  fair(create_unchanged(ranges, data1.len(), data2.len()))
}

fn process_last_ranges<T: PartialEq>(data1: &[T], data2: &[T], ranges: &mut Vec<Range>, get_shift: &GetShift<'_>) {
  if ranges.len() < 2 {
    // nothing to do
    return;
  }

  let range1 = ranges[ranges.len() - 2];
  let range2 = ranges[ranges.len() - 1];
  if range1.end1 != range2.start1 && range1.end2 != range2.start2 {
    // if changes do not touch and we still can perform one of these optimisations,
    // it means that given DiffIterable is not LCS (because we can build a smaller one). This should not happen.
    return;
  }

  let count1 = range1.end1 - range1.start1;
  let count2 = range2.end1 - range2.start1;

  let equal_forward = expand_forward(data1, data2, range1.end1, range1.end2, range1.end1 + count2, range1.end2 + count2);
  let equal_backward = expand_backward(data1, data2, range2.start1 - count1, range2.start2 - count1, range2.start1, range2.start2);

  // nothing to do
  if equal_forward == 0 && equal_backward == 0 {
    return;
  }

  // merge chunks left [A]B[B] -> [AB]B
  if equal_forward == count2 {
    ranges.truncate(ranges.len() - 2);
    ranges.push(Range::new(range1.start1, range1.end1 + count2, range1.start2, range1.end2 + count2));
    process_last_ranges(data1, data2, ranges, get_shift);
    return;
  }

  // merge chunks right [A]A[B] -> A[AB]
  if equal_backward == count1 {
    ranges.truncate(ranges.len() - 2);
    ranges.push(Range::new(range2.start1 - count1, range2.end1, range2.start2 - count1, range2.end2));
    process_last_ranges(data1, data2, ranges, get_shift);
    return;
  }

  let touch_side = Side::from_left(range1.end1 == range2.start1);

  let shift = get_shift(touch_side, equal_forward, equal_backward, range1, range2);
  if shift != 0 {
    ranges.truncate(ranges.len() - 2);
    ranges.push(Range::new(range1.start1, shifted(range1.end1, shift), range1.start2, shifted(range1.end2, shift)));
    ranges.push(Range::new(shifted(range2.start1, shift), range2.end1, shifted(range2.start2, shift), range2.end2));
  }
}

const fn shifted(value: usize, shift: isize) -> usize {
  value.checked_add_signed(shift).expect("a shift stays inside the text")
}

//
// Implementations
//

/// Moves line changes:
///
/// ```text
/// 1. Minimise amount of chunks
///      good: "AX[AB]" - "[AB]"
///      bad: "[A]XA[B]" - "[A][B]"
///
/// 2. Prefer insertions/deletions, that are bounded by empty(or 'unimportant') line
///      good: "ABooYZ [ABuuYZ ]ABzzYZ" - "ABooYZ []ABzzYZ"
///      bad: "ABooYZ AB[uuYZ AB]zzYZ" - "ABooYZ AB[]zzYZ"
/// ```
pub(crate) fn optimize_line_chunks(lines1: &[Line<'_>], lines2: &[Line<'_>], changes: &DiffIterable) -> DiffIterable {
  let get_shift = |touch_side: Side, equal_forward: usize, equal_backward: usize, range1: Range, range2: Range| -> isize {
    let threshold = unimportant_line_char_count();
    let boundary = LineBoundary {
      lines1,
      lines2,
      touch_side,
      equal_forward,
      equal_backward,
      range1,
      range2,
    };

    boundary
      .unchanged_boundary_shift(0)
      .or_else(|| boundary.changed_boundary_shift(0))
      .or_else(|| boundary.unchanged_boundary_shift(threshold))
      .or_else(|| boundary.changed_boundary_shift(threshold))
      .unwrap_or(0)
  };
  optimize(lines1, lines2, changes, &get_shift)
}

/// The arguments of `LineChunkOptimizer.getShift`
struct LineBoundary<'a, 'b> {
  lines1: &'a [Line<'b>],
  lines2: &'a [Line<'b>],
  touch_side: Side,
  equal_forward: usize,
  equal_backward: usize,
  range1: Range,
  range2: Range,
}

impl LineBoundary<'_, '_> {
  /// search for an empty line boundary in unchanged lines
  /// ie: we want insertion/deletion to go right before/after of an empty line
  fn unchanged_boundary_shift(&self, threshold: usize) -> Option<isize> {
    let touch_lines = self.touch_side.select(self.lines1, self.lines2);
    let touch_start = self.touch_side.select(self.range2.start1, self.range2.start2);

    let shift_forward = find_next_unimportant_line(touch_lines, touch_start, self.equal_forward + 1, threshold);
    let shift_backward = find_prev_unimportant_line(touch_lines, touch_start - 1, self.equal_backward + 1, threshold);

    get_shift(shift_forward, shift_backward)
  }

  /// search for an empty line boundary in changed lines
  /// ie: we want insertion/deletion to start/end with an empty line
  fn changed_boundary_shift(&self, threshold: usize) -> Option<isize> {
    let non_touch_side = self.touch_side.other();
    let non_touch_lines = non_touch_side.select(self.lines1, self.lines2);
    let change_start = non_touch_side.select(self.range1.end1, self.range1.end2);
    let change_end = non_touch_side.select(self.range2.start1, self.range2.start2);

    let shift_forward = find_next_unimportant_line(non_touch_lines, change_start, self.equal_forward + 1, threshold);
    let shift_backward = find_prev_unimportant_line(non_touch_lines, change_end - 1, self.equal_backward + 1, threshold);

    get_shift(shift_forward, shift_backward)
  }
}

fn find_next_unimportant_line(lines: &[Line<'_>], offset: usize, count: usize, threshold: usize) -> Option<usize> {
  (0..count).find(|&i| lines[offset + i].non_space_chars() <= threshold)
}

fn find_prev_unimportant_line(lines: &[Line<'_>], offset: usize, count: usize, threshold: usize) -> Option<usize> {
  (0..count).find(|&i| lines[offset - i].non_space_chars() <= threshold)
}

const fn get_shift(shift_forward: Option<usize>, shift_backward: Option<usize>) -> Option<isize> {
  match (shift_forward, shift_backward) {
    (None, None) => None,
    (Some(0), _) | (_, Some(0)) => Some(0),
    (Some(forward), _) => Some(signed(forward)),
    (None, Some(backward)) => Some(-signed(backward)),
  }
}

#[expect(clippy::cast_possible_wrap, reason = "a text is shorter than isize::MAX")]
pub(crate) const fn signed(value: usize) -> isize {
  value as isize
}

/// Moves word changes:
///
/// ```text
/// 1. Minimise amount of chunks
///      good: "AX[AB]" - "[AB]"
///      bad: "[A]XA[B]" - "[A][B]"
///
/// 2. Minimise amount of modified 'sentences', where sentence is a sequence of words, that are not separated by whitespace
///      good: "[AX] [AZ]" - "[AX] AY [AZ]"
///      bad: "[AX A][Z]" - "[AX A]Y A[Z]"
///      ex: "1.0.123 1.0.155" vs "1.0.123 1.0.134 1.0.155"
/// ```
pub(crate) fn optimize_word_chunks(text1: &[u16], text2: &[u16], words1: &[InlineChunk<'_>], words2: &[InlineChunk<'_>], changes: &DiffIterable) -> DiffIterable {
  let get_shift = |touch_side: Side, equal_forward: usize, equal_backward: usize, _range1: Range, range2: Range| -> isize {
    let touch_words = touch_side.select(words1, words2);
    let touch_text = touch_side.select(text1, text2);
    let touch_start = touch_side.select(range2.start1, range2.start2);

    // check if chunks are already separated by whitespaces
    if is_separated_with_whitespace(touch_text, &touch_words[touch_start - 1], &touch_words[touch_start]) {
      return 0;
    }

    // shift chunks left [X]A Y[A ZA] -> [XA] YA [ZA]
    //                   [X][A ZA] -> [XA] [ZA]
    if let Some(left_shift) = find_sequence_edge_shift(touch_text, touch_words, touch_start, equal_forward, true) {
      return signed(left_shift);
    }

    // shift chunks right [AX A]Y A[Z] -> [AX] AY [AZ]
    //                    [AX A][Z] -> [AX] [AZ]
    if let Some(right_shift) = find_sequence_edge_shift(touch_text, touch_words, touch_start - 1, equal_backward, false) {
      return -signed(right_shift);
    }

    // nothing to do
    0
  };
  optimize(words1, words2, changes, &get_shift)
}

fn find_sequence_edge_shift(text: &[u16], words: &[InlineChunk<'_>], offset: usize, count: usize, left_to_right: bool) -> Option<usize> {
  (0..count)
    .find(|&i| {
      let (word1, word2) = if left_to_right {
        (&words[offset + i], &words[offset + i + 1])
      } else {
        (&words[offset - i - 1], &words[offset - i])
      };
      is_separated_with_whitespace(text, word1, word2)
    })
    .map(|i| i + 1)
}

fn is_separated_with_whitespace(text: &[u16], word1: &InlineChunk<'_>, word2: &InlineChunk<'_>) -> bool {
  if matches!(word1, InlineChunk::Newline(_)) || matches!(word2, InlineChunk::Newline(_)) {
    return true;
  }

  text[word1.offset2()..word2.offset1()].iter().any(|&c| is_space_enter_or_tab(c))
}
