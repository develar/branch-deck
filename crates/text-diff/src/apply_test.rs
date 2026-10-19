//! The blocks applied to bytes: known edits, the line ends at the end of a text, the ids, and property tests that all
//! blocks give the new text, none the old one, and a part of them the old text with those lines replaced. The result
//! of a merge on bytes: the resolves by words, the line ends, and property tests that a change of one side gives that
//! side.

use crate::apply::{BlockId, ComposedMerge, LineBlock, apply_blocks, compose_merge, split_lines};
use crate::merge_model::merge_texts;
use crate::model::{DiffBudget, diff_texts};
use crate::policy::WhitespacePolicy;
use crate::test_support::{proptest_config, text_of_lines};
use pretty_assertions::assert_eq;
use proptest::prelude::Strategy;
use proptest::proptest;

fn blocks(old: &str, new: &str) -> Vec<LineBlock> {
  diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget::NONE)
    .blocks
    .iter()
    .map(LineBlock::from)
    .collect()
}

fn apply(old: &str, new: &str, chosen: &[usize]) -> String {
  let all = blocks(old, new);
  let chosen: Vec<LineBlock> = chosen.iter().map(|&index| all[index]).collect();
  String::from_utf8(apply_blocks(old.as_bytes(), new.as_bytes(), &chosen).unwrap()).unwrap()
}

#[test]
fn lines_end_at_lf_crlf_and_a_lone_cr() {
  assert_eq!(split_lines(b"a\nb\r\nc\rd"), [&b"a\n"[..], b"b\r\n", b"c\r", b"d"]);
  assert_eq!(split_lines(b"a\n"), [&b"a\n"[..], b""]);
  assert_eq!(split_lines(b""), [&b""[..]]);
}

#[test]
fn one_of_two_blocks_applies_alone() {
  let (old, new) = ("a\nb\nc\nd\ne\n", "a\nB\nc\nd\nE\n");
  assert_eq!(blocks(old, new).len(), 2);
  assert_eq!(apply(old, new, &[0]), "a\nB\nc\nd\ne\n");
  assert_eq!(apply(old, new, &[1]), "a\nb\nc\nd\nE\n");
  assert_eq!(apply(old, new, &[0, 1]), new);
}

#[test]
fn each_line_keeps_its_own_end() {
  // the end of a matched line is in no block: it stays
  assert_eq!(apply("c\n", "a\nc\r\n", &[0]), "a\nc\n");
  let (old, new) = ("a\r\nb\r\nc\r\n", "a\r\nB\nc\r\n");
  assert_eq!(apply(old, new, &[0]), new);
  // a lone `\r` ends a line, so the block after it applies to the right line
  let (old, new) = ("a\rb\rc\n", "a\rB\rc\n");
  assert_eq!(apply(old, new, &[0]), new);
}

#[test]
fn a_block_at_the_end_decides_the_final_newline() {
  assert_eq!(apply("a", "a\n", &[0]), "a\n");
  assert_eq!(apply("a\nb", "a\n", &[0]), "a\n");
  assert_eq!(apply("a\nb\n", "a\nb\nc", &[0]), "a\nb\nc");
}

#[test]
fn a_block_outside_the_text_is_refused() {
  let block = LineBlock {
    old_start: 3,
    old_end: 4,
    new_start: 0,
    new_end: 1,
  };
  apply_blocks(b"a\n", b"b\n", &[block]).unwrap_err();
}

#[test]
fn an_id_names_the_lines_and_fails_for_another_content() {
  let (old, new) = ("a\nb\nc\n", "a\nB\nB2\nc\n");
  let block = blocks(old, new)[0];
  let id = BlockId::of(old.as_bytes(), new.as_bytes(), block);
  let text = id.to_string();
  assert!(text.starts_with("2,1:2,2@"), "{text}");
  assert_eq!(BlockId::parse(&text), Some(id.clone()));
  assert_ne!(BlockId::of(old.as_bytes(), b"a\nX\nB2\nc\n", block), id);
  // an empty side names the line before it, as git does
  let insertion = blocks("a\nc\n", "a\nb\nc\n")[0];
  assert!(BlockId::of(b"a\nc\n", b"a\nb\nc\n", insertion).to_string().starts_with("1,0:2,1@"));
  assert_eq!(BlockId::parse("1,0:2,1@zzzzzzzz"), None);
}

fn text() -> impl Strategy<Value = String> {
  text_of_lines(vec!["a", "b", "c", "", "  ", "x y", "}"], vec!["\n", "\n", "\r\n", "\r"], 10)
}

proptest! {
  #![proptest_config(proptest_config(500))]

  #[test]
  fn blocks_apply_to_the_bytes(old in text(), new in text(), mask in proptest::num::u32::ANY) {
    let all = blocks(&old, &new);
    let applied = |chosen: &[LineBlock]| String::from_utf8(apply_blocks(old.as_bytes(), new.as_bytes(), chosen).unwrap()).unwrap();
    // line ends compare as equal, so the end of a matched line is in no block: it stays as the old text has it
    let normalized = |text: &str| text.replace("\r\n", "\n").replace('\r', "\n");
    let exact = !old.contains('\r') && !new.contains('\r');
    let assert_same = |actual: &str, expected: &str| if exact { assert_eq!(actual, expected) } else { assert_eq!(normalized(actual), normalized(expected)) };
    assert_same(&applied(&all), &new);
    assert_eq!(applied(&[]), old);
    // the reverse of all blocks gives the old text back
    let reversed: Vec<LineBlock> = all.iter().map(|block| block.reversed()).collect();
    assert_same(&String::from_utf8(apply_blocks(new.as_bytes(), old.as_bytes(), &reversed).unwrap()).unwrap(), &old);
    // a part of the blocks: the old lines outside them, the new lines inside them
    let chosen: Vec<LineBlock> = all.iter().enumerate().filter(|(index, _)| mask & (1 << (index % 32)) != 0).map(|(_, block)| *block).collect();
    let result = applied(&chosen);
    let content = |text: &str| -> Vec<String> { text.replace("\r\n", "\n").replace('\r', "\n").split('\n').map(str::to_string).collect() };
    let (old_lines, new_lines) = (content(&old), content(&new));
    let mut expected = Vec::new();
    let mut next = 0;
    for block in &chosen {
      expected.extend_from_slice(&old_lines[next..block.old_start]);
      expected.extend_from_slice(&new_lines[block.new_start..block.new_end]);
      next = block.old_end;
    }
    expected.extend_from_slice(&old_lines[next..]);
    let result_lines = content(&result);
    // the content of the lines is exact; only an empty last line may differ, by the final line end
    let trim = |lines: &[String]| -> Vec<String> { let mut lines = lines.to_vec(); if lines.last().is_some_and(String::is_empty) { lines.pop(); } lines };
    assert_eq!(trim(&result_lines), trim(&expected));
  }
}

/// The result of the merge of three texts, and whether it is exact
fn composed(left: &str, base: &str, right: &str) -> (String, bool) {
  let merge = merge_texts(left, base, right, WhitespacePolicy::Default, &DiffBudget::NONE);
  let ComposedMerge { bytes, exact } = compose_merge(left.as_bytes(), base.as_bytes(), right.as_bytes(), &merge);
  (String::from_utf8(bytes).unwrap(), exact)
}

#[test]
fn a_conflict_that_its_words_resolve_takes_both_changes() {
  assert_eq!(
    composed("fun f(A, b, c)\nx\n", "fun f(a, b, c)\nx\n", "fun f(a, b, C)\nx\n"),
    ("fun f(A, b, C)\nx\n".to_string(), true)
  );
  // without a final newline, the resolved line stays the last one without an end
  assert_eq!(composed("A b c", "a b c", "a b C"), ("A b C".to_string(), true));
}

#[test]
fn the_changes_of_both_sides_apply_to_the_base() {
  let base = "a\nb\nc\nd\ne\n";
  assert_eq!(composed("A\nb\nc\nd\ne\n", base, "a\nb\nc\nd\nE\n"), ("A\nb\nc\nd\nE\n".to_string(), true));
  // one side adds the final newline, the other changes a line
  assert_eq!(composed("a\nb", "a\nb", "A\nb"), ("A\nb".to_string(), true));
  assert_eq!(composed("a\nb\n", "a\nb", "A\nb"), ("A\nb\n".to_string(), true));
}

#[test]
fn a_conflict_that_does_not_resolve_keeps_its_base_and_is_not_exact() {
  assert_eq!(composed("x\nA\n", "x\na\n", "x\nB\n"), ("x\na\n".to_string(), false));
}

#[test]
fn the_lines_keep_the_ends_of_the_texts() {
  let crlf = |text: &str| text.replace('\n', "\r\n");
  let (left, base, right) = (crlf("fun f(A, b, c)\nx\ny\n"), crlf("fun f(a, b, c)\nx\ny\n"), crlf("fun f(a, b, C)\nx\nY\n"));
  assert_eq!(composed(&left, &base, &right), (crlf("fun f(A, b, C)\nx\nY\n"), true));
  let cr = |text: &str| text.replace('\n', "\r");
  assert_eq!(composed(&cr("A b c\nx\n"), &cr("a b c\nx\n"), &cr("a b C\nx\n")), (cr("A b C\nx\n"), true));
  // the merge takes a line that one side ends otherwise as equal: the result keeps the base's end and is not exact
  assert_eq!(composed("A\nx\r\n", "a\nx\n", "a\nx\n"), ("A\nx\n".to_string(), false));
  // a resolve by words over lines that end in more than one way keeps its base and is not exact either
  let base = "a b\r\nc d\n";
  assert_eq!(composed("A b\r\nC d\n", base, "a b\r\nc D\n"), (base.to_string(), false));
}

fn merge_text() -> impl Strategy<Value = String> {
  text_of_lines(vec!["a", "b", "a b", "a c", "x y z", "", "}"], vec!["\n", "\n", "\r\n"], 8)
}

proptest! {
  #![proptest_config(proptest_config(400))]

  #[test]
  fn a_change_of_one_side_gives_that_side(base in merge_text(), changed in merge_text()) {
    for (left, right) in [(base.as_str(), changed.as_str()), (changed.as_str(), base.as_str())] {
      let (result, exact) = composed(left, &base, right);
      if exact {
        assert_eq!(result, changed);
      }
      if !changed.contains('\r') && !base.contains('\r') {
        assert!(exact, "{left:?} {base:?} {right:?}");
      }
    }
    // the same change on both sides
    let (result, exact) = composed(&changed, &base, &changed);
    assert!(exact || base.contains('\r') || changed.contains('\r'));
    if exact {
      assert_eq!(result, changed);
    }
  }

  #[test]
  fn an_exact_result_has_the_lines_of_a_merge(left in merge_text(), base in merge_text(), right in merge_text()) {
    let merge = merge_texts(&left, &base, &right, WhitespacePolicy::Default, &DiffBudget::NONE);
    let composed = compose_merge(left.as_bytes(), base.as_bytes(), right.as_bytes(), &merge);
    assert_eq!(composed.exact && merge.unresolved > 0, false);
    if merge.unresolved == 0 && !left.contains('\r') && !base.contains('\r') && !right.contains('\r') {
      assert!(composed.exact, "{left:?} {base:?} {right:?}");
      // each side merged with the result gives the result: nothing of a side is lost
      let result = String::from_utf8(composed.bytes).unwrap();
      for side in [&left, &right] {
        let again = merge_texts(side, &base, &result, WhitespacePolicy::Default, &DiffBudget::NONE);
        assert_eq!(again.unresolved, 0, "{side:?} {base:?} {result:?}");
      }
    }
  }
}
