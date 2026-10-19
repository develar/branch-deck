// Ported from `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/ComparisonUtilTest.kt` of
// IntelliJ Platform (Apache License 2.0, Copyright 2000-2020 JetBrains s.r.o. and contributors). `!0 - 1` of the
// source is the pair `(0, 1)`.

use crate::cancellation::Deadline;
use crate::manager::{compare_lines, is_equal};
use crate::policy::ComparisonPolicy;
use crate::test_support::parse_source;
use crate::text::Utf16Text;
use pretty_assertions::assert_eq;

#[test]
fn trim_equals() {
  do_test_trim(true, "", "");
  do_test_trim(true, "", "   ");
  do_test_trim(true, "   ", "   ");
  do_test_trim(true, "\n   ", "  \n");
  do_test_trim(true, "asd ", "asd  ");
  do_test_trim(true, "    asd", "asd");
  do_test_trim(true, "\n\n\n", "\n\n\n");
  do_test_trim(true, "\n  \n  \n ", "  \n \n\n  ");
  do_test_trim(false, "\n\n", "\n\n\n");

  do_test_trim(false, "\nasd ", "asd\n  ");
  do_test_trim(true, "\nasd \n", "\n asd\n  ");
  do_test_trim(false, "x", "y");
  do_test_trim(false, "\n", " ");

  do_test_trim(true, "\t ", "");
  do_test_trim(false, "", "\t\n \n\t");
  do_test_trim(false, "\t", "\n");

  do_test_trim(true, "x", " x");
  do_test_trim(true, "x", "x ");
  do_test_trim(false, "x\n", "x");

  do_test_trim(false, "abc", "a\nb\nc\n");
  do_test_trim(true, "\nx y x\n", "\nx y x\n");
  do_test_trim(false, "\nxyx\n", "\nx y x\n");
  do_test_trim(true, "\nx y x", "\nx y x");
  do_test_trim(false, "\nxyx", "\nx y x");
  do_test_trim(true, "x y x", "x y x");
  do_test_trim(false, "xyx", "x y x");
  do_test_trim(true, "  x y x  ", "x y x");

  do_test_trim(false, "x", "\t\n ");
  do_test_trim(false, "", " x ");
  do_test_trim(false, "", "x ");
  do_test_trim(false, "", " x");
  do_test_trim(false, "xyx", "xxx");
  do_test_trim(false, "xyx", "xYx");
}

#[test]
fn line_fragment() {
  do_test_line_fragment("", "x", (0, 0), (0, 1), (0, 1), (0, 1));

  do_test_line_fragment("x", "y", (0, 1), (0, 1), (0, 1), (0, 1));

  do_test_line_fragment("x_", "y_", (0, 2), (0, 2), (0, 1), (0, 1));

  do_test_line_fragment("x", "y_", (0, 1), (0, 2), (0, 1), (0, 2));

  do_test_line_fragment("x", "x_", (1, 1), (2, 2), (1, 1), (1, 2));

  do_test_line_fragment("x_y_z_", "x_Y_z_", (2, 4), (2, 4), (1, 2), (1, 2));

  do_test_line_fragment("x_y_z_", "x_y_Z_", (4, 6), (4, 6), (2, 3), (2, 3));

  do_test_line_fragment("x_y_z_", "x_y_Z_", (4, 6), (4, 6), (2, 3), (2, 3));

  do_test_line_fragment("x_y_z_", "x_y_Z", (4, 6), (4, 5), (2, 4), (2, 3));

  do_test_line_fragment("x_y_z", "x_y_Z_", (4, 5), (4, 6), (2, 3), (2, 4));

  do_test_line_fragment(" ", "_ ", (0, 0), (0, 1), (0, 0), (0, 1));

  do_test_line_fragment(" ", " _", (1, 1), (2, 2), (1, 1), (1, 2));
}

fn do_test_line_fragment(string1: &str, string2: &str, offsets1: (usize, usize), offsets2: (usize, usize), lines1: (usize, usize), lines2: (usize, usize)) {
  let text1 = Utf16Text::new(&parse_source(string1));
  let text2 = Utf16Text::new(&parse_source(string2));
  let fragments = compare_lines(&text1, &text2, ComparisonPolicy::Default, &Deadline::NONE).expect("no limits");
  assert_eq!(fragments.len(), 1, "Side: {}", fragments.len());
  let fragment = &fragments[0];

  assert_eq!(offsets1, (fragment.start_offset1, fragment.end_offset1), "{fragment:?}");
  assert_eq!(offsets2, (fragment.start_offset2, fragment.end_offset2), "{fragment:?}");
  assert_eq!(lines1, (fragment.start_line1, fragment.end_line1), "{fragment:?}");
  assert_eq!(lines2, (fragment.start_line2, fragment.end_line2), "{fragment:?}");
}

fn do_test_trim(expected: bool, string1: &str, string2: &str) {
  let text1: Vec<u16> = string1.encode_utf16().collect();
  let text2: Vec<u16> = string2.encode_utf16().collect();
  let result = is_equal(&text1, &text2, ComparisonPolicy::TrimWhitespaces);
  assert_eq!(expected, result, "---\n{string1}\n---\n{string2}\n---");
}
