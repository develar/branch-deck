//! The merge model of the views: the kinds, sides, resolutions and ranges of known merges, and property tests that the
//! blocks partition the three texts.

use crate::merge_model::{MergeBlockKind, MergeResolution, Sides, TextMerge, merge_texts};
use crate::model::LineEndKind;
use crate::test_support::{proptest_config, text_of_lines};
use line_diff::cancellation::Deadline;
use line_diff::comparison_util::is_equal_texts;
use line_diff::model::DiffBudget;
use line_diff::policy::{ComparisonPolicy, WhitespacePolicy};
use pretty_assertions::assert_eq;
use proptest::prelude::Strategy;
use proptest::proptest;
use std::time::Duration;

fn merge(left: &str, base: &str, right: &str) -> TextMerge {
  merge_texts(left, base, right, WhitespacePolicy::Default, &DiffBudget::NONE)
}

/// `(left lines, base lines, right lines, kind, resolution)` of each block
fn shape(merge: &TextMerge) -> Vec<([u32; 6], MergeBlockKind, Option<MergeResolution>)> {
  merge
    .blocks
    .iter()
    .map(|block| {
      (
        [block.left_start, block.left_end, block.base_start, block.base_end, block.right_start, block.right_end],
        block.kind,
        block.resolution,
      )
    })
    .collect()
}

#[test]
fn a_change_of_one_side_takes_that_side() {
  let result = merge("a\nB\nc\n", "a\nb\nc\n", "a\nb\nc\n");
  assert_eq!(shape(&result), vec![([1, 2, 1, 2, 1, 2], MergeBlockKind::Modified, Some(MergeResolution::Default))]);
  let block = &result.blocks[0];
  assert_eq!((block.left_changed, block.right_changed, block.ignored), (true, false, false));
  assert_eq!((result.conflicts, result.unresolved), (0, 0));
}

#[test]
fn a_conflict_of_other_words_resolves_by_its_words() {
  let result = merge("let x = 1, y = 0;\n", "let x = 0, y = 0;\n", "let x = 0, y = 3;\n");
  assert_eq!(shape(&result), vec![([0, 1, 0, 1, 0, 1], MergeBlockKind::Conflict, Some(MergeResolution::Text))]);
  let block = &result.blocks[0];
  assert_eq!(block.resolved_text.as_deref(), Some("let x = 1, y = 3;"));
  assert_eq!(
    (block.left_ranges.clone(), block.base_ranges.clone(), block.right_ranges.clone()),
    (vec![vec![8, 9, 15, 16]], vec![vec![8, 9, 15, 16]], vec![vec![8, 9, 15, 16]])
  );
  // git stops here; the words resolve the conflict
  assert_eq!((result.conflicts, result.unresolved), (1, 0));
}

#[test]
fn a_conflict_of_the_same_words_stays_with_the_user() {
  let result = merge("x = 1\n", "x = 0\n", "x = 2\n");
  assert_eq!(shape(&result), vec![([0, 1, 0, 1, 0, 1], MergeBlockKind::Conflict, None)]);
  assert_eq!(result.blocks[0].resolved_text, None);
  assert_eq!((result.conflicts, result.unresolved), (1, 1));
}

/// A conflict that only a resolve ignoring whitespace, the line ends too, would take stays with the user: that resolve
/// joins lines into one that no side has (`ab` of `b\nb` and `a b\n\n`, `a bx y z` of `a b` and `b\nx y z\n}`), so the
/// merge resolves by words under the default policy alone (`merge_resolve_util::try_resolve_by_default`); the ported
/// `try_resolve` still takes it
#[test]
fn a_conflict_that_only_ignoring_whitespace_resolves_stays_with_the_user() {
  for (left, base, right, joined) in [("b\nb", "b", "a b\n\n", "ab\nb"), ("a b", "b\n", "b\nx y z\n}", "a bx y z\n}")] {
    let result = merge(left, base, right);
    assert_eq!((result.conflicts, result.unresolved), (1, 1), "{left:?} {base:?} {right:?}");
    let [left, base, right] = [left, base, right].map(|text| text.encode_utf16().collect::<Vec<u16>>());
    let resolved = line_diff::merge_resolve_util::try_resolve(&left, &base, &right, &Deadline::NONE).map(|text| String::from_utf16_lossy(&text));
    assert_eq!(resolved.as_deref(), Some(joined));
  }
}

#[test]
fn equal_changes_and_insertions_have_their_kinds() {
  assert_eq!(
    shape(&merge("a\nx\nb\n", "a\nb\n", "a\nx\nb\n")),
    vec![([1, 2, 1, 1, 1, 2], MergeBlockKind::Inserted, Some(MergeResolution::Default))]
  );
  assert_eq!(
    shape(&merge("a\n", "a\nb\n", "a\n")),
    vec![([1, 1, 1, 2, 1, 1], MergeBlockKind::Deleted, Some(MergeResolution::Default))]
  );
  // two different insertions at one place
  assert_eq!(shape(&merge("a\nx\n", "a\n", "a\ny\n")), vec![([1, 2, 1, 1, 1, 2], MergeBlockKind::Conflict, None)]);
}

/// The `commit_whitespace` fixture as a merge: base and left are the old `Format.kt`, right the new one (two lines
/// indented again, a blank line added, `3` to `4`). IntelliJ's merge viewer drops no block under any policy: every
/// block is a change, and one equal under the policy is painted pale without word ranges (`DiffUtil.compareThreesideInner`
/// gives it empty lists, `DiffViewerHighlighters.createHighlighter` paints `withIgnored`)
#[test]
fn the_whitespace_fixture_merges_alike_under_each_policy() {
  let base = "class Format {\n  fun first() {\n    val a = 1\n    val b = 2\n  }\n  fun middle() = 0\n  fun second() = 3\n}\n";
  let right = "class Format {\n  fun first() {\n      val a = 1\n      val b = 2\n  }\n\n  fun middle() = 0\n  fun second() = 4\n}\n";
  // the merge keeps a block equal under the policy, so that a side can still apply it (`MergeRangeUtil.getMergeType`
  // takes the side that changed by the true equality)
  let indented = |ignored| ([2, 4, 2, 4, 2, 4], MergeBlockKind::Modified, ignored);
  let blank = |ignored| ([5, 5, 5, 5, 5, 6], MergeBlockKind::Inserted, ignored);
  let four = ([6, 7, 6, 7, 7, 8], MergeBlockKind::Modified, false);
  let expected = [
    (WhitespacePolicy::Default, vec![indented(false), blank(false), four]),
    // a blank line differs in its line end under `Trim` (`equalsTrimWhitespaces` compares line by line)
    (WhitespacePolicy::Trim, vec![indented(false), blank(false), four]),
    // the blank line: its changed side equals the others ignoring whitespace. The indentation: the unchanged left side
    // counts as "", so its words are compared, and none differ
    (WhitespacePolicy::Ignore, vec![indented(false), blank(true), four]),
    // IntelliJ's merge offers no such policy; its comparison is `Ignore`, and nothing is cut off
    (WhitespacePolicy::IgnoreBlankLines, vec![indented(false), blank(true), four]),
  ];
  for (policy, blocks) in expected {
    let result = merge_texts(base, base, right, policy, &DiffBudget::NONE);
    let found: Vec<([u32; 6], MergeBlockKind, bool)> = result
      .blocks
      .iter()
      .map(|block| {
        (
          [block.left_start, block.left_end, block.base_start, block.base_end, block.right_start, block.right_end],
          block.kind,
          block.ignored,
        )
      })
      .collect();
    assert_eq!(found, blocks, "{policy:?}");
    if policy != WhitespacePolicy::Default {
      // the indentation has no changed words: pale without words, as IntelliJ paints a block with empty inner lists
      assert_eq!(result.blocks[0].right_ranges, vec![Vec::<u32>::new(); 2], "{policy:?}");
    }
    assert!(result.blocks.iter().all(|block| block.right_changed && !block.left_changed), "{policy:?}");
    assert_eq!((result.conflicts, result.unresolved), (0, 0), "{policy:?}");
  }
}

/// The lines, kind, left and right changed, and `ignored` of a block
type BlockShape = ([u32; 6], MergeBlockKind, bool, bool, bool);

/// The shape of each block, and the conflicts, under `policy`
fn merge_shape(left: &str, base: &str, right: &str, policy: WhitespacePolicy) -> (Vec<BlockShape>, u32) {
  let result = merge_texts(left, base, right, policy, &DiffBudget::NONE);
  let blocks = result
    .blocks
    .iter()
    .map(|block| {
      (
        [block.left_start, block.left_end, block.base_start, block.base_end, block.right_start, block.right_end],
        block.kind,
        block.left_changed,
        block.right_changed,
        block.ignored,
      )
    })
    .collect();
  (blocks, result.conflicts)
}

/// The fixture split over the sides: the left indents the block of `first` again, the right adds the blank line and
/// changes `3` to `4`; and one line that both sides change in whitespace only, each in another way
#[test]
fn whitespace_changes_on_both_sides_under_each_policy() {
  let base = "class Format {\n  fun first() {\n    val a = 1\n    val b = 2\n  }\n  fun middle() = 0\n  fun second() = 3\n}\n";
  let left = "class Format {\n  fun first() {\n      val a = 1\n      val b = 2\n  }\n  fun middle() = 0\n  fun second() = 3\n}\n";
  let right = "class Format {\n  fun first() {\n    val a = 1\n    val b = 2\n  }\n\n  fun middle() = 0\n  fun second() = 4\n}\n";
  // the re-indent: equal under `Trim` and `Ignore`, kept as the left's change (`MergeRangeUtil.getMergeType`, the true
  // equality), never ignored: the unchanged right counts as "" (`DiffUtil.compareThreesideInner`)
  let indented = ([2, 4, 2, 4, 2, 4], MergeBlockKind::Modified, true, false, false);
  // the blank line: ignored only under the `Ignore` comparison, where "" and a blank line are equal
  let blank = |ignored| ([5, 5, 5, 5, 5, 6], MergeBlockKind::Inserted, false, true, ignored);
  let four = ([6, 7, 6, 7, 7, 8], MergeBlockKind::Modified, false, true, false);
  // one line that both sides change in whitespace only: a conflict under `Default`; under `Trim` and `Ignore` equal on
  // all sides, so a change of both, ignored where all whitespace is ignored
  let (same_base, same_left, same_right) = ("a\n  x = 1\nb\n", "a\n    x = 1\nb\n", "a\n\tx = 1\nb\n");
  let same = |kind, ignored| vec![([1, 2, 1, 2, 1, 2], kind, true, true, ignored)];
  let expected = [
    (WhitespacePolicy::Default, vec![indented, blank(false), four], same(MergeBlockKind::Conflict, false), 1),
    (WhitespacePolicy::Trim, vec![indented, blank(false), four], same(MergeBlockKind::Modified, false), 0),
    (WhitespacePolicy::Ignore, vec![indented, blank(true), four], same(MergeBlockKind::Modified, true), 0),
    (
      WhitespacePolicy::IgnoreBlankLines,
      vec![indented, blank(true), four],
      same(MergeBlockKind::Modified, true),
      0,
    ),
  ];
  for (policy, blocks, same_blocks, same_conflicts) in expected {
    assert_eq!(merge_shape(left, base, right, policy), (blocks, 0), "{policy:?}");
    assert_eq!(merge_shape(same_left, same_base, same_right, policy), (same_blocks, same_conflicts), "{policy:?}");
  }
}

/// IntelliJ's rule (`DiffUtil.compareThreesideInner`): under `Ignore` a block is ignored when its changed sides equal
/// each other ignoring whitespace, an unchanged side as "". A blank line both sides add is; a whitespace change of one
/// side is not (its words are compared, and none differ), nor anything under `Default`
#[test]
fn a_block_is_ignored_where_its_changed_sides_are_equal_ignoring_whitespace() {
  let (left, base, right) = ("a\n\nb\nc\n", "a\nb\nc\n", "a\n\nb\nc\n");
  let result = merge_texts(left, base, right, WhitespacePolicy::Ignore, &DiffBudget::NONE);
  let ignored: Vec<(MergeBlockKind, bool)> = result.blocks.iter().map(|block| (block.kind, block.ignored)).collect();
  assert_eq!(ignored, [(MergeBlockKind::Inserted, true)]);
  assert!(result.blocks[0].left_changed && result.blocks[0].right_changed);

  let (left, base, right) = ("a\n  b\nc\n", "a\nb\nc\n", "a\nb\nC\n");
  let result = merge_texts(left, base, right, WhitespacePolicy::Ignore, &DiffBudget::NONE);
  let ignored: Vec<bool> = result.blocks.iter().map(|block| block.ignored).collect();
  assert_eq!(ignored, [false, false]);
  assert!(result.blocks[0].left_changed && !result.blocks[0].right_changed);
  assert_eq!(result.blocks[0].left_ranges, vec![Vec::<u32>::new()]);
  assert!(merge(left, base, right).blocks.iter().all(|block| !block.ignored));
}

/// The last line of a text pairs only with the last line of the base of its kind, as git merges: the empty line after a
/// final line end (or the one line of an empty text) with its like, so two insertions at the end of a file are one
/// conflict, never two insertions on two sides of that line; a line without an end with an equal line without an end,
/// so a change of the final line end is a change of that line; the conflicts are those of `git merge-file`
/// (`word_resolve_test::the_end_of_a_file_merges_as_git_merge_file_merges_it` of git-ops)
#[test]
fn the_last_line_pairs_only_with_the_last_line_of_the_base_of_its_kind() {
  let conflict = |lines: [u32; 6]| vec![(lines, MergeBlockKind::Conflict, None)];
  // a blank line on one side and a line on the other, with any line end, and from an empty base
  assert_eq!(shape(&merge("x\n\n", "x\n", "x\ny\n")), conflict([1, 2, 1, 1, 1, 2]));
  assert_eq!(shape(&merge("x\ny\n", "x\n", "x\n\n")), conflict([1, 2, 1, 1, 1, 2]));
  assert_eq!(shape(&merge("x\r\n\r\n", "x\r\n", "x\r\ny\r\n")), conflict([1, 2, 1, 1, 1, 2]));
  assert_eq!(shape(&merge("\n", "", "x\n")), conflict([0, 1, 0, 0, 0, 1]));
  // a blank line removed on one side, a line added after it on the other
  assert_eq!(shape(&merge("x\n", "x\n\n", "x\n\nz\n")), conflict([1, 1, 1, 2, 1, 3]));
  // a side without the final line end: its last line pairs with no line that has an end, so a change of the final line
  // end is a change of that line, as in git
  assert_eq!(shape(&merge("x", "x\n", "x\ny\n")), conflict([0, 1, 0, 2, 0, 3]));
  assert_eq!(shape(&merge("a\nb\n\n", "a\nb\n", "a\nb")), conflict([1, 4, 1, 3, 1, 2]));
  let words = |lines: [u32; 6]| vec![(lines, MergeBlockKind::Conflict, Some(MergeResolution::Text))];
  assert_eq!(shape(&merge("a", "}\na", "a\n")), words([0, 1, 0, 2, 0, 2]));
  assert_eq!(shape(&merge("a\nb", "a\nb\n", "A\nb\n")), words([0, 2, 0, 3, 0, 3]));
  // apart from a change of the other side, a change of the final line end merges
  assert_eq!(
    shape(&merge("a\nb\nc", "a\nb\nc\n", "A\nb\nc\n")),
    vec![
      ([0, 1, 0, 1, 0, 1], MergeBlockKind::Modified, Some(MergeResolution::Default)),
      ([2, 3, 2, 4, 2, 4], MergeBlockKind::Modified, Some(MergeResolution::Default)),
    ]
  );
  // an insertion at the end beside a change elsewhere: before the empty last line, no conflict
  let result = merge("A\nb\nc\n", "a\nb\nc\n", "a\nb\nc\n\nd\n");
  assert_eq!(
    shape(&result),
    vec![
      ([0, 1, 0, 1, 0, 1], MergeBlockKind::Modified, Some(MergeResolution::Default)),
      ([3, 3, 3, 3, 3, 5], MergeBlockKind::Inserted, Some(MergeResolution::Default)),
    ]
  );
  // the same blank line on both sides
  assert_eq!(
    shape(&merge("x\n\n", "x\n", "x\n\n")),
    vec![([1, 2, 1, 1, 1, 2], MergeBlockKind::Inserted, Some(MergeResolution::Default))]
  );
}

#[test]
fn the_line_ends_and_the_final_newline_of_the_sides_are_named() {
  let result = merge("a\r\nb\r\n", "a\nb\n", "a\nb");
  assert_eq!(
    result.line_ends,
    Some(Sides {
      left: Some(LineEndKind::Crlf),
      base: Some(LineEndKind::Lf),
      right: Some(LineEndKind::Lf),
    })
  );
  assert_eq!(
    result.final_newline,
    Some(Sides {
      left: true,
      base: true,
      right: false
    })
  );
  assert_eq!(
    result.empty_last_line,
    Sides {
      left: true,
      base: true,
      right: false
    }
  );
}

#[test]
fn out_of_time_the_blocks_stay_and_nothing_resolves_by_words() {
  let budget = DiffBudget {
    inner: Some(Deadline::after(Duration::ZERO)),
    lines: Deadline::NONE,
  };
  let result = merge_texts("let x = 1, y = 0;\n", "let x = 0, y = 0;\n", "let x = 0, y = 3;\n", WhitespacePolicy::Default, &budget);
  assert!(result.line_level_only);
  assert_eq!(shape(&result), vec![([0, 1, 0, 1, 0, 1], MergeBlockKind::Conflict, None)]);
  let budget = DiffBudget {
    inner: None,
    lines: Deadline::after(Duration::ZERO),
  };
  let result = merge_texts("a\n", "b\n", "c\n", WhitespacePolicy::Default, &budget);
  assert_eq!(shape(&result), vec![([0, 2, 0, 2, 0, 2], MergeBlockKind::Conflict, None)]);
}

#[test]
fn the_model_is_camel_case_json() {
  let json = serde_json::to_value(merge("x = 1\n", "x = 0\n", "x = 2\n")).expect("serializable");
  assert_eq!(json["blocks"][0]["kind"], "conflict");
  assert_eq!(json["blocks"][0]["leftChanged"], true);
  assert_eq!(json["unresolved"], 1);
}

//
// Properties
//

fn text() -> impl Strategy<Value = String> {
  text_of_lines(vec!["a", "b", "c", "", "  ", "a b", "x y z", "fn f() {", "}"], vec!["\n", "\n", "\r\n"], 10)
}

fn split_lines(text: &str) -> Vec<Vec<u16>> {
  text
    .replace("\r\n", "\n")
    .replace('\r', "\n")
    .split('\n')
    .map(|line| line.encode_utf16().collect())
    .collect()
}

/// The blocks partition the three texts, the lines between them are equal under the policy, the ranges lie in their
/// lines, an ignored block exists only under a policy that ignores all whitespace, and the counts count the conflicts
fn check(texts: [&str; 3], policy: WhitespacePolicy, result: &TextMerge) {
  let lines = texts.map(split_lines);
  let counts = [result.left_line_count, result.base_line_count, result.right_line_count].map(|count| count as usize);
  assert_eq!(counts, [lines[0].len(), lines[1].len(), lines[2].len()]);
  let mut position = [0usize; 3];
  for block in &result.blocks {
    let bounds = [(block.left_start, block.left_end), (block.base_start, block.base_end), (block.right_start, block.right_end)].map(|(start, end)| (start as usize, end as usize));
    let gap = bounds[1].0 - position[1];
    for side in 0..3 {
      assert!(bounds[side].0 >= position[side] && bounds[side].0 <= bounds[side].1, "{block:?}");
      assert_eq!(bounds[side].0 - position[side], gap, "{block:?}");
    }
    for offset in 0..gap {
      for side in [0, 2] {
        assert!(is_equal_texts(
          &lines[1][position[1] + offset],
          &lines[side][position[side] + offset],
          policy.comparison_policy()
        ));
      }
    }
    assert!(policy.comparison_policy() == ComparisonPolicy::IgnoreWhitespaces || !block.ignored);
    assert!(block.resolved_text.is_none() || block.resolution == Some(MergeResolution::Text));
    for (side, ranges) in [&block.left_ranges, &block.base_ranges, &block.right_ranges].into_iter().enumerate() {
      if ranges.is_empty() {
        continue;
      }
      assert_eq!(ranges.len(), bounds[side].1 - bounds[side].0);
      for (line, line_ranges) in (bounds[side].0..bounds[side].1).zip(ranges) {
        let length = u32::try_from(lines[side][line].len()).unwrap();
        for range in line_ranges.chunks(2) {
          assert!(range[0] < range[1] && range[1] <= length);
        }
      }
    }
    position = bounds.map(|(_, end)| end);
  }
  let tail = counts[1] - position[1];
  assert_eq!([counts[0] - position[0], counts[2] - position[2]], [tail, tail], "{:?}", result.blocks);
  // the lines outside the blocks pair in order: the last line of a text pairs only with the last line of the base of its
  // kind, the empty line after a final line end or a line without an end
  let empty = [result.empty_last_line.left, result.empty_last_line.base, result.empty_last_line.right];
  let last_kind = |side: usize, line: usize| (line + 1 == counts[side]).then_some(empty[side]);
  let bounds = result.blocks.iter().map(|block| {
    let start = [block.left_start, block.base_start, block.right_start].map(|line| line as usize);
    (start, [block.left_end, block.base_end, block.right_end].map(|line| line as usize))
  });
  let mut start = [0usize; 3];
  for (block_start, block_end) in bounds.chain([(counts, counts)]) {
    for offset in 0..block_start[1] - start[1] {
      let last = [0, 1, 2].map(|side| last_kind(side, start[side] + offset));
      assert_eq!(last, [last[1]; 3], "{:?}: line {} of the base", result.blocks, start[1] + offset);
    }
    start = block_end;
  }
  let conflicts = result.blocks.iter().filter(|block| block.kind == MergeBlockKind::Conflict).count();
  let unresolved = result
    .blocks
    .iter()
    .filter(|block| block.kind == MergeBlockKind::Conflict && block.resolution.is_none())
    .count();
  assert_eq!((result.conflicts as usize, result.unresolved as usize), (conflicts, unresolved));
}

proptest! {
  #![proptest_config(proptest_config(300))]

  #[test]
  fn blocks_partition_the_three_texts(left in text(), base in text(), right in text()) {
    for policy in [WhitespacePolicy::Default, WhitespacePolicy::Trim, WhitespacePolicy::Ignore] {
      let result = merge_texts(&left, &base, &right, policy, &DiffBudget::NONE);
      check([&left, &base, &right], policy, &result);
    }
  }
}
