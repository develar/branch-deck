use crate::ci_text::{cut_head, cut_tail, strip_ansi};
use pretty_assertions::assert_eq;

#[test]
fn colours_are_removed_and_the_text_kept() {
  assert_eq!(
    strip_ansi("\u{1b}[35m\n----Driver Error----\n \u{1b}[0m\u{1b}[34m'MyModel' not found\u{1b}[0m"),
    "\n----Driver Error----\n 'MyModel' not found"
  );
  assert_eq!(strip_ansi("plain — text"), "plain — text");
}

#[test]
fn the_head_is_cut_by_lines_and_by_bytes() {
  let text = "one\ntwo\nthree\n";
  assert_eq!(cut_head(text, 2, 100), ("one\ntwo".to_string(), true));
  assert_eq!(cut_head(text, 3, 100), ("one\ntwo\nthree".to_string(), false));
  // at a character, not inside one
  assert_eq!(cut_head("ab€cd", 1, 3), ("ab".to_string(), true));
}

#[test]
fn the_tail_starts_at_a_line() {
  assert_eq!(cut_tail("first line\nsecond\nthird\n", 12), ("second\nthird".to_string(), true));
  assert_eq!(cut_tail("first line\nsecond\nthird\n", 11), ("third".to_string(), true));
  assert_eq!(cut_tail("short\n", 12), ("short".to_string(), false));
  // one line longer than the limit is cut inside it
  assert_eq!(cut_tail("0123456789", 4), ("6789".to_string(), true));
}
