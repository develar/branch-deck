//! The model of the views: blocks, kinds, ranges, line ends and counts of known edits, and property tests that the
//! blocks partition both texts.

use crate::cancellation::Deadline;
use crate::model::{BlockKind, DiffBlock, DiffBudget, EmptyLastLine, FinalNewline, LineEnds, MAX_INNER_LINES, TextDiff, diff_texts};
use crate::policy::WhitespacePolicy;
use crate::test_support::{proptest_config, text_of_lines};
use crate::text::{LineEndKind, SourceText};
use pretty_assertions::assert_eq;
use proptest::prelude::Strategy;
use proptest::proptest;
use std::fmt::Write;
use std::time::Duration;

fn diff(old: &str, new: &str) -> TextDiff {
  diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget::NONE)
}

fn diff_with(old: &str, new: &str, policy: WhitespacePolicy) -> TextDiff {
  diff_texts(old, new, policy, &DiffBudget::NONE)
}

/// `(old lines, new lines, kind)` of each block
fn shape(diff: &TextDiff) -> Vec<(std::ops::Range<u32>, std::ops::Range<u32>, BlockKind)> {
  diff
    .blocks
    .iter()
    .map(|block| (block.old_start..block.old_end, block.new_start..block.new_end, block.kind))
    .collect()
}

#[test]
fn a_changed_word_is_a_range_of_its_line() {
  let result = diff("fn main() {\n  let x = 1;\n}\n", "fn main() {\n  let y = 1;\n}\n");
  assert_eq!(
    result.blocks,
    vec![DiffBlock {
      old_start: 1,
      old_end: 2,
      new_start: 1,
      new_end: 2,
      kind: BlockKind::Changed,
      inline_pairs: true,
      old_ranges: vec![vec![6, 7]],
      new_ranges: vec![vec![6, 7]],
    }]
  );
  assert_eq!((result.old_line_count, result.new_line_count, result.deletions, result.additions), (4, 4, 1, 1));
  assert!(!result.line_level_only);
}

#[test]
fn each_line_of_a_block_has_its_ranges() {
  let result = diff("a\nlet x = 1;\nlet y = 2;\nb\n", "a\nlet x = 10;\nlet z = 2;\nb\n");
  let block = &result.blocks[0];
  assert_eq!(block.old_ranges, vec![vec![8, 9], vec![4, 5]]);
  assert_eq!(block.new_ranges, vec![vec![8, 10], vec![4, 5]]);
  assert!(block.inline_pairs);
}

#[test]
fn ranges_count_utf16_code_units() {
  let result = diff("😂 x\n", "😂 y\n");
  assert_eq!(result.blocks[0].old_ranges, vec![vec![3, 4]]);
}

#[test]
fn a_rewritten_body_drops_its_ranges() {
  let result = diff("fn size(&self) -> usize {\n  self.items.len()\n}\n", "fn size(&self) -> usize {\n  42 + offset * 7\n}\n");
  let block = &result.blocks[0];
  assert_eq!((block.kind, block.inline_pairs, block.old_ranges.len()), (BlockKind::Changed, false, 0));
}

#[test]
fn a_renamed_parameter_reads_as_one_row_per_line() {
  let old = "fun load(path: String): Text {\n  val file = File(path)\n  return file.readText()\n}\n";
  let new = "fun load(location: String): Text {\n  val file = File(location)\n  return file.readText()\n}\n";
  let result = diff(old, new);
  assert_eq!(shape(&result), vec![(0..2, 0..2, BlockKind::Changed)]);
  assert!(result.blocks[0].inline_pairs);
}

#[test]
fn a_reindented_line_is_a_change_under_default_and_no_block_under_trim() {
  let (old, new) = ("class A {\n  fun f() = 1\n}\n", "class A {\n    fun f() = 1\n}\n");
  assert_eq!(shape(&diff(old, new)), vec![(1..2, 1..2, BlockKind::Changed)]);
  assert_eq!(shape(&diff_with(old, new, WhitespacePolicy::Trim)), vec![]);
}

/// A deleted blank line, as IntelliJ shows it: under `Default` and `Trim` a change in the full colour (the line end
/// differs, `ComparisonUtil.equalsTrimWhitespaces`); under `Ignore` a pale change that counts (equal ignoring
/// whitespace, an empty list of inner changes); under `IgnoreBlankLines` cut off (`processBlocks` with `trim`)
#[test]
fn a_blank_line_is_pale_under_ignore_and_cut_off_under_ignore_blank_lines() {
  let (old, new) = ("a\n\nb\n", "a\nb\n");
  let expected = [
    (WhitespacePolicy::Default, BlockKind::Changed, 1),
    (WhitespacePolicy::Trim, BlockKind::Changed, 1),
    (WhitespacePolicy::Ignore, BlockKind::WhitespaceOnly, 1),
    (WhitespacePolicy::IgnoreBlankLines, BlockKind::Ignored, 0),
  ];
  for (policy, kind, deletions) in expected {
    let result = diff_with(old, new, policy);
    assert_eq!(shape(&result), vec![(1..2, 1..1, kind)], "{policy:?}");
    assert_eq!((result.deletions, result.additions), (deletions, 0), "{policy:?}");
  }
}

#[test]
fn a_cut_off_block_near_a_change_stays_beside_it() {
  let (old, new) = ("a\nb\nc\nd\n", "a\n\nb\nc\nX\n");
  let result = diff_with(old, new, WhitespacePolicy::IgnoreBlankLines);
  assert_eq!(shape(&result), vec![(1..1, 1..2, BlockKind::Ignored), (3..4, 4..5, BlockKind::Changed)]);
  assert_eq!((result.deletions, result.additions), (1, 1));
  let result = diff_with(old, new, WhitespacePolicy::Ignore);
  assert_eq!(shape(&result), vec![(1..1, 1..2, BlockKind::WhitespaceOnly), (3..4, 4..5, BlockKind::Changed)]);
  assert_eq!((result.deletions, result.additions), (1, 2));
  assert_eq!(shape(&diff(old, new)), vec![(1..1, 1..2, BlockKind::Changed), (3..4, 4..5, BlockKind::Changed)]);
}

/// The `commit_whitespace` fixture of the frontend (`Format.kt`): two lines indented again, a blank line added apart
/// from them, and `3` changed to `4`. Each policy as IntelliJ shows it: the blocks, and the counts of the meta line
#[test]
fn the_whitespace_fixture_under_each_policy() {
  let old = "class Format {\n  fun first() {\n    val a = 1\n    val b = 2\n  }\n  fun middle() = 0\n  fun second() = 3\n}\n";
  let new = "class Format {\n  fun first() {\n      val a = 1\n      val b = 2\n  }\n\n  fun middle() = 0\n  fun second() = 4\n}\n";
  let indented = (2..4, 2..4, BlockKind::Changed);
  let four = (6..7, 7..8, BlockKind::Changed);
  let expected = [
    (WhitespacePolicy::Default, vec![indented, (5..5, 5..6, BlockKind::Changed), four.clone()], (3, 4)),
    (WhitespacePolicy::Trim, vec![(5..5, 5..6, BlockKind::Changed), four.clone()], (1, 2)),
    (WhitespacePolicy::Ignore, vec![(5..5, 5..6, BlockKind::WhitespaceOnly), four.clone()], (1, 2)),
    (WhitespacePolicy::IgnoreBlankLines, vec![(5..5, 5..6, BlockKind::Ignored), four], (1, 1)),
  ];
  for (policy, blocks, counts) in expected {
    let result = diff_with(old, new, policy);
    assert_eq!(shape(&result), blocks, "{policy:?}");
    assert_eq!((result.deletions, result.additions), counts, "{policy:?}");
  }
}

#[test]
fn the_default_policy_never_ignores_a_block() {
  let result = diff("a\nb c\nd\n", "a\nb  c\nd\n");
  assert_eq!(shape(&result), vec![(1..2, 1..2, BlockKind::Changed)]);
  assert_eq!(result.blocks[0].new_ranges, vec![vec![1, 2]]);
}

/// A text that ends with a lone `\r`, a `\r\n` or a `\n` ends with a line end, and its empty last line does not count:
/// the lines are split before they are counted, so a `\r` is a line end there too
#[test]
fn every_line_end_ends_a_text_alike() {
  for end in ["\r", "\r\n", "\n"] {
    let source = SourceText::new(&format!("a{end}"));
    assert_eq!((source.line_count(), source.counted_lines(), source.ends_with_line_end()), (2, 1, true), "{end:?}");
    let result = diff("a", &format!("a{end}"));
    assert_eq!((result.deletions, result.additions), (0, 0), "{end:?}");
    assert_eq!(result.final_newline, Some(FinalNewline { old: false, new: true }), "{end:?}");
  }
  let source = SourceText::new("a");
  assert_eq!((source.line_count(), source.counted_lines(), source.ends_with_line_end()), (1, 1, false));
  let source = SourceText::new("");
  assert_eq!((source.line_count(), source.counted_lines(), source.ends_with_line_end()), (1, 0, false));
}

#[test]
fn line_ends_compare_as_equal_and_are_named() {
  let result = diff("a\nb\n", "a\r\nb\r\n");
  assert_eq!(result.blocks, vec![]);
  assert_eq!(
    result.line_ends,
    Some(LineEnds {
      old: LineEndKind::Lf,
      new: LineEndKind::Crlf,
      only: true,
    })
  );
  let result = diff("a\nb\n", "a\r\nB\r\n");
  assert_eq!(shape(&result), vec![(1..2, 1..2, BlockKind::Changed)]);
  assert!(!result.line_ends.expect("other line ends").only);
  assert_eq!(diff("a\r\nb\n", "a\r\nc\n").line_ends, None);
}

#[test]
fn a_lone_cr_ends_a_line() {
  let result = diff("a\rb\r", "a\rc\r");
  assert_eq!((result.old_line_count, result.new_line_count), (3, 3));
  assert_eq!(shape(&result), vec![(1..2, 1..2, BlockKind::Changed)]);
  assert_eq!(diff("a\rb", "a\nb").line_ends.map(|ends| (ends.old, ends.new)), Some((LineEndKind::Cr, LineEndKind::Lf)));
  assert_eq!(SourceText::new("a\r\nb\rc\n").separator(), Some(LineEndKind::Mixed));
}

#[test]
fn an_added_and_a_deleted_file_count_their_lines() {
  let result = diff("", "a\nb\n");
  assert_eq!(shape(&result), vec![(0..0, 0..2, BlockKind::Changed)]);
  assert_eq!((result.old_line_count, result.new_line_count, result.deletions, result.additions), (1, 3, 0, 2));
  let result = diff("a\nb", "");
  assert_eq!((result.deletions, result.additions), (2, 0));
}

#[test]
fn a_new_final_line_end_is_a_block_of_the_empty_last_line() {
  let result = diff("a", "a\n");
  // the inserted text is the line end alone
  assert_eq!(shape(&result), vec![(1..1, 1..2, BlockKind::WhitespaceOnly)]);
  assert_eq!(result.final_newline, Some(FinalNewline { old: false, new: true }));
  assert_eq!(diff("a\nb\n", "a\nc").final_newline, Some(FinalNewline { old: true, new: false }));
  assert_eq!(diff("a\n", "b\r\n").final_newline, None);
  // an added file says nothing about a newline it added
  assert_eq!(diff("", "a").final_newline, None);
  // the empty line after a final line end does not count
  assert_eq!((result.deletions, result.additions), (0, 0));
  assert_eq!(result.empty_last_line, EmptyLastLine { old: false, new: true });
  // the one line of an empty text does not count either
  assert_eq!(diff("", "a").empty_last_line, EmptyLastLine { old: true, new: false });
}

#[test]
fn equal_texts_have_no_blocks() {
  assert_eq!(
    diff("a\nb\n", "a\nb\n"),
    TextDiff {
      old_line_count: 3,
      new_line_count: 3,
      empty_last_line: EmptyLastLine { old: true, new: true },
      ..TextDiff::default()
    }
  );
}

#[test]
fn a_side_over_the_line_limit_is_compared_by_line() {
  let mut old = String::new();
  for line in 0..=MAX_INNER_LINES {
    writeln!(old, "line {line}").unwrap();
  }
  let new = old.replacen("line 7\n", "line seven\n", 1);
  let result = diff(&old, &new);
  assert!(result.line_level_only);
  assert_eq!(shape(&result), vec![(7..8, 7..8, BlockKind::Changed)]);
  assert_eq!(result.blocks[0].old_ranges, Vec::<Vec<u32>>::new());
}

#[test]
fn out_of_time_the_lines_are_compared_then_the_whole_texts() {
  let (old, new) = ("a\nb\nc\n", "a\nB\nc\n");
  let expired = Deadline::after(Duration::ZERO);
  let result = diff_texts(
    old,
    new,
    WhitespacePolicy::Default,
    &DiffBudget {
      inner: Some(expired),
      lines: Deadline::NONE,
    },
  );
  assert!(result.line_level_only);
  assert_eq!(shape(&result), vec![(1..2, 1..2, BlockKind::Changed)]);
  let result = diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget { inner: None, lines: expired });
  assert!(result.line_level_only);
  assert_eq!(shape(&result), vec![(0..4, 0..4, BlockKind::Changed)]);
  let result = diff_texts(old, old, WhitespacePolicy::Default, &DiffBudget { inner: None, lines: expired });
  assert_eq!(result.blocks, vec![]);
}

#[test]
fn the_model_is_camel_case_json() {
  let json = serde_json::to_value(diff("a\n", "b\n")).expect("serializable");
  assert_eq!(json["blocks"][0]["kind"], "changed");
  assert_eq!(json["oldLineCount"], 2);
  assert_eq!(json["lineLevelOnly"], false);
}

//
// Properties
//

/// A text of up to 12 lines from a few contents, whitespace and every line end among them
fn text() -> impl Strategy<Value = String> {
  text_of_lines(
    vec!["a", "b", "c", "", "  ", "a b", "a  b", "fn f() {", "}", "😂", "x y z"],
    vec!["\n", "\n", "\r\n", "\r"],
    12,
  )
}

/// The lines as the frontend splits them: at `\r\n`, `\r` and `\n`
fn split_lines(text: &str) -> Vec<String> {
  text.replace("\r\n", "\n").replace('\r', "\n").split('\n').map(str::to_string).collect()
}

/// The blocks partition both texts in order, the lines between them are as many on both sides, the ranges lie inside
/// their lines, and the counts are the lines of the blocks that are not ignored
fn check(old: &str, new: &str, policy: WhitespacePolicy, result: &TextDiff) {
  let (old_lines, new_lines) = (split_lines(old), split_lines(new));
  assert_eq!((result.old_line_count as usize, result.new_line_count as usize), (old_lines.len(), new_lines.len()));
  let (mut old_position, mut new_position) = (0, 0);
  for block in &result.blocks {
    assert!(block.old_start < block.old_end || block.new_start < block.new_end, "{block:?}");
    assert!(block.old_start >= old_position && block.new_start >= new_position, "{block:?}");
    assert_eq!(block.old_start - old_position, block.new_start - new_position, "{block:?}");
    if policy == WhitespacePolicy::Default {
      for offset in 0..block.old_start - old_position {
        assert_eq!(old_lines[(old_position + offset) as usize], new_lines[(new_position + offset) as usize]);
      }
    }
    // only `IgnoreBlankLines` cuts blocks off, as IntelliJ's `processBlocks` with `trim`
    if policy != WhitespacePolicy::IgnoreBlankLines {
      assert_ne!(block.kind, BlockKind::Ignored);
    }
    for (ranges, start, end, lines) in [
      (&block.old_ranges, block.old_start, block.old_end, &old_lines),
      (&block.new_ranges, block.new_start, block.new_end, &new_lines),
    ] {
      if ranges.is_empty() {
        continue;
      }
      assert_eq!(ranges.len(), (end - start) as usize);
      for (line, line_ranges) in (start..end).zip(ranges) {
        let length = u32::try_from(lines[line as usize].encode_utf16().count()).unwrap();
        for range in line_ranges.chunks(2) {
          assert!(range[0] < range[1] && range[1] <= length, "line {line}: {range:?} of {length}");
        }
      }
    }
    (old_position, new_position) = (block.old_end, block.new_end);
  }
  assert!(old_position <= result.old_line_count && new_position <= result.new_line_count);
  assert_eq!(result.old_line_count - old_position, result.new_line_count - new_position);

  let ends = |text: &str| text.ends_with(['\n', '\r']);
  let expected = (!old.is_empty() && !new.is_empty() && ends(old) != ends(new)).then(|| FinalNewline { old: ends(old), new: ends(new) });
  assert_eq!(result.final_newline, expected);

  let counted = |text: &str, lines: &[String]| lines.len() - usize::from(text.is_empty() || text.ends_with(['\n', '\r']));
  let sum = |side: fn(&DiffBlock) -> (u32, u32), limit: usize| -> u32 {
    let limit = u32::try_from(limit).unwrap();
    result
      .blocks
      .iter()
      .filter(|block| block.kind != BlockKind::Ignored)
      .map(|block| side(block).1.min(limit).saturating_sub(side(block).0))
      .sum()
  };
  assert_eq!(result.deletions, sum(|block| (block.old_start, block.old_end), counted(old, &old_lines)));
  assert_eq!(result.additions, sum(|block| (block.new_start, block.new_end), counted(new, &new_lines)));
}

proptest! {
  #![proptest_config(proptest_config(500))]

  #[test]
  fn blocks_partition_both_texts(old in text(), new in text()) {
    for policy in [WhitespacePolicy::Default, WhitespacePolicy::Trim, WhitespacePolicy::Ignore, WhitespacePolicy::IgnoreBlankLines] {
      check(&old, &new, policy, &diff_with(&old, &new, policy));
      let by_line = diff_texts(&old, &new, policy, &DiffBudget { inner: None, lines: Deadline::NONE });
      check(&old, &new, policy, &by_line);
    }
  }
}
