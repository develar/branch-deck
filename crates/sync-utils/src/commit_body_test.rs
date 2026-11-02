use crate::commit_body::{commit_body, has_body, message_body};
use domain_types::commit_body::{CommitBody, CommitBodyBlock, LabelKind};
use pretty_assertions::assert_eq;

fn blocks(body: &str) -> Vec<CommitBodyBlock> {
  commit_body(body).blocks
}

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

fn heading(level: u8, text: &str) -> CommitBodyBlock {
  CommitBodyBlock::Heading { level, text: text.to_string() }
}

/// A plain label on a line of its own
fn label(text: &str) -> CommitBodyBlock {
  labelled(text, LabelKind::Plain)
}

/// A label of `kind` on a line of its own
fn labelled(text: &str, kind: LabelKind) -> CommitBodyBlock {
  CommitBodyBlock::Label {
    text: text.to_string(),
    kind,
    run_in: false,
  }
}

/// A plain label that starts its paragraph
fn run_in(text: &str) -> CommitBodyBlock {
  run_in_labelled(text, LabelKind::Plain)
}

/// A label of `kind` that starts its paragraph
fn run_in_labelled(text: &str, kind: LabelKind) -> CommitBodyBlock {
  CommitBodyBlock::Label {
    text: text.to_string(),
    kind,
    run_in: true,
  }
}

fn strings(lines: &[&str]) -> Vec<String> {
  lines.iter().map(ToString::to_string).collect()
}

/// A paragraph of `lines` lines of 100 characters: the lines the fold counts
fn lines_of_prose(lines: usize) -> String {
  "word ".repeat(20 * lines).trim_end().to_string()
}

fn all(body: &CommitBody) -> u32 {
  u32::try_from(body.blocks.len()).unwrap()
}

#[test]
fn joins_hard_wrapped_lines_into_one_paragraph_and_starts_a_block_after_a_blank_line() {
  assert_eq!(
    blocks("The pool size was too small\nfor the nightly import."),
    [paragraph(&["The pool size was too small for the nightly import."])]
  );
  assert_eq!(
    blocks("First paragraph.\n\nSecond paragraph."),
    [paragraph(&["First paragraph."]), paragraph(&["Second paragraph."])]
  );
  assert_eq!(
    blocks("First line\r\nsecond line\r\n\r\n- item"),
    [paragraph(&["First line second line"]), list(false, 1, &["item"])]
  );
}

#[test]
fn keeps_a_line_after_a_hard_break_a_quote_line_and_a_table_row_its_own_and_a_heading_and_a_rule_a_block_of_their_own() {
  assert_eq!(
    blocks("A hard break  \nafter it, and another\\\nafter it."),
    [paragraph(&["A hard break", "after it, and another", "after it."])]
  );
  assert_eq!(
    blocks("Intro:\n| a | table |\n| - | ----- |\n> a quote\n> of two lines\nmore"),
    [paragraph(&["Intro:", "| a | table |", "| - | ----- |", "> a quote", "> of two lines", "more"])]
  );
  assert_eq!(
    blocks("Before\n## A heading\nunder it\n---\nafter it\n\n# Top\n###### Deepest"),
    [
      paragraph(&["Before"]),
      heading(2, "A heading"),
      paragraph(&["under it"]),
      CommitBodyBlock::Rule,
      paragraph(&["after it"]),
      heading(1, "Top"),
      heading(6, "Deepest"),
    ]
  );
  // a `#` without text and seven of them are prose; `#1` is no heading
  assert_eq!(
    blocks("#\n\n####### seven\n\n#1 first"),
    [paragraph(&["#"]), paragraph(&["####### seven"]), paragraph(&["#1 first"])]
  );
}

#[test]
fn reads_lists_without_their_markers_each_kind_its_own() {
  assert_eq!(
    blocks("- dash item\n* star item\n+ plus item\n• bullet item"),
    [list(false, 1, &["dash item", "star item", "plus item", "bullet item"])]
  );
  assert_eq!(blocks("1. first\n2. second"), [list(true, 1, &["first", "second"])]);
  assert_eq!(blocks("3) third\n4) fourth"), [list(true, 3, &["third", "fourth"])]);
  assert_eq!(blocks("- bullet\n1. number"), [list(false, 1, &["bullet"]), list(true, 1, &["number"])]);
  // a wrapped, an indented and an unindented line continue the item; a heading ends the list
  assert_eq!(
    blocks("- a long item that git\n  wrapped here\n- next item\nunindented continuation\n    indented too\n## Next"),
    [
      list(false, 1, &["a long item that git wrapped here", "next item unindented continuation indented too"]),
      heading(2, "Next")
    ]
  );
  // a rule is no item
  assert_eq!(blocks("- - -"), [CommitBodyBlock::Rule]);
}

#[test]
fn keeps_code_verbatim_without_the_common_indent_and_the_fences() {
  assert_eq!(
    blocks("Run it with:\n    pnpm test:unit\n      --watch"),
    [label("Run it with"), preformatted("pnpm test:unit\n  --watch")]
  );
  assert_eq!(blocks("\tfirst()\n\tsecond()"), [preformatted("first()\nsecond()")]);
  assert_eq!(
    blocks("Before.\n```ts\nconst a = 1\n\nconst b = 2\n```\nAfter."),
    [paragraph(&["Before."]), preformatted("const a = 1\n\nconst b = 2"), paragraph(&["After."])]
  );
  assert_eq!(blocks("~~~\nwith ``` inside\n~~~"), [preformatted("with ``` inside")]);
  // an unclosed fence runs to the end of the body
  assert_eq!(blocks("```\nfirst\n\nsecond\n"), [preformatted("first\n\nsecond")]);
}

#[test]
fn reads_a_last_block_of_trailers_with_one_that_only_a_trailer_names() {
  let body = commit_body("Body.\n\nSigned-off-by: Ann <ann@example.com>\nCo-Authored-By: Bob <bob@example.com>");
  assert_eq!(body.blocks, [paragraph(&["Body."])]);
  assert_eq!(body.trailers, strings(&["Signed-off-by: Ann <ann@example.com>", "Co-Authored-By: Bob <bob@example.com>"]));
  // an indented line continues its trailer
  assert_eq!(
    commit_body("Body.\n\nFixes: a long trailer value\n  that continues\nSigned-off-by: Ann <ann@example.com>").trailers,
    strings(&["Fixes: a long trailer value that continues", "Signed-off-by: Ann <ann@example.com>"])
  );
  assert_eq!(
    commit_body("Body.\n\n(cherry picked from commit 0123abcd)\nSigned-off-by: Ann <ann@example.com>").trailers,
    strings(&["(cherry picked from commit 0123abcd)", "Signed-off-by: Ann <ann@example.com>"])
  );
  // a body of trailers alone has no block
  let only = commit_body("Co-Authored-By: Claude Opus <noreply@anthropic.com>");
  assert_eq!((only.blocks.len(), only.trailers.len(), only.lead, only.shown), (0, 1, false, 0));
}

#[test]
fn keeps_as_prose_a_trailer_shaped_block_that_is_not_the_last_one_has_prose_or_only_one_word_tokens() {
  let first = commit_body("Signed-off-by: Ann <ann@example.com>\n\nThe body.");
  assert_eq!(first.blocks, [paragraph(&["Signed-off-by: Ann <ann@example.com>"]), paragraph(&["The body."])]);
  assert_eq!(first.trailers, Vec::<String>::new());
  // prose, and a one-word token alone: a run-in label
  assert_eq!(blocks("Fixes: JIRA-1\nand some prose"), [run_in("Fixes"), paragraph(&["JIRA-1 and some prose"])]);
  assert_eq!(
    blocks("The body.\n\nNote: the session ends."),
    [paragraph(&["The body."]), run_in("Note"), paragraph(&["the session ends."])]
  );
}

#[test]
fn the_body_of_a_message_is_after_its_subject_line() {
  assert_eq!(message_body("Subject\n\nBody"), "\nBody");
  assert_eq!(blocks(message_body("Subject\n\n\nBody")), [paragraph(&["Body"])]);
  assert_eq!(message_body("Subject"), "");
}

#[test]
fn a_message_has_a_body_with_a_block_after_its_subject_that_is_no_trailers() {
  for message in [
    "Subject\n\nWhy it changed",
    "Subject\n\nWhy\n\nCo-Authored-By: Claude <noreply@anthropic.com>",
    "Subject\n\n```\nlog\n```",
    "Subject\r\n\r\nWhy",
  ] {
    assert!(has_body(message), "{message:?}");
  }
  for message in [
    "",
    "Subject",
    "Subject\n",
    "Subject\n\n  \n",
    "Subject\n\nCo-Authored-By: Claude <noreply@anthropic.com>\nSigned-off-by: A <a@b.c>",
  ] {
    assert!(!has_body(message), "{message:?}");
  }
  // a one-word token alone is prose, as `commit_body` reads it
  assert!(has_body("Subject\n\nNote: read this"));
  assert_eq!(blocks("Note: read this"), [run_in("Note"), paragraph(&["read this"])]);
}

#[test]
fn a_paragraph_that_is_only_a_label_heads_the_next_blocks_and_a_short_run_in_label_the_rest_of_its_paragraph() {
  let list_of_two = list(false, 1, &["one", "two"]);
  assert_eq!(blocks("What changed:\n\n- one\n- two"), [label("What changed"), list_of_two.clone()]);
  assert_eq!(
    blocks("Gates on the final tree, each exit 0:\n\n- one\n- two"),
    [labelled("Gates on the final tree, each exit 0", LabelKind::Verification), list_of_two.clone()]
  );
  // the lines of a run-in paragraph stay after its label
  assert_eq!(
    blocks("Why: the pool\nran dry.  \nThen more."),
    [run_in("Why"), paragraph(&["the pool ran dry.", "Then more."])]
  );
  assert_eq!(
    blocks("Left out: the cache."),
    [run_in_labelled("Left out", LabelKind::Verification), paragraph(&["the cache."])]
  );
  // a label of any script, its run-in length in characters
  assert_eq!(blocks("Причина: пул иссяк."), [run_in("Причина"), paragraph(&["пул иссяк."])]);
  assert_eq!(blocks("Почему так сделано: пул."), [run_in("Почему так сделано"), paragraph(&["пул."])]);
  assert_eq!(blocks("Änderungen:\n\n- eins"), [label("Änderungen"), list(false, 1, &["eins"])]);
  assert_eq!(blocks("Résultats: tout passe."), [run_in("Résultats"), paragraph(&["tout passe."])]);
  // the unlabelled blocks after a label are its blocks, as they are
  assert_eq!(
    blocks("Intro.\n\nWhy: the pool ran dry.\n\nMore detail.\n\n- one\n- two"),
    [
      paragraph(&["Intro."]),
      run_in("Why"),
      paragraph(&["the pool ran dry."]),
      paragraph(&["More detail."]),
      list_of_two
    ]
  );
}

#[test]
fn a_label_without_blocks_to_head_a_long_one_and_a_sentence_stay_prose() {
  // the last block, one followed only by trailers, one before a heading
  assert_eq!(blocks("Notes:"), [paragraph(&["Notes:"])]);
  assert_eq!(commit_body("Notes:\n\nSigned-off-by: Ann <ann@example.com>").blocks, [paragraph(&["Notes:"])]);
  assert_eq!(blocks("Notes:\n\n## Next"), [paragraph(&["Notes:"]), heading(2, "Next")]);
  // a run-in label longer than 20 characters, with a capitalised second word, with a lowercase first one
  assert_eq!(blocks("Something rather long: the text."), [paragraph(&["Something rather long: the text."])]);
  assert_eq!(blocks("Air Settings: the text."), [paragraph(&["Air Settings: the text."])]);
  assert_eq!(blocks("why: the text."), [paragraph(&["why: the text."])]);
  // a lead-in of 40 characters, and one with a colon inside
  let long = format!("{}:", "A".repeat(40));
  assert_eq!(blocks(&format!("{long}\n\n- one")), [paragraph(&[long.as_str()]), list(false, 1, &["one"])]);
  assert_eq!(blocks("At 10:30:\n\n- one"), [paragraph(&["At 10:30:"]), list(false, 1, &["one"])]);
}

#[test]
fn a_label_line_right_before_a_list_heads_it_after_prose_or_another_list() {
  assert_eq!(
    blocks("The fix moves the check.\nThe cause:\n- the cache was shared\nTests:\n- the new test\n1. Gates:\n2. kept"),
    [
      paragraph(&["The fix moves the check."]),
      label("The cause"),
      list(false, 1, &["the cache was shared"]),
      labelled("Tests", LabelKind::Verification),
      list(false, 1, &["the new test"]),
      list(true, 1, &["Gates:", "kept"]),
    ]
  );
  // an indented `Label:` continues its item, a lowercase line too
  assert_eq!(
    blocks("- one\n  Note:\n- two\nand then:\n- three"),
    [list(false, 1, &["one Note:", "two and then:", "three"])]
  );
}

#[test]
fn a_label_is_of_the_kind_its_words_name_the_case_a_leading_the_and_a_comma_aside() {
  let kind = |text: &str| match blocks(&format!("{text}:\n- item")).first() {
    Some(CommitBodyBlock::Label { kind, .. }) => *kind,
    other => panic!("{text} is no label: {other:?}"),
  };
  for text in [
    "Tests",
    "Test",
    "Evidence",
    "The evidence",
    "Evidence, on the final code",
    "Gates",
    "Verified",
    "Verification",
    "Results",
    "Proof",
    "Left out",
    "Tests run",
  ] {
    assert_eq!(kind(text), LabelKind::Verification, "{text}");
  }
  assert_eq!(kind("Look closely"), LabelKind::LookClosely);
  for text in ["The fix", "Fix", "The cause", "Testing notes", "Theory"] {
    assert_eq!(kind(text), LabelKind::Plain, "{text}");
  }
  assert_eq!(
    blocks("Look closely: the lock."),
    [run_in_labelled("Look closely", LabelKind::LookClosely), paragraph(&["the lock."])]
  );
}

#[test]
fn the_lead_is_a_paragraph_before_any_label_or_heading() {
  assert!(commit_body("The summary.\n\nWhy: more.").lead);
  assert!(!commit_body("Why: the summary.").lead);
  assert!(!commit_body("## Summary\n\nThe text.").lead);
  assert!(!commit_body("- an item").lead);
  assert!(!commit_body("").lead);
}

#[test]
fn a_short_body_shows_whole_and_a_long_one_folds_at_a_block_after_about_twelve_lines() {
  let short = commit_body("The summary.\n\nWhy: more.\n\n- one\n- two");
  assert_eq!(short.shown, all(&short));
  assert_eq!(commit_body("").shown, 0);
  // 4 + 4 + 4 lines fit, the fourth paragraph folds, and the one after it with it
  assert_eq!(commit_body(&[4, 4, 4, 1, 1].map(lines_of_prose).join("\n\n")).shown, 3);
  // the lead shows however long it is
  assert_eq!(commit_body(&[20, 1].map(lines_of_prose).join("\n\n")).shown, 1);
  // an item and a line of code count one line each, an item its wraps: 1 + 7 + 2 + 2 fit, the line after them folds
  let items = format!("The summary.\n\n{}- {}\n\n    a\n    b\n\nAfter.", "- item\n".repeat(7), lines_of_prose(2));
  assert_eq!(commit_body(&items).shown, 3);
}

#[test]
fn a_fold_never_cuts_a_section_off_its_label_and_a_verification_section_costs_no_line() {
  let body = format!(
    "{}\n\nThe cause:\n\n{}\n\n{}\n\nThe fix:\n\n{}",
    lines_of_prose(2),
    lines_of_prose(3),
    lines_of_prose(3),
    lines_of_prose(5)
  );
  let body = commit_body(&body);
  // the lead (2) and the cause (1 + 3 + 3) fit; the fix (1 + 5) would end at 15: it folds whole, label and all
  assert_eq!(body.shown, 4);
  assert_eq!(body.blocks[4], label("The fix"));
  // a section that does not fit folds whole, though its label and first paragraph alone would
  let big = commit_body(&format!("{}\n\nThe cause:\n\n{}\n\n{}", lines_of_prose(2), lines_of_prose(6), lines_of_prose(6)));
  assert_eq!(big.shown, 1);
  // a body that opens with a section shows it whole however long it is, as it would its first block
  assert_eq!(commit_body(&format!("Why: {}", lines_of_prose(20))).shown, 2);
  let summary = commit_body(&format!(
    "## Summary\n\n{}\n\n{}\n\nThe fix:\n\n{}",
    lines_of_prose(10),
    lines_of_prose(10),
    lines_of_prose(1)
  ));
  assert_eq!(summary.shown, 3);
  assert_eq!(summary.blocks[3], label("The fix"));
  // the tests in between cost nothing: the fix after them still shows
  let tests = format!("{}\n\nTests:\n{}\nThe fix:\n\n{}", lines_of_prose(4), "- a test\n".repeat(20), lines_of_prose(6));
  let tests = commit_body(&tests);
  assert_eq!(tests.shown, all(&tests));
  assert_eq!(tests.blocks[1], labelled("Tests", LabelKind::Verification));
  assert_eq!(tests.blocks[3], label("The fix"));
}
