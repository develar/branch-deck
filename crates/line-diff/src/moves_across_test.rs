//! The moves between the files of a subject (`moves_across.rs`): what a pair leaves to the other files, what is a
//! move between two files and what is none, which of two is taken, and the limits of the search by its work.

use crate::model::{DiffBudget, LineDiff, LineRun, RunKind, diff_texts};
use crate::moves::find_moves;
use crate::moves_across::{AcrossLimits, AcrossWork, FileRows, LeftRow, MAX_ACROSS_LINES, MAX_ACROSS_WORK, MoveAcross, find_moves_across, find_moves_across_within, left_rows};
use crate::policy::WhitespacePolicy;
use crate::test_support::text;
use pretty_assertions::assert_eq;
use std::fmt::Write as _;

const TITLE: [&str; 4] = [
  "fun reportTitle(ledger: Ledger, account: String): String {",
  "  val balance = ledger.balance(account)",
  "  return \"$account: $balance\"",
  "}",
];
const TOTAL: [&str; 4] = [
  "fun reportTotal(ledger: Ledger): Long {",
  "  val accounts = ledger.accounts()",
  "  return accounts.sumOf { ledger.balance(it) }",
  "}",
];
const KEPT: [&str; 3] = ["class Ledger {", "  fun clear() = entries.clear()", "}"];

fn diff(old: &str, new: &str) -> LineDiff {
  diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget::NONE)
}

/// What the pair of `old` and `new` leaves, with the moves within it found first
fn left(old: &str, new: &str) -> [Vec<LeftRow>; 2] {
  left_of(old, new, false)
}

/// `left` of a file whose indentation has a meaning (`indented`) or has none
fn left_of(old: &str, new: &str, indented: bool) -> [Vec<LeftRow>; 2] {
  let diff = diff(old, new);
  let moves = find_moves(old, new, &diff.blocks, &[], WhitespacePolicy::Default, indented);
  left_rows(old, new, &diff.blocks, &[], &moves, indented)
}

/// The moves between files whose sides are the texts `pairs`, each the old and the new text of a file
fn moves_of(pairs: &[(&str, &str)]) -> Vec<MoveAcross> {
  moves_between(pairs, false)
}

/// `moves_of` between files whose indentation has a meaning (`indented`) or has none
fn moves_between(pairs: &[(&str, &str)], indented: bool) -> Vec<MoveAcross> {
  let rows: Vec<[Vec<LeftRow>; 2]> = pairs.iter().map(|(old, new)| left_of(old, new, indented)).collect();
  let files: Vec<FileRows<'_>> = rows.iter().map(|[old, new]| FileRows { old, new }).collect();
  find_moves_across(&files).0
}

/// The lines of the rows: the first of each and the one after its last
fn lines_of(rows: &[LeftRow]) -> Vec<(u32, u32)> {
  rows.iter().map(|row| (row.start, row.start + u32::try_from(row.keys.len()).unwrap())).collect()
}

#[test]
fn a_pair_leaves_the_lines_of_its_changes_that_no_mark_took() {
  // the function left the file: its four lines, with the blank line before it cut off the row
  let old = text(&[&KEPT, &[""], &TITLE]);
  let new = text(&[&KEPT]);
  let [old_rows, new_rows] = left(&old, &new);
  assert_eq!((lines_of(&old_rows), lines_of(&new_rows)), (vec![(4, 8)], vec![]));
  assert_eq!(old_rows[0].keys.len(), 4);
  assert!(old_rows[0].keys.iter().all(|key| *key != 0));
  assert_eq!(old_rows[0].solid, [52, 34, 25, 1]);

  // an added file leaves all its lines but the empty one after its last line end; an indent and a trailing blank
  // are no part of a key
  let [none, added] = left("", &text(&[&TITLE]));
  assert_eq!((lines_of(&none), lines_of(&added)), (vec![], vec![(0, 4)]));
  let [_, indented] = left(
    "",
    "    fun reportTitle(ledger: Ledger, account: String): String {  \n\tval balance = ledger.balance(account)\n",
  );
  assert_eq!(indented[0].keys, added[0].keys[..2]);

  // a line of a run and a line of a move within the file are left by no pair
  let diff = diff(&old, &new);
  let run = LineRun {
    old_start: 4,
    old_end: 5,
    new_start: 3,
    new_end: 3,
    kind: RunKind::Comments,
    quiet: false,
  };
  let [cut, _] = left_rows(&old, &new, &diff.blocks, std::slice::from_ref(&run), &[], false);
  // the three lines after the run are enough by themselves
  assert_eq!(lines_of(&cut), [(5, 8)]);
  let [cut, _] = left_rows(&old, &new, &diff.blocks, &[LineRun { old_start: 5, old_end: 6, ..run }], &[], false);
  // a line before the run and two short ones after it are each too little
  assert_eq!(lines_of(&cut), []);
  let moved = text(&[&TITLE, &[""], &KEPT]);
  let [old_rows, new_rows] = left(&old, &moved);
  assert_eq!((lines_of(&old_rows), lines_of(&new_rows)), (vec![], vec![]), "the function moved within the file");
}

#[test]
fn a_line_at_another_depth_is_another_line_where_the_indentation_has_a_meaning() {
  const KEEP: &str = "def keep():\n    return 1\n\n";
  const OTHER: &str = "def total(ledger):\n    return sum(ledger.balances())\n\n";
  let report = |last: &str| {
    format!("def report(ledger, account):\n    if ledger.has_account(account):\n        balance = ledger.balance(account)\n{last}return format_title(account, balance)\n")
  };
  // the function left one file for another, where its last line is inside the `if`: it returns nothing otherwise
  let (flat, nested) = (report("    "), report("        "));
  let left_file = (format!("{KEEP}{flat}"), KEEP.to_string());
  let nested_file = (OTHER.to_string(), format!("{OTHER}{nested}"));
  let pairs = [(left_file.0.as_str(), left_file.1.as_str()), (nested_file.0.as_str(), nested_file.1.as_str())];
  // only the three lines at their depth are the same lines; the fourth is another, and no line of a move
  let three = MoveAcross {
    old_file: 0,
    old_start: 3,
    new_file: 1,
    new_start: 3,
    len: 3,
  };
  assert_eq!(moves_between(&pairs, true), [three]);
  // where the indentation has none, the indent of a line is no part of it: all four lines moved
  assert_eq!(moves_between(&pairs, false), [MoveAcross { len: 4, ..three }]);
  // the same function whole at its depth is one move there too, and whole at another depth it is none: every line
  // of it is nested otherwise
  let same_file = (OTHER.to_string(), format!("{OTHER}{flat}"));
  assert_eq!(
    moves_between(&[pairs[0], (same_file.0.as_str(), same_file.1.as_str())], true),
    [MoveAcross { len: 4, ..three }]
  );
  let deeper = flat.lines().fold(String::new(), |mut deeper, line| {
    writeln!(deeper, "    {line}").unwrap();
    deeper
  });
  let class_file = (OTHER.to_string(), format!("{OTHER}class Reports:\n{deeper}"));
  assert_eq!(moves_between(&[pairs[0], (class_file.0.as_str(), class_file.1.as_str())], true), []);
  // a tab is not the spaces it stands for, and a blank line has no depth: it is a line of the move at any indent
  let spaced = "def first(ledger):\n    total = ledger.total_of_all_accounts()\n\n    return total\n";
  let tabbed = spaced.replace("    return", "\treturn");
  let blank = spaced.replace("\n\n", "\n        \n");
  let whole = MoveAcross {
    old_file: 0,
    old_start: 0,
    new_file: 1,
    new_start: 0,
    len: 4,
  };
  assert_eq!(moves_between(&[(spaced, ""), ("", tabbed.as_str())], true), [MoveAcross { len: 2, ..whole }]);
  assert_eq!(moves_between(&[(spaced, ""), ("", blank.as_str())], true), [whole]);
}

#[test]
fn a_row_that_cannot_hold_a_move_between_files_is_not_kept() {
  // one line, however long; two lines of too few characters; blank lines alone
  for lines in [
    vec!["val message = \"a line that is long enough to be a move within a file, by itself\""],
    vec!["}", "}", "}"],
    vec!["", "", ""],
    vec!["return total", "}"],
  ] {
    let [none, rows] = left("", &text(&[&lines]));
    assert_eq!((none.len(), rows.len()), (0, 0), "{lines:?}");
  }
  // two lines of 48 characters together are
  let [_, rows] = left("", "val first = compute(alpha, beta)\nval second = compute(gamma, delta)\n");
  assert_eq!(lines_of(&rows), [(0, 2)]);
}

#[test]
fn lines_that_left_one_file_and_stand_in_another_are_a_move() {
  let ledger = (text(&[&KEPT, &[""], &TITLE]), text(&[&KEPT]));
  let report = (text(&[&TOTAL]), text(&[&TOTAL, &[""], &TITLE]));
  assert_eq!(
    moves_of(&[(&ledger.0, &ledger.1), (&report.0, &report.1)]),
    [MoveAcross {
      old_file: 0,
      old_start: 4,
      new_file: 1,
      new_start: 5,
      len: 4,
    }]
  );
  // whichever of the two files comes first, and into a file the change added
  assert_eq!(
    moves_of(&[("", &text(&[&TITLE])), (&ledger.0, &ledger.1)]),
    [MoveAcross {
      old_file: 1,
      old_start: 4,
      new_file: 0,
      new_start: 0,
      len: 4,
    }]
  );
}

#[test]
fn a_move_between_files_keeps_its_blank_lines_inside_and_is_at_another_indent() {
  let both: Vec<&str> = TITLE.iter().copied().chain([""]).chain(TOTAL).collect();
  let indented: Vec<String> = both.iter().map(|line| if line.is_empty() { String::new() } else { format!("  {line}") }).collect();
  let indented: Vec<&str> = indented.iter().map(String::as_str).collect();
  let old = text(&[&KEPT, &[""], &both]);
  let new = text(&[&["object Reports {"], &indented, &["}"]]);
  assert_eq!(
    moves_of(&[(&old, &text(&[&KEPT])), ("", &new)]),
    [MoveAcross {
      old_file: 0,
      old_start: 4,
      new_file: 1,
      new_start: 1,
      len: 9,
    }]
  );
}

#[test]
fn an_edited_line_ends_a_move_between_files() {
  // the first line changed on the way: the three lines after it are the move
  let mut edited = TOTAL;
  edited[0] = "fun reportSum(ledger: Ledger): Long {";
  let moves = moves_of(&[(&text(&[&KEPT, &TOTAL]), &text(&[&KEPT])), ("", &text(&[&edited]))]);
  assert_eq!(
    moves,
    [MoveAcross {
      old_file: 0,
      old_start: 4,
      new_file: 1,
      new_start: 1,
      len: 3,
    }]
  );
  // the second line changed: the line before it is too little by itself, and so are the two after it
  edited = TOTAL;
  edited[1] = "  val accounts = ledger.openAccounts()";
  assert_eq!(moves_of(&[(&text(&[&KEPT, &TOTAL]), &text(&[&KEPT])), ("", &text(&[&edited]))]), []);
}

#[test]
fn lines_that_match_two_places_are_a_move_to_the_first() {
  let old = (text(&[&KEPT, &TITLE]), text(&[&KEPT]));
  let copy = text(&[&TITLE]);
  // two files got the lines one file lost
  assert_eq!(
    moves_of(&[("", &copy), (&old.0, &old.1), ("", &copy)]),
    [MoveAcross {
      old_file: 1,
      old_start: 3,
      new_file: 0,
      new_start: 0,
      len: 4,
    }]
  );
  // two files lost the lines one file got: the first of them moved
  assert_eq!(
    moves_of(&[(&copy, ""), (&old.0, &old.1), ("", &copy)]),
    [MoveAcross {
      old_file: 0,
      old_start: 0,
      new_file: 2,
      new_start: 0,
      len: 4,
    }]
  );
}

#[test]
fn the_longest_move_is_taken_first_and_the_rest_of_a_row_is_asked_again() {
  // the first file lost both functions; the second got the title alone, the third both: the longest row takes the
  // third file, and the second keeps its added lines
  let both = text(&[&TITLE, &TOTAL]);
  assert_eq!(
    moves_of(&[(&both, ""), ("", &text(&[&TITLE])), ("", &both)]),
    [MoveAcross {
      old_file: 0,
      old_start: 0,
      new_file: 2,
      new_start: 0,
      len: 8,
    }]
  );
  // two files lost a function each, and one file got both in a row: two moves, each line once
  assert_eq!(
    moves_of(&[(&text(&[&TITLE]), ""), (&text(&[&TOTAL]), ""), ("", &both)]),
    [
      MoveAcross {
        old_file: 0,
        old_start: 0,
        new_file: 2,
        new_start: 0,
        len: 4,
      },
      MoveAcross {
        old_file: 1,
        old_start: 0,
        new_file: 2,
        new_start: 4,
        len: 4,
      },
    ]
  );
}

#[test]
fn boilerplate_and_lines_that_moved_within_a_file_are_no_move_between_files() {
  // the same short end of a class in two files
  let (first, second) = (text(&[&KEPT, &["  }", "}"]]), text(&[&TOTAL, &["  }", "}"]]));
  assert_eq!(moves_of(&[(&first, &text(&[&KEPT])), (&text(&[&TOTAL]), &second)]), []);
  // lines that left a file and stand again in the same file are its own
  let moved = (text(&[&TITLE, &KEPT]), text(&[&KEPT, &TITLE]));
  assert_eq!(moves_of(&[(&moved.0, &moved.1), ("", &text(&[&TOTAL]))]), []);
  // a line that stands in more than sixteen places starts no row, and lines that are all it are none
  let many = "val entry = registry.lookup(name) ?: return null\n".repeat(17);
  assert_eq!(moves_of(&[(&many, ""), ("", &many)]), []);
}

/// `count` lines that are each another text of 30 characters
fn numbered(prefix: &str, count: usize) -> String {
  let mut text = String::new();
  for index in 0..count {
    writeln!(text, "{prefix} = compute(value{index:06})").unwrap();
  }
  text
}

#[test]
fn the_search_is_bound_by_the_lines_of_its_files_and_by_the_pairs_it_reads() {
  let (big, small) = (numbered("val big", 40), numbered("val small", 4));
  let rows: Vec<[Vec<LeftRow>; 2]> = vec![left(&big, ""), left("", &big), left(&small, ""), left("", &small)];
  let files: Vec<FileRows<'_>> = rows.iter().map(|[old, new]| FileRows { old, new }).collect();
  let all = |limits: AcrossLimits| find_moves_across_within(&files, &limits);
  let moved = |old_file: u32, new_file: u32, len: u32| MoveAcross {
    old_file,
    old_start: 0,
    new_file,
    new_start: 0,
    len,
  };

  let (moves, work) = find_moves_across(&files);
  assert_eq!(moves, [moved(0, 1, 40), moved(2, 3, 4)]);
  assert_eq!(
    work,
    AcrossWork {
      lines: 88,
      skipped_files: 0,
      skipped_lines: 0,
      pairs: 88,
      cut: false,
    }
  );

  // the second big file does not fit beside the first: it takes no part, and the small ones after it do
  let (moves, work) = all(AcrossLimits { lines: 60, work: MAX_ACROSS_WORK });
  assert_eq!(moves, [moved(2, 3, 4)]);
  assert_eq!((work.lines, work.skipped_files, work.skipped_lines, work.cut), (48, 1, 40, false));

  // the search ends at its work with the moves found until then
  let (moves, work) = all(AcrossLimits {
    lines: MAX_ACROSS_LINES,
    work: 41,
  });
  assert_eq!(moves, [moved(0, 1, 40)]);
  assert!(work.cut);
  assert_eq!(work.pairs, 41);
}
