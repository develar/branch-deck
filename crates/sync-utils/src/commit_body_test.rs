use crate::commit_body::{body_blocks, message_body};
use domain_types::commit_body::CommitBodyBlock;
use pretty_assertions::assert_eq;

fn paragraph(lines: &[&str]) -> CommitBodyBlock {
  CommitBodyBlock::Paragraph {
    lines: lines.iter().map(ToString::to_string).collect(),
  }
}

fn list(ordered: bool, start: u32, items: &[&str]) -> CommitBodyBlock {
  CommitBodyBlock::List {
    ordered,
    start,
    items: items.iter().map(ToString::to_string).collect(),
  }
}

fn preformatted(text: &str) -> CommitBodyBlock {
  CommitBodyBlock::Preformatted { text: text.to_string() }
}

fn trailers(lines: &[&str]) -> CommitBodyBlock {
  CommitBodyBlock::Trailers {
    lines: lines.iter().map(ToString::to_string).collect(),
  }
}

#[test]
fn joins_hard_wrapped_lines_into_one_paragraph_and_starts_a_block_after_a_blank_line() {
  assert_eq!(
    body_blocks("The pool size was too small\nfor the nightly import."),
    [paragraph(&["The pool size was too small for the nightly import."])]
  );
  assert_eq!(
    body_blocks("First paragraph.\n\nSecond paragraph."),
    [paragraph(&["First paragraph."]), paragraph(&["Second paragraph."])]
  );
  assert_eq!(
    body_blocks("First line\r\nsecond line\r\n\r\n- item"),
    [paragraph(&["First line second line"]), list(false, 1, &["item"])]
  );
}

#[test]
fn keeps_a_line_after_a_hard_break_a_quote_line_and_a_table_row_its_own_and_a_heading_and_a_rule_a_paragraph() {
  assert_eq!(
    body_blocks("A hard break  \nafter it, and another\\\nafter it."),
    [paragraph(&["A hard break", "after it, and another", "after it."])]
  );
  assert_eq!(
    body_blocks("Intro:\n| a | table |\n| - | ----- |\n> a quote\n> of two lines\nmore"),
    [paragraph(&["Intro:", "| a | table |", "| - | ----- |", "> a quote", "> of two lines", "more"])]
  );
  assert_eq!(
    body_blocks("Before\n## A heading\nunder it\n---\nafter it"),
    [
      paragraph(&["Before"]),
      paragraph(&["## A heading"]),
      paragraph(&["under it"]),
      paragraph(&["---"]),
      paragraph(&["after it"])
    ]
  );
}

#[test]
fn reads_lists_without_their_markers_each_kind_its_own() {
  assert_eq!(
    body_blocks("- dash item\n* star item\n+ plus item\n• bullet item"),
    [list(false, 1, &["dash item", "star item", "plus item", "bullet item"])]
  );
  assert_eq!(body_blocks("1. first\n2. second"), [list(true, 1, &["first", "second"])]);
  assert_eq!(body_blocks("3) third\n4) fourth"), [list(true, 3, &["third", "fourth"])]);
  assert_eq!(body_blocks("- bullet\n1. number"), [list(false, 1, &["bullet"]), list(true, 1, &["number"])]);
  // a wrapped, an indented and an unindented line continue the item; a heading ends the list
  assert_eq!(
    body_blocks("- a long item that git\n  wrapped here\n- next item\nunindented continuation\n    indented too\n## Next"),
    [
      list(false, 1, &["a long item that git wrapped here", "next item unindented continuation indented too"]),
      paragraph(&["## Next"])
    ]
  );
  // a rule is no item
  assert_eq!(body_blocks("- - -"), [paragraph(&["- - -"])]);
}

#[test]
fn keeps_code_verbatim_without_the_common_indent_and_the_fences() {
  assert_eq!(
    body_blocks("Run it with:\n    pnpm test:unit\n      --watch"),
    [paragraph(&["Run it with:"]), preformatted("pnpm test:unit\n  --watch")]
  );
  assert_eq!(body_blocks("\tfirst()\n\tsecond()"), [preformatted("first()\nsecond()")]);
  assert_eq!(
    body_blocks("Before.\n```ts\nconst a = 1\n\nconst b = 2\n```\nAfter."),
    [paragraph(&["Before."]), preformatted("const a = 1\n\nconst b = 2"), paragraph(&["After."])]
  );
  assert_eq!(body_blocks("~~~\nwith ``` inside\n~~~"), [preformatted("with ``` inside")]);
  // an unclosed fence runs to the end of the body
  assert_eq!(body_blocks("```\nfirst\n\nsecond\n"), [preformatted("first\n\nsecond")]);
}

#[test]
fn reads_a_last_block_of_trailers_with_one_that_only_a_trailer_names() {
  assert_eq!(
    body_blocks("Body.\n\nSigned-off-by: Ann <ann@example.com>\nCo-Authored-By: Bob <bob@example.com>"),
    [
      paragraph(&["Body."]),
      trailers(&["Signed-off-by: Ann <ann@example.com>", "Co-Authored-By: Bob <bob@example.com>"])
    ]
  );
  // an indented line continues its trailer
  assert_eq!(
    body_blocks("Body.\n\nFixes: a long trailer value\n  that continues\nSigned-off-by: Ann <ann@example.com>"),
    [
      paragraph(&["Body."]),
      trailers(&["Fixes: a long trailer value that continues", "Signed-off-by: Ann <ann@example.com>"])
    ]
  );
  assert_eq!(
    body_blocks("Body.\n\n(cherry picked from commit 0123abcd)\nSigned-off-by: Ann <ann@example.com>"),
    [
      paragraph(&["Body."]),
      trailers(&["(cherry picked from commit 0123abcd)", "Signed-off-by: Ann <ann@example.com>"])
    ]
  );
}

#[test]
fn keeps_as_prose_a_trailer_shaped_block_that_is_not_the_last_one_has_prose_or_only_one_word_tokens() {
  assert_eq!(
    body_blocks("Signed-off-by: Ann <ann@example.com>\n\nThe body."),
    [paragraph(&["Signed-off-by: Ann <ann@example.com>"]), paragraph(&["The body."])]
  );
  assert_eq!(body_blocks("Fixes: JIRA-1\nand some prose"), [paragraph(&["Fixes: JIRA-1 and some prose"])]);
  assert_eq!(
    body_blocks("The body.\n\nNote: the session ends."),
    [paragraph(&["The body."]), paragraph(&["Note: the session ends."])]
  );
}

#[test]
fn the_body_of_a_message_is_after_its_subject_line() {
  assert_eq!(message_body("Subject\n\nBody"), "\nBody");
  assert_eq!(body_blocks(message_body("Subject\n\n\nBody")), [paragraph(&["Body"])]);
  assert_eq!(message_body("Subject"), "");
}
