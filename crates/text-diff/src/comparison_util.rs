// Ported and modified from IntelliJ Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and
// contributors): `community/platform/util/diff/src/com/intellij/diff/comparison/ComparisonUtil.kt` and the string
// helpers it uses from `community/platform/util/base/multiplatform/src/com/intellij/openapi/util/text/StringsKmp.kt`.

//! Equality and hashes of texts under a comparison policy.

use crate::lcs::diff::UNIMPORTANT_LINE_CHAR_COUNT;
use crate::policy::ComparisonPolicy;
use crate::trim_util::{is_space_enter_or_tab, trim_end, trim_start};

/// Whether two texts are equal; under `TrimWhitespaces` only the whitespace at the ends of the whole texts is ignored
pub fn is_equals(text1: &[u16], text2: &[u16], policy: ComparisonPolicy) -> bool {
  match policy {
    ComparisonPolicy::Default => text1 == text2,
    ComparisonPolicy::TrimWhitespaces => equals_trim_whitespace(text1, text2),
    ComparisonPolicy::IgnoreWhitespaces => equals_ignore_whitespaces(text1, text2),
  }
}

/// A hash that is equal for texts that `is_equals` finds equal
pub fn hash_code(text: &[u16], policy: ComparisonPolicy) -> i32 {
  match policy {
    ComparisonPolicy::Default => string_hash_code(text),
    ComparisonPolicy::TrimWhitespaces => {
      let offset1 = trim_start(text, 0, text.len());
      let offset2 = trim_end(text, offset1, text.len());
      string_hash_code(&text[offset1..offset2])
    }
    ComparisonPolicy::IgnoreWhitespaces => string_hash_code_ignore_whitespaces(text),
  }
}

/// Whether two texts are equal; under `TrimWhitespaces` the whitespace at the ends of every line is ignored
pub fn is_equal_texts(text1: &[u16], text2: &[u16], policy: ComparisonPolicy) -> bool {
  match policy {
    ComparisonPolicy::Default => text1 == text2,
    ComparisonPolicy::TrimWhitespaces => equals_trim_whitespaces(text1, text2),
    ComparisonPolicy::IgnoreWhitespaces => equals_ignore_whitespaces(text1, text2),
  }
}

/// Unlike `Strings.equalsTrimWhitespaces`, the leading and trailing whitespace of inner lines is ignored as well.
///
/// Ex: "\nXY\n" and "\n XY \n" are equal, "\nXY\n" and "\nX Y\n" are different.
pub fn equals_trim_whitespaces(s1: &[u16], s2: &[u16]) -> bool {
  let mut index1 = 0;
  let mut index2 = 0;

  loop {
    let (end1, last_line1) = line_end(s1, index1);
    let (end2, last_line2) = line_end(s2, index2);
    if last_line1 != last_line2 {
      return false;
    }

    if !equals_trim_whitespace(&s1[index1..end1], &s2[index2..end2]) {
      return false;
    }

    index1 = end1;
    index2 = end2;
    if last_line1 {
      return true;
    }
  }
}

/// The end of the line at `start` after its `\n`, and whether it is the last line
fn line_end(text: &[u16], start: usize) -> (usize, bool) {
  match text[start..].iter().position(|&c| c == u16::from(b'\n')) {
    Some(index) => (start + index + 1, false),
    None => (text.len(), true),
  }
}

/// The number of non-space chars at or below which a line is unimportant
pub const fn unimportant_line_char_count() -> usize {
  UNIMPORTANT_LINE_CHAR_COUNT
}

//
// StringsKmp
//

/// `String.hashCode` of UTF-16 code units
pub fn string_hash_code(text: &[u16]) -> i32 {
  text.iter().fold(0_i32, |h, &c| h.wrapping_mul(31).wrapping_add(i32::from(c)))
}

fn string_hash_code_ignore_whitespaces(text: &[u16]) -> i32 {
  text
    .iter()
    .filter(|&&c| !is_space_enter_or_tab(c))
    .fold(0_i32, |h, &c| h.wrapping_mul(31).wrapping_add(i32::from(c)))
}

/// Whether two texts are equal besides their whitespace
pub fn equals_ignore_whitespaces(text1: &[u16], text2: &[u16]) -> bool {
  let len1 = text1.len();
  let len2 = text2.len();

  let mut index1 = 0;
  let mut index2 = 0;
  while index1 < len1 && index2 < len2 {
    if text1[index1] == text2[index2] {
      index1 += 1;
      index2 += 1;
      continue;
    }

    let mut skipped = false;
    while index1 != len1 && is_space_enter_or_tab(text1[index1]) {
      skipped = true;
      index1 += 1;
    }
    while index2 != len2 && is_space_enter_or_tab(text2[index2]) {
      skipped = true;
      index2 += 1;
    }

    if !skipped {
      return false;
    }
  }

  text1[index1..].iter().all(|&c| is_space_enter_or_tab(c)) && text2[index2..].iter().all(|&c| is_space_enter_or_tab(c))
}

fn equals_trim_whitespace(text1: &[u16], text2: &[u16]) -> bool {
  trim_whitespace(text1) == trim_whitespace(text2)
}

fn trim_whitespace(text: &[u16]) -> &[u16] {
  let start = trim_start(text, 0, text.len());
  let end = trim_end(text, start, text.len());
  &text[start..end]
}
