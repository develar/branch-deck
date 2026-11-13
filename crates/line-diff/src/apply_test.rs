//! The blocks applied to bytes: known edits, the line ends at the end of a text, the ids, and property tests that all
//! blocks give the new text, none the old one, and a part of them the old text with those lines replaced.

use crate::apply::{BlockId, LineBlock, apply_blocks, split_lines};
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
