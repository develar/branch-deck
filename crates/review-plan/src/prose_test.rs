use crate::prose::{ProseChange, ProseLink, ProseParagraph, TextRange, paragraph_spans, paragraphs, prose_changes, strip_links};
use crate::units::UnitBlock;
use pretty_assertions::assert_eq;
use std::fmt::Write;

fn block(index: u32, old: (u32, u32), new: (u32, u32)) -> UnitBlock {
  UnitBlock {
    index,
    old_start: old.0,
    old_end: old.1,
    new_start: new.0,
    new_end: new.1,
  }
}

/// A paragraph as a reader sees it: `+ ` added, `- ` removed, a changed one with what the commits wrote in `{+…+}` and
/// what they removed in `{-…-}`
fn render(paragraph: &ProseParagraph) -> String {
  match &paragraph.change {
    ProseChange::Added => format!("+ {}", paragraph.text),
    ProseChange::Removed => format!("- {}", paragraph.text),
    ProseChange::Changed { inserted, deleted } => {
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
  }
}

/// The paragraphs a file of prose changes, as the brief reads them: each block widened to whole paragraphs, then
/// paired by word (`render`)
fn changes(old: &str, new: &str, blocks: &[UnitBlock]) -> Vec<String> {
  let old: Vec<&str> = old.split('\n').collect();
  let new: Vec<&str> = new.split('\n').collect();
  paragraph_spans(&old, &new, blocks)
    .into_iter()
    .flat_map(|(old_range, new_range, _)| prose_changes(&paragraphs(&old[old_range]).0, &paragraphs(&new[new_range]).0, "spec/a.spec.md", |_| true))
    .map(|paragraph| render(&paragraph))
    .collect()
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
    "The store keeps at most 500 projects.".to_string(),
    "An older build ignores the order of the tool windows.".to_string(),
  ];
  let new = [
    "The store keeps at most 500 projects.".to_string(),
    "The focus is a fact the window derives, and `FocusRecorder` writes it.".to_string(),
    "An older build ignores the order and the focus of the tool windows.".to_string(),
  ];
  let changes = prose_changes(&old, &new, "spec/a.spec.md", |_| true);
  assert_eq!(
    changes.iter().map(render).collect::<Vec<_>>(),
    [
      "+ The focus is a fact the window derives, and `FocusRecorder` writes it.",
      "~ An older build ignores the order{+ and the focus+} of the tool windows."
    ]
  );
  assert_eq!(changes[0].code.len(), 1, "its code is set as code");
}

/// The `[@test]` lines of an item are not its prose: the paths they name prove it
#[test]
fn the_test_lines_of_an_item_name_its_tests() {
  let (texts, tests) = paragraphs(&[
    "- The window keeps its layout across a restart, and the project view does",
    "  not take its place.",
    "  [@test] ../tests/restart.json",
  ]);
  assert_eq!(
    (texts, tests),
    (
      vec!["The window keeps its layout across a restart, and the project view does not take its place.".to_string()],
      vec!["tests/restart.json".to_string()]
    )
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

/// A long paragraph changed in one sentence shows that sentence first (`excerpt`); a short one, or one changed
/// throughout, shows whole
#[test]
fn a_long_paragraph_changed_in_one_sentence_shows_it_first() {
  let sentence = |index: usize| format!("Sentence {index} says one more thing about the layout of the window and its store.");
  let before: Vec<String> = (0..6).map(sentence).collect();
  let mut after = before.clone();
  after[3] = after[3].replace("one more thing", "one other thing");
  let excerpt_of = |before: &[String], after: &[String]| prose_changes(&[before.join(" ")], &[after.join(" ")], "a.md", |_| true)[0].excerpt;
  let text = after.join(" ");
  let start = u32::try_from(text.find("Sentence 3").unwrap()).unwrap();
  assert_eq!(
    excerpt_of(&before, &after),
    Some(TextRange {
      start,
      end: start + u32::try_from(after[3].len()).unwrap()
    })
  );
  let mut short = before[..2].to_vec();
  short[1] = short[1].replace("one more thing", "one other thing");
  assert_eq!(excerpt_of(&before[..2], &short), None, "a short paragraph shows whole");
  let mut throughout = before.clone();
  throughout[0] = throughout[0].replace("one more", "a further");
  throughout[5] = throughout[5].replace("one more", "a further");
  assert_eq!(excerpt_of(&before, &throughout), None, "changes at both ends leave out too little");
}
