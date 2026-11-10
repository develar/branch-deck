use crate::prose::{ParagraphText, ProseChange, ProseForm, ProseLink, ProseParagraph, TextRange, front_matter_end, paragraph_spans, paragraphs, prose_changes, strip_links};
use crate::units::UnitBlock;
use pretty_assertions::assert_eq;
use std::fmt::Write;

fn prose_text(text: &str) -> ParagraphText {
  ParagraphText {
    text: text.to_string(),
    form: ProseForm::Prose,
  }
}

fn block(index: u32, old: (u32, u32), new: (u32, u32)) -> UnitBlock {
  UnitBlock {
    index,
    old_start: old.0,
    old_end: old.1,
    new_start: new.0,
    new_end: new.1,
    additions: new.1 - new.0,
    deletions: old.1 - old.0,
  }
}

/// A paragraph as a reader sees it: `+ ` added, `- ` removed, a changed one with what the commits wrote in `{+…+}` and
/// what they removed in `{-…-}`
fn render(paragraph: &ProseParagraph) -> String {
  match &paragraph.change {
    ProseChange::Added => format!("+ {}", paragraph.text),
    ProseChange::Removed => format!("- {}", paragraph.text),
    ProseChange::Changed { inserted, deleted, .. } => {
      let units: Vec<u16> = paragraph.text.encode_utf16().collect();
      let mut out = String::from("~ ");
      for at in 0..=units.len() {
        let at32 = u32::try_from(at).unwrap();
        for deletion in deleted.iter().filter(|deletion| deletion.at == at32) {
          write!(out, "{{-{}-}}", deletion.text).unwrap();
        }
        if inserted.iter().any(|range| range.start == at32) {
          out.push_str("{+");
        }
        if inserted.iter().any(|range| range.end == at32) {
          out.push_str("+}");
        }
        if let Some(&unit) = units.get(at) {
          out.push_str(&String::from_utf16_lossy(&[unit]));
        }
      }
      out.replace("+}{+", "")
    }
    ProseChange::Substituted { .. } => format!("~ {}", paragraph.text),
  }
}

/// The paragraphs a file of prose changes, as the brief reads them: each block widened to whole paragraphs, then
/// paired by word
fn changes_of(old: &str, new: &str, blocks: &[UnitBlock]) -> Vec<ProseParagraph> {
  let old: Vec<&str> = old.split('\n').collect();
  let new: Vec<&str> = new.split('\n').collect();
  paragraph_spans(&old, &new, blocks)
    .into_iter()
    .flat_map(|(old_range, new_range, _)| prose_changes(&paragraphs(&old[old_range]), &paragraphs(&new[new_range]), "spec/a.spec.md"))
    .collect()
}

/// The paragraphs a file of prose changes (`changes_of`) as a reader sees them (`render`)
fn changes(old: &str, new: &str, blocks: &[UnitBlock]) -> Vec<String> {
  changes_of(old, new, blocks).iter().map(render).collect()
}

const WRAPPED: &str = "# Storage\n\nThe window keeps its layout in a file of the config directory, so a checkout never\nchanges it. The file holds the side and the size of each tool window. It is written\nwhen the window closes.\n\nThe store keeps at most 500 projects.\n";

/// A paragraph wrapped at a fixed width and changed in its middle line is the whole paragraph once, what the commits
/// wrote and removed in it by word: never a fragment that starts and ends inside a sentence
#[test]
fn a_hard_wrapped_paragraph_changed_in_one_line_is_the_whole_paragraph_by_word() {
  let new = WRAPPED.replace("the side and the size of each", "the side, the size and the focus of each");
  assert_eq!(
    changes(WRAPPED, &new, &[block(0, (3, 4), (3, 4))]),
    [
      "~ The window keeps its layout in a file of the config directory, so a checkout never changes it. The file holds the side{+, the size+} and the {-size-}{+focus+} of each tool window. It is written when the window closes."
    ]
  );
}

/// A paragraph wrapped anew says the same: no change
#[test]
fn a_paragraph_wrapped_anew_is_no_change() {
  let new = WRAPPED.replace("so a checkout never\nchanges it.", "so a checkout\nnever changes it.");
  assert_eq!(changes(WRAPPED, &new, &[block(0, (2, 4), (2, 4))]), Vec::<String>::new());
}

/// A paragraph the commits add between two others is added alone; lines appended to a paragraph change it
#[test]
fn a_new_paragraph_is_added_alone_and_lines_appended_to_one_change_it() {
  let added = WRAPPED.replace("\nThe store keeps", "\nThe focus is a fact the window derives.\n\nThe store keeps");
  assert_eq!(changes(WRAPPED, &added, &[block(0, (6, 6), (6, 8))]), ["+ The focus is a fact the window derives."]);
  let appended = WRAPPED.replace("at most 500 projects.\n", "at most 500 projects. A write above the bound drops\nthe oldest.\n");
  assert_eq!(
    changes(WRAPPED, &appended, &[block(0, (6, 7), (6, 8))]),
    ["~ The store keeps at most 500 projects.{+ A write above the bound drops the oldest.+}"]
  );
  // a line inserted inside a paragraph widens both sides to it
  let inside = WRAPPED.replace("It is written\nwhen", "It is written\nonce,\nwhen");
  assert_eq!(
    changes(WRAPPED, &inside, &[block(0, (4, 4), (4, 5))]),
    [
      "~ The window keeps its layout in a file of the config directory, so a checkout never changes it. The file holds the side and the size of each tool window. It is written{+ once,+} when the window closes."
    ]
  );
}

/// A paragraph rewritten whole is the old one removed and the new one added; two blocks of one paragraph are one place
#[test]
fn a_rewritten_paragraph_is_removed_and_added_and_two_blocks_of_one_paragraph_are_one() {
  let rewritten = WRAPPED.replace("The store keeps at most 500 projects.", "Projects past the bound lose their layouts first.");
  assert_eq!(
    changes(WRAPPED, &rewritten, &[block(0, (6, 7), (6, 7))]),
    ["- The store keeps at most 500 projects.", "+ Projects past the bound lose their layouts first."]
  );
  let both = WRAPPED
    .replace("config directory", "IDE's config directory")
    .replace("window closes", "window closes for good");
  let old: Vec<&str> = WRAPPED.split('\n').collect();
  let new: Vec<&str> = both.split('\n').collect();
  assert_eq!(
    paragraph_spans(&old, &new, &[block(0, (2, 3), (2, 3)), block(1, (4, 5), (4, 5))]),
    [(2..5, 2..5, vec![0, 1])]
  );
}

/// A paragraph added before a changed one does not take its place: the changed one pairs with its old self
#[test]
fn a_paragraph_added_before_a_changed_one_leaves_it_its_pair() {
  let old = [
    prose_text("The store keeps at most 500 projects."),
    prose_text("An older build ignores the order of the tool windows."),
  ];
  let new = [
    prose_text("The store keeps at most 500 projects."),
    prose_text("The focus is a fact the window derives, and `FocusRecorder` writes it."),
    prose_text("An older build ignores the order and the focus of the tool windows."),
  ];
  let changes = prose_changes(&old, &new, "spec/a.spec.md");
  assert_eq!(
    changes.iter().map(render).collect::<Vec<_>>(),
    [
      "+ The focus is a fact the window derives, and `FocusRecorder` writes it.",
      "~ An older build ignores the order{+ and the focus+} of the tool windows."
    ]
  );
  assert_eq!(changes[0].code.len(), 1, "its code is set as code");
}

/// The `[@test]` lines of an item are not its prose (the tests they name: `brief::proven_by`)
#[test]
fn the_test_lines_of_an_item_are_no_prose() {
  let texts = paragraphs(&[
    "- The window keeps its layout across a restart, and the project view does",
    "  not take its place.",
    "  [@test] ../tests/restart.json",
  ]);
  assert_eq!(
    texts,
    vec![prose_text("The window keeps its layout across a restart, and the project view does not take its place.")]
  );
}

#[test]
fn a_fenced_block_is_one_paragraph_with_its_lines_preserved() {
  let old_cli = concat!("b", "d");
  let before_markdown = format!("```sh\n{old_cli} workspace -C <repo>\necho [@test] not-a-test\n```");
  let after_markdown = "```sh\nbrd workspace -C <repo>\necho [@test] not-a-test\n```";
  let before_lines: Vec<&str> = before_markdown.lines().collect();
  let after_lines: Vec<&str> = after_markdown.lines().collect();
  let before = paragraphs(&before_lines);
  let after = paragraphs(&after_lines);
  assert_eq!(
    crate::brief::proven_by("docs/guide.md", &after_lines),
    Vec::<String>::new(),
    "a fenced `[@test]` names no test"
  );
  assert_eq!(before[0].text, format!("{old_cli} workspace -C <repo>\necho [@test] not-a-test"));
  assert_eq!(before[0].form, ProseForm::Code { language: Some("sh".to_string()) });

  let changed = prose_changes(&before, &after, "docs/guide.md");
  assert_eq!(changed.len(), 1, "the changed block is one paragraph");
  assert_eq!(changed[0].form, ProseForm::Code { language: Some("sh".to_string()) });
  assert_eq!(changed[0].code, Vec::new());
  assert_eq!(changed[0].links, Vec::new());
  let ProseChange::Changed { inserted, deleted, .. } = &changed[0].change else {
    panic!("the command change is word-diffed");
  };
  assert_eq!(
    inserted.iter().map(|range| &changed[0].text[range.start as usize..range.end as usize]).collect::<Vec<_>>(),
    ["brd"]
  );
  assert_eq!(deleted.iter().map(|deletion| deletion.text.as_str()).collect::<Vec<_>>(), [old_cli]);
}

#[test]
fn a_long_code_block_shows_changed_lines_with_one_line_of_context() {
  let before: Vec<String> = (0..12).map(|index| format!("command-{index} --old")).collect();
  let mut after = before.clone();
  after[3] = after[3].replace("old", "new");
  after[8] = after[8].replace("old", "new");
  let paragraph = |lines: &[String]| ParagraphText {
    text: lines.join("\n"),
    form: ProseForm::Code { language: Some("sh".to_string()) },
  };
  let changed = prose_changes(&[paragraph(&before)], &[paragraph(&after)], "docs/guide.md");
  let excerpt = changed[0].excerpt.expect("multiple changed lines in a long code block have an excerpt");
  assert_eq!(
    &changed[0].text[excerpt.start as usize..excerpt.end as usize],
    "command-2 --old\ncommand-3 --new\ncommand-4 --old\ncommand-5 --old\ncommand-6 --old\ncommand-7 --old\ncommand-8 --new\ncommand-9 --old\n"
  );
}

/// A link of Markdown reads as its text, its target a path of the repository resolved against the file, or a URL as
/// written; an image and a link inside code stay as written
#[test]
fn a_link_reads_as_its_text_with_its_target() {
  let (text, links) = strip_links(
    "[Issue Links](../sessions/issue-links.spec.md#rules) owns them, see [the docs](https://example.com/a) and `[a](b)` or ![icon](i.png).",
    "plugins/air/spec/core",
  );
  assert_eq!(text, "Issue Links owns them, see the docs and `[a](b)` or ![icon](i.png).");
  assert_eq!(
    links,
    [
      ProseLink {
        start: 0,
        end: 11,
        target: "plugins/air/spec/sessions/issue-links.spec.md".to_string()
      },
      ProseLink {
        start: 27,
        end: 35,
        target: "https://example.com/a".to_string()
      }
    ]
  );
}

/// A paragraph of several sentences changed in one shows that sentence first (`excerpt`), a short one of two
/// sentences too; one of one sentence, or one changed throughout, shows whole
#[test]
fn a_paragraph_changed_in_one_of_its_sentences_shows_it_first() {
  let sentence = |index: usize| format!("Sentence {index} says one more thing about the layout of the window and its store.");
  let before: Vec<String> = (0..6).map(sentence).collect();
  let mut after = before.clone();
  after[3] = after[3].replace("one more thing", "one other thing");
  let excerpt_of = |before: &[String], after: &[String]| prose_changes(&[prose_text(&before.join(" "))], &[prose_text(&after.join(" "))], "a.md")[0].excerpt;
  let text = after.join(" ");
  let start = u32::try_from(text.find("Sentence 3").unwrap()).unwrap();
  assert_eq!(
    excerpt_of(&before, &after),
    Some(TextRange {
      start,
      end: start + u32::try_from(after[3].len()).unwrap()
    })
  );
  let mut one = before[..1].to_vec();
  one[0] = one[0].replace("one more thing", "one other thing");
  assert_eq!(excerpt_of(&before[..1], &one), None, "a paragraph of one sentence shows whole");
  let mut throughout = before.clone();
  throughout[0] = throughout[0].replace("one more", "a further");
  throughout[5] = throughout[5].replace("one more", "a further");
  assert_eq!(excerpt_of(&before, &throughout), None, "changes at both ends leave out too little");
}

/// A short paragraph of two sentences changed in its second shows that sentence first: no length makes a paragraph
/// worth quoting whole when one sentence of it changed
#[test]
fn a_short_paragraph_of_two_sentences_changed_in_one_shows_that_sentence() {
  let before = "The store keeps at most 500 projects. A write above the bound drops the layouts of the oldest.";
  let after = "The store keeps at most 500 projects. A write above the bound drops the layouts of the oldest that pin no tool window.";
  let changed = prose_changes(&[prose_text(before)], &[prose_text(after)], "spec/a.spec.md");
  let excerpt = changed[0].excerpt.expect("the changed sentence alone");
  assert_eq!(
    (render(&changed[0]), &after[excerpt.start as usize..excerpt.end as usize]),
    (
      "~ The store keeps at most 500 projects. A write above the bound drops the layouts of the oldest{+ that pin no tool window+}.".to_string(),
      "A write above the bound drops the layouts of the oldest that pin no tool window."
    )
  );
}

/// The front matter of a file is its lines from its first `---` up to the next one; a file without one, or with a
/// `---` never closed, has none
#[test]
fn the_front_matter_is_the_lines_up_to_its_closing_rule() {
  assert_eq!(front_matter_end(&["---", "name: Window", "targets:", "  - ../../src/Window.kt", "---", "", "# Window"]), 5);
  assert_eq!(front_matter_end(&["# Window", "", "---", "The rule."]), 0);
  assert_eq!(front_matter_end(&["---", "The rule."]), 0);
}

/// A thematic break ends a paragraph and is no prose of its own
#[test]
fn a_thematic_break_ends_a_paragraph() {
  let texts = paragraphs(&["The window opens.", "---", "The window closes.", "***", "_ _ _", "The store keeps 500."]);
  assert_eq!(
    texts,
    [prose_text("The window opens."), prose_text("The window closes."), prose_text("The store keeps 500.")]
  );
}

/// A line of metadata (one key, then references and paths) ends a paragraph and is no prose: a change of it alone
/// changes no paragraph; a label, or a key followed by words, is prose
#[test]
fn a_line_of_metadata_is_no_prose() {
  let texts = paragraphs(&[
    "The window opens.",
    "refs: flow:layout, spec:window, flag:restore",
    "targets: ../../src/Window.kt, ../../src/Layout.kt",
    "Look closely:",
    "Note: the store keeps 500.",
  ]);
  assert_eq!(texts, [prose_text("The window opens."), prose_text("Look closely: Note: the store keeps 500.")]);
  let old = "# Window\n\nThe window opens.\n\nrefs: flow:layout, spec:window\n";
  let new = "# Window\n\nThe window opens.\n\nrefs: flow:layout, spec:window, spec:focus\n";
  assert_eq!(changes(old, new, &[block(0, (4, 5), (4, 5))]), Vec::<String>::new());
}

const TABLE: &str = "# Ownership\n\n| Topic | Owning spec |\n| --- | :---: |\n| Window | [Window](window.spec.md) |\n| Store | `store.spec.md` |\n\nThe end.\n";

/// A table is its header row and each of its rows, a row one paragraph under the cells of its header: a row the change
/// adds is that row alone, its header with it; the header and the rows it keeps are no change
#[test]
fn a_table_with_one_added_row_is_its_header_and_that_row() {
  let new = TABLE.replace("| Store |", "| Focus | the focus spec |\n| Store |");
  let old_lines: Vec<&str> = TABLE.split('\n').collect();
  let new_lines: Vec<&str> = new.split('\n').collect();
  let spans = paragraph_spans(&old_lines, &new_lines, &[block(0, (5, 5), (5, 6))]);
  let [(old_range, new_range, _)] = spans.as_slice() else {
    panic!("one place: {spans:?}");
  };
  assert_eq!((old_range.clone(), new_range.clone()), (2..6, 2..7), "the whole table on both sides");
  let changed = prose_changes(&paragraphs(&old_lines[2..6]), &paragraphs(&new_lines[new_range.clone()]), "spec/ownership.spec.md");
  assert_eq!(
    changed,
    [ProseParagraph {
      text: "Topic | Owning spec\nFocus | the focus spec".to_string(),
      form: ProseForm::Row {
        header: vec![TextRange { start: 0, end: 5 }, TextRange { start: 8, end: 19 }],
        cells: vec![TextRange { start: 20, end: 25 }, TextRange { start: 28, end: 42 }],
      },
      code: Vec::new(),
      links: Vec::new(),
      change: ProseChange::Added,
      excerpt: None,
    }]
  );
}

/// A row changed in a word is that row by word, the words of its cells alone; its links read as their text and its code
/// is the code of its cell
#[test]
fn a_changed_row_is_diffed_by_word_in_its_cells() {
  let new = TABLE.replace("[Window](window.spec.md)", "[Window Layout](window.spec.md)");
  let changed: Vec<ProseParagraph> = changes_of(TABLE, &new, &[block(0, (4, 5), (4, 5))]);
  assert_eq!(changed.iter().map(render).collect::<Vec<_>>(), ["~ Topic | Owning spec\nWindow | Window{+ Layout+}"]);
  assert_eq!(
    changed[0].links,
    [ProseLink {
      start: 29,
      end: 42,
      target: "spec/window.spec.md".to_string()
    }]
  );
  let code = TABLE.replace("`store.spec.md`", "`store.spec.md` and `focus.spec.md`");
  let changed = changes_of(TABLE, &code, &[block(0, (5, 6), (5, 6))]);
  let ProseForm::Row { cells, .. } = &changed[0].form else {
    panic!("a row: {changed:?}");
  };
  assert!(
    changed[0].code.iter().all(|span| span.start >= cells[1].start && span.end <= cells[1].end) && changed[0].code.len() == 2,
    "the code of the cell: {:?}",
    changed[0].code
  );
}

/// A line of pipes with no delimiter row below it is no table: it reads as prose
#[test]
fn a_line_of_pipes_without_a_delimiter_row_is_prose() {
  let texts = paragraphs(&["Run a | b to pipe.", "The | stays."]);
  assert_eq!(texts, [prose_text("Run a | b to pipe. The | stays.")]);
}
