//! The model of the views: blocks, kinds, ranges, line ends and counts of known edits, and property tests that the
//! blocks partition both texts.

use crate::cancellation::Deadline;
use crate::model::{BlockKind, DiffBlock, DiffBudget, EmptyLastLine, FinalNewline, LineEnds, MAX_INNER_LINES, RangeKind, TextDiff, diff_texts};
use crate::policy::WhitespacePolicy;
use crate::test_support::{proptest_config, text_of_lines};
use crate::text::{LineEndKind, SourceText};
use pretty_assertions::assert_eq;
use proptest::prelude::Strategy;
use proptest::proptest;
use std::fmt::Write;
use std::time::Duration;

const MODIFIED: u32 = RangeKind::Modified as u32;

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
      old_ranges: vec![vec![6, 7, MODIFIED]],
      new_ranges: vec![vec![6, 7, MODIFIED]],
    }]
  );
  assert_eq!((result.old_line_count, result.new_line_count, result.deletions, result.additions), (4, 4, 1, 1));
  assert!(!result.line_level_only);
}

#[test]
fn each_line_of_a_block_has_its_ranges() {
  let result = diff("a\nlet x = 1;\nlet y = 2;\nb\n", "a\nlet x = 10;\nlet z = 2;\nb\n");
  let block = &result.blocks[0];
  assert_eq!(block.old_ranges, vec![vec![8, 9, MODIFIED], vec![4, 5, MODIFIED]]);
  assert_eq!(block.new_ranges, vec![vec![8, 10, MODIFIED], vec![4, 5, MODIFIED]]);
  assert!(block.inline_pairs);
}

#[test]
fn an_inserted_word_is_typed_and_marks_its_empty_side() {
  // IntelliJ types a fragment by its sides, and draws the empty side as a thin bar at its offset
  let result = diff("a\ncall(x)\nb\n", "a\ncall(x, y)\nb\n");
  let block = &result.blocks[0];
  let [start, end, kind] = block.new_ranges[0][..] else { panic!("{block:?}") };
  assert_eq!(kind, RangeKind::Inserted as u32);
  assert_eq!(
    block.old_ranges,
    vec![vec![start, start, RangeKind::Inserted as u32]],
    "the marker of the empty old side at the insertion"
  );
  assert!(end > start);
  let result = diff("a\ncall(x, y)\nb\n", "a\ncall(x)\nb\n");
  let block = &result.blocks[0];
  assert_eq!(block.old_ranges[0][2], RangeKind::Deleted as u32);
  assert_eq!(
    (block.new_ranges[0][0] == block.new_ranges[0][1], block.new_ranges[0][2]),
    (true, RangeKind::Deleted as u32)
  );
}

#[test]
fn ranges_count_utf16_code_units() {
  let result = diff("😂 x\n", "😂 y\n");
  assert_eq!(result.blocks[0].old_ranges, vec![vec![3, 4, MODIFIED]]);
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

/// A call wrapped again that loses an argument: `gamma,` goes up a line and `, true` goes. The changes are of the
/// text of the block, so the line end before `gamma` is a change whose sides stand in lines apart, and line 1 of the
/// two sides is no pair: a row of both would show the new line without the deleted `, true`
#[test]
fn a_change_across_a_line_end_leaves_a_block_without_inline_pairs() {
  let old = "fun f() {\n  val result = compute(alpha, beta,\n      gamma, delta, epsilon, zeta, eta, true)\n}\n";
  let new = "fun f() {\n  val result = compute(alpha, beta, gamma,\n      delta, epsilon, zeta, eta)\n}\n";
  let result = diff(old, new);
  assert_eq!(shape(&result), vec![(1..3, 1..3, BlockKind::Changed)]);
  let block = &result.blocks[0];
  assert!(!block.inline_pairs, "{block:?}");
  // the ranges stay, for the rows of each line: `, true` is a deleted range of the old line
  let deleted = [38, 44, RangeKind::Deleted as u32];
  assert!(block.old_ranges[1].chunks(3).any(|range| range == deleted), "{block:?}");

  // the same lines with the argument dropped where it stands are two pairs
  let dropped = "fun f() {\n  val result = compute(alpha, beta,\n      gamma, delta, epsilon, zeta, eta)\n}\n";
  let result = diff(old, dropped);
  assert_eq!(shape(&result), vec![(2..3, 2..3, BlockKind::Changed)]);
  assert!(result.blocks[0].inline_pairs);
  assert_eq!(result.blocks[0].old_ranges, vec![vec![38, 44, RangeKind::Deleted as u32]]);
  assert_eq!(result.blocks[0].new_ranges, vec![vec![38, 38, RangeKind::Deleted as u32]]);
}

/// Under `Ignore` a line end may move without a change: `third` stands in line 1 of the new side and in line 2 of the
/// old one, and no range says so. Lines 1 and 2 of the sides are no pairs then, though their ranges align
#[test]
fn a_line_end_that_moves_under_ignore_leaves_a_block_without_inline_pairs() {
  let result = diff_with("a\nfirst second third x\nb\n", "a\nfirst second third y\nb\n", WhitespacePolicy::Ignore);
  assert!(result.blocks[0].inline_pairs);
  let result = diff_with(
    "a\nfirst second\nthird fourth fifth x\nb\n",
    "a\nfirst second third\nfourth fifth y\nb\n",
    WhitespacePolicy::Ignore,
  );
  assert_eq!(shape(&result), vec![(1..3, 1..3, BlockKind::Changed)]);
  let block = &result.blocks[0];
  assert_eq!(
    (&block.old_ranges, &block.new_ranges),
    (&vec![vec![], vec![19, 20, MODIFIED]], &vec![vec![], vec![13, 14, MODIFIED]])
  );
  assert!(!block.inline_pairs, "{block:?}");
}

/// Under `Trim` the indent of a pair may differ beside its change: the ranges align, the text before them does not
/// have the same length
#[test]
fn a_reindented_and_edited_line_is_a_pair_under_trim() {
  let result = diff_with("fun f() {\n    foo(bar)\n}\n", "fun f() {\n        foo(baz)\n}\n", WhitespacePolicy::Trim);
  let block = &result.blocks[0];
  assert!(block.inline_pairs);
  assert_eq!((&block.old_ranges, &block.new_ranges), (&vec![vec![8, 11, MODIFIED]], &vec![vec![12, 15, MODIFIED]]));
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
  // the inserted space, and the marker of its empty old side
  assert_eq!(result.blocks[0].new_ranges, vec![vec![1, 2, RangeKind::Inserted as u32]]);
  assert_eq!(result.blocks[0].old_ranges, vec![vec![1, 1, RangeKind::Inserted as u32]]);
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

const POLICIES: [WhitespacePolicy; 4] = [
  WhitespacePolicy::Default,
  WhitespacePolicy::Trim,
  WhitespacePolicy::Ignore,
  WhitespacePolicy::IgnoreBlankLines,
];

/// The empty last line of a text (after a final line end, or the one line of an empty text) pairs with no line but the
/// empty last line of the other, under every policy: a blank line added before the end is that blank line, and the
/// counts are git's (`git diff --numstat`)
#[test]
fn the_empty_last_line_pairs_only_with_the_empty_last_line() {
  // a class that grows by a method and two declarations after it, where its blank line and its brace meet the new ones
  let old = "class A {\n  fun a() {\n    a()\n  }\n\n}\n";
  let new = "class A {\n  fun a() {\n    a()\n  }\n\n  fun b() {\n    b()\n  }\n}\n\nprivate fun c() {\n  c()\n}\n\nprivate fun d() = 1\n";
  for policy in POLICIES {
    let result = diff_with(old, new, policy);
    assert_eq!(shape(&result), vec![(5..5, 5..8, BlockKind::Changed), (6..6, 9..15, BlockKind::Changed)], "{policy:?}");
    assert_eq!((result.deletions, result.additions), (0, 9), "{policy:?}");
  }

  // a blank line added or removed at the end, with any line end: the blank line, before the empty last line; a block of
  // blank lines alone is pale under `Ignore` and cut off under `IgnoreBlankLines`
  for (old, new, lines, counts, blank) in [
    ("x\n", "x\n\n", (1..1, 1..2), (0, 1), true),
    ("x\r\n", "x\r\n\r\n", (1..1, 1..2), (0, 1), true),
    ("x\n\n", "x\n", (1..2, 1..1), (1, 0), true),
    ("a\n", "a\n\nb\n\n", (1..1, 1..4), (0, 3), false),
    // the one line of an empty text is no line either
    ("", "\n", (0..0, 0..1), (0, 1), true),
    ("", "\nx\n", (0..0, 0..2), (0, 2), false),
    ("\nx\n", "", (0..2, 0..0), (2, 0), false),
  ] {
    for policy in POLICIES {
      let result = diff_with(old, new, policy);
      let (kind, counts) = match policy {
        WhitespacePolicy::Ignore if blank => (BlockKind::WhitespaceOnly, counts),
        WhitespacePolicy::IgnoreBlankLines if blank => (BlockKind::Ignored, (0, 0)),
        _ => (BlockKind::Changed, counts),
      };
      assert_eq!(shape(&result), vec![(lines.0.clone(), lines.1.clone(), kind)], "{old:?} to {new:?} under {policy:?}");
      assert_eq!((result.deletions, result.additions), counts, "{old:?} to {new:?} under {policy:?}");
    }
  }

  // only one text ends with a line end: its empty last line is a line of the block that reaches the end
  let result = diff("a\nb\n", "a\nb\n\nc");
  assert_eq!(shape(&result), vec![(2..3, 2..4, BlockKind::Changed)]);
  assert_eq!((result.deletions, result.additions), (0, 2));
  assert_eq!(shape(&diff("a\nb\n\nc", "a\nb\n")), vec![(2..4, 2..3, BlockKind::Changed)]);
  assert_eq!(shape(&diff("x", "x\n\n")), vec![(1..1, 1..3, BlockKind::Changed)]);
  // neither does: every line is compared
  assert_eq!(shape(&diff("a\n\nb", "a\n\nb\n\nc")), vec![(3..3, 3..5, BlockKind::Changed)]);
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
    vec!["a", "b", "c", "", "  ", "a b", "b c", "a  b", "fn f() {", "}", "😂", "x y z", "x y", "y z"],
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
        for range in line_ranges.chunks(3) {
          assert!(
            range[0] <= range[1] && range[1] <= length && range[2] <= RangeKind::Modified as u32,
            "line {line}: {range:?} of {length}"
          );
          // only the empty side of an inserted or a deleted change is a marker
          assert!(range[0] < range[1] || range[2] != RangeKind::Modified as u32, "line {line}: {range:?}");
        }
      }
    }
    if block.inline_pairs {
      check_pairs(block, &old_lines, &new_lines, policy);
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

  // the empty last line of a text pairs with no line but the empty last line of the other: then the counted lines of
  // all blocks, also the ignored ones, differ by what the file grew by, as git's counts do
  let all = |side: fn(&DiffBlock) -> (u32, u32), limit: usize| -> i64 {
    let limit = u32::try_from(limit).unwrap();
    result.blocks.iter().map(|block| i64::from(side(block).1.min(limit).saturating_sub(side(block).0))).sum()
  };
  let (old_counted, new_counted) = (counted(old, &old_lines), counted(new, &new_lines));
  assert_eq!(
    all(|block| (block.new_start, block.new_end), new_counted) - all(|block| (block.old_start, block.old_end), old_counted),
    i64::try_from(new_counted).unwrap() - i64::try_from(old_counted).unwrap(),
    "{:?}",
    result.blocks
  );
}

/// The pairs of a block with `inline_pairs`: as many lines on both sides, as many ranges in line i of both, of the
/// same kinds, and between them the same text but for whitespace (the same text under `Default`). So a row of line i
/// of both sides shows all of both: no text of the old line stands in the row of another pair
fn check_pairs(block: &DiffBlock, old_lines: &[String], new_lines: &[String], policy: WhitespacePolicy) {
  assert_eq!(block.old_end - block.old_start, block.new_end - block.new_start, "{block:?}");
  /// The text of a line between its ranges, one part before each range and one after the last
  fn between(line: &str, ranges: &[u32], keep_whitespace: bool) -> Vec<String> {
    let units: Vec<u16> = line.encode_utf16().collect();
    let mut bounds = vec![0];
    bounds.extend(ranges.chunks(3).flat_map(|range| [range[0] as usize, range[1] as usize]));
    bounds.push(units.len());
    bounds
      .chunks(2)
      .map(|part| String::from_utf16_lossy(&units[part[0]..part[1]]))
      .map(|part| if keep_whitespace { part } else { part.replace([' ', '\t'], "") })
      .collect()
  }
  for pair in 0..(block.old_end - block.old_start) as usize {
    let (old_ranges, new_ranges): (&[u32], &[u32]) = (block.old_ranges.get(pair).map_or(&[], Vec::as_slice), block.new_ranges.get(pair).map_or(&[], Vec::as_slice));
    let kinds = |ranges: &[u32]| ranges.chunks(3).map(|range| range[2]).collect::<Vec<u32>>();
    assert_eq!(kinds(old_ranges), kinds(new_ranges), "pair {pair} of {block:?}");
    let exact = policy == WhitespacePolicy::Default;
    assert_eq!(
      between(&old_lines[block.old_start as usize + pair], old_ranges, exact),
      between(&new_lines[block.new_start as usize + pair], new_ranges, exact),
      "pair {pair} of {block:?} under {policy:?}"
    );
  }
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
