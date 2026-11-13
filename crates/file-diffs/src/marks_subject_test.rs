//! The pass over the files of a subject (`marks_subject.rs`): the lines that moved between two files, the names one
//! file declares and another uses, and what each answer of the index proves of them. Every file of a case is read as
//! the service reads a pair: its diff, its marks and what it left, from its two texts.

use crate::bindings::CodeSymbolBindings;
use crate::marks_test::MarkedDiff;
use code_tokens::tokens;
use diff_marks::index::{Access, DeclaredAt, MovedDeclaration, MovesAnswer, RenameAnswer, RenameConflict, RenamePair, RenameUse, RenameUses, RenamedDeclaration, Resolved};
use diff_marks::leftovers::{Leftovers, PendingBlock, PendingRun};
use diff_marks::marks::{is_quiet, marks_of, pair_grammar, pair_indented, without_moved_lines};
use diff_marks::marks_subject::{IndexAnswer, PairFacts, Subject, as_confirmed};
use diff_marks::model::{DeclarationMove, DiffMarks, FileMove, RenameProof, SubjectMarks};
use line_diff::model::{BlockKind, DiffBudget, LineMove, LineRun, RunKind, diff_texts};
use line_diff::policy::WhitespacePolicy;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use test_log::test;

/// A file of a case: its path and its two texts, `None` for a side it does not exist on
struct File {
  path: &'static str,
  old: Option<String>,
  new: Option<String>,
}

fn changed(path: &'static str, old: &str, new: &str) -> File {
  File {
    path,
    old: Some(old.to_string()),
    new: Some(new.to_string()),
  }
}

fn added(path: &'static str, new: &str) -> File {
  File {
    path,
    old: None,
    new: Some(new.to_string()),
  }
}

/// The diff of a file under `policy` with the marks of its pair, and what the pair left
fn read(file: &File, policy: WhitespacePolicy) -> (MarkedDiff, Leftovers) {
  let (old, new) = (file.old.as_deref().unwrap_or_default(), file.new.as_deref().unwrap_or_default());
  let mut diff = MarkedDiff::from(diff_texts(old, new, policy, &DiffBudget::NONE));
  let grammar = pair_grammar(file.path, file.path);
  let indented = pair_indented(file.path, file.path);
  let pair = marks_of(
    &diff,
    old,
    new,
    grammar,
    policy,
    indented,
    |new_side| tokens(grammar?, if new_side { new } else { old }).map(Arc::new),
    &CodeSymbolBindings,
  );
  let (marks, left) = Leftovers::of(&diff.blocks, pair, old, new, indented);
  diff.marks = marks;
  (diff, left)
}

/// The files of a case, read
struct Case {
  files: Vec<File>,
  pairs: Vec<(MarkedDiff, Leftovers)>,
}

impl Case {
  /// The files read with every character counted
  fn of(files: Vec<File>) -> Self {
    Self::under(files, WhitespacePolicy::Default)
  }

  fn under(files: Vec<File>, policy: WhitespacePolicy) -> Self {
    let pairs = files.iter().map(|file| read(file, policy)).collect();
    Self { files, pairs }
  }

  fn facts(&self) -> Vec<Option<PairFacts<'_>>> {
    self
      .files
      .iter()
      .zip(&self.pairs)
      .map(|(file, (diff, left))| {
        Some(PairFacts {
          old_path: file.old.as_ref().map(|_| file.path),
          new_path: file.new.as_ref().map(|_| file.path),
          blocks: &diff.blocks,
          marks: &diff.marks,
          left,
        })
      })
      .collect()
  }

  /// The marks of each file with `answer` of the index, and the questions it was asked
  fn marks(&self, answer: IndexAnswer<'_>) -> (Vec<Option<DiffMarks>>, Vec<RenamePair>) {
    let facts = self.facts();
    let subject = Subject::of(&facts);
    (subject.marks(answer), subject.questions().to_vec())
  }

  /// The marks of each file, the index answering no rename and `moves` of the declarations the change moved whole
  fn moved(&self, moves: &MovesAnswer) -> Vec<Option<DiffMarks>> {
    let facts = self.facts();
    Subject::with_moves(&facts, moves).marks(IndexAnswer::Unavailable)
  }

  /// What the subject says of the file `index`
  fn subject(&self, answer: IndexAnswer<'_>, index: usize) -> SubjectMarks {
    let subject = self.marks(answer).0[index].clone().and_then(|marks| marks.subject);
    *subject.unwrap_or_else(|| panic!("the subject says nothing of {}", self.files[index].path))
  }

  /// The answer of an index that lists the places of every rename of the two names as the uses of the one
  /// declaration: what accio answers of a rename that is complete and final
  fn answer(&self, old_name: &str, new_name: &str) -> RenameAnswer {
    let facts = self.facts();
    let subject = Subject::of(&facts);
    let marks = subject.marks(IndexAnswer::Asked);
    let (mut base, mut head) = (Vec::new(), Vec::new());
    for ((file, (diff, _)), marks) in self.files.iter().zip(&self.pairs).zip(&marks) {
      let used = marks
        .iter()
        .flat_map(|marks| marks.subject.iter().flat_map(|subject| subject.used.iter().map(|used| &used.rename)));
      for rename in diff.marks.renames.iter().chain(used).filter(|rename| rename.old == old_name && rename.new == new_name) {
        for place in 0..rename.sites {
          let kind = if rename.declared.contains(&place) { Access::Declaration } else { Access::Call };
          let used = |sites: &[u32]| RenameUse {
            path: file.path.to_string(),
            line: sites[place as usize * 3] + 1,
            column: sites[place as usize * 3 + 1],
            end_column: sites[place as usize * 3 + 2],
            kind,
            resolved: Resolved::Typed,
            text: None,
          };
          base.push(used(&rename.old_sites));
          head.push(used(&rename.new_sites));
        }
      }
    }
    let uses = |uses: Vec<RenameUse>| Some(RenameUses { total: uses.len(), uses });
    let kotlin = || Some(RenamedDeclaration { language: "kotlin".to_string() });
    RenameAnswer {
      unavailable: false,
      same: true,
      renamed: true,
      complete: true,
      old: kotlin(),
      new: kotlin(),
      base: uses(base),
      head: uses(head),
      is_final: true,
      ..RenameAnswer::default()
    }
  }
}

fn text(lines: &[&str]) -> String {
  let mut text = String::new();
  for line in lines {
    text.push_str(line);
    text.push('\n');
  }
  text
}

const LEDGER: [&str; 4] = ["class Ledger {", "  private val entries = mutableListOf<Entry>()", "  fun clear() = entries.clear()", "}"];
const TITLE: [&str; 5] = [
  "",
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

fn joined(parts: &[&[&str]]) -> String {
  text(&parts.concat())
}

/// A ledger that loses its title function, and a report that gets it
fn moved_title() -> Case {
  Case::of(vec![
    changed("src/Ledger.kt", &joined(&[&LEDGER, &TITLE]), &joined(&[&LEDGER])),
    changed("src/Report.kt", &joined(&[&TOTAL]), &joined(&[&TOTAL, &TITLE])),
  ])
}

/// The runs, the moves, the renames and the starts of `marks`, those of the files of `case` in their order, are
/// those of their pairs: what the subject added is a label
fn assert_marks_of_pairs(case: &Case, marks: &[&DiffMarks]) {
  for (marks, (diff, _)) in marks.iter().zip(&case.pairs) {
    assert_eq!(
      (&marks.runs, &marks.moves, &marks.renames, &marks.starts),
      (&diff.marks.runs, &diff.marks.moves, &diff.marks.renames, &diff.marks.starts)
    );
  }
}

#[test]
fn lines_that_moved_between_two_files_are_a_label_at_both_ends() {
  let case = moved_title();
  let (marks, questions) = case.marks(IndexAnswer::Unavailable);
  assert_eq!(questions, []);
  let (ledger, report) = (marks[0].clone().unwrap(), marks[1].clone().unwrap());
  // the four lines of the function; the blank line before it is no line of the move
  assert_eq!(
    ledger.subject.as_deref(),
    Some(&SubjectMarks {
      moved_out: vec![FileMove {
        start: 5,
        len: 4,
        file: 1,
        line: 5,
        declaration: None,
      }],
      ..SubjectMarks::default()
    })
  );
  assert_eq!(
    report.subject.as_deref(),
    Some(&SubjectMarks {
      moved_in: vec![FileMove {
        start: 5,
        len: 4,
        file: 0,
        line: 5,
        declaration: None,
      }],
      ..SubjectMarks::default()
    })
  );
  // a label: no run, no move within a file, and every block is read where it was
  assert_marks_of_pairs(&case, &[&ledger, &report]);
  for marks in [ledger, report] {
    assert_eq!(marks.starts.len(), 3, "one block, a stop");
  }
}

#[test]
fn lines_that_match_two_places_move_to_the_first_and_boilerplate_moves_nowhere() {
  // two files got the function one file lost: the first of them, in the order of git
  let case = Case::of(vec![
    added("src/Copy.kt", &joined(&[&TITLE[1..]])),
    changed("src/Ledger.kt", &joined(&[&LEDGER, &TITLE]), &joined(&[&LEDGER])),
    added("src/Report.kt", &joined(&[&TITLE[1..]])),
  ]);
  let (marks, _) = case.marks(IndexAnswer::Unavailable);
  assert_eq!(
    marks[0].clone().unwrap().subject.unwrap().moved_in,
    [FileMove {
      start: 0,
      len: 4,
      file: 1,
      line: 5,
      declaration: None,
    }]
  );
  assert_eq!(marks[1].clone().unwrap().subject.unwrap().moved_out.len(), 1);
  assert_eq!(marks[2], None, "the second place is plain added lines: the marks of its pair stand");

  // the end of a class in two files, and one long line: below what a move between files needs
  let short = Case::of(vec![
    changed("src/Ledger.kt", &joined(&[&LEDGER, &["  }", "}"]]), &joined(&[&LEDGER])),
    changed("src/Report.kt", &joined(&[&TOTAL]), &joined(&[&TOTAL, &["  }", "}"]])),
    changed("src/One.kt", &joined(&[&LEDGER, &[TITLE[1]]]), &joined(&[&LEDGER])),
    changed("src/Other.kt", &joined(&[&TOTAL]), &joined(&[&TOTAL, &[TITLE[1]]])),
  ]);
  assert_eq!(short.marks(IndexAnswer::Unavailable).0, [None, None, None, None]);
}

const STORE: [&str; 7] = [
  "package demo",
  "",
  "class Store(val path: String)",
  "",
  "fun openStore(path: String): Store {",
  "  return Store(path)",
  "}",
];
const REPORT: [&str; 8] = [
  "package demo",
  "",
  "fun report(path: String): String {",
  "  val store = openStore(path)",
  "  return store.path",
  "}",
  "",
  "fun other(path: String): Store = openStore(path)",
];

/// The answer of `accio moves` of one declaration `name` that moved whole from the lines `old` of `old_file` to the
/// lines `new` of `new_file` (from 1, both ends included)
fn moved_whole(name: &str, (old_file, old): (&str, [u32; 2]), (new_file, new): (&str, [u32; 2])) -> MovesAnswer {
  let at = |file: &str, [line, end_line]: [u32; 2]| DeclaredAt {
    file: file.to_string(),
    line,
    end_line,
    kind: "function".to_string(),
    name: name.to_string(),
    language: "kotlin".to_string(),
    ..DeclaredAt::default()
  };
  MovesAnswer {
    moved: vec![MovedDeclaration {
      old: at(old_file, old),
      new: at(new_file, new),
      file: old_file != new_file,
      owner: old_file == new_file,
    }],
    total: 1,
    complete: true,
    ..MovesAnswer::default()
  }
}

/// `reportTitle` with its parameters one a line: the same words in another layout
const TITLE_WRAPPED: [&str; 8] = [
  "",
  "fun reportTitle(",
  "  ledger: Ledger,",
  "  account: String",
  "): String {",
  "  val balance = ledger.balance(account)",
  "  return \"$account: $balance\"",
  "}",
];

#[test]
fn a_declaration_the_index_says_moved_whole_is_a_move_between_files_in_another_layout_too() {
  let case = Case::of(vec![
    changed("src/Ledger.kt", &joined(&[&LEDGER, &TITLE]), &joined(&[&LEDGER])),
    changed("src/Report.kt", &joined(&[&TOTAL]), &joined(&[&TOTAL, &TITLE_WRAPPED])),
  ]);
  // without the index: the last three lines alone are the same lines
  let (plain, _) = case.marks(IndexAnswer::Unavailable);
  let row = FileMove {
    start: 6,
    len: 3,
    file: 1,
    line: 9,
    declaration: None,
  };
  assert_eq!(plain[0].clone().unwrap().subject.unwrap().moved_out, [row]);
  // the index says the function moved whole: its four old lines and its seven new ones are one move, which takes
  // the row of the same lines
  let moves = moved_whole("reportTitle", ("src/Ledger.kt", [6, 9]), ("src/Report.kt", [6, 12]));
  let marks = case.moved(&moves);
  let (ledger, report) = (marks[0].clone().unwrap(), marks[1].clone().unwrap());
  let declaration = |other_len: u32| {
    Some(DeclarationMove {
      name: "reportTitle".to_string(),
      other_len,
    })
  };
  assert_eq!(
    ledger.subject.as_deref().map(|subject| &subject.moved_out),
    Some(&vec![FileMove {
      start: 5,
      len: 4,
      file: 1,
      line: 5,
      declaration: declaration(7),
    }])
  );
  assert_eq!(
    report.subject.as_deref().map(|subject| &subject.moved_in),
    Some(&vec![FileMove {
      start: 5,
      len: 7,
      file: 0,
      line: 5,
      declaration: declaration(4),
    }])
  );
  // a label as a row of the same lines is: the runs, the moves, the renames and the starts are those of the pairs
  assert_marks_of_pairs(&case, &[&ledger, &report]);

  // an end that holds a line the change left alone, a place of another file, or the lines of a file that is not of
  // the subject is no move of a declaration: the rows of the same lines are what they were
  for moves in [
    moved_whole("reportTitle", ("src/Ledger.kt", [6, 9]), ("src/Report.kt", [1, 12])),
    moved_whole("reportTitle", ("src/Ledger.kt", [4, 9]), ("src/Report.kt", [6, 12])),
    moved_whole("reportTitle", ("src/Other.kt", [6, 9]), ("src/Report.kt", [6, 12])),
    moved_whole("reportTitle", ("src/Ledger.kt", [6, 9]), ("src/Report.kt", [0, 12])),
  ] {
    assert_eq!(case.moved(&moves), plain, "{moves:?}");
  }
}

#[test]
fn a_row_of_the_same_lines_that_holds_declarations_the_index_says_moved_whole_as_they_are_stays_one_move() {
  // a ledger whose two functions move to the report as they are, one row of the same lines
  let old_ledger = joined(&[&LEDGER, &[""], &TOTAL, &TITLE]);
  let case = Case::of(vec![
    changed("src/Ledger.kt", &old_ledger, &joined(&[&LEDGER])),
    changed("src/Report.kt", &text(&["fun other() = 0"]), &joined(&[&["fun other() = 0", ""], &TOTAL, &TITLE])),
  ]);
  let (plain, _) = case.marks(IndexAnswer::Unavailable);
  let row = FileMove {
    start: 5,
    len: 9,
    file: 1,
    line: 2,
    declaration: None,
  };
  assert_eq!(plain[0].clone().unwrap().subject.unwrap().moved_out, [row]);
  // the index names each of them: the row maps the lines of each onto its lines, so it stays the one move it is
  let mut moves = moved_whole("reportTotal", ("src/Ledger.kt", [6, 9]), ("src/Report.kt", [3, 6]));
  moves
    .moved
    .extend(moved_whole("reportTitle", ("src/Ledger.kt", [11, 14]), ("src/Report.kt", [8, 11])).moved);
  assert_eq!(case.moved(&moves), plain);
}

#[test]
fn a_row_of_the_same_lines_keeps_its_parts_beside_a_declaration_of_another_layout_that_are_a_row_by_themselves() {
  // the doc comment of the function moves with it, and its last line is wrapped otherwise: the doc, the head and the
  // first line of the body are one row of the same lines, and the index counts the function from its head
  let body = ["fun reportTitle(ledger: Ledger, account: String): String {", "  val balance = ledger.balance(account)"];
  let (old_end, new_end) = (["  return \"$account: $balance\"", "}"], ["  return \"$account: \" +", "    balance", "}"]);
  // the ends where the lines left: the first line, the lines, the line at the other end, the declaration named
  type Ends = Vec<(u32, u32, u32, Option<String>)>;
  let moved_out = |doc: &[&str]| -> [Ends; 2] {
    let case = Case::of(vec![
      changed("src/Ledger.kt", &joined(&[&LEDGER, doc, &body, &old_end]), &joined(&[&LEDGER])),
      changed("src/Report.kt", &joined(&[&TOTAL]), &joined(&[&TOTAL, doc, &body, &new_end])),
    ]);
    let ends = |marks: &[Option<DiffMarks>]| -> Ends {
      let subject = marks[0].clone().unwrap().subject.unwrap();
      subject
        .moved_out
        .iter()
        .map(|moved| (moved.start, moved.len, moved.line, moved.declaration.clone().map(|it| it.name)))
        .collect()
    };
    let (plain, _) = case.marks(IndexAnswer::Unavailable);
    [
      ends(&plain),
      ends(&case.moved(&moved_whole("reportTitle", ("src/Ledger.kt", [9, 12]), ("src/Report.kt", [9, 13])))),
    ]
  };
  let declaration = (8, 4, 8, Some("reportTitle".to_string()));
  // the declaration takes its lines, and the doc stays a move of its own: two lines and enough characters
  let [plain, moved] = moved_out(&["", "/**", " * The title of the report of one account, with the balance of it.", " */"]);
  assert_eq!(plain, [(5, 5, 5, None)], "the doc, the head and the line after it");
  assert_eq!(moved, [(5, 3, 5, None), declaration.clone()]);
  // a doc too short to be a row by itself is no move of its own: no note for a few characters beside the declaration
  let [plain, moved] = moved_out(&["", "/**", " * The title.", " */"]);
  assert_eq!(plain, [(5, 5, 5, None)]);
  assert_eq!(moved, [declaration]);
}

#[test]
fn a_declaration_moved_under_another_owner_within_its_file_is_a_move_to_the_file_itself() {
  // `clear` leaves the class for the top of the file, on two lines
  let old = joined(&[&LEDGER, &["", "fun total(): Long = 0"]]);
  let new = text(&[
    "class Ledger {",
    "  private val entries = mutableListOf<Entry>()",
    "}",
    "",
    "fun total(): Long = 0",
    "",
    "fun clear() =",
    "  entries.clear()",
  ]);
  let case = Case::of(vec![changed("src/Ledger.kt", &old, &new)]);
  let marks = case.moved(&moved_whole("clear", ("src/Ledger.kt", [3, 3]), ("src/Ledger.kt", [7, 8])))[0].clone().unwrap();
  let subject = marks.subject.unwrap();
  let declaration = |other_len: u32| {
    Some(DeclarationMove {
      name: "clear".to_string(),
      other_len,
    })
  };
  assert_eq!(
    (subject.moved_out, subject.moved_in),
    (
      vec![FileMove {
        start: 2,
        len: 1,
        file: 0,
        line: 6,
        declaration: declaration(2),
      }],
      vec![FileMove {
        start: 6,
        len: 2,
        file: 0,
        line: 2,
        declaration: declaration(1),
      }]
    )
  );
}

/// A function that moves from the object `Ledger` to the object `Report`, as it is, and a file whose one call of it
/// names the new owner in place of the old one
fn moved_under_another_owner(use_old: &[&str], use_new: &[&str]) -> Case {
  let title = ["  fun title(account: String): String {", "    return \"Title of $account\"", "  }"];
  Case::of(vec![
    changed("src/Ledger.kt", &joined(&[&["object Ledger {"], &title, &["}"]]), &text(&["object Ledger {", "}"])),
    changed("src/Report.kt", &text(&["object Report {", "}"]), &joined(&[&["object Report {"], &title, &["}"]])),
    changed("src/Use.kt", &text(use_old), &text(use_new)),
  ])
}

/// What accio moves answers of the title function of `moved_under_another_owner`: from the object `Ledger` of
/// src/Ledger.kt to the object `Report` of src/Report.kt, its lines 2 to 4 (from 1) in both
pub(crate) fn title_moves() -> MovesAnswer {
  let at = |file: &str, parent: &str| DeclaredAt {
    file: file.to_string(),
    line: 2,
    end_line: 4,
    kind: "function".to_string(),
    name: "title".to_string(),
    parent: Some(parent.to_string()),
    language: "kotlin".to_string(),
    ..DeclaredAt::default()
  };
  MovesAnswer {
    moved: vec![MovedDeclaration {
      old: at("src/Ledger.kt", "Ledger"),
      new: at("src/Report.kt", "Report"),
      file: true,
      owner: true,
    }],
    total: 1,
    complete: true,
    ..MovesAnswer::default()
  }
}

/// What accio renames answers of the moved title function, its declaration and the uses of `calls` (the line from 1
/// and the column of the name, from 0) in `src/Use.kt` of each tree, `changed` writing what is not the same of the
/// answer as JSON
fn title_answer(calls: [&[(u32, u32)]; 2], changed: &serde_json::Value) -> RenameAnswer {
  let used = |path: &str, line: u32, column: u32, kind: &str| serde_json::json!({ "file": path, "line": line, "column": column, "end_column": column + 5, "kind": kind, "resolved": "typed" });
  let uses = |owner_file: &str, calls: &[(u32, u32)]| {
    let mut uses = vec![used(owner_file, 2, 6, "declaration")];
    uses.extend(calls.iter().map(|&(line, column)| used("src/Use.kt", line, column, "call")));
    serde_json::json!({ "total": uses.len(), "uses": uses })
  };
  let kotlin = serde_json::json!({ "language": "kotlin" });
  let mut answer = serde_json::json!({
    "same": false, "differs": ["owner"], "renamed": false, "moved": true, "body_equal": true, "complete": true, "final": true,
    "old": kotlin, "new": kotlin, "base": uses("src/Ledger.kt", calls[0]), "head": uses("src/Report.kt", calls[1]), "conflicts": [],
  });
  for (key, value) in changed.as_object().unwrap() {
    answer[key] = value.clone();
  }
  serde_json::from_value(answer).unwrap()
}

#[test]
fn a_use_whose_owner_the_change_wrote_anew_after_a_move_under_another_owner_is_quiet_once_the_index_proves_it() {
  let call = |owner: &str| format!("fun use(account: String) = {owner}.title(account)");
  let (old_call, new_call) = (call("Ledger"), call("Report"));
  let case = moved_under_another_owner(&[&old_call], &[&new_call]);
  let facts = case.facts();
  let moves = title_moves();
  // without the moves of the index the new owner is a name no file declares, and with the moves alone it is no mark
  // either: a label that the call follows a move must be true
  assert_eq!(Subject::of(&facts).marks(IndexAnswer::Unavailable)[2], None);
  assert_eq!(Subject::with_moves(&facts, &moves).marks(IndexAnswer::Unavailable)[2], None);
  // it may follow the move of `title`: asked of the index by the lines of the two declarations
  let following = Subject::following_moves(&facts, &moves);
  let pair = RenamePair {
    old: "src/Ledger.kt:2".to_string(),
    new: "src/Report.kt:2".to_string(),
  };
  assert_eq!(following.move_questions(), [pair]);
  let use_of = |answer: RenameAnswer| -> Option<DiffMarks> {
    let followed = following.followed(&[answer]);
    Subject::with_followed(&facts, &moves, &followed).marks(IndexAnswer::Unavailable)[2].clone()
  };

  // the index lists the call behind each owner, in both trees: proven, and the run of the call is quiet
  let columns = [(1, 34)];
  let answered = use_of(title_answer([&columns, &columns], &serde_json::json!({}))).unwrap();
  let subject_marks = answered.subject.as_ref().unwrap();
  assert_eq!(
    subject_marks
      .used
      .iter()
      .map(|used| (used.rename.old.as_str(), used.rename.new.as_str(), used.file, used.line, used.proof, used.moved.as_deref()))
      .collect::<Vec<_>>(),
    [("Ledger", "Report", 1, 1, RenameProof::Confirmed, Some("title"))]
  );
  assert_eq!(subject_marks.confirmed, [0]);
  assert_eq!(answered.runs.iter().map(|run| (run.kind, run.quiet)).collect::<Vec<_>>(), [(RunKind::Renamed, true)]);
  assert_eq!(answered.starts.len() + 3, case.pairs[2].0.marks.starts.len(), "its block is passed");

  // what proves nothing, and leaves the file as its pair has it: another declaration than the one that moved, a use
  // of it the change left at the old owner, an answer that is not complete, a use the other tree does not list behind
  // the new owner
  let left = {
    let mut answer = title_answer([&columns, &columns], &serde_json::json!({}));
    let left = serde_json::json!({ "file": "src/Other.kt", "line": 3, "column": 9, "end_column": 14, "kind": "call", "resolved": "typed" });
    answer.base.as_mut().unwrap().uses.push(serde_json::from_value(left).unwrap());
    answer.base.as_mut().unwrap().total += 1;
    answer
  };
  let not_proven: [(&str, RenameAnswer); 4] = [
    ("its words changed", title_answer([&columns, &columns], &serde_json::json!({ "body_equal": false }))),
    ("a use left in an unchanged file", left),
    ("not complete", title_answer([&columns, &columns], &serde_json::json!({ "complete": false }))),
    ("no twin behind the new owner", title_answer([&columns, &[]], &serde_json::json!({}))),
  ];
  for (name, answer) in not_proven {
    assert_eq!(use_of(answer), None, "{name}");
  }

  // an owner the file names where no use of the moved declaration stands behind it: the change did more than follow
  // the move there, and the name is not proven
  let typed = |owner: &str| format!("fun use(account: String, ledger: {owner}) = {owner}.title(account)");
  let (old_typed, new_typed) = (typed("Ledger"), typed("Report"));
  let case = moved_under_another_owner(&[&old_typed], &[&new_typed]);
  let facts = case.facts();
  let following = Subject::following_moves(&facts, &moves);
  let columns = [(1, 50)];
  assert!(following.followed(&[title_answer([&columns, &columns], &serde_json::json!({}))]).is_empty());
}

fn renamed(lines: &[&str]) -> String {
  text(lines).replace("openStore", "connectStore")
}

/// A store that renames its function, and a report that calls it at two places
fn renamed_store() -> Case {
  Case::of(vec![
    changed("src/Report.kt", &text(&REPORT), &renamed(&REPORT)),
    changed("src/Store.kt", &text(&STORE), &renamed(&STORE)),
  ])
}

fn run(old: u32, new: u32) -> LineRun {
  LineRun {
    old_start: old,
    old_end: old + 1,
    new_start: new,
    new_end: new + 1,
    kind: RunKind::Renamed,
    quiet: false,
  }
}

#[test]
fn a_name_another_file_declares_is_a_rename_of_the_file_that_uses_it() {
  let case = renamed_store();
  // the pairs alone: the report marks nothing, the store names its rename
  assert_eq!((case.pairs[0].0.marks.runs.len(), case.pairs[0].0.marks.renames.len()), (0, 0));
  assert_eq!(case.pairs[0].1.undeclared.len(), 1);
  assert_eq!(case.pairs[1].0.marks.renames.len(), 1);

  let (marks, questions) = case.marks(IndexAnswer::Unavailable);
  let report = marks[0].clone().unwrap();
  // the two lines that changed in the name alone are runs of the rename, labels; the file is read as before
  assert_eq!(report.runs, [run(3, 3), run(7, 7)]);
  assert_eq!(report.starts, case.pairs[0].0.marks.starts);
  assert_eq!(report.renames, [], "the renames of the marks are those the file declares");
  let subject = report.subject.unwrap();
  let used = &subject.used[0];
  assert_eq!(
    (used.rename.old.as_str(), used.rename.new.as_str(), used.rename.sites, &used.rename.declared),
    ("openStore", "connectStore", 2, &vec![])
  );
  assert_eq!((used.file, used.line, used.proof), (1, 4, RenameProof::Unavailable));
  assert_eq!((subject.used.len(), &subject.confirmed), (1, &vec![]));

  // the declaration is asked of the index by its place in both trees, the line and the column from 1
  assert_eq!(
    questions,
    [RenamePair {
      old: "src/Store.kt:5:5".to_string(),
      new: "src/Store.kt:5:5".to_string(),
    }]
  );
  let store = marks[1].clone().unwrap();
  assert_eq!(store.runs, [], "the declaration is in no run");
  assert_eq!(
    store.subject.unwrap().declared.iter().map(|check| (check.rename, check.proof)).collect::<Vec<_>>(),
    [(0, RenameProof::Unavailable)]
  );
}

#[test]
fn a_declaration_says_how_many_places_of_the_subject_hold_its_name_and_in_how_many_files() {
  let reach = |case: &Case, store: usize| {
    let check = &case.subject(IndexAnswer::Unavailable, store).declared[0];
    (check.sites, check.files)
  };
  // the declaration of the store and the two calls of the report
  assert_eq!(reach(&renamed_store(), 1), (3, 2));
  // the store alone: its own place
  assert_eq!(reach(&Case::of(vec![changed("src/Store.kt", &text(&STORE), &renamed(&STORE))]), 0), (1, 1));

  // a second file that calls the name once more, and a third that renamed its call and added one: no rename of that
  // file (not every place changed in place), so its calls are read and not counted
  let caller = ["package demo", "", "fun load(path: String): Store = openStore(path)"];
  let grown = ["package demo", "", "fun first(path: String): Store = openStore(path)"];
  let more = "fun second(path: String): Store = connectStore(path + path)\n";
  let case = Case::of(vec![
    changed("src/Caller.kt", &text(&caller), &renamed(&caller)),
    changed("src/Grown.kt", &text(&grown), &format!("{}{more}", renamed(&grown))),
    changed("src/Report.kt", &text(&REPORT), &renamed(&REPORT)),
    changed("src/Store.kt", &text(&STORE), &renamed(&STORE)),
  ]);
  assert_eq!(case.subject(IndexAnswer::Unavailable, 0).used.len(), 1);
  assert_eq!(case.marks(IndexAnswer::Unavailable).0[1], None, "the file that added a call names no rename");
  assert_eq!(reach(&case, 3), (4, 3));
}

#[test]
fn a_rename_every_use_of_which_the_index_lists_is_confirmed_and_its_declaration_stays_a_stop() {
  let case = renamed_store();
  // while the index is asked
  assert_eq!(case.subject(IndexAnswer::Asked, 0).used[0].proof, RenameProof::Asked);
  assert_eq!(case.subject(IndexAnswer::Asked, 1).declared[0].proof, RenameProof::Asked);

  let answer = case.answer("openStore", "connectStore");
  let (marks, _) = case.marks(IndexAnswer::Answered(std::slice::from_ref(&answer)));
  let (report, store) = (marks[0].clone().unwrap(), marks[1].clone().unwrap());
  let subject = report.subject.clone().unwrap();
  assert_eq!((subject.used[0].proof, &subject.confirmed), (RenameProof::Confirmed, &vec![0, 1]));
  assert_eq!(store.subject.clone().unwrap().declared[0].proof, RenameProof::Confirmed);
  assert_eq!(store.subject.as_ref().unwrap().confirmed, [] as [u32; 0], "the store has no run");

  // the kind is switched on: the uses the index confirmed are passed, the declaration is read
  assert!(is_quiet(RunKind::Renamed));
  assert_eq!(report.runs, [LineRun { quiet: true, ..run(3, 3) }, LineRun { quiet: true, ..run(7, 7) }]);
  assert_eq!(report.starts, [] as [u32; 0]);
  assert_eq!(store.starts, [0, 1, 4]);
  // and they are the marks as confirmed: nothing is left for the switch to change
  assert_eq!(as_confirmed(&report, &case.pairs[0].0.blocks), report);
  assert_eq!(as_confirmed(&store, &case.pairs[1].0.blocks), store);
  // before the answer, and without one, the same runs are labels and each block is read
  for answer in [IndexAnswer::Asked, IndexAnswer::Unavailable] {
    let (marks, _) = case.marks(answer);
    let report = marks[0].clone().unwrap();
    assert_eq!((report.runs, report.starts), (vec![run(3, 3), run(7, 7)], vec![0, 1, 3, 1, 1, 7]));
  }
}

/// A case of a name `TIMEOUT` declared as `LIMIT`: what it is, the lines that declare it before and after, and
/// whether they are the same code
type Redeclared = (&'static str, &'static [&'static str], &'static [&'static str], bool);

/// A name that is not local is the same thing under another name only where the code that declares it changed in
/// names alone: a constant that got another value with its other name is another constant, and a line that reads
/// it does something else, the same tokens but for the name. So such a name is not asked of the index, an answer
/// that lists every use proves nothing of it, and its uses stay labels and stops (found in the measured runs:
/// `private val outOfSync = Message(…)` became `private val updateLock = "Update lock"` with six uses the index
/// confirmed). A function is what its body does; a type is its head, whatever its members do
#[test]
fn a_name_declared_by_other_code_is_another_thing_and_the_index_is_not_asked_of_it() {
  let uses = ["package demo", "", "fun first() {", "  pause(TIMEOUT)", "}", "", "fun second() = pause(TIMEOUT)"];
  let cases: &[Redeclared] = &[
    (
      "a constant with another value",
      &["package demo", "", "const val TIMEOUT = 5"],
      &["package demo", "", "const val LIMIT = 10"],
      false,
    ),
    (
      "a constant with the same value",
      &["package demo", "", "const val TIMEOUT = 5"],
      &["package demo", "", "const val LIMIT = 5"],
      true,
    ),
    (
      "a value that goes on over a line that changed",
      &["package demo", "", "val TIMEOUT = seconds(", "  5)"],
      &["package demo", "", "val LIMIT = seconds(", "  10)"],
      false,
    ),
    (
      "a value wrapped otherwise",
      &["package demo", "", "val TIMEOUT = seconds(5)"],
      &["package demo", "", "val LIMIT = seconds(", "  5)"],
      true,
    ),
    (
      "a property with another getter",
      &["package demo", "", "val TIMEOUT: Int", "  get() = 5"],
      &["package demo", "", "val LIMIT: Int", "  get() = 10"],
      false,
    ),
    (
      "a function with another body",
      &["package demo", "", "fun TIMEOUT(): Int {", "  return 5", "}"],
      &["package demo", "", "fun LIMIT(): Int {", "  return 10", "}"],
      false,
    ),
    (
      "a function with the same body",
      &["package demo", "", "fun TIMEOUT(): Int {", "  return 5", "}"],
      &["package demo", "", "fun LIMIT(): Int {", "  return 5", "}"],
      true,
    ),
    (
      "a function with a parameter a call may name under another name",
      &["package demo", "", "fun TIMEOUT(scale: Int): Int {", "  return 5 * scale", "}"],
      &["package demo", "", "fun LIMIT(factor: Int): Int {", "  return 5 * factor", "}"],
      false,
    ),
    (
      "a function with a local of its body under another name",
      &["package demo", "", "fun TIMEOUT(): Int {", "  val base = 5", "  return base", "}"],
      &["package demo", "", "fun LIMIT(): Int {", "  val unit = 5", "  return unit", "}"],
      true,
    ),
    (
      "an object with another member: a type is its head",
      &["package demo", "", "object TIMEOUT : Setting {", "  val seconds = 5", "}"],
      &["package demo", "", "object LIMIT : Setting {", "  val seconds = 10", "}"],
      true,
    ),
    (
      "an object of another type",
      &["package demo", "", "object TIMEOUT : Setting {", "  val seconds = 5", "}"],
      &["package demo", "", "object LIMIT : Bound {", "  val seconds = 5", "}"],
      false,
    ),
  ];
  let (old_name, new_name) = ("TIMEOUT", "LIMIT");
  for (name, old, new, same) in cases {
    let case = Case::of(vec![
      changed("src/Uses.kt", &text(&uses).replace("TIMEOUT", old_name), &text(&uses).replace("TIMEOUT", new_name)),
      changed("src/Settings.kt", &text(old), &text(new)),
    ]);
    assert_eq!(case.pairs[1].1.declarations.iter().map(|it| it.redeclared).collect::<Vec<_>>(), [!same], "{name}");
    // the answer of an index that lists every place of the two names
    let answer = case.answer(old_name, new_name);
    let (marks, questions) = case.marks(IndexAnswer::Answered(std::slice::from_ref(&answer)));
    let (uses, settings) = (marks[0].clone().unwrap(), marks[1].clone().unwrap());
    let proof = (uses.subject.as_ref().unwrap().used[0].proof, settings.subject.as_ref().unwrap().declared[0].proof);
    let labels = vec![run(3, 3), run(6, 6)];
    if *same {
      assert_eq!((questions.len(), proof), (1, (RenameProof::Confirmed, RenameProof::Confirmed)), "{name}");
      assert_eq!(
        (uses.runs, uses.starts),
        (labels.into_iter().map(|run| LineRun { quiet: true, ..run }).collect(), vec![]),
        "{name}"
      );
    } else {
      assert_eq!((questions.len(), proof), (0, (RenameProof::OtherDeclaration, RenameProof::OtherDeclaration)), "{name}");
      assert_eq!((uses.runs, uses.starts), (labels, vec![0, 1, 3, 1, 1, 6]), "{name}: the uses are labels, and read");
      assert_eq!(uses.subject.unwrap().confirmed, [] as [u32; 0], "{name}");
      // the diff of the file says so itself: before an index is asked, and where there is none
      for answer in [IndexAnswer::Asked, IndexAnswer::Unavailable] {
        let (marks, _) = case.marks(answer);
        let proofs = [0, 1].map(|file| marks[file].clone().unwrap().subject.unwrap());
        assert_eq!((proofs[0].used[0].proof, proofs[1].declared[0].proof), proof, "{name}");
      }
    }
    // the declaration is read either way
    assert!(!settings.starts.is_empty(), "{name}");
  }
}

/// The proof of the use in the report and of the declaration in the store, with `answer` of the index
fn proofs(case: &Case, answer: &RenameAnswer) -> (RenameProof, RenameProof, Vec<u32>) {
  let (marks, _) = case.marks(IndexAnswer::Answered(std::slice::from_ref(answer)));
  let report = marks[0].clone().unwrap().subject.unwrap();
  let store = marks[1].clone().unwrap().subject.unwrap();
  (report.used[0].proof, store.declared[0].proof, report.confirmed)
}

fn used_at(path: &str, line: u32, column: u32, end_column: u32) -> RenameUse {
  RenameUse {
    path: path.to_string(),
    line,
    column,
    end_column,
    kind: Access::Call,
    resolved: Resolved::Typed,
    text: None,
  }
}

fn conflict(kind: &str) -> RenameConflict {
  RenameConflict { kind: kind.to_string() }
}

#[test]
fn every_other_answer_leaves_the_rename_a_label_and_says_why() {
  let case = renamed_store();
  let complete = case.answer("openStore", "connectStore");
  let check = |name: &str, answer: RenameAnswer, proof: RenameProof| {
    assert_eq!(proofs(&case, &answer), (proof, proof, vec![]), "{name}");
  };
  let with = |change: &dyn Fn(&mut RenameAnswer)| {
    let mut answer = complete.clone();
    change(&mut answer);
    answer
  };

  // a use outside the change: in a file the change did not touch, as the index says it, or as a use it lists there
  check(
    "a use the change left behind",
    with(&|answer| answer.conflicts.push(conflict("left"))),
    RenameProof::UsesOutside,
  );
  check(
    "a use in a file outside the subject",
    with(&|answer| {
      let uses = answer.base.as_mut().unwrap();
      uses.uses.push(used_at("src/Other.kt", 3, 8, 17));
      uses.total += 1;
    }),
    RenameProof::UsesOutside,
  );
  // on a line of a file of the subject that the change left alone
  check(
    "a use on a line the change left alone",
    with(&|answer| {
      let uses = answer.base.as_mut().unwrap();
      uses.uses.push(used_at("src/Report.kt", 5, 9, 18));
      uses.total += 1;
    }),
    RenameProof::UsesOutside,
  );

  // the rename changes what a name means
  for kind in ["captured", "overload", "overrides", "overridden", "hides", "hidden", "namesake"] {
    check(kind, with(&|answer| answer.conflicts.push(conflict(kind))), RenameProof::OtherMeaning);
  }
  check(
    "a use of the old tree that is none of the new one: another name took it",
    with(&|answer| {
      let uses = answer.head.as_mut().unwrap();
      uses.uses.remove(0);
      uses.total -= 1;
    }),
    RenameProof::OtherMeaning,
  );
  check(
    "a use of the new name on a line the change left alone: it named something else before",
    with(&|answer| {
      let uses = answer.head.as_mut().unwrap();
      uses.uses.push(used_at("src/Report.kt", 5, 9, 21));
      uses.total += 1;
    }),
    RenameProof::OtherMeaning,
  );

  // the two names are not one declaration
  check("another signature", with(&|answer| answer.same = false), RenameProof::OtherDeclaration);

  // the index does not know every use
  check("an answer that is not complete", with(&|answer| answer.complete = false), RenameProof::Incomplete);
  check(
    "a use tied by its name alone",
    with(&|answer| answer.base.as_mut().unwrap().uses[0].resolved = Resolved::Name),
    RenameProof::Incomplete,
  );
  check(
    "more uses than the answer lists",
    with(&|answer| answer.head.as_mut().unwrap().total += 1),
    RenameProof::Incomplete,
  );
  check("the budget ran out before the uses of a tree", with(&|answer| answer.head = None), RenameProof::Incomplete);
  check(
    "a language whose scopes the index does not read",
    with(&|answer| answer.new.as_mut().unwrap().language = "typescript".to_string()),
    RenameProof::Incomplete,
  );
  // a place of the name in the report that the index lists for no declaration: the declaration is renamed, the use
  // it does list is proven, and the name of the report is not, as a whole
  let partly = with(&|answer| {
    for uses in [answer.base.as_mut().unwrap(), answer.head.as_mut().unwrap()] {
      uses.uses.retain(|used| !(used.path == "src/Report.kt" && used.line == 8));
      uses.total -= 1;
    }
  });
  assert_eq!(proofs(&case, &partly), (RenameProof::Incomplete, RenameProof::Confirmed, vec![0]));

  // no answer of the pair: a place that names no declaration
  check(
    "a pair the index could not ask",
    RenameAnswer {
      unavailable: true,
      ..RenameAnswer::default()
    },
    RenameProof::Unavailable,
  );
  // what the index finds wrong comes before what it could not say
  check(
    "a conflict of an answer that is not complete",
    with(&|answer| {
      answer.complete = false;
      answer.conflicts.push(conflict("captured"));
    }),
    RenameProof::OtherMeaning,
  );
}

#[test]
fn a_use_on_a_line_that_is_read_anyway_and_a_link_of_a_doc_comment_do_not_stand_in_the_way() {
  // the change also deletes a call of the renamed function in a third file, and adds one there, each on lines of
  // its own: lines a reader stops at whatever the rename is
  let keep = ["", "fun keep(): Int = 1"];
  let gone = ["", "fun gone(path: String) {", "  check(openStore(path).path.isNotEmpty())", "}"];
  let added = ["", "fun added(): Store {", "  return connectStore(\"new\")", "}"];
  let case = Case::of(vec![
    changed("src/Report.kt", &text(&REPORT), &renamed(&REPORT)),
    changed("src/Store.kt", &text(&STORE), &renamed(&STORE)),
    changed("src/Extra.kt", &joined(&[&["package demo"], &keep, &gone]), &joined(&[&["package demo"], &added, &keep])),
  ]);
  assert_eq!(case.pairs[2].1.undeclared, [], "the third file renamed nothing in place");
  let mut answer = case.answer("openStore", "connectStore");
  answer.base.as_mut().unwrap().uses.push(used_at("src/Extra.kt", 6, 8, 17));
  answer.head.as_mut().unwrap().uses.push(used_at("src/Extra.kt", 4, 9, 21));
  // and the doc comment of a file outside the subject names the function, before and after
  for uses in [answer.base.as_mut().unwrap(), answer.head.as_mut().unwrap()] {
    uses.uses.push(RenameUse {
      kind: Access::Doc,
      resolved: Resolved::Name,
      ..used_at("docs/Guide.kt", 2, 7, 16)
    });
    uses.total = uses.uses.len();
  }
  assert_eq!(proofs(&case, &answer), (RenameProof::Confirmed, RenameProof::Confirmed, vec![0, 1]));

  // the same use on a line the change left alone is a use outside it
  let deleted = answer.base.as_mut().unwrap().uses.iter_mut().find(|used| used.path == "src/Extra.kt").unwrap();
  deleted.line = 3;
  assert_eq!(proofs(&case, &answer), (RenameProof::UsesOutside, RenameProof::UsesOutside, vec![]));
}

const DESCRIBE: [&str; 4] = [
  "fun describeTheStoreOfTheBackup(): String {",
  "  val backup = openStore(\"backup-of-the-primary-store\")",
  "  return backup.path + \" is the backup of the primary\"",
  "}",
];
const FIRST: [&str; 4] = ["fun first(): Store {", "  val primary = openStore(\"primary\")", "  return primary", "}"];
const SECOND: [&str; 3] = ["fun second(): Int {", "  return 2", "}"];
const HEADER: [&str; 2] = ["package demo", ""];
const GAP: [&str; 1] = [""];
/// No line of a side that a move rewrote (`Leftovers::rewritten`)
const NONE_REWRITTEN: [Vec<u32>; 2] = [Vec::new(), Vec::new()];

/// The proof of the declaration of the store, the file `store` of `case`, with `answer` of the index
fn declared_proof(case: &Case, store: usize, answer: &RenameAnswer) -> RenameProof {
  case.subject(IndexAnswer::Answered(std::slice::from_ref(answer)), store).declared[0].proof
}

/// A view shows a line a move took as the same line at another place, so no reader asks what a name on it means
/// there: a use on such a line is not read anyway. Only a line the move rewrote is a changed line
#[test]
fn a_use_on_a_line_a_move_took_as_it_is_is_not_read_anyway() {
  // what accio answered of this very change: the store renames `openStore`, a new file declares a function of the
  // old name that calls the renamed one, one call is renamed and the function with the other call only moved below
  // two others. The call that moved names the new function now, which the head tree therefore does not list
  let answer: RenameAnswer = serde_json::from_str(
    r#"{"base": {"by_name": 0, "complete": true, "total": 3, "uses": [
        {"column": 4, "end_column": 13, "file": "src/Store.kt", "kind": "declaration", "line": 5, "resolved": "typed"},
        {"column": 15, "end_column": 24, "file": "src/Use.kt", "kind": "call", "line": 4, "resolved": "import"},
        {"column": 16, "end_column": 25, "file": "src/Use.kt", "kind": "call", "line": 9, "resolved": "import"}]},
      "body_equal": true, "complete": true, "conflicts": [],
      "head": {"by_name": 0, "complete": true, "total": 3, "uses": [
        {"column": 9, "end_column": 21, "file": "src/Compat.kt", "kind": "call", "line": 4, "resolved": "import"},
        {"column": 4, "end_column": 16, "file": "src/Store.kt", "kind": "declaration", "line": 5, "resolved": "typed"},
        {"column": 16, "end_column": 28, "file": "src/Use.kt", "kind": "call", "line": 4, "resolved": "import"}]},
      "moved": false, "new": {"file": "src/Store.kt", "kind": "function", "language": "kotlin", "line": 5, "name": "connectStore"},
      "old": {"file": "src/Store.kt", "kind": "function", "language": "kotlin", "line": 5, "name": "openStore"},
      "quality": "structural", "renamed": true, "same": true}"#,
  )
  .unwrap();
  let compat = || {
    added(
      "src/Compat.kt",
      "package demo\n\nfun openStore(path: String): Store {\n  return connectStore(path.trim().lowercase())\n}\n",
    )
  };
  let store = || changed("src/Store.kt", &text(&STORE), &renamed(&STORE));
  let old = joined(&[&HEADER, &DESCRIBE, &GAP, &FIRST, &GAP, &SECOND]);
  let first = renamed(&FIRST);
  let first: Vec<&str> = first.lines().collect();

  // within the file: the four lines are a move of its pair, the line of the call one of them as it is
  let within = Case::of(vec![
    compat(),
    store(),
    changed("src/Use.kt", &old, &joined(&[&HEADER, &first, &GAP, &SECOND, &GAP, &DESCRIBE])),
  ]);
  let moves = &within.pairs[2].0.marks.moves;
  assert_eq!(
    moves.iter().map(|moved| (moved.old_start, moved.new_start, moved.len, &moved.edited)).collect::<Vec<_>>(),
    [(2, 11, 4, &vec![])]
  );
  assert_eq!(within.pairs[2].1.rewritten, NONE_REWRITTEN);
  assert_eq!(declared_proof(&within, 1, &answer), RenameProof::UsesOutside);

  // to another file: the same answer of accio, and a move between the two files
  let across = Case::of(vec![
    compat(),
    added("src/Describe.kt", &joined(&[&HEADER, &DESCRIBE])),
    store(),
    changed("src/Use.kt", &old, &joined(&[&HEADER, &first, &GAP, &SECOND])),
  ]);
  let moved = FileMove {
    start: 2,
    len: 4,
    file: 1,
    line: 2,
    declaration: None,
  };
  assert_eq!(across.subject(IndexAnswer::Unavailable, 3).moved_out, [moved]);
  assert_eq!(declared_proof(&across, 2, &answer), RenameProof::UsesOutside);

  // the new name on a line that only moved named something else where the line stood: the base tree lists no use
  // on it, and no other file calls the function
  let mut captured = answer.clone();
  captured.base.as_mut().unwrap().uses.remove(1);
  let head = captured.head.as_mut().unwrap();
  head.uses.remove(0);
  head.uses.push(used_at("src/Use.kt", 13, 15, 27));
  let new_name = |lines: &[&str]| renamed(lines).lines().map(str::to_string).collect::<Vec<_>>();
  let describe = new_name(&DESCRIBE);
  let describe: Vec<&str> = describe.iter().map(String::as_str).collect();
  let kept = Case::of(vec![
    store(),
    changed(
      "src/Use.kt",
      &joined(&[&HEADER, &describe, &GAP, &FIRST, &GAP, &SECOND]),
      &joined(&[&HEADER, &first, &GAP, &SECOND, &GAP, &describe]),
    ),
  ]);
  assert_eq!(declared_proof(&kept, 0, &captured), RenameProof::OtherMeaning);

  // a line the move rewrote is a changed line: the call renamed on the way is read where it stands now, in both
  // trees, and the rename is confirmed
  let rewritten = Case::of(vec![store(), changed("src/Use.kt", &old, &joined(&[&HEADER, &first, &GAP, &SECOND, &GAP, &describe]))]);
  assert_eq!(rewritten.pairs[1].0.marks.moves.iter().map(|moved| &moved.edited).collect::<Vec<_>>(), [&vec![1]]);
  assert_eq!(rewritten.pairs[1].1.rewritten, [vec![3], vec![12]]);
  let mut both = answer.clone();
  both.head = captured.head.clone();
  assert_eq!(declared_proof(&rewritten, 0, &both), RenameProof::Confirmed);

  // a place of the rename on a line a move took as it is: the line of a call became the call by the new name with
  // another argument, and stands again, as it was, in a function the change added. The index lists the two calls
  // of that line as the use of each tree, and the old name stands on at the other end of the move
  let stay: Vec<String> = (0..8).map(|index| format!("  stay{index}()")).collect();
  let stay: Vec<&str> = stay.iter().map(String::as_str).collect();
  let call = "  val primary = openStore(\"primary-store-of-the-application\") + 1";
  let other = "  val primary = connectStore(\"primary-store-of-the-application\") + 2";
  let place = Case::of(vec![
    store(),
    changed(
      "src/Use.kt",
      &joined(&[&HEADER, &["fun first() {", call, "  done()", "}"], &GAP, &stay]),
      &joined(&[&HEADER, &["fun first() {", other, "  done()", "}"], &GAP, &stay, &GAP, &["fun last() {", call, "}"]]),
    ),
  ]);
  let (diff, left) = &place.pairs[1];
  assert_eq!(
    diff.marks.moves.iter().map(|moved| (moved.old_start, moved.new_start, &moved.edited)).collect::<Vec<_>>(),
    [(3, 17, &vec![])]
  );
  assert_eq!(
    left
      .undeclared
      .iter()
      .map(|rename| (rename.old_sites.as_slice(), rename.new_sites.as_slice()))
      .collect::<Vec<_>>(),
    [(&[3, 16, 25][..], &[3, 16, 28][..])]
  );
  let listed = place.answer("openStore", "connectStore");
  assert_eq!((listed.base.as_ref().unwrap().total, listed.head.as_ref().unwrap().total), (2, 2));
  let (marks, _) = place.marks(IndexAnswer::Answered(std::slice::from_ref(&listed)));
  let subjects: Vec<SubjectMarks> = marks.into_iter().map(|marks| *marks.unwrap().subject.unwrap()).collect();
  assert_eq!(
    (subjects[0].declared[0].proof, subjects[1].used[0].proof),
    (RenameProof::UsesOutside, RenameProof::UsesOutside)
  );

  // a line the move edited in its whitespace alone is the same line still: the old name on it is a use left behind
  let spaced = [DESCRIBE[0], "  val backup = openStore( \"backup-of-the-primary-store\" )", DESCRIBE[2], DESCRIBE[3]];
  let respaced = Case::of(vec![
    compat(),
    store(),
    changed("src/Use.kt", &old, &joined(&[&HEADER, &first, &GAP, &SECOND, &GAP, &spaced])),
  ]);
  assert_eq!(respaced.pairs[2].0.marks.moves.iter().map(|moved| &moved.edited).collect::<Vec<_>>(), [&vec![1]]);
  assert_eq!(respaced.pairs[2].1.rewritten, NONE_REWRITTEN);
  assert_eq!(declared_proof(&respaced, 1, &answer), RenameProof::UsesOutside);
}

/// A block that is equal to itself under the policy of the diff is drawn pale, the lines it was: a use on such a
/// line is not read anyway either
#[test]
fn a_use_on_a_line_of_a_pale_block_is_not_read_anyway() {
  // a third file joins the two lines of a `return` and a call of the old name, which names another function now.
  // A line break after `return` ends the statement in Kotlin, so the lines are no formatting and in no run
  let files = || {
    vec![
      changed("src/Report.kt", &text(&REPORT), &renamed(&REPORT)),
      changed("src/Store.kt", &text(&STORE), &renamed(&STORE)),
      changed(
        "src/Extra.kt",
        "package demo\n\nfun keep(path: String): String {\n  return\n    openStore(path).path\n}\n",
        "package demo\n\nfun keep(path: String): String {\n  return openStore(path).path\n}\n",
      ),
    ]
  };
  let answer = |case: &Case| {
    let mut answer = case.answer("openStore", "connectStore");
    let uses = answer.base.as_mut().unwrap();
    uses.uses.push(used_at("src/Extra.kt", 5, 4, 13));
    uses.total += 1;
    answer
  };
  let shape = |case: &Case| {
    let (diff, _) = &case.pairs[2];
    (
      diff.blocks.iter().map(|block| (block.old_start..block.old_end, block.kind)).collect::<Vec<_>>(),
      diff.marks.runs.len(),
    )
  };
  // with every character counted the two lines are a change no run takes, and are read
  let counted = Case::of(files());
  assert_eq!(shape(&counted), (vec![(3..5, BlockKind::Changed)], 0));
  assert_eq!(proofs(&counted, &answer(&counted)), (RenameProof::Confirmed, RenameProof::Confirmed, vec![0, 1]));
  // under the policy that counts no whitespace the block is pale: the lines it was
  let pale = Case::under(files(), WhitespacePolicy::Ignore);
  assert_eq!(shape(&pale), (vec![(3..5, BlockKind::WhitespaceOnly)], 0));
  assert_eq!(proofs(&pale, &answer(&pale)), (RenameProof::UsesOutside, RenameProof::UsesOutside, vec![]));
}

#[test]
fn a_name_no_file_of_the_subject_declares_and_a_line_that_needs_one_have_no_run() {
  // the report alone: the name changed at every place, and nothing says it was renamed
  let alone = Case::of(vec![changed("src/Report.kt", &text(&REPORT), &renamed(&REPORT))]);
  assert_eq!(alone.marks(IndexAnswer::Unavailable), (vec![None], vec![]));

  // a line that changed in two names, one of which no file declares, is read; the line of the declared one alone is
  // a run
  let old: Vec<String> = REPORT
    .iter()
    .map(|line| line.replace("Store = openStore(path)", "Store = openStore(loadPath(path))"))
    .collect();
  let old: Vec<&str> = old.iter().map(String::as_str).collect();
  let new = renamed(&old).replace("loadPath", "readPath");
  let case = Case::of(vec![changed("src/Report.kt", &text(&old), &new), changed("src/Store.kt", &text(&STORE), &renamed(&STORE))]);
  assert_eq!(
    case.pairs[0].1.undeclared.iter().map(|rename| rename.old.as_str()).collect::<Vec<_>>(),
    ["openStore", "loadPath"]
  );
  let (marks, _) = case.marks(IndexAnswer::Unavailable);
  let report = marks[0].clone().unwrap();
  assert_eq!(report.runs, [run(3, 3)]);
  assert_eq!(report.subject.unwrap().used.iter().map(|used| used.rename.old.as_str()).collect::<Vec<_>>(), ["openStore"]);
}

/// A quiet run of the pair stays the run it is when the subject adds its runs: the line of a renamed local went out
/// with the rows of the file, shown once, and a run of the other reading that holds it beside the call of a name
/// another file declares, in the member above it, would make it a label again, after the rows were drawn. That run
/// is none, and the line of the call is read as any changed line
#[test]
fn a_run_the_subject_adds_takes_no_line_of_a_quiet_run_of_the_pair() {
  // a property that calls the function another file declares, and below it one whose lambda has a parameter
  let lines = |open: &str, total: &str| {
    text(&[
      "package demo",
      "",
      &format!("val opened = {open}(\"primary\")"),
      &format!("val twice = {{ {total}: Int -> {total} * 2 }}"),
    ])
  };
  let loader = || changed("src/Loader.kt", &lines("openStore", "total"), &lines("connectStore", "sum"));
  let quiet = LineRun {
    old_start: 3,
    old_end: 4,
    new_start: 3,
    new_end: 4,
    kind: RunKind::Renamed,
    quiet: true,
  };
  // alone: the line of the parameter is a quiet run, the call is read
  let alone = Case::of(vec![loader()]);
  assert_eq!((&alone.pairs[0].0.marks.runs, &alone.pairs[0].0.marks.starts), (&vec![quiet.clone()], &vec![0, 1, 2]));
  // the pair kept the run the two lines are with the name of the call read as one
  let pending: Vec<(u32, u32)> = alone.pairs[0]
    .1
    .pending
    .iter()
    .flat_map(|block| &block.runs)
    .map(|run| (run.run.old_start, run.run.old_end))
    .collect();
  assert_eq!(pending, [(2, 4)]);

  // beside the file that declares the name: the call is a place of its rename, the quiet run and the stop are the
  // pair's
  let case = Case::of(vec![loader(), changed("src/Store.kt", &text(&STORE), &renamed(&STORE))]);
  let (marks, _) = case.marks(IndexAnswer::Unavailable);
  let loader = marks[0].clone().unwrap();
  assert_eq!((&loader.runs, &loader.starts), (&vec![quiet], &vec![0, 1, 2]));
  assert_eq!(loader.subject.unwrap().used.len(), 1);
}

#[test]
fn a_run_the_subject_adds_takes_its_lines_from_the_moves_between_files() {
  // two lines of the report changed in the renamed name alone, and another file got lines that read as they did
  let calls = ["  val store = openStore(path)", "  val backup = openStore(path + \".bak\")"];
  let old: Vec<&str> = REPORT[..3].iter().chain(&calls).chain(&REPORT[4..]).copied().collect();
  let report = || changed("src/Report.kt", &text(&old), &renamed(&old));
  let copy = || added("src/Copy.kt", &joined(&[&["fun copy(path: String) {"], &calls, &["}"]]));

  // without the file that declares the name the two lines are deleted lines like any other: they stand in the copy
  let unexplained = Case::of(vec![copy(), report()]);
  let (marks, _) = unexplained.marks(IndexAnswer::Unavailable);
  assert_eq!(
    marks[1].clone().unwrap().subject.unwrap().moved_out,
    [FileMove {
      start: 3,
      len: 2,
      file: 0,
      line: 1,
      declaration: None,
    }]
  );

  // with it they are a run of the rename, and a line of a run is in no move
  let case = Case::of(vec![copy(), report(), changed("src/Store.kt", &text(&STORE), &renamed(&STORE))]);
  let (marks, _) = case.marks(IndexAnswer::Unavailable);
  assert_eq!(marks[0], None);
  let report = marks[1].clone().unwrap();
  assert_eq!(
    report.runs.iter().map(|run| (run.old_start, run.old_end, run.kind)).collect::<Vec<_>>(),
    [(3, 5, RunKind::Renamed), (8, 9, RunKind::Renamed)]
  );
  assert_eq!(report.subject.unwrap().moved_out, []);
}

#[test]
fn the_marks_of_a_subject_are_the_same_whenever_they_are_read() {
  let case = renamed_store();
  let answer = case.answer("openStore", "connectStore");
  for answer in [IndexAnswer::Unavailable, IndexAnswer::Asked, IndexAnswer::Answered(std::slice::from_ref(&answer))] {
    assert_eq!(case.marks(answer), case.marks(answer));
  }
  // and of the pairs read again from their texts
  let again = renamed_store();
  assert_eq!(case.marks(IndexAnswer::Asked), again.marks(IndexAnswer::Asked));
  let moved = moved_title();
  assert_eq!(moved.marks(IndexAnswer::Unavailable), moved_title().marks(IndexAnswer::Unavailable));
}

/// The runs of the blocks of a pair that hold a place of a name the pair declares none of, as it keeps them
fn pending(case: &Case, file: usize) -> Vec<(u32, Vec<String>)> {
  let shown = |pending: &PendingRun| {
    let run = &pending.run;
    let kind = match run.kind {
      RunKind::Formatting => "formatting",
      RunKind::Comments => "comments",
      RunKind::Renamed => "renamed",
    };
    format!(
      "{}..{}/{}..{} {kind}{} {:?}",
      run.old_start,
      run.old_end,
      run.new_start,
      run.new_end,
      if run.quiet { " quiet" } else { "" },
      pending.names
    )
  };
  case.pairs[file]
    .1
    .pending
    .iter()
    .map(|block| (block.block, block.runs.iter().map(shown).collect()))
    .collect()
}

/// A pair keeps, beside the names it declares none of, the runs their blocks have with those names read as one that
/// lean on such a name: runs of the kind `Renamed`, each with the names it leans on, so the pass over the subject
/// reads no text. The runs of the pair itself are as they were: nothing of such a name is a mark of its pair
#[test]
fn a_pair_keeps_the_runs_a_name_it_declares_none_of_would_have() {
  // each call a block of its own
  let report = Case::of(vec![changed("src/Report.kt", &text(&REPORT), &renamed(&REPORT))]);
  assert_eq!(
    pending(&report, 0),
    [(0, vec!["3..4/3..4 renamed [0]".to_string()]), (1, vec!["7..8/7..8 renamed [0]".to_string()])]
  );
  assert_eq!(report.pairs[0].0.marks.runs, []);

  // a call wrapped beside the line of the name: a run of formatting of the pair. The block read again has it too,
  // and keeps the run of the name alone: a run of formatting is the pair's to find
  let old = [
    "fun f(path: String) {",
    "  val store = openStore(path)",
    "  check(store.path.isNotEmpty(), path)",
    "  val local = store.path",
    "}",
  ];
  let new = "fun f(path: String) {\n  val store = connectStore(path)\n  check(\n    store.path.isNotEmpty(),\n    path,\n  )\n  val local = store.path\n}\n";
  let wrapped = Case::of(vec![changed("src/Use.kt", &text(&old), &new.replace("path,\n  )", "path\n  )"))]);
  assert_eq!(pending(&wrapped, 0), [(0, vec!["1..2/1..2 renamed [0]".to_string()])]);
  let runs = &wrapped.pairs[0].0.marks.runs;
  assert_eq!(
    runs.iter().map(|run| (run.old_start, run.new_start, run.kind, run.quiet)).collect::<Vec<_>>(),
    [(2, 2, RunKind::Formatting, true)]
  );

  // a file with no such name keeps nothing
  assert_eq!(pending(&moved_title(), 0), []);

  // a run that shares a line with a move of the file is none: the moves were read with the runs of the pair
  let run = |old: u32, names: Vec<u32>| PendingRun {
    run: LineRun {
      old_start: old,
      old_end: old + 1,
      new_start: old + 10,
      new_end: old + 11,
      kind: RunKind::Renamed,
      quiet: false,
    },
    names,
  };
  let mut kept = vec![
    PendingBlock {
      block: 0,
      runs: vec![run(3, vec![0]), run(5, vec![0])],
    },
    PendingBlock {
      block: 2,
      runs: vec![run(8, vec![0])],
    },
  ];
  let moved = |old_start: u32, new_start: u32| LineMove {
    old_start,
    new_start,
    len: 1,
    ..LineMove::default()
  };
  // the old line 3 moved away, and a line moved to the new line 18, where the old line 8 stands
  without_moved_lines(&mut kept, &[moved(3, 40), moved(30, 18)]);
  assert_eq!(
    kept,
    [PendingBlock {
      block: 0,
      runs: vec![run(5, vec![0])],
    }],
    "the second block holds no run any more"
  );
}

/// The index is asked of a declaration by the line and the character of its name, each from 1, as it counts them:
/// a character outside the basic plane is one character and two units of the columns of a view. A text with a lone
/// carriage return, whose lines a view counts otherwise than the index, asks nothing, and its rename is not proven
#[test]
fn the_declarations_are_asked_by_the_lines_and_the_characters_of_their_texts() {
  let store = |name: &str| format!("package demo\n\n/* \u{1F600}\u{1F600} */ fun {name}(path: String): String {{\n  return path\n}}\n");
  let case = Case::of(vec![changed("src/Store.kt", &store("openStore"), &store("connectStore"))]);
  let rename = &case.pairs[0].0.marks.renames[0];
  assert_eq!((rename.old_sites.as_slice(), rename.local), (&[2, 15, 24][..], false));
  assert_eq!(
    case.marks(IndexAnswer::Asked).1,
    [RenamePair {
      old: "src/Store.kt:3:14".to_string(),
      new: "src/Store.kt:3:14".to_string(),
    }]
  );
  assert_eq!(case.subject(IndexAnswer::Asked, 0).declared[0].proof, RenameProof::Asked);

  let lone = Case::of(vec![changed(
    "src/Store.kt",
    &store("openStore").replace("package demo\n\n", "package demo\r\r"),
    &store("connectStore").replace("package demo\n\n", "package demo\r\r"),
  )]);
  assert_eq!(lone.pairs[0].0.marks.renames.len(), 1, "the rename is a label of its file all the same");
  let (marks, questions) = lone.marks(IndexAnswer::Asked);
  assert_eq!(questions, []);
  assert_eq!(marks[0].clone().unwrap().subject.unwrap().declared[0].proof, RenameProof::Unavailable);

  // a path that cannot stand on a line of the list of the pairs asks nothing either
  let tabbed = Case::of(vec![changed("src/a\tb/Store.kt", &store("openStore"), &store("connectStore"))]);
  assert_eq!(tabbed.marks(IndexAnswer::Asked).1, []);
}

/// Whether the run `held` holds the run `run`: of its kind, around its lines on both sides
fn holds(held: &LineRun, run: &LineRun) -> bool {
  held.kind == run.kind && held.old_start <= run.old_start && run.old_end <= held.old_end && held.new_start <= run.new_start && run.new_end <= held.new_end
}

/// What the subject may differ in from the marks of the pair of the file `file`, which went out with its rows:
/// it adds runs that are labels. The quiet runs, the stops, the moves and the renames are those of the pair, every
/// run of the pair lies in a run of its kind, and no two runs share a line
fn assert_adds_labels_alone(case: &Case, file: usize, name: &str) {
  let pair = &case.pairs[file].0.marks;
  let Some(late) = case.marks(IndexAnswer::Unavailable).0[file].clone() else { return };
  let quiet = |runs: &[LineRun]| runs.iter().filter(|run| run.quiet).cloned().collect::<Vec<_>>();
  assert_eq!(quiet(&late.runs), quiet(&pair.runs), "the quiet runs: {name}");
  assert_eq!((&late.starts, &late.moves, &late.renames), (&pair.starts, &pair.moves, &pair.renames), "{name}");
  for run in &pair.runs {
    assert!(late.runs.iter().any(|held| holds(held, run)), "a run of the pair is gone: {run:?} of {name}");
  }
  for run in late.runs.iter().filter(|run| !pair.runs.contains(run)) {
    assert!(run.kind == RunKind::Renamed && !run.quiet, "a run the subject added is no label: {run:?} of {name}");
  }
  assert!(
    late
      .runs
      .windows(2)
      .all(|runs| runs[0].old_end <= runs[1].old_start && runs[0].new_end <= runs[1].new_start),
    "two runs share a line: {name}"
  );
}

/// The files of a subject are put together after the rows of each were drawn, so they add labels and nothing else:
/// a line of formatting that is quiet only with a name read as one is no run, whoever declares the name
#[test]
fn the_subject_adds_labels_alone_never_a_quiet_run_or_a_stop() {
  let store = || changed("src/Store.kt", &text(&STORE), &renamed(&STORE));
  let report = |old: &[&str], new: &[&str]| changed("src/Report.kt", &text(old), &text(new));
  // a call wrapped otherwise at the end of its block, before a line that changed in a name alone: with that name
  // read as one the wrapped lines are the same code up to the same token, a run of formatting. The pair has none,
  // and neither has the subject, whether the name after it is the one the store renamed or one no file declares
  let wrapped = |after: [&str; 2]| {
    let old = [
      "fun report(path: String) {",
      "  val c = other(openStore(a))",
      "  call(x,",
      "    y)",
      "  // a note of it",
      after[0],
      "  keep()",
      "}",
    ];
    let new = [
      "fun report(path: String) {",
      "  val c = other(connectStore(a))",
      "  call(",
      "    x, y)",
      "  // a note of it",
      after[1],
      "  keep()",
      "}",
    ];
    Case::of(vec![report(&old, &new), store()])
  };
  for (name, after, runs) in [
    ("a name no file declares", ["  loadPath(x).use(y)", "  readPath(x).use(y)"], vec![run(1, 1)]),
    (
      "the name the store renamed",
      ["  openStore(x).use(y)", "  connectStore(x).use(y)"],
      vec![run(1, 1), run(5, 5)],
    ),
  ] {
    let case = wrapped(after);
    assert_eq!(case.pairs[0].0.marks.runs, [], "{name}");
    assert_eq!(case.marks(IndexAnswer::Unavailable).0[0].clone().unwrap().runs, runs, "{name}");
    assert_adds_labels_alone(&case, 0, name);
  }

  // a line of the renamed name at the end of its block is a run of the rename only while the code after the block
  // reads the same: by a name no file of the subject declares it does not
  let leaning = |after: [&str; 2]| {
    let old = ["fun report(path: String) {", "  val c = other(openStore(a))", "  // a note of it", after[0], "}"];
    let new = ["fun report(path: String) {", "  val c = other(connectStore(a))", "  // a note of it", after[1], "}"];
    Case::of(vec![report(&old, &new), store()])
  };
  let unexplained = leaning(["  loadPath(x).use(y)", "  readPath(x).use(y)"]);
  assert_eq!(
    pending(&unexplained, 0),
    [(0, vec!["1..2/1..2 renamed [0, 1]".to_string()]), (1, vec!["3..4/3..4 renamed [1]".to_string()])],
    "the run of the first block leans on both names"
  );
  let marks = unexplained.marks(IndexAnswer::Unavailable).0[0].clone().unwrap();
  assert_eq!(
    (marks.runs, marks.subject.unwrap().used.len()),
    (vec![], 1),
    "the name is a rename of the file, its line is read"
  );
  let explained = leaning(["  openStore(x).use(y)", "  connectStore(x).use(y)"]);
  assert_eq!(explained.marks(IndexAnswer::Unavailable).0[0].clone().unwrap().runs, [run(1, 1), run(3, 3)]);

  // and of every report made of such statements, each changed one way or another
  let statements: [(&[&str], &[&[&str]]); 9] = [
    (
      &["  val a = openStore(path)"],
      &[
        &["  val a = connectStore(path)"],
        &["  val a = connectStore( path )"],
        &["  val a =", "    connectStore(path)"],
      ],
    ),
    (
      &["  call(openStore(path), other)"],
      &[
        &["  call(connectStore(path), other)"],
        &["  call(", "    connectStore(path),", "    other)"],
        &["  call(connectStore(path),", "    other)"],
      ],
    ),
    (
      &["  call(", "    openStore(path),", "    other)"],
      &[
        &["  call(connectStore(path), other)"],
        &["  call(", "    connectStore(path),", "    other)"],
        &["  call(", "    connectStore(path), other)"],
      ],
    ),
    (&["  call(x, y)"], &[&["  call(x, y)"], &["  call( x, y )"], &["  call(x,", "    y)"], &["  call(x, z)"]]),
    (&["  call(x,", "    y)"], &[&["  call(x, y)"], &["  call(x,", "    y)"], &["  call(", "    x, y)"]]),
    (
      &["  val b = compute(1, 2)"],
      &[&["  val b = compute(1, 2)"], &["  val b = compute(1,2)"], &["  val b = compute(1, 3)"]],
    ),
    (&["  // a note of it"], &[&["  // a note of it"], &["  // another note of it"], &["  //a note of it"]]),
    (
      &["  openStore(x).use(y)"],
      &[&["  connectStore(x).use(y)"], &["  connectStore(x)", "    .use(y)"], &["  connectStore(x).use( y )"]],
    ),
    (&["  loadPath(x).use(y)"], &[&["  readPath(x).use(y)"], &["  readPath(x)", "    .use(y)"]]),
  ];
  // a generator of its own, so the cases are the same on every run
  let mut state = 1u64;
  let mut next = |bound: usize| {
    state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    usize::try_from((state >> 33) % bound as u64).unwrap()
  };
  let mut added = 0;
  for case in 0..600 {
    let (mut old, mut new) = (vec!["fun report(path: String) {"], vec!["fun report(path: String) {"]);
    for _ in 0..2 + next(6) {
      let (old_lines, news) = statements[next(statements.len())];
      old.extend(old_lines);
      new.extend(news[next(news.len())]);
      if next(4) == 0 {
        old.push("  keep()");
        new.push("  keep()");
      }
    }
    old.push("}");
    new.push("}");
    let name = format!("case {case}:\n{}\n->\n{}", old.join("\n"), new.join("\n"));
    let case = Case::of(vec![report(&old, &new), store()]);
    added += usize::from(
      case.marks(IndexAnswer::Unavailable).0[0]
        .as_ref()
        .is_some_and(|late| late.runs != case.pairs[0].0.marks.runs),
    );
    assert_adds_labels_alone(&case, 0, &name);
  }
  assert!(added > 100, "the cases have runs to add: {added}");
}

/// A use the index lists without a word (a positional argument that sets a property: as many columns before it as
/// after it) has no name to change, and a use by another word (an alias of an import, an accessor) that both trees
/// hold at the same place of a line the change left alone asks nothing of the rename
#[test]
fn a_use_that_writes_no_word_or_the_same_other_word_in_both_trees_does_not_stand_in_the_way() {
  // what accio answered of this very change: `Item("x", 1)` stands untouched and sets the renamed property
  let item = |name: &str| format!("package demo\n\ndata class Item(val {name}: String, val count: Int)\n\nfun describe(item: Item): String {{\n  return item.{name}\n}}\n");
  let other = |name: &str| format!("package demo\n\nfun show(item: Item): String {{\n  val text = item.{name}\n  return text\n}}\n");
  let makes = |name: &str| format!("package demo\n\nfun make(): Item {{\n  return Item(\"x\", 1)\n}}\n\nfun named(): Item {{\n  return Item({name} = \"y\", count = 2)\n}}\n");
  let case = Case::of(vec![
    changed("src/Item.kt", &item("label"), &item("title")),
    changed("src/Other.kt", &other("label"), &other("title")),
    changed("src/Use.kt", &makes("label"), &makes("title")),
  ]);
  let uses = r#"{"total": 5, "uses": [
    {"column": 20, "end_column": 25, "file": "src/Item.kt", "kind": "declaration", "line": 3, "resolved": "typed"},
    {"column": 14, "end_column": 19, "file": "src/Item.kt", "kind": "read", "line": 6, "resolved": "typed"},
    {"column": 18, "end_column": 23, "file": "src/Other.kt", "kind": "read", "line": 4, "resolved": "typed"},
    {"column": 14, "end_column": 14, "file": "src/Use.kt", "kind": "write", "line": 4, "resolved": "typed"},
    {"column": 14, "end_column": 19, "file": "src/Use.kt", "kind": "write", "line": 8, "resolved": "typed"}]}"#;
  let answer: RenameAnswer = serde_json::from_str(&format!(
    r#"{{"base": {uses}, "head": {uses}, "complete": true, "conflicts": [], "same": true,
      "old": {{"language": "kotlin", "name": "label"}}, "new": {{"language": "kotlin", "name": "title"}}}}"#
  ))
  .unwrap();
  let (marks, questions) = case.marks(IndexAnswer::Answered(std::slice::from_ref(&answer)));
  assert_eq!(questions.len(), 1);
  let subjects: Vec<SubjectMarks> = marks.into_iter().map(|marks| *marks.unwrap().subject.unwrap()).collect();
  assert_eq!(subjects[0].declared[0].proof, RenameProof::Confirmed);
  assert_eq!(
    subjects[1..].iter().map(|subject| subject.used[0].proof).collect::<Vec<_>>(),
    [RenameProof::Confirmed, RenameProof::Confirmed]
  );

  // a use by another word: an alias of the import of the function, in a third file whose import changed and whose
  // call by the alias did not, one line further down in the new text
  let case = Case::of(vec![
    changed("src/Report.kt", &text(&REPORT), &renamed(&REPORT)),
    changed("src/Store.kt", &text(&STORE), &renamed(&STORE)),
    changed(
      "src/Alias.kt",
      "package other\n\nimport demo.openStore as open\n\nfun aliased() = open(\"a\")\n",
      "package other\n\nimport demo.connectStore as open\nimport demo.Store\n\nfun aliased() = open(\"a\")\n",
    ),
  ]);
  let complete = case.answer("openStore", "connectStore");
  let word = |path: &str, line: u32, word: &str| RenameUse {
    text: Some(word.to_string()),
    ..used_at(path, line, 16, 20)
  };
  let with = |base: &[RenameUse], head: &[RenameUse]| {
    let mut answer = complete.clone();
    for (uses, more) in [(answer.base.as_mut().unwrap(), base), (answer.head.as_mut().unwrap(), head)] {
      uses.uses.extend_from_slice(more);
      uses.total = uses.uses.len();
    }
    answer
  };
  let confirmed = (RenameProof::Confirmed, RenameProof::Confirmed, vec![0, 1]);
  let incomplete = (RenameProof::Incomplete, RenameProof::Incomplete, vec![]);
  let alias = |line: u32| word("src/Alias.kt", line, "open");
  assert_eq!(
    proofs(&case, &with(&[alias(5)], &[alias(6)])),
    confirmed,
    "the same word at the same place of a line the change left alone"
  );
  // and in a file the change did not touch, where a line is the same line in both trees
  let outside = word("src/Far.kt", 9, "open");
  assert_eq!(proofs(&case, &with(std::slice::from_ref(&outside), std::slice::from_ref(&outside))), confirmed);

  // nothing is known of such a word that the other tree does not hold at its place: no proof, and no verdict of
  // what the rename changed either
  for (name, base, head) in [
    ("the old tree alone", vec![alias(5)], vec![]),
    ("the new tree alone", vec![], vec![alias(6)]),
    ("another line", vec![alias(5)], vec![alias(5)]),
    ("another word", vec![alias(5)], vec![word("src/Alias.kt", 6, "connect")]),
    ("another line of a file outside the change", vec![outside], vec![word("src/Far.kt", 10, "open")]),
  ] {
    assert_eq!(proofs(&case, &with(&base, &head)), incomplete, "{name}");
  }
  // a use without a word is no use of the name, wherever it stands
  let wordless = RenameUse {
    end_column: 9,
    ..used_at("src/Report.kt", 5, 9, 9)
  };
  assert_eq!(proofs(&case, &with(std::slice::from_ref(&wordless), &[])), confirmed);
}

/// The index names a line by the line feeds before it, the diff also by a carriage return that no line feed
/// follows: a use the index lists in a text with one is on no line the diff knows, so the answer proves nothing
#[test]
fn a_use_in_a_text_with_a_lone_carriage_return_proves_nothing() {
  // a third file of the subject deletes a call of the renamed function, on a line a reader stops at anyway
  let extra = |end: &str| {
    changed(
      "src/Extra.kt",
      &format!("package demo{end}{end}fun keep(): Int = 1\n\nfun gone(path: String) {{\n  check(openStore(path).path.isNotEmpty())\n}}\n"),
      &format!("package demo{end}{end}fun keep(): Int = 1\n"),
    )
  };
  let case = |end: &str| {
    Case::of(vec![
      changed("src/Report.kt", &text(&REPORT), &renamed(&REPORT)),
      changed("src/Store.kt", &text(&STORE), &renamed(&STORE)),
      extra(end),
    ])
  };
  let (plain, lone) = (case("\n"), case("\r"));
  assert_eq!((plain.pairs[2].1.lone_returns, lone.pairs[2].1.lone_returns), ([false, false], [true, true]));
  let mut answer = plain.answer("openStore", "connectStore");
  let uses = answer.base.as_mut().unwrap();
  uses.uses.push(used_at("src/Extra.kt", 5, 8, 17));
  uses.total += 1;
  assert_eq!(proofs(&plain, &answer), (RenameProof::Confirmed, RenameProof::Confirmed, vec![0, 1]));
  // the same answer of the same lines: the line the index names is not the line of the diff
  assert_eq!(proofs(&lone, &answer), (RenameProof::Incomplete, RenameProof::Incomplete, vec![]));
  // what the index found wrong itself stays said
  answer.conflicts.push(conflict("left"));
  assert_eq!(proofs(&lone, &answer), (RenameProof::UsesOutside, RenameProof::UsesOutside, vec![]));
}

/// The index proves the uses of a declaration of Kotlin and of Java alone, so a name a file of another language
/// declares is asked of no declaration, and says so at once
#[test]
fn a_name_declared_in_a_language_the_index_proves_nothing_of_is_not_asked() {
  let store = |name: &str| format!("export function {name}(path: string): string {{\n  return path\n}}\n");
  let report = |name: &str| format!("import {{ {name} }} from \"./store\"\n\nexport function report(path: string): string {{\n  return {name}(path)\n}}\n");
  let case = Case::of(vec![
    changed("src/report.ts", &report("openStore"), &report("connectStore")),
    changed("src/store.ts", &store("openStore"), &store("connectStore")),
  ]);
  assert_eq!(case.pairs[1].0.marks.renames.len(), 1, "the store names its rename");
  for answer in [IndexAnswer::Asked, IndexAnswer::Unavailable] {
    let (marks, questions) = case.marks(answer);
    assert_eq!(questions, [], "no process for an answer that proves nothing");
    let (report, store) = (marks[0].clone().unwrap().subject.unwrap(), marks[1].clone().unwrap().subject.unwrap());
    assert_eq!((report.used[0].proof, store.declared[0].proof), (RenameProof::Incomplete, RenameProof::Incomplete));
  }
  // beside a declaration of Kotlin in the same subject, which is asked
  let mixed = Case::of(vec![
    changed("src/store.ts", &store("openStore"), &store("connectStore")),
    changed("src/Store.kt", &text(&STORE), &text(&STORE).replace("openStore", "attachStore")),
  ]);
  let (marks, questions) = mixed.marks(IndexAnswer::Asked);
  assert_eq!(questions.len(), 1);
  let proofs: Vec<RenameProof> = marks.into_iter().map(|marks| marks.unwrap().subject.unwrap().declared[0].proof).collect();
  assert_eq!(proofs, [RenameProof::Incomplete, RenameProof::Asked]);
}
