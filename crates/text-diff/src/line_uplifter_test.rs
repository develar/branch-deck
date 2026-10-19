// Ported from `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/LineUplifterTest.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors).

use crate::cancellation::Deadline;
use crate::comparison_util_test_base::check_consistency_lines;
use crate::fragments::LineFragment;
use crate::manager::compare_lines_word_first;
use crate::policy::ComparisonPolicy;
use crate::range::Range;
use crate::test_support::CH_GUN;
use crate::text::Utf16Text;
use pretty_assertions::assert_eq;

#[test]
fn simple_cases() {
  word_first("", "");

  word_first("X_[Y]_Z", "X_[A_B]_Z");

  word_first("X_[Y]_Z", "X_|Z");
}

#[test]
fn tricky_edge_cases() {
  word_first("_[x]", "_[x ]");

  word_first(&format!("[a\n{CH_GUN}]\n|{CH_GUN}{CH_GUN}>"), &format!("[b]\n[]\n{CH_GUN}{CH_GUN}\n[\n!]"));

  // TODO: 3 changed blocks instead of one, first and third compensate each other
  word_first("X_|_*_Z", "X_[Y]__*_Z");

  word_first("|X_", "[Y_Y]_X_");
}

/// `[` and `]` enclose the lines of a changed block, `|` is an empty block before a line, `>` an empty block after one
fn word_first(input1: &str, input2: &str) {
  let text1 = Utf16Text::new(&parse_uplifter_source(input1));
  let text2 = Utf16Text::new(&parse_uplifter_source(input2));
  let expected = parse_expected_ranges(input1, input2);

  let fragments = compare_lines_word_first(&text1, &text2, ComparisonPolicy::Default, &Deadline::NONE).expect("no limits");
  check_consistency_lines(&fragments, &text1, &text2);

  check_line_ranges(&fragments, &expected);
}

fn parse_uplifter_source(text: &str) -> String {
  text.replace('_', "\n").replace(['[', '|', ']', '>'], "")
}

fn parse_expected_ranges(input1: &str, input2: &str) -> Vec<Range> {
  let lines1 = parse_uplifter_line_ranges(input1);
  let lines2 = parse_uplifter_line_ranges(input2);

  assert_eq!(lines1.len(), lines2.len());
  lines1
    .iter()
    .zip(&lines2)
    .map(|(lines1, lines2)| Range::new(lines1.0, lines1.1, lines2.0, lines2.1))
    .collect()
}

fn parse_uplifter_line_ranges(text: &str) -> Vec<(usize, usize)> {
  let mut result = Vec::new();

  let mut line = 0;
  let mut start = None;
  for ch in text.chars() {
    match ch {
      '[' => start = Some(line),
      ']' => {
        result.push((start.take().expect("a block start"), line + 1));
      }
      '|' => {
        result.push((line, line));
        start = None;
      }
      '>' => {
        result.push((line + 1, line + 1));
        start = None;
      }
      '_' | '\n' => line += 1,
      _ => {}
    }
  }

  result
}

fn check_line_ranges(fragments: &[LineFragment], expected: &[Range]) {
  let ranges: Vec<Range> = fragments.iter().map(LineFragment::lines).collect();
  assert_eq!(expected, ranges);
}
