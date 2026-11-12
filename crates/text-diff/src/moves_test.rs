//! The moves of a diff (`moves.rs`): what is a move, what is none, which of two is taken, what an edited line and a
//! shifted indent are, and the limits of the search by its work.

use crate::model::{BlockKind, DiffBlock, DiffBudget, LineMove, LineRun, RangeKind, TextDiff, diff_texts, starts_of};
use crate::moves::{
  LIMITS, Limits, MAX_COMPARED_PAIRS, MAX_KEY_CANDIDATES, MAX_MOVE_LINES, MAX_MOVE_WORK, MAX_UNREAD_PAIRS, MIN_MOVE_CHARS, MoveWork, find_moves, find_moves_counted,
  find_moves_within, rewritten_lines,
};
use crate::policy::WhitespacePolicy;
use crate::test_support::{between, proptest_config, run, text};
use pretty_assertions::assert_eq;
use proptest::prelude::Strategy;
use proptest::proptest;
use std::fmt::Write as _;

const ALPHA: [&str; 5] = [
  "fn alpha(items: &[Item]) -> u64 {",
  "  let total = compute_total(items);",
  "  report(total);",
  "  total",
  "}",
];
const BETA: [&str; 5] = ["fn beta(names: &[Name]) -> usize {", "  let count = count_names(names);", "  print(count);", "  count", "}"];
const GAMMA: [&str; 5] = ["fn gamma(rows: &[Row]) -> bool {", "  let found = find_row(rows);", "  show(found);", "  found", "}"];
const DELTA: [&str; 3] = ["fn delta(pages: &[Page]) -> Page {", "  first_page(pages)", "}"];

/// The lines of `part`, each after `indent`
fn indented(part: &[&str], indent: &str) -> Vec<String> {
  part.iter().map(|line| if line.is_empty() { String::new() } else { format!("{indent}{line}") }).collect()
}

fn refs(lines: &[String]) -> Vec<&str> {
  lines.iter().map(String::as_str).collect()
}

fn diff(old: &str, new: &str) -> TextDiff {
  diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget::NONE)
}

/// The lines of a text as the comparison counts them
fn lines_of(text: &str) -> Vec<String> {
  text.replace("\r\n", "\n").replace('\r', "\n").split('\n').map(str::to_string).collect()
}

fn trimmed(line: &str) -> &str {
  line.trim_matches([' ', '\t'])
}

/// The whitespace before the text of a line
fn indent_of(line: &str) -> &str {
  &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// What made the indent `new` of `old`: the whitespace added (or taken, `false`) at its start, and at its end
fn shifts_between<'t>(old: &'t str, new: &'t str) -> Vec<(bool, bool, &'t str)> {
  let added = new.len() >= old.len();
  let (short, long) = if added { (old, new) } else { (new, old) };
  let mut shifts = Vec::new();
  shifts.extend(long.strip_suffix(short).map(|text| (added, false, text)));
  shifts.extend(long.strip_prefix(short).map(|text| (added, true, text)));
  shifts
}

/// The lines of a text that count: all but the empty line after its last line end, and the one line of an empty text
fn counted_lines(lines: &[String]) -> u32 {
  to_u32(lines.len() - usize::from(lines.last().is_some_and(String::is_empty)))
}

/// What holds of the moves of every diff: in the order of their new lines, a line of a side in one move at most and
/// in no run, the old lines inside one block that is a change and the new lines inside another, all of them lines
/// their text counts (never its empty last line), the blank lines beside it blank; a line that is not edited the same on both sides but for the indent of the move; each range of a
/// line a text that is another on the other side, of the kind its sides say, with the same text between the ranges;
/// under the policy that counts every character no whitespace at the ends of a line changed without a range, but by
/// the indent of the move; the ends exact or blank, at most a quarter of the lines that are not blank edited in
/// their text or their nesting, and the exact lines that stand once among the lines a move may take long enough
fn check(old: &str, new: &str, blocks: &TextDiff, runs: &[LineRun], moves: &[LineMove], policy: WhitespacePolicy, indentation_matters: bool) {
  let (old_lines, new_lines) = (lines_of(old), lines_of(new));
  let ends_count = policy == WhitespacePolicy::Default;
  let block_of = |new_side: bool, lines: std::ops::Range<u32>| {
    blocks.blocks.iter().position(|block| {
      let side = if new_side { block.new_start..block.new_end } else { block.old_start..block.old_end };
      block.kind == BlockKind::Changed && side.start <= lines.start && lines.end <= side.end
    })
  };
  let (mut old_taken, mut new_taken) = (vec![false; old_lines.len()], vec![false; new_lines.len()]);
  for run in runs {
    old_taken[run.old_start as usize..run.old_end as usize].fill(true);
    new_taken[run.new_start as usize..run.new_end as usize].fill(true);
  }
  // how often each line a move may take stands on its side, by its key: before the moves take theirs
  let stands = |lines: &[String], taken: &[bool], new_side: bool| {
    let mut stands: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for block in blocks.blocks.iter().filter(|block| block.kind == BlockKind::Changed) {
      let side = if new_side { block.new_start..block.new_end } else { block.old_start..block.old_end };
      for line in side.start..side.end.min(counted_lines(lines)) {
        if !taken[line as usize] {
          *stands.entry(trimmed(&lines[line as usize]).to_string()).or_default() += 1;
        }
      }
    }
    stands
  };
  let (old_stands, new_stands) = (stands(&old_lines, &old_taken, false), stands(&new_lines, &new_taken, true));
  let mut last_new = 0;
  // the texts of a failure, unless they are files
  let texts = if old.len() + new.len() < 4_000 {
    format!(" of {old:?} -> {new:?}")
  } else {
    String::new()
  };
  for line_move in moves {
    let context = format!("{line_move:?}{texts}");
    assert!(line_move.len > 0 && line_move.new_lines().start >= last_new, "{context}");
    last_new = line_move.new_lines().end;
    let (old_block, new_block) = (block_of(false, line_move.old_lines()), block_of(true, line_move.new_lines()));
    assert!(old_block.is_some() && new_block.is_some() && old_block != new_block, "{context}: {:?}", blocks.blocks);
    assert!(
      line_move.old_lines().end <= counted_lines(&old_lines) && line_move.new_lines().end <= counted_lines(&new_lines),
      "{context}: the empty last line of a text is a line of the move"
    );
    for (taken, lines) in [(&mut old_taken, line_move.old_lines()), (&mut new_taken, line_move.new_lines())] {
      for line in lines {
        assert!(!taken[line as usize], "{context}: line {line} is taken twice");
        taken[line as usize] = true;
      }
    }
    // the blank lines beside it: blank, and of one text where the whitespace of a line counts
    let (old_end, new_end) = (line_move.old_start + line_move.len, line_move.new_start + line_move.len);
    let beside = [
      (line_move.old_lines().start..line_move.old_start, new_end..line_move.new_lines().end),
      (old_end..line_move.old_lines().end, line_move.new_lines().start..line_move.new_start),
    ];
    for (old_blanks, new_blanks) in beside {
      assert_eq!(old_blanks.len(), new_blanks.len(), "{context}");
      let texts: Vec<&str> = old_blanks
        .map(|line| old_lines[line as usize].as_str())
        .chain(new_blanks.map(|line| new_lines[line as usize].as_str()))
        .collect();
      assert!(texts.iter().all(|text| trimmed(text).is_empty()), "{context}: {texts:?}");
      assert!(!ends_count || texts.iter().all(|text| *text == texts[0]), "{context}: {texts:?}");
    }

    assert_eq!(line_move.old_ranges.len(), line_move.new_ranges.len(), "{context}");
    assert!(line_move.old_ranges.is_empty() || line_move.old_ranges.len() == line_move.len as usize, "{context}");
    assert!(
      line_move.edited.is_sorted_by(|before, after| before < after) && line_move.edited.iter().all(|line| *line < line_move.len),
      "{context}"
    );
    let (mut counted, mut exact, mut solid) = (0, 0, 0);
    // what made the new indent of the old one, of each exact line
    let mut shifts: Vec<Vec<(bool, bool, &str)>> = Vec::new();
    // and of each line edited in its nesting alone
    let mut nestings: Vec<Vec<(bool, bool, &str)>> = Vec::new();
    let mut ends: Vec<bool> = Vec::new();
    for offset in 0..line_move.len as usize {
      let (old_text, new_text) = (
        old_lines[line_move.old_start as usize + offset].as_str(),
        new_lines[line_move.new_start as usize + offset].as_str(),
      );
      let (old_ranges, new_ranges): (&[u32], &[u32]) = (
        line_move.old_ranges.get(offset).map_or(&[], Vec::as_slice),
        line_move.new_ranges.get(offset).map_or(&[], Vec::as_slice),
      );
      let line = format!("{context}: line {offset}");
      let edited = line_move.edited.contains(&to_u32(offset));
      assert!(edited || (old_ranges.is_empty() && new_ranges.is_empty()), "{line}: ranges of a line that is not edited");
      // the ranges: of the same kinds, each another text on the other side, the same text between them
      assert_eq!(old_ranges.len(), new_ranges.len(), "{line}");
      let units = |text: &str| text.encode_utf16().collect::<Vec<u16>>();
      for (old_range, new_range) in old_ranges.chunks(3).zip(new_ranges.chunks(3)) {
        let (old_part, new_part) = (
          &units(old_text)[old_range[0] as usize..old_range[1] as usize],
          &units(new_text)[new_range[0] as usize..new_range[1] as usize],
        );
        let kind = match (old_part.is_empty(), new_part.is_empty()) {
          (true, _) => RangeKind::Inserted,
          (_, true) => RangeKind::Deleted,
          _ => RangeKind::Modified,
        };
        assert!(old_part != new_part, "{line}: a range of the same text on both sides, {old_range:?}/{new_range:?}");
        assert_eq!((old_range[2], new_range[2]), (kind as u32, kind as u32), "{line}");
      }
      let (old_parts, new_parts) = (between(old_text, old_ranges), between(new_text, new_ranges));
      let last = old_parts.len() - 1;
      for (index, (old_part, new_part)) in old_parts.iter().zip(&new_parts).enumerate() {
        // before the first range the indent, which the move says; after the last one the whitespace that ends the line
        let (mut old_part, mut new_part) = (old_part.as_str(), new_part.as_str());
        if index == 0 {
          (old_part, new_part) = (old_part.trim_start_matches([' ', '\t']), new_part.trim_start_matches([' ', '\t']));
        }
        if index == last && !ends_count {
          (old_part, new_part) = (old_part.trim_end_matches([' ', '\t']), new_part.trim_end_matches([' ', '\t']));
        }
        assert_eq!(old_part, new_part, "{line}: part {index} of {old_parts:?} / {new_parts:?}");
      }
      let blank = trimmed(old_text).is_empty();
      assert_eq!(blank, trimmed(new_text).is_empty(), "{line}");
      // the indent of a line: in a range, the same, or moved as the move says
      let (old_indent, new_indent) = (indent_of(old_text), indent_of(new_text));
      let in_range = old_ranges.first() == Some(&0) && new_ranges.first() == Some(&0) && (old_ranges[1] as usize, new_ranges[1] as usize) == (old_indent.len(), new_indent.len());
      let moved_by = shifts_between(old_indent, new_indent);
      let columns = i64::try_from(new_indent.len()).unwrap() - i64::try_from(old_indent.len()).unwrap();
      let follows = !moved_by.is_empty() && columns == i64::from(line_move.indent);
      if ends_count {
        assert!(in_range || old_indent == new_indent || follows, "{line}: an indent that changed without a range");
      }
      // an edited line: its text, its nesting where the indentation has a meaning (its indent is a range, or the same
      // text in a move that shifted), or the whitespace at its ends alone, which is a range
      let text_edited = trimmed(old_text) != trimmed(new_text);
      let stayed = old_indent == new_indent && line_move.indent != 0;
      let nested = ends_count && indentation_matters && !blank && !text_edited && (in_range || stayed);
      assert!(!edited || text_edited || nested || !old_ranges.is_empty(), "{line}: an edited line that is the same");
      assert!(edited || !(text_edited || nested), "{line}: a line that is not the same is not edited");
      if blank {
        continue;
      }
      // a line that counts in the quarter: of another text, or at another nesting
      let apart = text_edited || nested;
      ends.push(apart);
      if apart {
        counted += 1;
        if !text_edited {
          nestings.push(moved_by);
        }
      } else {
        exact += 1;
        // only a line that stands once on each side says which line moved
        let key = trimmed(old_text);
        if old_stands.get(key) == Some(&1) && new_stands.get(key) == Some(&1) {
          solid += key.chars().filter(|character| ![' ', '\t'].contains(character)).count();
        }
        shifts.push(moved_by);
      }
    }
    assert!(ends.first() == Some(&false) && ends.last() == Some(&false), "{context}: an edited line at an end");
    assert!(counted * 4 <= counted + exact, "{context}: {counted} edited of {}", counted + exact);
    assert!(solid >= MIN_MOVE_CHARS, "{context}: {solid} characters in the exact lines that stand once");
    // the indent of the move: the shift of every exact line where the indentation has a meaning, and of no line
    // edited in its nesting; of one of the exact lines else
    let of_move = |shift: &(bool, bool, &str)| i64::try_from(shift.2.len()).unwrap() * if shift.0 { 1 } else { -1 } == i64::from(line_move.indent);
    if ends_count && indentation_matters {
      let shared: Vec<&(bool, bool, &str)> = shifts[0].iter().filter(|shift| of_move(shift) && shifts.iter().all(|line| line.contains(shift))).collect();
      assert!(!shared.is_empty(), "{context}: {shifts:?}");
      for nesting in &nestings {
        assert!(shared.iter().any(|shift| !nesting.contains(shift)), "{context}: {nesting:?} moved as the move, {shared:?}");
      }
    } else {
      assert!(nestings.is_empty(), "{context}");
      assert!(line_move.indent == 0 || shifts.iter().flatten().any(of_move), "{context}: {shifts:?}");
    }
  }
}

/// The moves of the diff of two texts of a file whose indentation has no meaning, every character counted, checked
fn moves(old: &str, new: &str) -> Vec<LineMove> {
  moves_where(old, new, false)
}

fn moves_where(old: &str, new: &str, indentation_matters: bool) -> Vec<LineMove> {
  moves_under(old, new, WhitespacePolicy::Default, indentation_matters)
}

/// The moves of the diff of two texts under `policy`, checked
fn moves_under(old: &str, new: &str, policy: WhitespacePolicy, indentation_matters: bool) -> Vec<LineMove> {
  let blocks = diff_texts(old, new, policy, &DiffBudget::NONE);
  let moves = find_moves(old, new, &blocks.blocks, &[], policy, indentation_matters);
  check(old, new, &blocks, &[], &moves, policy, indentation_matters);
  moves
}

/// `check` of the moves `found` of `blocks` of two texts, every character counted, where the indentation has no meaning
fn check_of(old: &str, new: &str, blocks: &[DiffBlock], found: &[LineMove]) {
  let diff = TextDiff {
    blocks: blocks.to_vec(),
    ..TextDiff::default()
  };
  check(old, new, &diff, &[], found, WhitespacePolicy::Default, false);
}

/// The moves of `blocks` of two texts, every character counted
fn moves_of(old: &str, new: &str, blocks: &[DiffBlock], indentation_matters: bool) -> Vec<LineMove> {
  find_moves(old, new, blocks, &[], WhitespacePolicy::Default, indentation_matters)
}

/// The moves as one line: the old and the new lines of each, the blank lines that moved with it (before and after
/// its old lines), its indent when not 0, and its edited lines by their place in it with their ranges
fn show(moves: &[LineMove]) -> String {
  let mut shown = Vec::new();
  for line_move in moves {
    let mut text = format!(
      "{}..{} -> {}..{}",
      line_move.old_start,
      line_move.old_start + line_move.len,
      line_move.new_start,
      line_move.new_start + line_move.len
    );
    if line_move.blanks_before + line_move.blanks_after > 0 {
      write!(text, " blanks {}+{}", line_move.blanks_before, line_move.blanks_after).unwrap();
    }
    if line_move.indent != 0 {
      write!(text, " indent {:+}", line_move.indent).unwrap();
    }
    for offset in &line_move.edited {
      match (line_move.old_ranges.get(*offset as usize), line_move.new_ranges.get(*offset as usize)) {
        (Some(old), Some(new)) if !old.is_empty() => write!(text, " edited {offset}: {old:?}/{new:?}").unwrap(),
        _ => write!(text, " edited {offset}").unwrap(),
      }
    }
    shown.push(text);
  }
  shown.join("; ")
}

const BLANK: [&str; 1] = [""];
const INSERTED: u32 = RangeKind::Inserted as u32;
const DELETED: u32 = RangeKind::Deleted as u32;
const MODIFIED: u32 = RangeKind::Modified as u32;

/// Four functions with a blank line between them
fn four() -> String {
  text(&[&BETA, &BLANK, &ALPHA, &BLANK, &GAMMA, &BLANK, &DELTA])
}

/// `four` with `alpha` after `gamma`: lines 6..12 of `four`, the function and the blank line after it, at 12..18
fn four_with(alpha: &[&str]) -> String {
  text(&[&BETA, &BLANK, &GAMMA, &BLANK, alpha, &BLANK, &DELTA])
}

/// Eight lines that stay where they are: more than any lines the tests move, so the comparison keeps them
fn stay() -> Vec<String> {
  (0..8).map(|index| format!("stay_{index}();")).collect()
}

/// `moved` before the lines that stay, and `now` after them: the comparison deletes lines 0.. and adds lines 8..
fn below(moved: &[&str], now: &[&str]) -> (String, String) {
  let stay = stay();
  let stay = refs(&stay);
  (text(&[moved, &stay]), text(&[&stay, now]))
}

/// A block of the lines `old` and `new`, a change
fn block(old: std::ops::Range<u32>, new: std::ops::Range<u32>) -> DiffBlock {
  crate::test_support::block(old, new, BlockKind::Changed)
}

#[test]
fn lines_deleted_in_one_block_and_added_in_another_are_a_move() {
  // a function moved below another, with the blank line after it
  let (old, new) = (four(), four_with(&ALPHA));
  let found = moves(&old, &new);
  assert_eq!(show(&found), "6..12 -> 12..18");
  assert_eq!((found[0].indent, found[0].old_ranges.len(), found[0].new_ranges.len()), (0, 0, 0));
  // the other way the comparison keeps `alpha` where it is: `gamma` moved
  assert_eq!(show(&moves(&new, &old)), "6..12 -> 12..18");
  // to the end of the file: the comparison deletes the blank line after it and adds one before it, no pair of the
  // lines of the move: the blank line that moved with it, counted beside it
  let last = text(&[&BETA, &BLANK, &GAMMA, &BLANK, &DELTA, &BLANK, &ALPHA]);
  let found = moves(&old, &last);
  assert_eq!(show(&found), "6..11 -> 16..21 blanks 0+1");
  assert_eq!((found[0].old_lines(), found[0].new_lines()), (6..12, 15..21));
  // the lines of a text are those of the comparison: `\r\n` and a lone `\r` end one too
  for end in ["\r\n", "\r"] {
    let (old, new) = (old.replace('\n', end), new.replace('\n', end));
    assert_eq!(show(&moves(&old, &new)), "6..12 -> 12..18", "{end:?}");
  }
  let mixed = |text: &str| text.replace("{\n", "{\r").replace(";\n", ";\r\n");
  assert_eq!(show(&moves(&mixed(&old), &new)), "6..12 -> 12..18");
  // the same text has no move, an added and a deleted function neither
  assert_eq!(show(&moves(&old, &old)), "");
  assert_eq!(show(&moves(&text(&[&BETA, &BLANK, &GAMMA]), &old)), "");
  assert_eq!(show(&moves(&old, &text(&[&BETA, &BLANK, &GAMMA]))), "");
}

#[test]
fn a_move_says_the_columns_its_lines_were_indented_by() {
  let old = four();
  for (indent, columns) in [("  ", 2), ("\t", 1), ("    \t", 5)] {
    let inner = indented(&ALPHA, indent);
    let new = text(&[&BETA, &BLANK, &GAMMA, &BLANK, &["mod inner {"], &refs(&inner), &["}"], &BLANK, &DELTA]);
    // the blank line after the function is no pair of the `}` after it now
    assert_eq!(show(&moves(&old, &new)), format!("6..11 -> 13..18 indent +{columns}"), "{indent:?}");
    // and out of the module
    let (old, new) = below(&refs(&inner), &ALPHA);
    assert_eq!(show(&moves(&old, &new)), format!("0..5 -> 8..13 indent -{columns}"), "{indent:?}");
  }
  // lines indented unevenly, where the indentation has no meaning: the columns of most of them, of the first among
  // as many. A line that stayed at its indent is the same line
  let uneven = [ALPHA[0], "    let total = compute_total(items);", "    report(total);", "    total", ALPHA[4]];
  assert_eq!(show(&moves(&old, &four_with(&uneven))), "6..12 -> 12..18 indent +2");
  // tabs for the spaces of every line: no whitespace added to any, no shift; each indent is a range
  let tabs = [ALPHA[0], "\tlet total = compute_total(items);", "\treport(total);", "\ttotal", ALPHA[4]];
  let found = moves(&old, &four_with(&tabs));
  assert_eq!((found[0].indent, &found[0].edited), (0, &vec![1, 2, 3]));
  assert_eq!((&found[0].old_ranges[1], &found[0].new_ranges[1]), (&vec![0, 2, MODIFIED], &vec![0, 1, MODIFIED]));
  // whitespace added before the text of each line, after its tab, is a shift as whitespace added before the tab is
  let (tabbed, inner, outer) = (indented(&ALPHA, "\t"), indented(&ALPHA, "\t  "), indented(&ALPHA, "  \t"));
  for (now, expected) in [(&inner, "0..5 -> 8..13 indent +2"), (&outer, "0..5 -> 8..13 indent +2")] {
    let (old, new) = below(&refs(&tabbed), &refs(now));
    assert_eq!(show(&moves(&old, &new)), expected);
    assert_eq!(show(&moves_where(&old, &new, true)), expected);
    assert_eq!(show(&moves(&new, &old)), "8..13 -> 0..5 indent -2");
  }
}

/// Under the policy that counts every character, the whitespace at the ends of a line of a move that changed
/// otherwise than the move says is a range of an edited line: a view that shows the old lines of a move once hides
/// no change. Such a line is still one of the exact lines of the move where the indentation has no meaning. Under a
/// policy that leaves that whitespace out it is no change of a move
#[test]
fn the_whitespace_at_the_ends_of_a_line_that_changed_otherwise_than_its_move_is_a_range() {
  let old = four();
  let other = [WhitespacePolicy::Trim, WhitespacePolicy::Ignore, WhitespacePolicy::IgnoreBlankLines];
  // what the moves of `alpha` are under the policy that counts every character, and under the others
  let found = |alpha: &[&str], counted: &str, left_out: &str| {
    let new = four_with(alpha);
    assert_eq!(show(&moves(&old, &new)), counted, "{alpha:?}");
    for policy in other {
      assert_eq!(show(&moves_under(&old, &new, policy, false)), left_out, "{alpha:?} under {policy:?}");
      // where the indentation has a meaning too: the policy leaves the indent out
      assert_eq!(show(&moves_under(&old, &new, policy, true)), left_out, "{alpha:?} under {policy:?}");
    }
  };
  // two lines of five indented by two more columns than the others, which stayed: their indents are ranges, and
  // they are over a quarter of the lines, which they do not count in
  let halves = [ALPHA[0], ALPHA[1], "    report(total);", "    total", ALPHA[4]];
  found(&halves, "6..12 -> 12..18 edited 2: [0, 2, 2]/[0, 4, 2] edited 3: [0, 2, 2]/[0, 4, 2]", "6..12 -> 12..18");
  // a line of a raw string indented by eight more columns: the text of the string changed
  let raw = [ALPHA[0], ALPHA[1], "          report(total);", ALPHA[3], ALPHA[4]];
  found(&raw, "6..12 -> 12..18 edited 2: [0, 2, 2]/[0, 10, 2]", "6..12 -> 12..18");
  // a line that lost its indent, and one that got one
  let lost = [ALPHA[0], ALPHA[1], "report(total);", ALPHA[3], ALPHA[4]];
  found(&lost, "6..12 -> 12..18 edited 2: [0, 2, 1]/[0, 0, 1]", "6..12 -> 12..18");
  let got = ["  fn alpha(items: &[Item]) -> u64 {", ALPHA[1], ALPHA[2], ALPHA[3], ALPHA[4]];
  found(&got, "6..12 -> 12..18 edited 0: [0, 0, 0]/[0, 2, 0]", "6..12 -> 12..18");
  // tabs for the spaces of a line, as many: another text of the same width
  let tabs = [ALPHA[0], ALPHA[1], "\t\treport(total);", ALPHA[3], ALPHA[4]];
  found(&tabs, "6..12 -> 12..18 edited 2: [0, 2, 2]/[0, 2, 2]", "6..12 -> 12..18");
  // whitespace after the text of a line: added, lost (the hard line break of Markdown), another
  let trailing = [ALPHA[0], "  let total = compute_total(items);  \t", ALPHA[2], ALPHA[3], ALPHA[4]];
  found(&trailing, "6..12 -> 12..18 edited 1: [35, 35, 0]/[35, 38, 0]", "6..12 -> 12..18");
  let (before, after) = below(&trailing, &ALPHA);
  assert_eq!(show(&moves(&before, &after)), "0..5 -> 8..13 edited 1: [35, 38, 1]/[35, 35, 1]");
  let spaces = [ALPHA[0], "  let total = compute_total(items);   ", ALPHA[2], ALPHA[3], ALPHA[4]];
  let (before, after) = below(&trailing, &spaces);
  assert_eq!(show(&moves(&before, &after)), "0..5 -> 8..13 edited 1: [35, 38, 2]/[35, 38, 2]");
  // both ends of a line, and its text between them
  let both = [ALPHA[0], ALPHA[1], "      report(total, verbose); ", ALPHA[3], ALPHA[4]];
  found(
    &both,
    "6..12 -> 12..18 edited 2: [0, 2, 2, 14, 14, 0, 16, 16, 0]/[0, 6, 2, 18, 27, 0, 29, 30, 0]",
    "6..12 -> 12..18 edited 2: [14, 14, 0]/[18, 27, 0]",
  );
  // the first and the last line: the whitespace of a line does not make it an edited line that ends no move
  let ends = ["\tfn alpha(items: &[Item]) -> u64 {", ALPHA[1], ALPHA[2], ALPHA[3], "}  "];
  found(&ends, "6..12 -> 12..18 edited 0: [0, 0, 0]/[0, 1, 0] edited 4: [1, 1, 0]/[1, 3, 0]", "6..12 -> 12..18");
  // every line shifted alike and one by more: the shift of the move, and the indent of the one
  let shifted = [
    "  fn alpha(items: &[Item]) -> u64 {",
    "    let total = compute_total(items);",
    "          report(total);",
    "    total",
    "  }",
  ];
  found(&shifted, "6..12 -> 12..18 indent +2 edited 2: [0, 2, 2]/[0, 10, 2]", "6..12 -> 12..18 indent +2");
}

#[test]
fn a_move_holds_at_most_a_quarter_of_edited_lines_between_exact_ones() {
  let old = four();
  // one line of five edited: an argument added
  let one = [ALPHA[0], ALPHA[1], "  report(total, verbose);", ALPHA[3], ALPHA[4]];
  let found = moves(&old, &four_with(&one));
  assert_eq!(show(&found), "6..12 -> 12..18 edited 2: [14, 14, 0]/[14, 23, 0]");
  assert_eq!(found[0].old_ranges, vec![vec![], vec![], vec![14, 14, INSERTED], vec![], vec![], vec![]]);
  assert_eq!(found[0].new_ranges, vec![vec![], vec![], vec![14, 23, INSERTED], vec![], vec![], vec![]]);
  // a word changed, a word removed: the ranges are typed as those of a block
  let renamed = [ALPHA[0], "  let total = compute_total(rows);", ALPHA[2], ALPHA[3], ALPHA[4]];
  let found = moves(&old, &four_with(&renamed));
  assert_eq!(found[0].old_ranges[1], [28, 33, MODIFIED]);
  assert_eq!(found[0].new_ranges[1], [28, 32, MODIFIED]);
  let found = moves(&four_with(&one), &old);
  assert_eq!((&found[0].old_ranges[2], &found[0].new_ranges[2]), (&vec![14, 23, DELETED], &vec![14, 14, DELETED]));
  // the edited line is indented with the others: its ranges are of its own line
  let inner = [
    "  fn alpha(items: &[Item]) -> u64 {",
    "    let total = compute_total(items);",
    "    report(total, verbose);",
    "    total",
    "  }",
  ];
  assert_eq!(show(&moves(&old, &four_with(&inner))), "6..12 -> 12..18 indent +2 edited 2: [14, 14, 0]/[16, 25, 0]");
  // two lines of five edited are over a quarter: the lines before them and after them are asked alone, and only the
  // first is long enough
  let two = [ALPHA[0], "  let total = compute_total(rows);", "  report(total, verbose);", ALPHA[3], ALPHA[4]];
  assert_eq!(show(&moves(&old, &four_with(&two))), "6..7 -> 12..13");
  // a line with a word that is another is no similar pair: less than a third of a pair may change
  let word = [ALPHA[0], "  let total = compute_sum(items);", ALPHA[2], ALPHA[3], ALPHA[4]];
  assert_eq!(show(&moves(&old, &four_with(&word))), "6..7 -> 12..13");
  // an edited line at an end of the lines is no line of the move
  let first = ["fn alpha(items: &[Item]) -> u32 {", ALPHA[1], ALPHA[2], ALPHA[3], ALPHA[4]];
  assert_eq!(show(&moves(&old, &four_with(&first))), "7..12 -> 13..18");
  // a line that is another line ends the move: nothing pairs across it
  let other = [ALPHA[0], ALPHA[1], "  notify(listeners, Event::Done)?;", ALPHA[3], ALPHA[4]];
  assert_eq!(show(&moves(&old, &four_with(&other))), "6..8 -> 12..14");
  // an added line shifts the lines after it: two moves, when both are long enough
  let longer = [
    ALPHA[0],
    ALPHA[1],
    "  audit(total);",
    ALPHA[2],
    "  total.checked_add(offset_of(items)).unwrap_or(total)",
    "}",
  ];
  let longer_old = text(&[&BETA, &BLANK, &[ALPHA[0], ALPHA[1], ALPHA[2], longer[4], "}"], &BLANK, &GAMMA, &BLANK, &DELTA]);
  assert_eq!(show(&moves(&longer_old, &four_with(&longer))), "6..8 -> 12..14; 8..12 -> 15..19");
}

#[test]
fn the_whitespace_inside_a_line_is_part_of_its_key() {
  let line = |text: &'static str| [text];
  let file = |first: &'static str, last: &'static str| text(&[&line(first), &BETA, &line(last)]);
  // the same line with other whitespace inside is no exact line, so alone it is no move
  let (spaced, joined) = ("let total = compute_total(items, offset);", "let total = compute_total(items,offset);");
  assert_eq!(show(&moves(&file(spaced, "end"), &file("start", spaced))), "0..1 -> 6..7");
  assert_eq!(show(&moves(&file(spaced, "end"), &file("start", joined))), "");
  // among exact lines it is an edited line, its whitespace the range
  let old = four();
  let inside = [ALPHA[0], "  let total = compute_total( items );", ALPHA[2], ALPHA[3], ALPHA[4]];
  assert_eq!(
    show(&moves(&old, &four_with(&inside))),
    "6..12 -> 12..18 edited 1: [28, 28, 0, 33, 33, 0]/[28, 29, 0, 34, 35, 0]"
  );
}

#[test]
fn a_line_a_move_rewrote_is_an_edited_line_that_differs_in_more_than_whitespace() {
  let old = four();
  // the lines of both sides that the moves of `alpha` after `gamma` rewrote
  let rewritten = |alpha: &[&str]| {
    let new = four_with(alpha);
    let found = moves(&old, &new);
    assert_eq!(found.len(), 1, "{alpha:?}");
    (found[0].edited.clone(), rewritten_lines(&old, &new, &found))
  };
  // an exact move rewrote nothing
  assert_eq!(rewritten(&ALPHA), (vec![], [vec![], vec![]]));
  // an argument added, a word that is another: the old line and the new one, each by its number in its text
  let argument = [ALPHA[0], ALPHA[1], "  report(total, verbose);", ALPHA[3], ALPHA[4]];
  assert_eq!(rewritten(&argument), (vec![2], [vec![8], vec![14]]));
  let word = [ALPHA[0], "  let total = compute_total(rows);", ALPHA[2], ALPHA[3], ALPHA[4]];
  assert_eq!(rewritten(&word), (vec![1], [vec![7], vec![13]]));
  // edited in its whitespace alone: inside the line, in its indent, after its text, a tab for its spaces. The line
  // is the same code at another place
  for line in [
    "  let total = compute_total( items );",
    "      let total = compute_total(items);",
    "  let total = compute_total(items);  \t",
    "\t\tlet total = compute_total(items);",
  ] {
    let spaced = [ALPHA[0], line, ALPHA[2], ALPHA[3], ALPHA[4]];
    assert_eq!(rewritten(&spaced), (vec![1], [vec![], vec![]]), "{line:?}");
  }
  // its whitespace and its text: rewritten
  let both = [ALPHA[0], ALPHA[1], "      report(total, verbose); ", ALPHA[3], ALPHA[4]];
  assert_eq!(rewritten(&both), (vec![2], [vec![8], vec![14]]));

  // two functions that changed places below the lines that stay, each with a rewritten line: the lines of each side
  // in their order, whatever the order of the moves; and a text with carriage returns has the lines of the comparison
  let gamma = [GAMMA[0], GAMMA[1], "  show(found, verbose);", GAMMA[3], GAMMA[4]];
  let stay = stay();
  let stay = refs(&stay);
  for end in ["\n", "\r\n", "\r"] {
    let with_end = |text: String| text.replace('\n', end);
    let old = with_end(text(&[&ALPHA, &BLANK, &GAMMA, &BLANK, &stay]));
    let new = with_end(text(&[&stay, &BLANK, &gamma, &BLANK, &argument]));
    let found = moves_under(&old, &new, WhitespacePolicy::Default, false);
    assert_eq!(found.len(), 2, "{end:?}: {}", show(&found));
    let [old_lines, new_lines] = rewritten_lines(&old, &new, &found);
    let line = |text: &str, number: u32| lines_of(text)[number as usize].clone();
    assert_eq!(
      (
        old_lines.iter().map(|number| line(&old, *number)).collect::<Vec<_>>(),
        new_lines.iter().map(|number| line(&new, *number)).collect::<Vec<_>>()
      ),
      (vec![ALPHA[2].to_string(), GAMMA[2].to_string()], vec![gamma[2].to_string(), argument[2].to_string()]),
      "{end:?}: {}",
      show(&found)
    );
  }
}

/// Lines that stand at two places name no move: nothing says which of the two took them, or which of the two left.
/// A place that holds them all is taken where the other holds only lines that stand elsewhere too
#[test]
fn lines_that_stand_at_two_places_are_no_move_and_the_place_with_a_line_of_its_own_takes_them() {
  let stay = stay();
  let stay = refs(&stay);
  let old = text(&[&ALPHA, &BLANK, &stay, &["end();"]]);
  // the function stands at two places of the new text: either may be the one its lines went to
  let twice = text(&[&stay[..4], &ALPHA, &stay[4..], &ALPHA, &["end();"]]);
  assert_eq!(show(&moves(&old, &twice)), "");
  let beside = text(&[&stay, &BLANK, &ALPHA, &["between();"], &ALPHA, &["end();"]]);
  assert_eq!(show(&moves(&old, &beside)), "");
  // it stood at two places and stands at one: either may be the one that left
  let both = text(&[&ALPHA, &["between();"], &ALPHA, &BLANK, &stay]);
  assert_eq!(show(&moves(&both, &text(&[&stay, &BLANK, &ALPHA]))), "");
  // its first and its last line at the first place too, all of it at the second: the lines between them stand
  // once, and say that the second place took it
  let part = text(&[&stay[..4], &[ALPHA[0], "}"], &stay[4..], &ALPHA, &["end();"]]);
  assert_eq!(show(&moves(&old, &part)), "0..5 -> 10..15");
  // with a line edited at the first place and as it was at the second: that line stands once, at the second
  let edited = [ALPHA[0], "  let total = compute_total(items, verbose);", ALPHA[2], ALPHA[3], ALPHA[4]];
  let either = text(&[&stay[..4], &edited, &stay[4..], &ALPHA, &["end();"]]);
  assert_eq!(show(&moves(&old, &either)), "0..5 -> 13..18");
  // the lines that stand once count, the others do not: a short line of its own among lines that stand twice is no
  // move, a long one is
  for (own, expected) in [("  short();", ""), ("  a_line_of_its_own_that_is_long_enough();", "0..5 -> 12..17")] {
    let moved = [ALPHA[0], ALPHA[1], own, ALPHA[3], ALPHA[4]];
    let copy = [ALPHA[0], ALPHA[1], ALPHA[3], ALPHA[4]];
    let old = text(&[&moved, &BLANK, &stay, &["end();"]]);
    let new = text(&[&stay[..4], &copy, &stay[4..], &moved, &["end();"]]);
    assert_eq!(show(&moves(&old, &new)), expected, "{own}");
  }
}

#[test]
fn a_line_is_in_one_move_and_the_rest_of_a_longer_one_is_asked_again() {
  // two functions leave one place for two places
  let old = text(&[&BETA, &BLANK, &ALPHA, &BLANK, &GAMMA, &BLANK, &DELTA, &BLANK, &["end"]]);
  let apart = text(&[&ALPHA, &BLANK, &BETA, &BLANK, &DELTA, &BLANK, &GAMMA, &BLANK, &["end"]]);
  let found = moves(&old, &apart);
  assert_eq!(found.len(), 2, "{}", show(&found));
  // three functions deleted in one block; the first two stand at one place, the last two at another: the first place
  // takes its two, and of the other move the function that is left
  let old = text(&[&["head"], &ALPHA, &GAMMA, &DELTA, &["middle"], &BETA, &["tail"]]);
  let new = text(&[&["head"], &["middle"], &ALPHA, &GAMMA, &BETA, &GAMMA, &DELTA, &["tail"]]);
  let blocks = [block(1..14, 1..1), block(15..15, 2..12), block(20..20, 17..25)];
  let found = moves_of(&old, &new, &blocks, false);
  assert_eq!(show(&found), "1..11 -> 2..12; 11..14 -> 22..25");
}

#[test]
fn a_copy_is_no_move() {
  // the function stays where it was and stands a second time
  let old = four();
  let copied = text(&[&BETA, &BLANK, &ALPHA, &BLANK, &GAMMA, &BLANK, &ALPHA, &BLANK, &DELTA]);
  assert_eq!(show(&moves(&old, &copied)), "");
  // one of two copies deleted
  assert_eq!(show(&moves(&copied, &old)), "");
}

#[test]
fn one_long_line_is_a_move_and_short_lines_are_none() {
  let import = "use crate::model::{BlockKind, DiffBlock};";
  let (old, new) = below(&[import], &[import]);
  assert_eq!(show(&moves(&old, &new)), "0..1 -> 8..9");
  // the exact lines hold 24 characters that are no whitespace, or the lines are no move
  let line = |solid: usize| format!("let x = {};", "1".repeat(solid - "letx=;".len()));
  for (solid, expected) in [(MIN_MOVE_CHARS - 1, ""), (MIN_MOVE_CHARS, "0..1 -> 8..9")] {
    let moved = line(solid);
    let (old, new) = below(&[&moved], &[&moved]);
    assert_eq!(show(&moves(&old, &new)), expected, "{solid}");
  }
  // short lines count together, a blank line counts nothing
  let (old, new) = below(&["count += 1;", "", "total += 2;"], &["count += 1;", "", "total += 2;"]);
  assert_eq!(show(&moves(&old, &new)), "");
  let short = ["count += 1;", "", "total += 2;", "limit -= 3;"];
  let (old, new) = below(&short, &short);
  assert_eq!(show(&moves(&old, &new)), "0..4 -> 8..12");
  // the characters of an edited line do not count
  let long = ["count += 1;", "let total = compute_total(items);", "limit -= 3;", "x();"];
  let (old, new) = below(&long, &["count += 1;", "let total = compute_total(rows);", "limit -= 3;", "x();"]);
  assert_eq!(show(&moves(&old, &new)), "");
  let (old, new) = below(&long, &long);
  assert_eq!(show(&moves(&old, &new)), "0..4 -> 8..12");
}

#[test]
fn the_blank_lines_inside_a_move_are_lines_of_it() {
  let spaced = [
    "fn alpha(items: &[Item]) -> u64 {",
    "  let total = compute_total(items);",
    "",
    "  \t",
    "  report(total);",
    "",
    "  total",
    "}",
  ];
  let old = text(&[&BETA, &BLANK, &spaced, &BLANK, &GAMMA, &BLANK, &DELTA]);
  let new = text(&[&BETA, &BLANK, &GAMMA, &BLANK, &spaced, &BLANK, &DELTA]);
  assert_eq!(show(&moves(&old, &new)), "6..15 -> 12..21");
  // a blank line pairs with any blank line, whatever its whitespace, and with no other line; whitespace of it that
  // changed is a range where every character counts
  let other = [spaced[0], spaced[1], "    ", "", spaced[4], "", spaced[6], spaced[7]];
  let new = text(&[&BETA, &BLANK, &GAMMA, &BLANK, &other, &BLANK, &DELTA]);
  assert_eq!(show(&moves(&old, &new)), "6..15 -> 12..21 edited 2: [0, 0, 0]/[0, 4, 0] edited 3: [0, 3, 1]/[0, 0, 1]");
  assert_eq!(show(&moves_under(&old, &new, WhitespacePolicy::Trim, false)), "6..15 -> 12..21");
  // a blank line indented with the lines of its move is the same line
  let inner = indented(&[spaced[0], spaced[1], "", "  ", spaced[4], spaced[7]], "    ");
  let (old_inner, new_inner) = below(
    &[spaced[0], spaced[1], "", "  ", spaced[4], spaced[7]],
    &[&inner[0], &inner[1], "", "      ", &inner[4], &inner[5]],
  );
  assert_eq!(show(&moves(&old_inner, &new_inner)), "0..6 -> 8..14 indent +4");
  let closed = [spaced[0], spaced[1], spaced[4], "", spaced[6], spaced[7]];
  assert_eq!(show(&moves(&old, &text(&[&BETA, &BLANK, &GAMMA, &BLANK, &closed, &BLANK, &DELTA]))), "6..8 -> 12..14");
  // blank lines alone are no move
  let blanks = ["", "", "", ""];
  assert_eq!(show(&moves(&text(&[&blanks, &BETA, &GAMMA]), &text(&[&BETA, &blanks, &GAMMA]))), "");
}

/// The comparison deletes a function with the blank line on one side of it and adds it with the blank line on the
/// other: the move counts the blank lines beside it, as many as both blocks have, and the block the lines left has
/// nothing to read
#[test]
fn the_blank_lines_that_moved_with_a_move_are_counted_beside_it() {
  // the old lines 1.. deleted, the new lines 2.. added after `middle`
  let found_under = |deleted: &[&[&'static str]], added: &[&[&'static str]], policy: WhitespacePolicy| {
    let parts = |before: &[&'static str], lines: &[&[&'static str]], after: &[&'static str]| -> Vec<&'static str> {
      before
        .iter()
        .copied()
        .chain(lines.iter().flat_map(|part| part.iter().copied()))
        .chain(after.iter().copied())
        .collect()
    };
    let (old, new) = (
      text(&[&parts(&["head"], deleted, &["middle", "tail"])]),
      text(&[&parts(&["head", "middle"], added, &["tail"])]),
    );
    let lines = |parts: &[&[&str]]| to_u32(parts.iter().map(|part| part.len()).sum());
    let blocks = TextDiff {
      blocks: vec![block(1..1 + lines(deleted), 1..1), block(2 + lines(deleted)..2 + lines(deleted), 2..2 + lines(added))],
      ..TextDiff::default()
    };
    let found = find_moves(&old, &new, &blocks.blocks, &[], policy, false);
    check(&old, &new, &blocks, &[], &found, policy, false);
    format!("{} starts {:?}", show(&found), starts_of(&blocks.blocks, &[], &found))
  };
  let found = |deleted: &[&[&'static str]], added: &[&[&'static str]]| found_under(deleted, added, WhitespacePolicy::Default);
  // deleted before it and added after it, and the other way round
  assert_eq!(found(&[&BLANK, &ALPHA], &[&ALPHA, &BLANK]), "2..7 -> 2..7 blanks 1+0 starts [1, 1, 2]");
  assert_eq!(found(&[&ALPHA, &BLANK], &[&BLANK, &ALPHA]), "1..6 -> 3..8 blanks 0+1 starts [1, 1, 2]");
  // on the same side of it the blank line is a line of the move
  assert_eq!(found(&[&BLANK, &ALPHA], &[&BLANK, &ALPHA]), "1..7 -> 2..8 starts [1, 1, 2]");
  assert_eq!(found(&[&BLANK, &ALPHA, &BLANK], &[&BLANK, &ALPHA, &BLANK]), "1..8 -> 2..9 starts [1, 1, 2]");
  // as many blank lines as both blocks have: a blank line deleted for good is read, an added one is an added line
  assert_eq!(found(&[&BLANK, &BLANK, &ALPHA], &[&ALPHA, &BLANK]), "3..8 -> 2..7 blanks 1+0 starts [0, 0, 1, 1, 1, 2]");
  assert_eq!(found(&[&BLANK, &ALPHA], &[&ALPHA, &BLANK, &BLANK]), "2..7 -> 2..7 blanks 1+0 starts [1, 1, 2]");
  assert_eq!(found(&[&BLANK, &BLANK, &ALPHA], &[&ALPHA, &BLANK, &BLANK]), "3..8 -> 2..7 blanks 2+0 starts [1, 1, 2]");
  assert_eq!(found(&[&BLANK, &ALPHA, &BLANK], &[&ALPHA]), "2..7 -> 2..7 starts [0, 0, 1, 1, 1, 2]");
  // a line between it and the move keeps a blank line where it is
  assert_eq!(found(&[&BLANK, &["gone();"], &ALPHA], &[&ALPHA, &BLANK]), "3..8 -> 2..7 starts [0, 0, 1, 1, 1, 2]");
  // a blank line with whitespace is a blank line: where every character counts it left with the move only as the
  // same text, else it is a deleted line to read; under a policy that leaves its whitespace out it left
  let spaced: [&str; 1] = ["  \t"];
  assert_eq!(found(&[&spaced, &ALPHA], &[&ALPHA, &BLANK]), "2..7 -> 2..7 starts [0, 0, 1, 1, 1, 2]");
  assert_eq!(found(&[&spaced, &ALPHA], &[&ALPHA, &spaced]), "2..7 -> 2..7 blanks 1+0 starts [1, 1, 2]");
  assert_eq!(
    found(&[&BLANK, &spaced, &ALPHA], &[&ALPHA, &spaced, &BLANK]),
    "3..8 -> 2..7 blanks 1+0 starts [0, 0, 1, 1, 1, 2]"
  );
  for policy in [WhitespacePolicy::Trim, WhitespacePolicy::Ignore] {
    assert_eq!(
      found_under(&[&spaced, &ALPHA], &[&ALPHA, &BLANK], policy),
      "2..7 -> 2..7 blanks 1+0 starts [1, 1, 2]",
      "{policy:?}"
    );
  }
  // blank lines of two moves: each move counts those beside it
  assert_eq!(
    found(&[&BLANK, &ALPHA, &["gone();"], &BLANK, &GAMMA], &[&GAMMA, &BLANK, &["came();"], &ALPHA, &BLANK]),
    "9..14 -> 2..7 blanks 1+0; 2..7 -> 9..14 blanks 1+0 starts [0, 0, 7, 1, 1, 2]"
  );
}

/// The empty line after the last line end of a text is no line the comparison counts, and it pairs with no line but
/// its like: where only one text ends with a line end, the block at the end of the texts holds it, and it is no pair
/// of a deleted blank line, no blank line that left with a move, and no line of a move of its own text
#[test]
fn the_empty_last_line_of_a_text_is_no_line_of_a_move() {
  let moved = "fun moved_function_with_long_name() = compute()";
  let stay = stay().join("\n");
  let found = |old: &str, new: &str| {
    let blocks = diff(old, new);
    let found = moves(old, new);
    format!("{} starts {:?}", show(&found), starts_of(&blocks.blocks, &[], &found))
  };
  // the function left with the blank line after it, for the end of a text that ends with a line end: the blank line
  // is a deleted line to read, not the pair of the empty last line of the new text
  let old = format!("{moved}\n\n{stay}\nold_last_line()");
  let new = format!("{stay}\n{moved}\n");
  assert_eq!(diff(&old, &new).blocks.last().map(|block| (block.new_start, block.new_end)), Some((8, 10)));
  assert_eq!(found(&old, &new), "0..1 -> 8..9 starts [0, 0, 1, 1, 1, 8]");
  // as where both texts end with a line end, and where neither does
  let (both_old, both_new) = (format!("{moved}\n\n{stay}\nold_last_line()\n"), format!("{stay}\n{moved}\n"));
  assert_eq!(found(&both_old, &both_new), "0..1 -> 8..9 starts [0, 0, 1, 1, 1, 8]");
  assert_eq!(
    found(both_old.trim_end_matches('\n'), both_new.trim_end_matches('\n')),
    "0..1 -> 8..9 starts [0, 0, 1, 1, 1, 8]"
  );
  // the blank line before it did not leave with it for the empty last line
  let old = format!("\n{moved}\n{stay}\nold_last_line()");
  assert_eq!(found(&old, &new), "1..2 -> 8..9 starts [0, 0, 0, 1, 1, 8]");
  // the empty last line of the old text is no line that moved with the function before it
  let old = format!("{stay}\n{moved}\n");
  let new = format!("{moved}\n\n{stay}\nnew_last_line()");
  assert_eq!(diff(&old, &new).blocks.last().map(|block| (block.old_start, block.old_end)), Some((8, 10)));
  assert_eq!(found(&old, &new), "8..9 -> 0..1 starts [0, 1, 0, 1, 1, 10]");
  // a blank line that is a line of both texts moves with it
  let (old, new) = (format!("{moved}\n\n{stay}\nlast()\n"), format!("{stay}\n{moved}\n\nlast()\n"));
  assert_eq!(found(&old, &new), "0..2 -> 8..10 starts [1, 1, 8]");
  // a text of one line that left for the end of the other, and an empty text: no line of it is in a move
  let (old, new) = (format!("{moved}\n"), format!("{stay}\n{moved}"));
  assert_eq!(show(&moves(&old, &new)), "");
  assert_eq!(show(&moves("", &new)), "");
  assert_eq!(show(&moves(&old, "")), "");
}

#[test]
fn lines_that_cross_nothing_are_no_move() {
  // the two functions change places and are indented: one block holds both, with no line between them that stays
  let old = text(&[&["head"], &ALPHA, &BETA, &["tail"]]);
  let new = text(&[&["head"], &refs(&indented(&BETA, " ")), &refs(&indented(&ALPHA, " ")), &["tail"]]);
  let blocks = diff(&old, &new);
  assert_eq!(blocks.blocks.len(), 1, "{:?}", blocks.blocks);
  assert_eq!(show(&moves(&old, &new)), "");
  // so beside another block of the file
  let (old, new) = (format!("{old}more\nfirst();\n"), format!("{new}more\nsecond();\n"));
  assert_eq!(diff(&old, &new).blocks.len(), 2);
  assert_eq!(show(&moves(&old, &new)), "");
  // the same lines in two blocks are a move
  let apart = [block(1..6, 1..1), block(11..11, 6..11)];
  assert_eq!(show(&moves_of(&old, &new, &apart, false)), "1..6 -> 6..11 indent +1");
}

#[test]
fn where_the_indentation_has_a_meaning_a_line_at_another_indent_is_edited() {
  const RULE: [&str; 6] = [
    "def build_library(name, sources, visibility):",
    "    native.cc_library(",
    "        name = name,",
    "        srcs = sources,",
    "        visibility = visibility,",
    "    )",
  ];
  let found = |now: &[&str], indentation_matters: bool| {
    let (old, new) = below(&RULE, now);
    show(&moves_where(&old, &new, indentation_matters))
  };
  // the same lines at the same indent, and all of them shifted alike: the move says the shift, in every file
  let shifted = indented(&RULE, "    ");
  for indentation_matters in [false, true] {
    assert_eq!(found(&RULE, indentation_matters), "0..6 -> 8..14", "{indentation_matters}");
    assert_eq!(found(&refs(&shifted), indentation_matters), "0..6 -> 8..14 indent +4", "{indentation_matters}");
  }
  // one line indented by other columns than the others: its nesting changed, an edited line whose range is its
  // indent; elsewhere the same range of a line that is no edited line of the quarter
  let nested = [RULE[0], RULE[1], RULE[2], "            srcs = sources,", RULE[4], RULE[5]];
  assert_eq!(found(&nested, false), "0..6 -> 8..14 edited 3: [0, 8, 2]/[0, 12, 2]");
  assert_eq!(found(&nested, true), "0..6 -> 8..14 edited 3: [0, 8, 2]/[0, 12, 2]");
  // its text edited too: both ranges; shifted with the others, only its text
  let both = [RULE[0], RULE[1], RULE[2], "            srcs = sources + extra,", RULE[4], RULE[5]];
  assert_eq!(found(&both, true), "0..6 -> 8..14 edited 3: [0, 8, 2, 22, 22, 0]/[0, 12, 2, 26, 34, 0]");
  let text_only = [RULE[0], RULE[1], RULE[2], "        srcs = sources + extra,", RULE[4], RULE[5]];
  assert_eq!(found(&text_only, true), "0..6 -> 8..14 edited 3: [22, 22, 0]/[22, 30, 0]");
  // a line that lost its whole indent, and one that got one
  let outdented = [RULE[0], RULE[1], RULE[2], "srcs = sources,", RULE[4], RULE[5]];
  assert_eq!(found(&outdented, true), "0..6 -> 8..14 edited 3: [0, 8, 1]/[0, 0, 1]");
  let (old, new) = below(&outdented, &RULE);
  assert_eq!(show(&moves_where(&old, &new, true)), "0..6 -> 8..14 edited 3: [0, 0, 0]/[0, 8, 0]");
  // more than a quarter of the lines at another indent: the lines before them alone, the first of as many
  let half = [RULE[0], RULE[1], "    name = name,", "    srcs = sources,", RULE[4], RULE[5]];
  assert_eq!(found(&half, true), "0..2 -> 8..10");
  assert_eq!(found(&half, false), "0..6 -> 8..14 edited 2: [0, 8, 2]/[0, 4, 2] edited 3: [0, 8, 2]/[0, 4, 2]");
  // the last line at another indent: an edited line ends no move
  let last = [RULE[0], RULE[1], RULE[2], RULE[3], RULE[4], ")"];
  assert_eq!(found(&last, true), "0..5 -> 8..13");
  assert_eq!(found(&last, false), "0..6 -> 8..14 edited 5: [0, 4, 1]/[0, 0, 1]");

  // a line that stayed at its indent while its move shifted is at another nesting, though no character of it
  // changed: an edited line without a range, never a range that is empty or the same text on both sides. A line at
  // the first column (the text of a string, a comment) and a line at an indent of its own
  let shifted_but = |rule: &[&str], stayed: &[usize]| -> Vec<String> {
    let mut lines = indented(rule, "    ");
    for line in stayed {
      lines[*line] = rule[*line].to_string();
    }
    lines
  };
  let first_column = [RULE[0], RULE[1], "name = name,", RULE[3], RULE[4], RULE[5]];
  let (old, new) = below(&first_column, &refs(&shifted_but(&first_column, &[2])));
  let stayed = moves_where(&old, &new, true);
  assert_eq!(show(&stayed), "0..6 -> 8..14 indent +4 edited 2");
  assert_eq!((&stayed[0].edited, stayed[0].old_ranges.len(), stayed[0].new_ranges.len()), (&vec![2], 0, 0));
  assert_eq!(found(&refs(&shifted_but(&RULE, &[3])), true), "0..6 -> 8..14 indent +4 edited 3");
  // where the indentation has no meaning such a line is the same line
  assert_eq!(show(&moves_where(&old, &new, false)), "0..6 -> 8..14 indent +4");
  assert_eq!(found(&refs(&shifted_but(&RULE, &[3])), false), "0..6 -> 8..14 indent +4");
  // two such lines of six are over a quarter
  assert_eq!(found(&refs(&shifted_but(&RULE, &[2, 3])), true), "0..2 -> 8..10 indent +4");
  assert_eq!(found(&refs(&shifted_but(&RULE, &[2, 3])), false), "0..6 -> 8..14 indent +4");

  // the indent is a text, not a number of columns: a tab that became a space is another indent, an edited line
  // whose range is its indent
  let tabbed = [
    "build_everything_target: deps",
    "\tcompile --all --verbose sources",
    "\tlink --output binary objects",
    "\tstrip binary",
    "\tsign binary",
    "\tpublish binary",
  ];
  let spaced = [tabbed[0], tabbed[1], " link --output binary objects", tabbed[3], tabbed[4], tabbed[5]];
  let (old, new) = below(&tabbed, &spaced);
  assert_eq!(show(&moves_where(&old, &new, true)), "0..6 -> 8..14 edited 2: [0, 1, 2]/[0, 1, 2]");
  assert_eq!(show(&moves_where(&old, &new, false)), "0..6 -> 8..14 edited 2: [0, 1, 2]/[0, 1, 2]");
  // every line of the recipe: no line of it moved as it was; the target alone, where the indentation has a meaning
  let recipe = [tabbed[0], " compile --all --verbose sources", " link --output binary objects"];
  let (old, new) = below(&tabbed[..3], &recipe);
  assert_eq!(show(&moves_where(&old, &new, true)), "0..1 -> 8..9");
  assert_eq!(
    show(&moves_where(&old, &new, false)),
    "0..3 -> 8..11 edited 1: [0, 1, 2]/[0, 1, 2] edited 2: [0, 1, 2]/[0, 1, 2]"
  );
  // two lines that gained two columns, one of spaces and one of tabs, did not move alike
  let mixed = [RULE[0], RULE[1], RULE[2], "\t\t        srcs = sources,", RULE[4], RULE[5]];
  let spaces = indented(&RULE, "  ");
  let (old, new) = below(&RULE, &[&spaces[0], &spaces[1], &spaces[2], mixed[3], &spaces[4], &spaces[5]]);
  assert_eq!(show(&moves_where(&old, &new, true)), "0..6 -> 8..14 indent +2 edited 3: [0, 8, 2]/[0, 10, 2]");
  // under a policy that leaves the whitespace at the ends of a line out, the indent is no change of a move
  for policy in [WhitespacePolicy::Trim, WhitespacePolicy::Ignore] {
    let (old, new) = below(&tabbed[..3], &recipe);
    assert_eq!(show(&moves_under(&old, &new, policy, true)), "0..3 -> 8..11", "{policy:?}");
  }
}

#[test]
fn two_blocks_that_change_places_are_one_move() {
  // the comparison keeps one of the two in place: the other moved
  let (old, new) = (text(&[&ALPHA, &BLANK, &BETA]), text(&[&BETA, &BLANK, &ALPHA]));
  assert_eq!(show(&moves(&old, &new)), "0..5 -> 6..11 blanks 0+1");
  let (old, new) = (
    text(&[&["head"], &ALPHA, &["middle"], &DELTA, &["tail"]]),
    text(&[&["head"], &DELTA, &["middle"], &ALPHA, &["tail"]]),
  );
  // with a line between them the shorter one is deleted and added, and the line between is too short for a move
  assert_eq!(show(&moves(&old, &new)), "7..10 -> 1..4");
}

#[test]
fn the_lines_of_a_run_and_of_a_block_that_is_no_change_are_in_no_move() {
  // a function deleted in one block; the other block changes a line and adds the function after it
  let old = text(&[&["head"], &ALPHA, &["middle"], &["let wrapped = call(first, second);"], &["tail"]]);
  let new = text(&[&["head"], &["middle"], &["let wrapped = call(first,"], &["  second);"], &ALPHA, &["tail"]]);
  let blocks = TextDiff {
    blocks: vec![block(1..6, 1..1), block(7..8, 2..9)],
    ..TextDiff::default()
  };
  assert_eq!(show(&moves_of(&old, &new, &blocks.blocks, false)), "1..6 -> 4..9");
  // a run says what became of its lines at their place, quiet or not: a move takes the lines beside it
  for quiet in [true, false] {
    for (runs, expected) in [
      (vec![run(7..8, 2..4, quiet)], "1..6 -> 4..9"),
      (vec![run(7..8, 2..5, quiet)], "2..6 -> 5..9"),
      (vec![run(7..8, 2..6, quiet)], ""),
      (vec![run(7..8, 2..4, quiet), run(8..8, 5..6, quiet)], "1..2 -> 4..5"),
    ] {
      let found = find_moves(&old, &new, &blocks.blocks, &runs, WhitespacePolicy::Default, false);
      check(&old, &new, &blocks, &runs, &found, WhitespacePolicy::Default, false);
      assert_eq!(show(&found), expected, "{runs:?}");
    }
  }
  // a block that is pale under its policy, and one the policy cut off
  for kind in [BlockKind::WhitespaceOnly, BlockKind::Ignored] {
    for side in 0..2 {
      let mut blocks = blocks.blocks.clone();
      blocks[side].kind = kind;
      assert_eq!(show(&moves_of(&old, &new, &blocks, false)), "", "{kind:?} {side}");
    }
  }
}

/// A file of `lines` lines that repeat `keys` keys, with `own` in place of every `every`th line
fn repeated(lines: usize, keys: usize, every: usize, own: impl Fn(usize) -> String) -> Vec<String> {
  (0..lines)
    .map(|index| {
      if every > 0 && index % every == 0 {
        own(index)
      } else {
        format!("    register(handler_{});", index % keys)
      }
    })
    .collect()
}

/// The lines `moved` deleted before ten lines that stay, and added after them: the two blocks of the move
fn moved_block(moved: &[String]) -> (String, String, [DiffBlock; 2]) {
  moved_sides(moved, moved, 10)
}

/// The lines `old` deleted before `stay` lines that stay, and the lines `new`, as many, added after them: the two
/// texts and the two blocks of their diff
fn moved_sides(old: &[String], new: &[String], stay: u32) -> (String, String, [DiffBlock; 2]) {
  let kept: Vec<String> = (0..stay).map(|index| format!("stay_{index}();")).collect();
  let lines = to_u32(old.len());
  (
    text(&[&refs(old), &refs(&kept)]),
    text(&[&refs(&kept), &refs(new)]),
    [block(0..lines, 0..0), block(lines + stay..lines + stay, stay..lines + stay)],
  )
}

fn to_u32(value: usize) -> u32 {
  u32::try_from(value).unwrap()
}

#[test]
fn a_key_that_many_lines_have_starts_no_chain_and_is_a_line_of_one() {
  // 10,000 lines of eight keys: no line starts a chain, and the search reads no pair
  let (old, new, blocks) = moved_block(&repeated(10_000, 8, 0, |_| String::new()));
  let (found, work) = find_moves_counted(&old, &new, &blocks, &[], WhitespacePolicy::Default, false);
  assert_eq!((show(&found).as_str(), work), ("", MoveWork::default()));
  // with a line of its own among every hundred, the lines are one move, read once
  let (old, new, blocks) = moved_block(&repeated(10_000, 8, 100, |index| format!("    section_{index}();")));
  let (found, work) = find_moves_counted(&old, &new, &blocks, &[], WhitespacePolicy::Default, false);
  assert_eq!(show(&found), "0..10000 -> 10..10010");
  assert!(work.search < 10_200 && work.choice == 10_000 && (work.compared, work.unread) == (0, 0), "{work:?}");
  // as many lines of a key as start a chain, and one more: the pairs of the first are read, of the others none.
  // Neither is a move: no line of them stands once
  for (copies, read) in [(MAX_KEY_CANDIDATES, true), (MAX_KEY_CANDIDATES + 1, false)] {
    let (old, new, blocks) = moved_block(&vec!["    register(handler_of_the_same_key);".to_string(); copies]);
    let (found, work) = find_moves_counted(&old, &new, &blocks, &[], WhitespacePolicy::Default, false);
    assert_eq!((show(&found).as_str(), work.search > 0), ("", read), "{copies}: {work:?}");
  }
}

#[test]
fn the_work_of_a_search_grows_with_its_lines_and_ends_at_its_limit() {
  // 10,000 lines of five keys with a line of its own among every hundred, and among them fifty lines that each stand
  // sixteen times, 625 lines apart: each of them pairs with its sixteen places, along 31 diagonals that the repeated
  // lines follow for hundreds of lines
  let mut lines = repeated(10_000, 5, 100, |index| format!("    section_{index}();"));
  for own in 0..50 {
    for copy in 0..16 {
      lines[copy * 625 + own * 5] = format!("    marker_{own}();");
    }
  }
  let (old, new, blocks) = moved_block(&lines);
  let (found, work) = find_moves_counted(&old, &new, &blocks, &[], WhitespacePolicy::Default, false);
  assert_eq!(show(&found), "0..10000 -> 10..10010");
  // every pair of two lines of a key is asked once, and a pair of lines is read in one chain: some seventeen pairs a
  // line, and the lines of the sixteen longest chains for the choice. Within the limit, so the whole move is found
  assert!(work.search > 100_000 && work.search < 200_000 && work.choice <= 160_000, "{work:?}");
  assert!(work.search.max(work.choice) < MAX_MOVE_WORK, "{work:?}");
  // the answer is the same when asked again
  assert_eq!(find_moves_counted(&old, &new, &blocks, &[], WhitespacePolicy::Default, false), (found, work));

  // at the limit of its work the search ends with the moves found until then: here the lines before the first
  // marker of the block, which the first chain read before the work ran out
  let limits = Limits { search: 1_000, ..LIMITS };
  let (found, work) = find_moves_within(&old, &new, &blocks, &[], WhitespacePolicy::Default, false, &limits);
  assert_eq!((work.search, work.choice), (1_000, 1_000), "{work:?}");
  assert_eq!(show(&found), "0..1000 -> 10..1010");

  // the choice ends at its limit too, with the moves taken until then: the longest. Three functions deleted in one
  // block, the first two at one place, the last two at another
  let old = text(&[&["head"], &ALPHA, &GAMMA, &DELTA, &["middle"], &BETA, &["tail"]]);
  let new = text(&[&["head"], &["middle"], &ALPHA, &GAMMA, &BETA, &GAMMA, &DELTA, &["tail"]]);
  let blocks = [block(1..14, 1..1), block(15..15, 2..12), block(20..20, 17..25)];
  let (found, work) = find_moves_within(&old, &new, &blocks, &[], WhitespacePolicy::Default, false, &LIMITS);
  // the move of ten lines, the one of eight that shares five with it, and what is left of that one
  assert_eq!((show(&found).as_str(), work.choice), ("1..11 -> 2..12; 11..14 -> 22..25", 21));
  for (choice, expected) in [(10, "1..11 -> 2..12"), (18, "1..11 -> 2..12"), (19, "1..11 -> 2..12; 11..14 -> 22..25")] {
    let (found, _) = find_moves_within(&old, &new, &blocks, &[], WhitespacePolicy::Default, false, &Limits { choice, ..LIMITS });
    assert_eq!(show(&found), expected, "{choice}");
  }
}

/// The file a search costs most in: every line stands sixteen times on each side and no two lines follow each other
/// on both, so each pair of two lines of a key is a chain of its own that ends at once. The search reads
/// `MAX_MOVE_WORK` pairs and ends, whatever the lines are, the same when asked again: the cost of a file has a
/// ceiling of work (docs/design/diff-view.md has its time). It finds no move: no line of such a file stands once
#[test]
fn the_search_of_a_file_whose_every_line_stands_sixteen_times_ends_at_its_work() {
  const LINES: usize = 8_192;
  let keys = LINES / MAX_KEY_CANDIDATES;
  // lines that are nothing alike, and lines that differ in a number, each of which may be an edited line of a chain
  let unlike = |key: usize| format!("{:08x}", key * 2_654_435_761 % 0xffff_fffb).repeat(5);
  let alike = |key: usize| format!("    registry.register(handler_{key:05}, option, option, option);");
  for line in [&unlike as &dyn Fn(usize) -> String, &alike] {
    let side = |key: &dyn Fn(usize) -> usize| (0..LINES).map(|index| line(key(index))).collect::<Vec<String>>();
    // seven keys on from line to line on the new side: a line that follows another on one side never does on the other
    let (old, _, blocks) = moved_block(&side(&|index| index % keys));
    let (_, new, _) = moved_block(&side(&|index| index * 7 % keys));
    let (found, work) = find_moves_counted(&old, &new, &blocks, &[], WhitespacePolicy::Default, false);
    // at its limit, and past it by the last pair of long lines at most
    assert!((MAX_MOVE_WORK..MAX_MOVE_WORK + 16).contains(&work.search), "{work:?}");
    assert!(
      work.choice <= MAX_MOVE_WORK && work.compared <= MAX_COMPARED_PAIRS && work.unread <= MAX_UNREAD_PAIRS,
      "{work:?}"
    );
    // and names no move: no line of the file stands once
    assert!(found.is_empty(), "{}", found.len());
    assert_eq!(find_moves_counted(&old, &new, &blocks, &[], WhitespacePolicy::Default, false), (found, work));
  }
}

/// A list of lines that are all alike, sorted another way: every line stands at another place, and every row of the
/// two blocks is one chain through pairs that may be similar. The chains grow through `MAX_UNREAD_PAIRS` such pairs
/// and then end at them, so the search stays within its work and every line is found as the move it is
#[test]
fn a_sorted_list_of_lines_that_are_alike_is_searched_within_the_pairs_left_unread() {
  const LINES: usize = 2_000;
  let line = |index: usize| format!("    registry.register(handler_{index:04}, options);");
  // seven places on from line to line: no two lines follow each other on both sides
  let (old, _, blocks) = moved_block(&(0..LINES).map(line).collect::<Vec<String>>());
  let (_, new, _) = moved_block(&(0..LINES).map(|index| line(index * 7 % LINES)).collect::<Vec<String>>());
  let (found, work) = find_moves_counted(&old, &new, &blocks, &[], WhitespacePolicy::Default, false);
  assert_eq!((found.len(), work.unread, work.compared), (LINES, MAX_UNREAD_PAIRS, 0), "{work:?}");
  assert!(work.search < MAX_MOVE_WORK / 4, "{work:?}");
  assert!(found.iter().all(|line_move| line_move.len == 1 && line_move.edited.is_empty()));
  check_of(&old, &new, &blocks, &found);
  // without the limit the chains use up the work of the search, and the lines after them are not found
  let limits = Limits { unread: u32::MAX, ..LIMITS };
  let (unbound, work) = find_moves_within(&old, &new, &blocks, &[], WhitespacePolicy::Default, false, &limits);
  assert!(work.search >= MAX_MOVE_WORK && unbound.len() < LINES, "{} moves, {work:?}", unbound.len());
}

/// A row of many short moves: a list whose items moved with a line of each edited, or indented each line by other
/// columns where the indentation has a meaning. The longest row of such a chain is one line or two, and what is left
/// of the chain after it is nearly all of it: read again for each move, the chain costs the square of its lines.
/// It is read once for all its moves, the work of the search grows with the lines, and every move is found
#[test]
fn a_chain_of_many_short_moves_is_read_once_for_all_of_them() {
  // the search within the limits, and with every row read again for each of its moves, as a short row is
  let again = Limits { short_row: usize::MAX, ..LIMITS };
  let found_within = |old: &[String], new: &[String], indentation_matters: bool, limits: &Limits| {
    let (old, new, blocks) = moved_sides(old, new, 10);
    let (found, work) = find_moves_within(&old, &new, &blocks, &[], WhitespacePolicy::Default, indentation_matters, limits);
    let diff = TextDiff {
      blocks: blocks.to_vec(),
      ..TextDiff::default()
    };
    check(&old, &new, &diff, &[], &found, WhitespacePolicy::Default, indentation_matters);
    (found, work)
  };

  // a list of YAML, each item of five lines, moved and indented again by two columns a level in place of four: a
  // chain has the shift of the line that started it, and the lines of the other levels are edited lines of it
  let items = |count: usize, step: usize| -> Vec<String> {
    (0..count)
      .flat_map(|index| {
        [
          format!("{}- name: service_number_{index}_of_the_list", " ".repeat(step)),
          format!("{}spec:", " ".repeat(2 * step)),
          format!("{}containers:", " ".repeat(3 * step)),
          format!("{}image: registry/service_{index}:latest", " ".repeat(4 * step)),
          format!("{}port: {}", " ".repeat(4 * step), 8_000 + index),
        ]
      })
      .collect()
  };
  const ITEMS: usize = 2_000;
  let (old, new) = (items(ITEMS, 4), items(ITEMS, 2));
  let (found, work) = found_within(&old, &new, true, &LIMITS);
  // the name of each item, and its image with its port
  assert_eq!(show(&found[..2]), "0..1 -> 10..11 indent -2; 3..5 -> 13..15 indent -8");
  assert_eq!(found.len(), 2 * ITEMS);
  // two chains of all the lines: each pair read as the chain grows, once more for the reach of its starts, and
  // each start asked once
  assert!(work.search > 4 * 5 * ITEMS as u64 && work.search < 6 * 5 * ITEMS as u64, "{work:?}");
  // read again for each move, the first chain alone uses up the work of the search
  let (found_again, work_again) = found_within(&old, &new, true, &again);
  assert!(
    work_again.search >= MAX_MOVE_WORK && found_again.len() < ITEMS / 20,
    "{} moves, {work_again:?}",
    found_again.len()
  );
  // where the indentation has no meaning the same lines are one move, an indent of each item a range
  let (found, work) = found_within(&old, &new, false, &LIMITS);
  assert_eq!((found.len(), found[0].len as usize), (1, 5 * ITEMS));
  assert!(work.search < 2 * 5 * ITEMS as u64 + 100, "{work:?}");

  // every second line of a file whose indentation has a meaning under another parent, the lines between them at
  // their indent: no two lines in a row moved alike, each line is a move of its own
  const LINES: usize = 6_000;
  let side = |indent: &str| -> Vec<String> {
    (0..LINES)
      .map(|index| {
        if index % 2 == 0 {
          format!("{indent}key_number_{index}_of_the_block: value")
        } else {
          format!("  nested_key_number_{index}: value")
        }
      })
      .collect()
  };
  let (found, work) = found_within(&side(""), &side("  "), true, &LIMITS);
  assert_eq!(found.len(), LINES);
  assert!(found.iter().all(|line_move| line_move.len == 1 && line_move.edited.is_empty()));
  assert!(work.search <= 6 * LINES as u64, "{work:?}");
  let (found_again, work_again) = found_within(&side(""), &side("  "), true, &again);
  assert!(
    work_again.search >= MAX_MOVE_WORK && found_again.len() < LINES / 20,
    "{} moves, {work_again:?}",
    found_again.len()
  );

  // a line of its own, a line with another number, which a chain grows through unread, and two blank lines, again
  // and again: a move is the line of its own with the blank lines before it, and the line with the number is not
  // compared at all
  const GROUPS: usize = 2_500;
  let side = |number: usize| -> Vec<String> {
    (0..GROUPS)
      .flat_map(|index| {
        [
          format!("  open_section_number_{index}_of_the_file();"),
          format!("  let total_{index} = compute_total_of(items, {number});"),
          String::new(),
          String::new(),
        ]
      })
      .collect()
  };
  let (found, work) = found_within(&side(1), &side(2), false, &LIMITS);
  assert_eq!(show(&found[..2]), "0..1 -> 10..11; 2..5 -> 12..15");
  assert_eq!((found.len(), work.compared, work.unread as usize), (GROUPS, 0, GROUPS));
  assert!(work.search < 4 * 4 * GROUPS as u64, "{work:?}");
  let (found_again, work_again) = found_within(&side(1), &side(2), false, &again);
  assert!(
    work_again.search >= MAX_MOVE_WORK && found_again.len() < GROUPS / 10,
    "{} moves, {work_again:?}",
    found_again.len()
  );

  // at the limit of its work the search ends with the moves found until then
  let limits = Limits { search: 24_000, ..LIMITS };
  let (found, work) = found_within(&side(1), &side(2), false, &limits);
  assert_eq!(work.search, limits.search, "{work:?}");
  assert!(!found.is_empty() && found.len() < GROUPS, "{}", found.len());
}

#[test]
fn a_side_over_its_lines_and_pairs_over_their_number_are_not_read() {
  // more lines that a move may take than the limit: no move
  let (old, new, blocks) = moved_block(&repeated(MAX_MOVE_LINES + 1, 8, 100, |index| format!("    section_{index}();")));
  assert_eq!(
    find_moves_counted(&old, &new, &blocks, &[], WhitespacePolicy::Default, false),
    (Vec::new(), MoveWork::default())
  );
  let (old, new, blocks) = moved_block(&repeated(MAX_MOVE_LINES, 8, 100, |index| format!("    section_{index}();")));
  assert_eq!(show(&moves_of(&old, &new, &blocks, false)), format!("0..{MAX_MOVE_LINES} -> 10..{}", MAX_MOVE_LINES + 10));

  // groups of five lines, the third with an argument added, and a line of its own after each: a row of each
  // chain may be a move with its third line edited, so the pair is compared by word
  let groups = MAX_COMPARED_PAIRS as usize + 50;
  let side = |edited: bool| -> Vec<String> {
    (0..groups)
      .flat_map(|index| {
        [
          format!("    let section_{index} = open_section({index});"),
          format!("    prepare(section_{index});"),
          if edited {
            format!("    close(section_{index}, now);")
          } else {
            format!("    close(section_{index});")
          },
          format!("    report(section_{index});"),
          format!("    forget(section_{index});"),
          if edited {
            format!("    {index}_is_a_line_of_the_new_side_that_is_nothing_like_the_old_one;")
          } else {
            format!("    other_{index}();")
          },
        ]
      })
      .collect()
  };
  let (old, _, blocks) = moved_block(&side(false));
  let (_, new, _) = moved_block(&side(true));
  let (found, work) = find_moves_counted(&old, &new, &blocks, &[], WhitespacePolicy::Default, false);
  assert_eq!(work.compared, MAX_COMPARED_PAIRS, "{work:?}");
  // the groups whose pair was compared are a move of five lines with an edited one; a pair past the number is
  // not read and no edited line: the two lines before it and the two after it are two moves
  assert_eq!(found.len(), MAX_COMPARED_PAIRS as usize + 2 * 50);
  assert_eq!(show(&found[..1]), "0..5 -> 10..15 edited 2: [19, 19, 0]/[19, 24, 0]");
  let last = to_u32(6 * (groups - 1));
  assert_eq!(
    show(&found[found.len() - 2..]),
    format!(
      "{}..{} -> {}..{}; {}..{} -> {}..{}",
      last,
      last + 2,
      last + 10,
      last + 12,
      last + 3,
      last + 5,
      last + 13,
      last + 15
    )
  );
  // a pair in a row that is no move whatever the pair is costs no comparison: an edited line at the end of its row
  let ends = |edited: bool| -> Vec<String> {
    (0..100)
      .flat_map(|index| {
        [
          format!("    let section_{index} = open_section({index});"),
          if edited {
            format!("    close(section_{index}, now);")
          } else {
            format!("    close(section_{index});")
          },
          if edited {
            format!("    {index}_is_a_line_of_the_new_side_that_is_nothing_like_the_old_one;")
          } else {
            format!("    other_{index}();")
          },
        ]
      })
      .collect()
  };
  let (old, _, blocks) = moved_block(&ends(false));
  let (_, new, _) = moved_block(&ends(true));
  let (found, work) = find_moves_counted(&old, &new, &blocks, &[], WhitespacePolicy::Default, false);
  assert_eq!((found.len(), work.compared), (100, 0), "{work:?}");
  // a line longer than the comparison reads is an edited line of no move
  let long = |name: &str| format!("    let {name} = [{}];", "value, ".repeat(100));
  let (first, second) = (long("first"), long("second"));
  let old = text(&[&[ALPHA[0], ALPHA[1], first.as_str(), ALPHA[3], ALPHA[4]], &["stay();"]]);
  let new = text(&[&["stay();"], &[ALPHA[0], ALPHA[1], second.as_str(), ALPHA[3], ALPHA[4]]]);
  let (found, work) = find_moves_counted(&old, &new, &[block(0..5, 0..0), block(6..6, 1..6)], &[], WhitespacePolicy::Default, false);
  assert_eq!((show(&found).as_str(), work.compared), ("0..2 -> 1..3", 0));
}

/// A text of up to 14 lines of a few long and short lines, some of them the same but for their indent, the
/// whitespace inside them or a word
fn lines() -> impl Strategy<Value = String> {
  crate::test_support::text_of_lines(
    vec![
      "let total = compute_total(items);",
      "  let total = compute_total(items);",
      "    let total = compute_total(items);",
      "let total = compute_total( items );",
      "let total = compute_sum(items);",
      "report(total, listeners, verbose);",
      "\treport(total, listeners, verbose);",
      "report(total, listeners);",
      "fn alpha(items: &[Item]) -> u64 {",
      "}",
      "  }",
      "",
      "  ",
      "x();",
      "let 😂 = \"a string of some words\";",
      "let total = compute_total(items);  ",
      "\t\tlet total = compute_total(items);",
      "\t  let total = compute_total(items);",
      "\t",
    ],
    vec!["\n", "\n", "\r\n", "\r"],
    14,
  )
}

/// Up to 16 lines deleted in one block and added in another, each line of the new block changed in one way or not
/// at all: indented by two more columns, a word of it another, another line, blank on both sides, with other
/// whitespace inside, with whitespace after it, with tabs for its indent, without its indent, or with a tab before it
fn moved_lines() -> impl Strategy<Value = (String, String, [DiffBlock; 2])> {
  proptest::collection::vec(0..18u8, 1..16).prop_map(|changes| {
    let line = |index: usize, argument: &str| format!("  let value_{index} = compute_{index}({argument}, limit);");
    let old: Vec<String> = changes
      .iter()
      .enumerate()
      .map(|(index, change)| if *change == 12 { String::new() } else { line(index, "input") })
      .collect();
    let new: Vec<String> = changes
      .iter()
      .enumerate()
      .map(|(index, change)| match change {
        9 => format!("  {}", line(index, "input")),
        10 => line(index, "other"),
        11 => format!("  return finish_{index}();"),
        12 => "  ".to_string(),
        13 => line(index, "input").replace(", limit", ",  limit"),
        14 => format!("{} ", line(index, "input")),
        15 => format!("\t\t{}", line(index, "input").trim_start()),
        16 => line(index, "input").trim_start().to_string(),
        17 => format!("\t{}", line(index, "input")),
        _ => line(index, "input"),
      })
      .collect();
    moved_sides(&old, &new, 3)
  })
}

/// Up to 90 lines deleted in one block and added in another, the pairs of one chain: each pair the same line (long,
/// or too short to be a move alone), a line with another argument (similar by word), a line with its words in
/// another order (alike by its characters, not similar), a blank line, the same line at another indent, or another
/// line. Most of them the same
/// line, so the longest move holds most of a row, or (`sparse`) a third of them, so a row holds many short moves
fn chain_of_pairs() -> impl Strategy<Value = (String, String, [DiffBlock; 2])> {
  // what a pair is, of 24: by the first number of its kind
  const DENSE: [(u8, u8); 7] = [(0, 0), (14, 1), (16, 2), (18, 3), (20, 4), (22, 5), (23, 6)];
  const SPARSE: [(u8, u8); 7] = [(0, 0), (6, 1), (8, 2), (13, 3), (17, 4), (20, 5), (23, 6)];
  (proptest::bool::ANY, proptest::collection::vec(0..24u8, 1..90)).prop_map(|(sparse, numbers)| {
    let kinds = if sparse { SPARSE } else { DENSE };
    let kind_of = |number: u8| kinds.iter().rev().find(|(first, _)| *first <= number).map_or(0, |(_, kind)| *kind);
    let line = |index: usize, argument: &str| format!("  let value_{index} = compute_{index}({argument}, limit);");
    let words = |index: usize, turned: bool| {
      let mut words = ["alpha_one", "beta_two", "gamma_three", "delta_four"];
      if turned {
        words.reverse();
      }
      format!("  put_{index}({});", words.join(", "))
    };
    let old: Vec<String> = numbers
      .iter()
      .enumerate()
      .map(|(index, number)| match kind_of(*number) {
        1 => format!("  x{index}();"),
        3 => words(index, false),
        4 => String::new(),
        _ => line(index, "input"),
      })
      .collect();
    let new: Vec<String> = numbers
      .iter()
      .enumerate()
      .map(|(index, number)| match kind_of(*number) {
        1 => format!("  x{index}();"),
        2 => line(index, "other"),
        3 => words(index, true),
        4 => String::new(),
        5 => format!("  {}", line(index, "input")),
        6 => format!("  return finish_{index}();"),
        _ => line(index, "input"),
      })
      .collect();
    moved_sides(&old, &new, 3)
  })
}

proptest! {
  #![proptest_config(proptest_config(1500))]

  /// The moves of lines that moved with a few changes are moves (`check`), where the indentation has a meaning and
  /// where it has none; lines that did not change at all are one move
  #[test]
  fn the_moves_of_moved_lines_are_moves((old, new, blocks) in moved_lines()) {
    let diff = TextDiff { blocks: blocks.to_vec(), ..TextDiff::default() };
    for indentation_matters in [false, true] {
      for policy in [WhitespacePolicy::Default, WhitespacePolicy::Trim] {
        let found = find_moves(&old, &new, &blocks, &[], policy, indentation_matters);
        check(&old, &new, &diff, &[], &found, policy, indentation_matters);
        if old.lines().take(blocks[0].old_end as usize).eq(new.lines().skip(3)) {
          assert_eq!(show(&found), format!("0..{} -> 3..{}", blocks[0].old_end, blocks[1].new_end));
        }
      }
    }
  }

  /// The moves of any two texts are moves (`check`), the same when asked again, under each policy of the blocks
  #[test]
  fn the_moves_of_any_texts_are_moves(old in lines(), new in lines()) {
    for policy in [WhitespacePolicy::Default, WhitespacePolicy::Ignore, WhitespacePolicy::IgnoreBlankLines] {
      let blocks = diff_texts(&old, &new, policy, &DiffBudget::NONE);
      for indentation_matters in [false, true] {
        let found = find_moves(&old, &new, &blocks.blocks, &[], policy, indentation_matters);
        check(&old, &new, &blocks, &[], &found, policy, indentation_matters);
        assert_eq!(found, find_moves(&old, &new, &blocks.blocks, &[], policy, indentation_matters));
      }
    }
  }
}

proptest! {
  #![proptest_config(proptest_config(400))]

  /// The moves of a chain are the same whether what is left of a row is read again for each of its moves, or the
  /// row is read once for all of them: the longest row first, of as long ones the first, the pairs that matter
  /// compared by word
  #[test]
  fn the_moves_of_a_chain_are_the_same_read_once_or_again_for_each((old, new, blocks) in chain_of_pairs()) {
    let diff = TextDiff { blocks: blocks.to_vec(), ..TextDiff::default() };
    for indentation_matters in [false, true] {
      let within = |short_row: usize| find_moves_within(&old, &new, &blocks, &[], WhitespacePolicy::Default, indentation_matters, &Limits { short_row, ..LIMITS });
      let (again, again_work) = within(usize::MAX);
      check(&old, &new, &diff, &[], &again, WhitespacePolicy::Default, indentation_matters);
      for short_row in [0, 4, LIMITS.short_row] {
        let (once, once_work) = within(short_row);
        assert_eq!(show(&once), show(&again), "{short_row}");
        assert_eq!(once, again, "{short_row}");
        assert_eq!((once_work.compared, once_work.unread), (again_work.compared, again_work.unread), "{short_row}");
      }
    }
  }
}
