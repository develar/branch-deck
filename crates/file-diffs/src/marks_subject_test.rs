//! The pass over the files of a subject (`marks_subject.rs`): the lines that moved between two files, the names one
//! file declares and another uses, and what each answer of the index proves of them. Every file of a case is read as
//! the service reads a pair: its diff, its marks and what it left, from its two texts.

use crate::accio::{Access, RenameAnswer, RenameConflict, RenamePair, RenameUse, RenameUses, RenamedDeclaration, Resolved};
use crate::leftovers::{Leftovers, PendingBlock, PendingRun};
use crate::marks::{is_quiet, marks_of, pair_grammar, pair_indented, without_moved_lines};
use crate::marks_subject::{IndexAnswer, PairFacts, Subject, as_confirmed};
use code_symbols::tokens::tokens;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use test_log::test;
use text_diff::model::{DiffBudget, DiffMarks, FileMove, LineMove, LineRun, RenameProof, RunKind, SubjectMarks, TextDiff, diff_texts};
use text_diff::policy::WhitespacePolicy;

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

/// The diff of a file with the marks of its pair, and what the pair left
fn read(file: &File) -> (TextDiff, Leftovers) {
  let (old, new) = (file.old.as_deref().unwrap_or_default(), file.new.as_deref().unwrap_or_default());
  let mut diff = diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget::NONE);
  let grammar = pair_grammar(file.path, file.path);
  let pair = marks_of(&diff, old, new, grammar, WhitespacePolicy::Default, pair_indented(file.path, file.path), |new_side| {
    tokens(grammar?, if new_side { new } else { old }).map(Arc::new)
  });
  let (marks, left) = Leftovers::of(&diff.blocks, pair, old, new);
  diff.marks = marks;
  (diff, left)
}

/// The files of a case, read
struct Case {
  files: Vec<File>,
  pairs: Vec<(TextDiff, Leftovers)>,
}

impl Case {
  fn of(files: Vec<File>) -> Self {
    let pairs = files.iter().map(read).collect();
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
          diff,
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

  /// What the subject says of the file `index`
  fn subject(&self, answer: IndexAnswer<'_>, index: usize) -> SubjectMarks {
    let subject = self.marks(answer).0[index].clone().and_then(|marks| marks.subject);
    *subject.unwrap_or_else(|| panic!("the subject says nothing of {}", self.files[index].path))
  }

  /// The answer of an index that lists the places of every rename of the two names as the uses of the one
  /// declaration: what accio answers of a rename that is complete
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
      complete: true,
      old: kotlin(),
      new: kotlin(),
      base: uses(base),
      head: uses(head),
      conflicts: Vec::new(),
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
      }],
      ..SubjectMarks::default()
    })
  );
  // a label: no run, no move within a file, and every block is read where it was
  for (marks, (diff, _)) in [ledger, report].iter().zip(&case.pairs) {
    assert_eq!(
      (&marks.runs, &marks.moves, &marks.renames, &marks.starts),
      (&diff.marks.runs, &diff.marks.moves, &diff.marks.renames, &diff.marks.starts)
    );
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

  // the kind is not switched on: the confirmed runs are labels, and every block a stop
  assert!(!is_quiet(RunKind::Renamed));
  assert_eq!(report.runs, [run(3, 3), run(7, 7)]);
  assert_eq!(report.starts, [0, 1, 3, 1, 1, 7]);
  // once it is: the uses are passed, the declaration is read
  let proven = as_confirmed(&report, &case.pairs[0].0.blocks);
  assert_eq!(proven.runs.iter().map(|run| run.quiet).collect::<Vec<_>>(), [true, true]);
  assert_eq!(proven.starts, [] as [u32; 0]);
  assert_eq!(as_confirmed(&store, &case.pairs[1].0.blocks).starts, [0, 1, 4]);
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

/// A pair keeps, beside the names it declares none of, the runs their blocks have with those names read as one: all
/// the runs of each such block, each with the names it needs, so the pass over the subject reads no text. The runs
/// of the pair itself are as they were: nothing of such a name is a mark of its pair
#[test]
fn a_pair_keeps_the_runs_a_name_it_declares_none_of_would_have() {
  // each call a block of its own
  let report = Case::of(vec![changed("src/Report.kt", &text(&REPORT), &renamed(&REPORT))]);
  assert_eq!(
    pending(&report, 0),
    [(0, vec!["3..4/3..4 renamed [0]".to_string()]), (1, vec!["7..8/7..8 renamed [0]".to_string()])]
  );
  assert_eq!(report.pairs[0].0.marks.runs, []);

  // a call wrapped beside the line of the name: a run of formatting of the pair, and of the block read again
  let old = [
    "fun f(path: String) {",
    "  val store = openStore(path)",
    "  check(store.path.isNotEmpty(), path)",
    "  val local = store.path",
    "}",
  ];
  let new = "fun f(path: String) {\n  val store = connectStore(path)\n  check(\n    store.path.isNotEmpty(),\n    path,\n  )\n  val local = store.path\n}\n";
  let wrapped = Case::of(vec![changed("src/Use.kt", &text(&old), &new.replace("path,\n  )", "path\n  )"))]);
  assert_eq!(
    pending(&wrapped, 0),
    [(0, vec!["1..2/1..2 renamed [0]".to_string(), "2..3/2..6 formatting quiet []".to_string()])]
  );
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
      runs: vec![run(8, vec![0]), run(9, vec![])],
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
    "the second block holds no run of such a name any more"
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
