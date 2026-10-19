// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/comparison/TrimUtil.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors). Left out: the three-side forms
// of the merge. A slice stands for both a `CharSequence` and a `List`, so one generic function replaces each pair of
// overloads; a form that takes a `Range` replaces the form that takes its four values.

//! Trims ignored elements off a range, and expands a range over equal elements at its ends.

use crate::character_utils::{is_alphabetic, is_bmp_code_point, is_decimal_digit, is_hiragana_script, is_ideographic, is_javanese_script, is_katakana_script, is_thai_script};
use crate::comparison_util::is_equal_texts;
use crate::policy::ComparisonPolicy;
use crate::range::Range;

/// The ASCII punctuation, 33-47, 58-64, 91-96 and 123-126, without `_`
pub const fn is_punctuation(c: u32) -> bool {
  if c == 95 {
    // exclude '_'
    return false;
  }
  matches!(c, 33..=47 | 58..=64 | 91..=96 | 123..=126)
}

/// Neither whitespace nor punctuation
pub const fn is_alpha(c: u32) -> bool {
  if is_white_space_code_point(c) {
    return false;
  }
  !is_punctuation(c)
}

pub const fn is_white_space_code_point(c: u32) -> bool {
  c == b' ' as u32 || c == b'\t' as u32 || c == b'\n' as u32
}

/// A code point that is a word of its own: a char of a script without spaces between words (CJK, Thai, Javanese)
/// or a char that is not a letter (an emoji, a symbol)
pub fn is_continuous_script(c: u32) -> bool {
  if c < 128 {
    return false;
  }
  if is_decimal_digit(c) {
    return false;
  }

  if !is_bmp_code_point(c) {
    return true;
  }

  if is_ideographic(c) {
    return true;
  }
  if !is_alphabetic(c) {
    return true;
  }

  is_hiragana_script(c) || is_katakana_script(c) || is_thai_script(c) || is_javanese_script(c)
}

/// `\n`, `\t` and space, the whitespace of the comparison (`Strings.isWhiteSpace`)
pub const fn is_space_enter_or_tab(c: u16) -> bool {
  c == b'\n' as u16 || c == b'\t' as u16 || c == b' ' as u16
}

/// `[start, end)` of `text` without the whitespace at its ends
pub fn trim(text: &[u16], start: usize, end: usize) -> (usize, usize) {
  trim_with(start, end, |index| is_space_enter_or_tab(text[index]))
}

pub fn trim_start(text: &[u16], start: usize, end: usize) -> usize {
  trim_start_with(start, end, |index| is_space_enter_or_tab(text[index]))
}

pub fn trim_end(text: &[u16], start: usize, end: usize) -> usize {
  trim_end_with(start, end, |index| is_space_enter_or_tab(text[index]))
}

/// Both sides of `range` without the whitespace at their ends
pub fn trim_range(text1: &[u16], text2: &[u16], range: Range) -> Range {
  trim_both(
    range.start1,
    range.start2,
    range.end1,
    range.end2,
    |index| is_space_enter_or_tab(text1[index]),
    |index| is_space_enter_or_tab(text2[index]),
  )
}

/// The range without the equal elements at its start and at its end
pub fn expand<T: PartialEq>(text1: &[T], text2: &[T], start1: usize, start2: usize, end1: usize, end2: usize) -> Range {
  expand_with(start1, start2, end1, end2, |index1, index2| text1[index1] == text2[index2])
}

/// The number of equal elements at the start of the range
pub fn expand_forward<T: PartialEq>(text1: &[T], text2: &[T], start1: usize, start2: usize, end1: usize, end2: usize) -> usize {
  expand_forward_with(start1, start2, end1, end2, |index1, index2| text1[index1] == text2[index2])
}

/// The number of equal elements at the end of the range
pub fn expand_backward<T: PartialEq>(text1: &[T], text2: &[T], start1: usize, start2: usize, end1: usize, end2: usize) -> usize {
  expand_backward_with(start1, start2, end1, end2, |index1, index2| text1[index1] == text2[index2])
}

pub fn expand_range<T: PartialEq>(text1: &[T], text2: &[T], range: Range) -> Range {
  expand(text1, text2, range.start1, range.start2, range.end1, range.end2)
}

/// The range without the equal whitespace at its start and at its end
pub fn expand_whitespaces(text1: &[u16], text2: &[u16], range: Range) -> Range {
  expand_ignored(
    range.start1,
    range.start2,
    range.end1,
    range.end2,
    |index1, index2| text1[index1] == text2[index2],
    |index| is_space_enter_or_tab(text1[index]),
  )
}

pub fn expand_whitespaces_forward(text1: &[u16], text2: &[u16], start1: usize, start2: usize, end1: usize, end2: usize) -> usize {
  expand_ignored_forward(
    start1,
    start2,
    end1,
    end2,
    |index1, index2| text1[index1] == text2[index2],
    |index| is_space_enter_or_tab(text1[index]),
  )
}

pub fn expand_whitespaces_backward(text1: &[u16], text2: &[u16], start1: usize, start2: usize, end1: usize, end2: usize) -> usize {
  expand_ignored_backward(
    start1,
    start2,
    end1,
    end2,
    |index1, index2| text1[index1] == text2[index2],
    |index| is_space_enter_or_tab(text1[index]),
  )
}

/// Whether both sides of `range` hold the same chars
pub fn is_equals(text1: &[u16], text2: &[u16], range: Range) -> bool {
  is_equal_texts(&text1[range.start1..range.end1], &text2[range.start2..range.end2], ComparisonPolicy::Default)
}

/// Whether both sides of `range` hold the same chars besides whitespace
pub fn is_equals_ignore_whitespaces(text1: &[u16], text2: &[u16], range: Range) -> bool {
  is_equal_texts(&text1[range.start1..range.end1], &text2[range.start2..range.end2], ComparisonPolicy::IgnoreWhitespaces)
}

//
// Trim
//

fn trim_both(start1: usize, start2: usize, end1: usize, end2: usize, ignored1: impl Fn(usize) -> bool, ignored2: impl Fn(usize) -> bool) -> Range {
  let start1 = trim_start_with(start1, end1, &ignored1);
  let end1 = trim_end_with(start1, end1, &ignored1);
  let start2 = trim_start_with(start2, end2, &ignored2);
  let end2 = trim_end_with(start2, end2, &ignored2);

  Range::new(start1, end1, start2, end2)
}

/// `[start, end)` without the ignored elements at its ends
pub fn trim_with(start: usize, end: usize, ignored: impl Fn(usize) -> bool) -> (usize, usize) {
  let start = trim_start_with(start, end, &ignored);
  let end = trim_end_with(start, end, &ignored);
  (start, end)
}

fn trim_start_with(mut start: usize, end: usize, ignored: impl Fn(usize) -> bool) -> usize {
  while start < end {
    if !ignored(start) {
      break;
    }
    start += 1;
  }
  start
}

fn trim_end_with(start: usize, mut end: usize, ignored: impl Fn(usize) -> bool) -> usize {
  while start < end {
    if !ignored(end - 1) {
      break;
    }
    end -= 1;
  }
  end
}

//
// Expand
//

/// The range without the elements at its ends that `equals` matches
pub fn expand_with(mut start1: usize, mut start2: usize, mut end1: usize, mut end2: usize, equals: impl Fn(usize, usize) -> bool) -> Range {
  let count1 = expand_forward_with(start1, start2, end1, end2, &equals);
  start1 += count1;
  start2 += count1;

  let count2 = expand_backward_with(start1, start2, end1, end2, &equals);
  end1 -= count2;
  end2 -= count2;

  Range::new(start1, end1, start2, end2)
}

fn expand_forward_with(mut start1: usize, mut start2: usize, end1: usize, end2: usize, equals: impl Fn(usize, usize) -> bool) -> usize {
  let old_start1 = start1;
  while start1 < end1 && start2 < end2 {
    if !equals(start1, start2) {
      break;
    }
    start1 += 1;
    start2 += 1;
  }

  start1 - old_start1
}

fn expand_backward_with(start1: usize, start2: usize, mut end1: usize, mut end2: usize, equals: impl Fn(usize, usize) -> bool) -> usize {
  let old_end1 = end1;
  while start1 < end1 && start2 < end2 {
    if !equals(end1 - 1, end2 - 1) {
      break;
    }
    end1 -= 1;
    end2 -= 1;
  }

  old_end1 - end1
}

//
// Expand Ignored
//

fn expand_ignored(mut start1: usize, mut start2: usize, mut end1: usize, mut end2: usize, equals: impl Fn(usize, usize) -> bool, ignored1: impl Fn(usize) -> bool) -> Range {
  let count1 = expand_ignored_forward(start1, start2, end1, end2, &equals, &ignored1);
  start1 += count1;
  start2 += count1;

  let count2 = expand_ignored_backward(start1, start2, end1, end2, &equals, &ignored1);
  end1 -= count2;
  end2 -= count2;

  Range::new(start1, end1, start2, end2)
}

fn expand_ignored_forward(mut start1: usize, mut start2: usize, end1: usize, end2: usize, equals: impl Fn(usize, usize) -> bool, ignored1: impl Fn(usize) -> bool) -> usize {
  let old_start1 = start1;
  while start1 < end1 && start2 < end2 {
    if !equals(start1, start2) {
      break;
    }
    if !ignored1(start1) {
      break;
    }
    start1 += 1;
    start2 += 1;
  }

  start1 - old_start1
}

fn expand_ignored_backward(start1: usize, start2: usize, mut end1: usize, mut end2: usize, equals: impl Fn(usize, usize) -> bool, ignored1: impl Fn(usize) -> bool) -> usize {
  let old_end1 = end1;
  while start1 < end1 && start2 < end2 {
    if !equals(end1 - 1, end2 - 1) {
      break;
    }
    if !ignored1(end1 - 1) {
      break;
    }
    end1 -= 1;
    end2 -= 1;
  }

  old_end1 - end1
}

//
// Trim Expand
//

// jscpd:ignore-start: the forward and the backward walk mirror each other, as in TrimUtil.kt

/// The range without the equal elements at its ends, skipping the ignored ones of each side, then trimmed of them
pub fn trim_expand(
  mut start1: usize,
  mut start2: usize,
  mut end1: usize,
  mut end2: usize,
  equals: impl Fn(usize, usize) -> bool,
  ignored1: impl Fn(usize) -> bool,
  ignored2: impl Fn(usize) -> bool,
) -> Range {
  (start1, start2) = trim_expand_forward(start1, start2, end1, end2, &equals, &ignored1, &ignored2);
  (end1, end2) = trim_expand_backward(start1, start2, end1, end2, &equals, &ignored1, &ignored2);
  Range::new(start1, end1, start2, end2)
}

fn trim_expand_forward(
  mut start1: usize,
  mut start2: usize,
  end1: usize,
  end2: usize,
  equals: impl Fn(usize, usize) -> bool,
  ignored1: impl Fn(usize) -> bool,
  ignored2: impl Fn(usize) -> bool,
) -> (usize, usize) {
  while start1 < end1 && start2 < end2 {
    if equals(start1, start2) {
      start1 += 1;
      start2 += 1;
      continue;
    }

    let mut skipped = false;
    if ignored1(start1) {
      skipped = true;
      start1 += 1;
    }
    if ignored2(start2) {
      skipped = true;
      start2 += 1;
    }
    if !skipped {
      break;
    }
  }

  (trim_start_with(start1, end1, &ignored1), trim_start_with(start2, end2, &ignored2))
}

fn trim_expand_backward(
  start1: usize,
  start2: usize,
  mut end1: usize,
  mut end2: usize,
  equals: impl Fn(usize, usize) -> bool,
  ignored1: impl Fn(usize) -> bool,
  ignored2: impl Fn(usize) -> bool,
) -> (usize, usize) {
  while start1 < end1 && start2 < end2 {
    if equals(end1 - 1, end2 - 1) {
      end1 -= 1;
      end2 -= 1;
      continue;
    }

    let mut skipped = false;
    if ignored1(end1 - 1) {
      skipped = true;
      end1 -= 1;
    }
    if ignored2(end2 - 1) {
      skipped = true;
      end2 -= 1;
    }
    if !skipped {
      break;
    }
  }

  (trim_end_with(start1, end1, &ignored1), trim_end_with(start2, end2, &ignored2))
}
// jscpd:ignore-end
