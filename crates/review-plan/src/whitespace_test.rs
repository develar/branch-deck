use crate::whitespace::TextLines;
use pretty_assertions::assert_eq;
use text_diff::apply::split_lines;

/// The lines of a text are those the engine of the diffs splits it into, each without its line end, and a range of
/// lines of git names the lines of the diffs between the same two `\n`
#[test]
fn the_lines_of_a_text_are_those_of_the_diffs() {
  for text in [
    "",
    "a",
    "a\n",
    "a\nb",
    "a\r\nb\r\n",
    "a\rb\nc\n",
    "a\r\r\nb\r\r\n",
    "\r",
    "\n\r\n\r",
    "é\r😀\n",
    "/* a\rb */\npackage a\n\nfun one() = 1\n",
  ] {
    let lines = TextLines::of(text);
    let engine: Vec<&str> = split_lines(text.as_bytes())
      .into_iter()
      .map(|line| std::str::from_utf8(line).unwrap().trim_end_matches(['\n', '\r']))
      .collect();
    assert_eq!(lines.all(), engine, "{text:?}");
  }

  // a lone `\r` ends the lines 0 and 2 of the diffs: git has "a\rb", "c\rd", "e" and the empty line after them
  let lines = TextLines::of("a\rb\nc\rd\ne\n");
  assert_eq!(lines.all(), ["a", "b", "c", "d", "e", ""]);
  assert_eq!(
    [
      lines.of_git(0..1),
      lines.of_git(1..2),
      lines.of_git(2..3),
      lines.of_git(0..3),
      lines.of_git(1..1),
      lines.of_git(3..4),
      lines.of_git(7..9)
    ],
    [0..2, 2..4, 4..5, 0..5, 2..2, 5..6, 9..11]
  );
  assert_eq!([lines.text(&(1..3)), lines.text(&(4..9)), lines.text(&(7..9))], [&["b", "c"][..], &["e", ""][..], &[][..]]);
  // every `\r\r\n` is a line and an empty one
  let lines = TextLines::of("a\r\r\nb\r\r\n");
  assert_eq!(lines.all(), ["a", "", "b", "", ""]);
  assert_eq!([lines.of_git(0..1), lines.of_git(1..2)], [0..2, 2..4]);
  // without a lone `\r` the lines of git are those of the diffs
  let lines = TextLines::of("a\r\nb\nc");
  assert_eq!([lines.of_git(0..1), lines.of_git(1..3), lines.of_git(2..2)], [0..1, 1..3, 2..2]);
}
