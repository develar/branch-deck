//! The marks of a diff (`marks.rs`, `marks_runs.rs`): what is formatting, what is comments, what is never marked, the
//! limits, and that the question a block is asked before the tokens changes no answer.

use crate::bindings::CodeSymbolBindings;
use crate::fixture_test::unhurried;
use crate::model::{DiffContent, DiffSource};
use code_symbols::navigate::{Access, LocalBinding, NameQuestion, Position, Site, names_at};
use code_tokens::{Equality, Tokens, compare, tokens};
use code_tokens::{Grammar, grammar_of};
use diff_marks::marks::fits_inner;
use diff_marks::marks::{
  MAX_BLOCK_TOKENS, MAX_FILE_TOKEN_WORK, MAX_TOKEN_EDITS, SideText, has_proof, is_quiet, marks_of, may_hold_comment, pair_grammar, pair_indented, read_blocks, runs_of,
};
use diff_marks::marks_runs::{Asked, BlockLines, NO_RENAME, runs_of_block};
use diff_marks::model::{DiffMarks, FileMove, Rename, SubjectMarks};
use git_executor::git_command_executor::GitCommandExecutor;
use line_diff::cancellation::Deadline;
use line_diff::model::{BlockKind, DiffBudget, LineDiff, MAX_INNER_LINES, RunKind, diff_texts, starts_of};
use line_diff::policy::WhitespacePolicy;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::cell::{Cell, OnceCell};
use std::fmt::Write as _;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use WhitespacePolicy::{Default as Exact, Ignore, IgnoreBlankLines};

/// A diff of the comparison with the marks of its pair: what the tests of the marks read, of the values of line-diff
/// and diff-marks. It reads as its diff (`Deref`)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MarkedDiff {
  pub(crate) diff: LineDiff,
  pub(crate) marks: DiffMarks,
}

impl From<LineDiff> for MarkedDiff {
  /// The diff of the comparison alone, every block a stop
  fn from(diff: LineDiff) -> Self {
    let marks = DiffMarks {
      starts: starts_of(&diff.blocks, &[], &[]),
      ..DiffMarks::default()
    };
    Self { diff, marks }
  }
}

impl From<&text_diff::model::TextDiff> for MarkedDiff {
  /// The values of a diff of the service
  fn from(diff: &text_diff::model::TextDiff) -> Self {
    Self {
      diff: diff.line_diff(),
      marks: diff.marks.clone().into(),
    }
  }
}

impl std::ops::Deref for MarkedDiff {
  type Target = LineDiff;

  fn deref(&self) -> &LineDiff {
    &self.diff
  }
}

/// What a reading of a block asks of its lines where every line has text and no name is renamed
pub(crate) const ANY_TEXT: Asked<'static> = Asked {
  has_text: &|_, _| true,
  renamed: NO_RENAME,
};

/// The diff of two texts of a file at `path` with its marks as they are proven (`PairMarks::as_proven`: a run of a kind
/// that is not switched on is quiet here where its proof holds, so the tests ask the proof whatever is on), the
/// names it leaves to the other files of its change, and how often the tokens of each side were read. The marks a
/// view gets (`marks_of`) must be these with the runs of a kind that is not switched on as labels, and the starts
/// that follow. The runs and the renames are also read without the question before the tokens: it must change no
/// answer
pub(crate) fn read_pair(path: &str, old: &str, new: &str, policy: WhitespacePolicy) -> (MarkedDiff, Vec<Rename>, [u32; 2]) {
  let mut diff = MarkedDiff::from(diff_texts(old, new, policy, &DiffBudget::NONE));
  let grammar = pair_grammar(path, path);
  let asked = [Cell::new(0), Cell::new(0)];
  // each side is parsed once, however often it is asked
  let parsed: [OnceCell<Option<Arc<Tokens>>>; 2] = [OnceCell::new(), OnceCell::new()];
  let parse = |new_side: bool| {
    parsed[usize::from(new_side)]
      .get_or_init(|| tokens(grammar?, if new_side { new } else { old }).map(Arc::new))
      .clone()
  };
  let shown = marks_of(
    &diff,
    old,
    new,
    grammar,
    policy,
    pair_indented(path, path),
    |new_side| {
      let side = &asked[usize::from(new_side)];
      side.set(side.get() + 1);
      parse(new_side)
    },
    &CodeSymbolBindings,
  );
  let (marks, undeclared) = (shown.as_proven(&diff.blocks), shown.undeclared.clone());
  // what a view gets: the proven marks with the runs of a kind that is not switched on as labels, and their starts
  let mut labelled = marks.clone();
  let mut off = Vec::new();
  for (index, run) in (0u32..).zip(&mut labelled.runs) {
    if run.quiet && !is_quiet(run.kind) {
      run.quiet = false;
      off.push(index);
    }
  }
  labelled.starts = starts_of(&diff.blocks, &labelled.runs, &labelled.moves);
  assert_eq!(
    (&shown.marks, &shown.proven),
    (&labelled, &off),
    "{path}: {old:?} -> {new:?}: the marks of a view are not the proven ones with the kinds that are off as labels"
  );
  diff.marks = marks;
  if let Some(grammar) = grammar.filter(|_| fits_inner(old, diff.old_line_count) && fits_inner(new, diff.new_line_count)) {
    let unasked = runs_of(&diff, &SideText::new(grammar, old), &SideText::new(grammar, new), false, &parse, &CodeSymbolBindings);
    assert_eq!(
      (&diff.marks.runs, &diff.marks.renames, &undeclared),
      (&unasked.runs, &unasked.renames, &unasked.undeclared),
      "{path}: {old:?} -> {new:?}: the question before the tokens changed the answer"
    );
  }
  (diff, undeclared, asked.map(|side| side.get()))
}

fn read_counted(path: &str, old: &str, new: &str, policy: WhitespacePolicy) -> (MarkedDiff, [u32; 2]) {
  let (diff, _, asked) = read_pair(path, old, new, policy);
  (diff, asked)
}

fn read(path: &str, old: &str, new: &str, policy: WhitespacePolicy) -> MarkedDiff {
  read_counted(path, old, new, policy).0
}

/// A rename as words: its names, `local` for a name bound inside a body, the numbers of the places that declare it,
/// and `line:column` of each place of the old text, then of the new one
pub(crate) fn show_rename(rename: &Rename) -> String {
  let places = |sites: &[u32]| -> String {
    let places: Vec<String> = sites.as_chunks::<3>().0.iter().map(|site| format!("{}:{}", site[0], site[1])).collect();
    places.join(" ")
  };
  format!(
    "{}->{}{} declared {:?} at {} / {}",
    rename.old,
    rename.new,
    if rename.local { " local" } else { "" },
    rename.declared,
    places(&rename.old_sites),
    places(&rename.new_sites)
  )
}

/// The blocks, the runs, the moves, the renames and the starts of a diff as one line: `old/new` lines of each block
/// and each run, `old->new` lines of each move
pub(crate) fn show(diff: &MarkedDiff) -> String {
  let blocks: Vec<String> = diff
    .blocks
    .iter()
    .map(|block| {
      let kind = match block.kind {
        BlockKind::Changed => "",
        BlockKind::WhitespaceOnly => " pale",
        BlockKind::Ignored => " ignored",
      };
      format!("{}..{}/{}..{}{kind}", block.old_start, block.old_end, block.new_start, block.new_end)
    })
    .collect();
  let runs: Vec<String> = diff
    .marks
    .runs
    .iter()
    .map(|run| {
      let kind = match run.kind {
        RunKind::Formatting => "formatting",
        RunKind::Comments => "comments",
        RunKind::Renamed => "renamed",
      };
      format!(
        "{}..{}/{}..{} {kind}{}",
        run.old_start,
        run.old_end,
        run.new_start,
        run.new_end,
        if run.quiet { " quiet" } else { "" }
      )
    })
    .collect();
  let moves: Vec<String> = diff
    .marks
    .moves
    .iter()
    .map(|line_move| {
      let (edited, blanks) = (line_move.edited.len(), (line_move.blanks_before, line_move.blanks_after));
      format!(
        "{}..{}->{}..{}{}{}{}",
        line_move.old_start,
        line_move.old_start + line_move.len,
        line_move.new_start,
        line_move.new_start + line_move.len,
        if blanks == (0, 0) {
          String::new()
        } else {
          format!(" blanks {}+{}", blanks.0, blanks.1)
        },
        if line_move.indent == 0 {
          String::new()
        } else {
          format!(" indent {:+}", line_move.indent)
        },
        if edited == 0 { String::new() } else { format!(" edited {edited}") },
      )
    })
    .collect();
  // the moves only where a diff has them: most cases of the runs have none
  let moves = if moves.is_empty() { String::new() } else { format!(" moves [{}]", moves.join(", ")) };
  // the renames likewise
  let renames: Vec<String> = diff.marks.renames.iter().map(show_rename).collect();
  let renames = if renames.is_empty() {
    String::new()
  } else {
    format!(" renames [{}]", renames.join(", "))
  };
  let subject = diff.marks.subject.as_deref().map_or_default(show_subject);
  format!(
    "blocks [{}] runs [{}]{moves}{renames} starts {:?}{subject}",
    blocks.join(", "),
    runs.join(", "),
    diff.marks.starts
  )
}

/// What the other files of a subject say of a file, each part only where it says something: the lines that moved
/// out of it and into it (`lines->file:line`), the names another file declares (as a rename, with the file and the
/// line of the declaration and its proof), the proof of each rename it declares, and the runs the index confirmed
fn show_subject(subject: &SubjectMarks) -> String {
  let moved = |moves: &[FileMove]| -> Vec<String> {
    moves
      .iter()
      .map(|moved| format!("{}..{}->{}:{}", moved.start, moved.start + moved.len, moved.file, moved.line))
      .collect()
  };
  let used: Vec<String> = subject
    .used
    .iter()
    .map(|used| format!("{} in {}:{} {:?}", show_rename(&used.rename), used.file, used.line, used.proof))
    .collect();
  let declared: Vec<String> = subject.declared.iter().map(|check| format!("{} {:?}", check.rename, check.proof)).collect();
  let confirmed: Vec<String> = subject.confirmed.iter().map(u32::to_string).collect();
  [
    ("moved out", moved(&subject.moved_out)),
    ("moved in", moved(&subject.moved_in)),
    ("used", used),
    ("declared", declared),
    ("confirmed", confirmed),
  ]
  .into_iter()
  .filter(|(_, items)| !items.is_empty())
  .fold(String::new(), |mut shown, (name, items)| {
    write!(shown, " {name} [{}]", items.join(", ")).unwrap();
    shown
  })
}

/// Each case against what `show` gives for it; every failure in one message
fn check_cases(cases: &[(&str, WhitespacePolicy, &str, &str, &str)]) {
  let mut failed = String::new();
  for (path, policy, old, new, expected) in cases {
    let found = show(&read(path, old, new, *policy));
    if found != *expected {
      writeln!(failed, "{path} {policy:?}: {old:?} -> {new:?}\n  found    {found}\n  expected {expected}").unwrap();
    }
  }
  assert!(failed.is_empty(), "\n{failed}");
}

#[test]
fn lines_that_changed_in_their_formatting_alone_are_a_quiet_run() {
  check_cases(&[
    // a call wrapped inside its brackets, in each grammar
    (
      "a.kt",
      Exact,
      "fun f() {\n  call(a, b)\n}\n",
      "fun f() {\n  call(\n    a,\n    b\n  )\n}\n",
      "blocks [1..2/1..5] runs [1..2/1..5 formatting quiet] starts []",
    ),
    (
      "a.kts",
      Exact,
      "val x = call(a, b)\n",
      "val x = call(\n  a, b)\n",
      "blocks [0..1/0..2] runs [0..1/0..2 formatting quiet] starts []",
    ),
    (
      "A.java",
      Exact,
      "class A {\n  int x = call(a, b);\n}\n",
      "class A {\n  int x = call(a,\n      b);\n}\n",
      "blocks [1..2/1..3] runs [1..2/1..3 formatting quiet] starts []",
    ),
    // a trailing comma is a token
    (
      "a.rs",
      Exact,
      "fn f() {\n  call(a, b);\n}\n",
      "fn f() {\n  call(\n    a,\n    b,\n  );\n}\n",
      "blocks [1..2/1..5] runs [] starts [0, 1, 1]",
    ),
    (
      "a.rs",
      Exact,
      "fn f() {\n  call(a, b);\n}\n",
      "fn f() {\n  call(\n    a,\n    b\n  );\n}\n",
      "blocks [1..2/1..5] runs [1..2/1..5 formatting quiet] starts []",
    ),
    (
      "a.ts",
      Exact,
      "const x = call(a, b)\n",
      "const x = call(\n  a,\n  b)\n",
      "blocks [0..1/0..3] runs [0..1/0..3 formatting quiet] starts []",
    ),
    (
      "a.tsx",
      Exact,
      "const x = <A b={c} d={e} />\n",
      "const x = <A\n  b={c}\n  d={e} />\n",
      "blocks [0..1/0..3] runs [0..1/0..3 formatting quiet] starts []",
    ),
    (
      "a.js",
      Exact,
      "const x = call(a, b)\n",
      "const x = call(a,\n  b)\n",
      "blocks [0..1/0..2] runs [0..1/0..2 formatting quiet] starts []",
    ),
    (
      "A.cs",
      Exact,
      "class A {\n  int X = Call(a, b);\n}\n",
      "class A {\n  int X = Call(a,\n    b);\n}\n",
      "blocks [1..2/1..3] runs [1..2/1..3 formatting quiet] starts []",
    ),
    (
      "BUILD",
      Exact,
      "x = f(a, b)\n",
      "x = f(\n    a,\n    b)\n",
      "blocks [0..1/0..3] runs [0..1/0..3 formatting quiet] starts []",
    ),
    (
      "defs.bzl",
      Exact,
      "x = f(a, b)\n",
      "x = f(\n    a,\n    b)\n",
      "blocks [0..1/0..3] runs [0..1/0..3 formatting quiet] starts []",
    ),
    // lines indented again
    (
      "a.kt",
      Exact,
      "class A {\n  fun f() {\n    val a = 1\n    val b = 2\n  }\n}\n",
      "class A {\n  fun f() {\n      val a = 1\n      val b = 2\n  }\n}\n",
      "blocks [2..4/2..4] runs [2..4/2..4 formatting quiet] starts []",
    ),
    // the two `>` that close two lists of type arguments, a space between two tokens of a line
    (
      "A.java",
      Exact,
      "class A {\n  List<List<A>> a;\n}\n",
      "class A {\n  List<List<A> > a;\n}\n",
      "blocks [1..2/1..2] runs [1..2/1..2 formatting quiet] starts []",
    ),
    // the lines are those of the view: `\r\n` and a lone `\r` end a line, as for the blocks and the tokens
    (
      "a.kt",
      Exact,
      "val a = 1\rfun f() {\r\n  call(a, b)\r\n}\r\n",
      "val a = 1\rfun f() {\r\n  call(a,\r\n    b)\r\n}\r\n",
      "blocks [2..3/2..4] runs [2..3/2..4 formatting quiet] starts []",
    ),
    // a comment indented with its code is the same comment, and blank lines between the lines are with them
    (
      "a.kt",
      Exact,
      "fun f() {\n  // a note\n  a(1)\n  b(2)\n}\n",
      "fun f() {\n    // a note\n    a(1)\n\n    b(2)\n}\n",
      "blocks [1..4/1..5] runs [1..4/1..5 formatting quiet] starts []",
    ),
  ]);
}

/// A block that is equal under the policy (pale) is asked as any other: no shortcut makes it quiet, and none keeps
/// it a stop
#[test]
fn a_pale_block_passes_the_same_test_of_tokens() {
  check_cases(&[
    (
      "BUILD",
      Ignore,
      "x = f(a, b)\n",
      "x = f(\n    a,\n    b)\n",
      "blocks [0..1/0..3 pale] runs [0..1/0..3 formatting quiet] starts []",
    ),
    // equal without its whitespace, and another program: the line break ends the statement
    (
      "a.kt",
      Ignore,
      "fun f(): Int {\n  return\n  foo()\n}\n",
      "fun f(): Int {\n  return foo()\n}\n",
      "blocks [1..3/1..2 pale] runs [] starts [0, 1, 1]",
    ),
    // a blank line a raw string gains is content
    (
      "a.kt",
      Ignore,
      "val s = \"\"\"\n  a\n  b\n\"\"\"\n",
      "val s = \"\"\"\n  a\n\n  b\n\"\"\"\n",
      "blocks [2..2/2..3 pale] runs [] starts [0, 1, 2]",
    ),
  ]);
}

/// The negative cases of the tokens, each as a block of a diff: no quiet run, and the block stays a stop
#[test]
fn what_changes_the_code_is_never_formatting() {
  check_cases(&[
    // a space inside a string
    (
      "a.kt",
      Exact,
      "fun f() {\n  val s = \"a b\"\n}\n",
      "fun f() {\n  val s = \"a  b\"\n}\n",
      "blocks [1..2/1..2] runs [] starts [0, 1, 1]",
    ),
    // operators that join
    (
      "a.kt",
      Exact,
      "fun f() {\n  val s = a - -b\n}\n",
      "fun f() {\n  val s = a--b\n}\n",
      "blocks [1..2/1..2] runs [] starts [0, 1, 1]",
    ),
    (
      "A.java",
      Exact,
      "class A {\n  int a = b >> c;\n}\n",
      "class A {\n  int a = b > > c;\n}\n",
      "blocks [1..2/1..2] runs [] starts [0, 1, 1]",
    ),
    // a line break that ends a statement
    (
      "a.kt",
      Exact,
      "fun f(): Int {\n  return\n  foo()\n}\n",
      "fun f(): Int {\n  return foo()\n}\n",
      "blocks [1..3/1..2] runs [] starts [0, 1, 1]",
    ),
    (
      "a.js",
      Exact,
      "function f() {\n  return\n  foo()\n}\n",
      "function f() {\n  return foo()\n}\n",
      "blocks [1..3/1..2] runs [] starts [0, 1, 1]",
    ),
    // operands in another order: the same characters, other tokens
    (
      "a.kt",
      Exact,
      "fun f() {\n  val s = a + b\n}\n",
      "fun f() {\n  val s = b + a\n}\n",
      "blocks [1..2/1..2] runs [] starts [0, 1, 1]",
    ),
    // a line of a raw string that gains spaces at its end
    (
      "a.kt",
      Exact,
      "val s = \"\"\"\n  a\n  b\n\"\"\"\n",
      "val s = \"\"\"\n  a  \n  b\n\"\"\"\n",
      "blocks [1..2/1..2] runs [] starts [0, 1, 1]",
    ),
    // an indent of Starlark is its syntax
    (
      "defs.bzl",
      Exact,
      "def f():\n  x = 1\n",
      "def f():\n    x = 1\n",
      "blocks [1..2/1..2] runs [] starts [0, 1, 1]",
    ),
    // a line of Starlark joined with the one its `\\` continued
    ("defs.bzl", Exact, "x = 1 + \\\n    2\n", "x = 1 + 2\n", "blocks [0..2/0..1] runs [] starts [0, 1, 0]"),
    // a label of Kotlin with a space before its `@`, a lambda on the line after a name
    (
      "a.kt",
      Exact,
      "fun f() = run {\n  return@run 1\n}\n",
      "fun f() = run {\n  return @run 1\n}\n",
      "blocks [1..2/1..2] runs [] starts [0, 1, 1]",
    ),
    (
      "a.kt",
      Exact,
      "fun f() {\n  val a = foo\n  { x }\n}\n",
      "fun f() {\n  val a = foo { x }\n}\n",
      "blocks [1..3/1..2] runs [] starts [0, 1, 1]",
    ),
    // a line break before the `=>` of an arrow function, a zero width space between two tokens of a script
    (
      "a.js",
      Exact,
      "const f = (a)\n  => a\n",
      "const f = (a) => a\n",
      "blocks [0..2/0..1] runs [] starts [0, 1, 0]",
    ),
    (
      "a.ts",
      Exact,
      "const a = 1 +\u{200b} 2\n",
      "const a = 1 + 2\n",
      "blocks [0..1/0..1] runs [] starts [0, 1, 0]",
    ),
    // two spaces in the text of a JSX element
    (
      "a.tsx",
      Exact,
      "const x = <p>a  b</p>\n",
      "const x = <p>a b</p>\n",
      "blocks [0..1/0..1] runs [] starts [0, 1, 0]",
    ),
    // a blank line a text block of Java gains, the text of a branch of `#if` that C# does not take
    (
      "A.java",
      Exact,
      "class A {\n  String s = \"\"\"\n    a\n    b\n    \"\"\";\n}\n",
      "class A {\n  String s = \"\"\"\n    a\n\n    b\n    \"\"\";\n}\n",
      "blocks [3..3/3..4] runs [] starts [0, 1, 3]",
    ),
    (
      "A.cs",
      Exact,
      "class A {\n#if X\n  int a  = 1;\n#endif\n}\n",
      "class A {\n#if X\n  int a = 1;\n#endif\n}\n",
      "blocks [2..3/2..3] runs [] starts [0, 1, 2]",
    ),
    // a mark of the input of a Rust macro before a space, and before another mark
    (
      "a.rs",
      Exact,
      "fn f() {\n  m!(a - -b);\n}\n",
      "fn f() {\n  m!(a --b);\n}\n",
      "blocks [1..2/1..2] runs [] starts [0, 1, 1]",
    ),
    // a comment that reads otherwise is a label, never quiet
    (
      "a.rs",
      Exact,
      "//go:build a\nfn a() {}\n",
      "// go:build a\nfn a() {}\n",
      "blocks [0..1/0..1] runs [0..1/0..1 comments] starts [0, 1, 0]",
    ),
    // another name, which the file does not declare: no rename the file can prove (`marks_renames_test.rs`)
    (
      "a.kt",
      Exact,
      "fun f() {\n  total(1)\n}\n",
      "fun f() {\n    sum(1)\n}\n",
      "blocks [1..2/1..2] runs [] starts [0, 1, 1]",
    ),
  ]);
}

#[test]
fn lines_that_changed_in_their_comments_alone_are_a_run_that_is_a_label() {
  check_cases(&[
    (
      "a.kt",
      Exact,
      "fun f() {\n  // one\n  a()\n}\n",
      "fun f() {\n  // two\n  a()\n}\n",
      "blocks [1..2/1..2] runs [1..2/1..2 comments] starts [0, 1, 1]",
    ),
    // a line of a doc comment, and a line of a block comment that holds nothing but its words
    (
      "a.kt",
      Exact,
      "/**\n * one\n * two\n */\nfun f() {}\n",
      "/**\n * one\n * three\n */\nfun f() {}\n",
      "blocks [2..3/2..3] runs [2..3/2..3 comments] starts [0, 1, 2]",
    ),
    (
      "a.kt",
      Exact,
      "/*\n one\n two\n */\nfun f() {}\n",
      "/*\n one\n three\n */\nfun f() {}\n",
      "blocks [2..3/2..3] runs [2..3/2..3 comments] starts [0, 1, 2]",
    ),
    // the comment after the same code, the code wrapped
    (
      "a.kt",
      Exact,
      "fun f() {\n  a(1, 2) // one\n}\n",
      "fun f() {\n  a(1,\n    2) // two\n}\n",
      "blocks [1..2/1..3] runs [1..2/1..3 comments] starts [0, 1, 1]",
    ),
    // a comment that left its line
    (
      "a.kt",
      Exact,
      "fun f() {\n  a(1) // one\n  b(2)\n}\n",
      "fun f() {\n  a(1)\n  b(2)\n}\n",
      "blocks [1..2/1..2] runs [1..2/1..2 comments] starts [0, 1, 1]",
    ),
    (
      "BUILD",
      Exact,
      "# one\nx = 1\n",
      "# two\nx = 1\n",
      "blocks [0..1/0..1] runs [0..1/0..1 comments] starts [0, 1, 0]",
    ),
    (
      "A.cs",
      Exact,
      "class A {\n  /// <summary>One.</summary>\n  int x;\n}\n",
      "class A {\n  /// <summary>Two.</summary>\n  int x;\n}\n",
      "blocks [1..2/1..2] runs [1..2/1..2 comments] starts [0, 1, 1]",
    ),
    (
      "a.tsx",
      Exact,
      "const x = <A>\n  {/* one */}\n</A>\n",
      "const x = <A>\n  {/* two */}\n</A>\n",
      "blocks [1..2/1..2] runs [1..2/1..2 comments] starts [0, 1, 1]",
    ),
    // a comment of Razor is a comment to the parser of C#, on one line and a line inside it
    (
      "A.cs",
      Exact,
      "class A {\n  @* one *@\n  int x;\n}\n",
      "class A {\n  @* two *@\n  int x;\n}\n",
      "blocks [1..2/1..2] runs [1..2/1..2 comments] starts [0, 1, 1]",
    ),
    (
      "A.cs",
      Exact,
      "class A {\n  @* one\n  two\n  three *@\n  int x;\n}\n",
      "class A {\n  @* one\n  four\n  three *@\n  int x;\n}\n",
      "blocks [2..3/2..3] runs [2..3/2..3 comments] starts [0, 1, 2]",
    ),
    // a line comment a lexer reads on over a lone `\r`, where the view ends the line: the line after the `\r` is the
    // comment's, though nothing in it starts one
    (
      "a.rs",
      Exact,
      "fn f() {\n  // a\rfoo();\n  b();\n}\n",
      "fn f() {\n  // a\rbar();\n  b();\n}\n",
      "blocks [2..3/2..3] runs [2..3/2..3 comments] starts [0, 1, 2]",
    ),
  ]);
}

/// A block of several changes is cut where both sides have a line boundary between the same tokens: what is
/// formatting between the cuts is quiet, the reading starts at the first line that is not
#[test]
fn a_block_is_cut_into_runs_at_the_line_boundaries_both_sides_share() {
  check_cases(&[
    // lines wrapped into an `if`: the `if` is read, its body not
    (
      "a.kt",
      Exact,
      "fun f() {\n  call(a, b)\n  other()\n}\n",
      "fun f() {\n  if (x) {\n    call(a, b)\n    other()\n  }\n}\n",
      "blocks [1..3/1..5] runs [1..3/2..4 formatting quiet] starts [0, 1, 1]",
    ),
    // a real change after lines indented again, a blank line between them with the run
    (
      "a.kt",
      Exact,
      "fun f() {\n  a(1)\n  b(2)\n  c(3)\n}\n",
      "fun f() {\n    a(1)\n\n    b(2)\n    c(4)\n}\n",
      "blocks [1..4/1..5] runs [1..3/1..4 formatting quiet] starts [0, 1, 4]",
    ),
    // a real change before them: the reading starts at it
    (
      "a.kt",
      Exact,
      "fun f() {\n  a(1)\n  b(2)\n  c(3)\n}\n",
      "fun f() {\n    a(0)\n    b(2)\n    c(3)\n}\n",
      "blocks [1..4/1..4] runs [2..4/2..4 formatting quiet] starts [0, 1, 1]",
    ),
    // a comment added between them is read, the lines around it are two runs
    (
      "a.kt",
      Exact,
      "fun f() {\n  a(1)\n  b(2)\n}\n",
      "fun f() {\n    a(1)\n    // note\n    b(2)\n}\n",
      "blocks [1..3/1..4] runs [1..2/1..2 formatting quiet, 2..3/3..4 formatting quiet] starts [0, 1, 2]",
    ),
    // a line removed between them: the reading starts at the old line
    (
      "a.kt",
      Exact,
      "fun f() {\n  a(1)\n  gone()\n  b(2)\n}\n",
      "fun f() {\n    a(1)\n    b(2)\n}\n",
      "blocks [1..4/1..3] runs [1..2/1..2 formatting quiet, 3..4/2..3 formatting quiet] starts [0, 0, 2]",
    ),
    // a comment worded again among them is a label of its own
    (
      "a.kt",
      Exact,
      "fun f() {\n  a(1)\n  // one\n  b(2)\n}\n",
      "fun f() {\n    a(1)\n    // two\n    b(2)\n}\n",
      "blocks [1..4/1..4] runs [1..2/1..2 formatting quiet, 2..3/2..3 comments, 3..4/3..4 formatting quiet] starts [0, 1, 2]",
    ),
    // a comment that moved to a line of its own is the same comment after the same code: one run, though the two
    // lines by themselves differ in their comments
    (
      "a.kt",
      Exact,
      "fun f() {\n  a(1)\n  b(2) // x\n  c(3)\n}\n",
      "fun f() {\n    a(1)\n    b(2)\n    // x\n    c(4)\n}\n",
      "blocks [1..4/1..5] runs [1..3/1..4 formatting quiet] starts [0, 1, 4]",
    ),
    // a comment added on a line of its own between two comments worded again: one label
    (
      "a.kt",
      Exact,
      "fun f() {\n  a(1) // one\n  b(2) // two\n}\n",
      "fun f() {\n  a(1) // three\n  // four\n  b(2) // five\n}\n",
      "blocks [1..3/1..4] runs [1..3/1..4 comments] starts [0, 1, 1]",
    ),
    // a comment beside a real change of the same block
    (
      "a.kt",
      Exact,
      "fun f() {\n  a(1) // one\n  b(2)\n}\n",
      "fun f() {\n    a(1) // two\n    b(3)\n}\n",
      "blocks [1..3/1..3] runs [1..2/1..2 comments] starts [0, 1, 1]",
    ),
    // a call wrapped beside a changed call: the line boundaries are those of both sides
    (
      "a.kt",
      Exact,
      "fun f() {\n  a(1, 2)\n  b(2)\n}\n",
      "fun f() {\n  a(1,\n    2)\n  b(3)\n}\n",
      "blocks [1..3/1..4] runs [1..2/1..3 formatting quiet] starts [0, 1, 3]",
    ),
    // no boundary both sides share between the wrapped call and the change: no run
    (
      "a.kt",
      Exact,
      "fun f() {\n  a(1, 2); b(2)\n}\n",
      "fun f() {\n  a(1,\n    2); b(3)\n}\n",
      "blocks [1..2/1..3] runs [] starts [0, 1, 1]",
    ),
  ]);
}

#[test]
fn a_block_with_a_side_of_no_lines_or_of_blank_lines_alone_is_never_marked() {
  check_cases(&[
    // added and deleted lines, a comment among them
    (
      "a.kt",
      Exact,
      "fun f() {\n  a()\n}\n",
      "fun f() {\n  a()\n  b()\n}\n",
      "blocks [2..2/2..3] runs [] starts [0, 1, 2]",
    ),
    (
      "a.kt",
      Exact,
      "fun f() {\n  a()\n  b()\n}\n",
      "fun f() {\n  a()\n}\n",
      "blocks [2..3/2..2] runs [] starts [0, 0, 2]",
    ),
    (
      "a.kt",
      Exact,
      "fun f() {\n  a()\n}\n",
      "fun f() {\n  // note\n  a()\n}\n",
      "blocks [1..1/1..2] runs [] starts [0, 1, 1]",
    ),
    // a blank line added, under each policy that keeps it a block: a stop
    (
      "a.kt",
      Exact,
      "fun f() {\n  a()\n  b()\n}\n",
      "fun f() {\n  a()\n\n  b()\n}\n",
      "blocks [2..2/2..3] runs [] starts [0, 1, 2]",
    ),
    (
      "a.kt",
      Ignore,
      "fun f() {\n  a()\n  b()\n}\n",
      "fun f() {\n  a()\n\n  b()\n}\n",
      "blocks [2..2/2..3 pale] runs [] starts [0, 1, 2]",
    ),
    // blank lines on both sides: no token proves anything
    (
      "a.kt",
      Exact,
      "fun f() {\n  a()\n\n\n  b()\n}\n",
      "fun f() {\n  a()\n  \n  b()\n}\n",
      "blocks [2..4/2..3] runs [] starts [0, 1, 2]",
    ),
    // a block the policy cut off is not asked and is no stop
    (
      "a.kt",
      IgnoreBlankLines,
      "fun f() {\n  a()\n  b()\n}\n",
      "fun f() {\n  a()\n\n  b()\n}\n",
      "blocks [2..2/2..3 ignored] runs [] starts []",
    ),
  ]);
}

/// Without a grammar nothing proves a change of formatting: the block of whitespace stays the pale stop it is
#[test]
fn a_file_without_a_grammar_and_a_pair_of_two_grammars_have_no_runs() {
  check_cases(&[
    ("notes.txt", Exact, "a b\nc\n", "a  b\nc\n", "blocks [0..1/0..1] runs [] starts [0, 1, 0]"),
    (
      "a.py",
      Ignore,
      "def f():\n  call(a, b)\n",
      "def f():\n  call(a,\n    b)\n",
      "blocks [1..2/1..3 pale] runs [] starts [0, 1, 1]",
    ),
    (
      "a.go",
      Exact,
      "func f() {\n  call(a, b)\n}\n",
      "func f() {\n  call(a,\n    b)\n}\n",
      "blocks [1..2/1..3] runs [] starts [0, 1, 1]",
    ),
  ]);

  assert_eq!(pair_grammar("a.kt", "a.kt"), Some(Grammar::Kotlin));
  assert_eq!(pair_grammar("a.kt", "b.kt"), Some(Grammar::Kotlin));
  // a rename to a file another grammar reads, or to and from a file of none
  for (old, new) in [
    ("a.kt", "a.kts"),
    ("a.ts", "a.tsx"),
    ("a.js", "a.ts"),
    ("a.txt", "a.kt"),
    ("a.kt", "a.txt"),
    ("defs.bzl", "BUILD"),
  ] {
    assert_eq!(pair_grammar(old, new), None, "{old} -> {new}");
  }

  // the same texts, marked as one grammar and unmarked as none
  let (old, new) = ("fun f() {\n  call(a, b)\n}\n", "fun f() {\n  call(a,\n    b)\n}\n");
  let diff = diff_texts(old, new, Exact, &DiffBudget::NONE);
  let read = |grammar: Option<Grammar>| {
    marks_of(
      &diff,
      old,
      new,
      grammar,
      Exact,
      false,
      |new_side| tokens(Grammar::Kotlin, if new_side { new } else { old }).map(Arc::new),
      &CodeSymbolBindings,
    )
    .marks
  };
  assert_eq!(read(Some(Grammar::Kotlin)).runs.len(), 1);
  assert_eq!((read(None).runs.len(), read(None).starts), (0, vec![0, 1, 1]));
}

/// A text without tokens (over the size of the tokens, nested too deeply) and a pair with a side over the size the
/// inner ranges are compared for have no runs; every block of them is a stop
#[test]
fn a_text_without_tokens_and_a_side_over_the_size_of_the_inner_ranges_have_no_runs() {
  let (old, new) = ("fun f() {\n  call(a, b)\n}\n", "fun f() {\n  call(a,\n    b)\n}\n");
  let diff = diff_texts(old, new, Exact, &DiffBudget::NONE);
  let kotlin = |text: &str| tokens(Grammar::Kotlin, text).map(Arc::new);
  for missing in [false, true] {
    let marks = marks_of(
      &diff,
      old,
      new,
      Some(Grammar::Kotlin),
      Exact,
      false,
      |new_side| {
        if new_side == missing { None } else { kotlin(if new_side { new } else { old }) }
      },
      &CodeSymbolBindings,
    )
    .marks;
    assert_eq!(
      (marks.runs.len(), marks.starts),
      (0, vec![0, 1, 1]),
      "no tokens of the {} side",
      if missing { "new" } else { "old" }
    );
  }
  // tokens of another text than the diff's prove nothing
  let marks = marks_of(
    &diff,
    old,
    new,
    Some(Grammar::Kotlin),
    Exact,
    false,
    |new_side| kotlin(if new_side { "fun f() {\n  call(a, b)\n}\n\n\n" } else { old }),
    &CodeSymbolBindings,
  )
  .marks;
  assert_eq!(marks.runs.len(), 0);

  // the same change in texts of more lines than the inner ranges are compared for, and of ten lines less: the first
  // has no run and reads no tokens, by its size and not by the time its comparison took
  let read = |blank_lines: usize| {
    let (old, new) = (format!("{old}{}", "\n".repeat(blank_lines)), format!("{new}{}", "\n".repeat(blank_lines)));
    let diff = diff_texts(&old, &new, Exact, &DiffBudget::NONE);
    let asked = Cell::new(0);
    let marks = marks_of(
      &diff,
      &old,
      &new,
      Some(Grammar::Kotlin),
      Exact,
      false,
      |new_side| {
        asked.set(asked.get() + 1);
        kotlin(if new_side { &new } else { &old })
      },
      &CodeSymbolBindings,
    )
    .marks;
    (diff.line_level_only, marks.runs.len(), marks.starts, asked.get())
  };
  assert_eq!(read(MAX_INNER_LINES), (true, 0, vec![0, 1, 1], 0));
  assert_eq!(read(MAX_INNER_LINES - 10), (false, 1, vec![], 2));
}

/// The time changes no mark: a diff whose inner ranges ran out of their time, or were not asked for, is by lines
/// alone and has the marks of the diff in time, since the runs read the blocks and the tokens alone. A diff whose
/// lines ran out of time too is one block of both texts, and its marks are those of that block
#[test]
fn a_diff_the_time_cut_short_has_the_marks_of_its_blocks() {
  let (old, new) = ("fun f() {\n  a(1,\n    2)\n  b()\n}\n", "fun f() {\n  a(1, 2)\n  b()\n}\n");
  let read = |budget: &DiffBudget| {
    let mut diff = MarkedDiff::from(diff_texts(old, new, Exact, budget));
    diff.marks = marks_of(
      &diff,
      old,
      new,
      Some(Grammar::Kotlin),
      Exact,
      false,
      |new_side| tokens(Grammar::Kotlin, if new_side { new } else { old }).map(Arc::new),
      &CodeSymbolBindings,
    )
    .marks;
    (diff.line_level_only, show(&diff))
  };
  let in_time = "blocks [1..3/1..2] runs [1..3/1..2 formatting quiet] starts []";
  assert_eq!(read(&DiffBudget::NONE), (false, in_time.to_string()));
  let spent = Deadline::after(Duration::ZERO);
  for inner in [Some(spent), None] {
    let lines = Deadline::NONE;
    assert_eq!(read(&DiffBudget { inner, lines }), (true, in_time.to_string()), "the inner ranges: {inner:?}");
  }
  assert_eq!(
    read(&DiffBudget { inner: None, lines: spent }),
    (true, "blocks [0..6/0..5] runs [0..6/0..5 formatting quiet] starts []".to_string())
  );
}

/// A run that reaches an end of its block is one only when the code around the block reads the same there; inside
/// the block its neighbours are lines the reader stops at
#[test]
fn a_run_at_an_end_of_its_block_needs_the_code_around_the_block_to_read_the_same() {
  let kotlin = |text: &str| tokens(Grammar::Kotlin, text).unwrap();
  let runs = |old: &str, old_lines: Range<u32>, new: &str, new_lines: Range<u32>| {
    let lines = BlockLines { old: old_lines, new: new_lines };
    let mut work = MAX_FILE_TOKEN_WORK;
    runs_of_block(&kotlin(old), &kotlin(new), &lines, &ANY_TEXT, &mut work, &mut |_, _| {})
      .unwrap()
      .iter()
      .map(|run| format!("{}..{}/{}..{}", run.old_start, run.old_end, run.new_start, run.new_end))
      .collect::<Vec<_>>()
  };
  let old = "fun f() {\n  a()\n  b(1,\n    2)\n  c()\n}\n";
  // the same statement before and after the wrapped call
  assert_eq!(runs(old, 2..4, "fun f() {\n  a()\n  b(1, 2)\n  c()\n}\n", 2..3), ["2..4/2..3"]);
  // the lines of the call as a block of their own, while the code before them, or after them, is another
  assert_eq!(runs(old, 2..4, "fun f() {\n  b(1, 2)\n  c()\n}\n", 1..2), Vec::<String>::new());
  assert_eq!(runs(old, 2..4, "fun f() {\n  a()\n  b(1, 2)\n}\n", 2..3), Vec::<String>::new());
  // with the lines that differ in the block, the call is a run inside it
  assert_eq!(runs(old, 1..5, "fun f() {\n  b(1, 2)\n}\n", 1..2), ["2..4/1..2"]);
}

#[test]
fn a_file_whose_blocks_cannot_hold_a_run_is_not_parsed() {
  let asked = |path: &str, old: &str, new: &str| read_counted(path, old, new, Exact).1;
  // changed values, an operator, a line without a word added: no line of one side starts and ends as a line of the
  // other, and both sides hold the same words
  assert_eq!(
    asked("a.kt", "fun f() {\n  val a = 1\n  b(2 + a)\n}\n", "fun f() {\n  val a = 2\n  b(3 - a)\n  }\n}\n"),
    [0, 0]
  );
  // a changed name: it may be a rename. Asked, and it is none
  let (diff, asked_sides) = read_counted("a.kt", "fun f() {\n  val a = 1\n  b(2)\n}\n", "fun f() {\n  val a = 2\n  c(2)\n}\n", Exact);
  assert_eq!((diff.marks.runs.len(), diff.marks.renames.len(), asked_sides), (0, 0, [1, 1]));
  // lines added and deleted alone
  assert_eq!(asked("a.kt", "fun f() {\n  a()\n}\n", "fun f() {\n  a()\n  // note\n  b()\n}\n"), [0, 0]);
  // a file of no grammar
  assert_eq!(asked("a.txt", "a b\n", "a  b\n"), [0, 0]);
  // a wrapped call, a comment: each side once, whatever the number of blocks
  assert_eq!(
    asked(
      "a.kt",
      "fun f() {\n  a(1, 2)\n  b()\n  c(3, 4)\n}\n",
      "fun f() {\n  a(1,\n    2)\n  b()\n  c(3,\n    4)\n}\n"
    ),
    [1, 1]
  );
  assert_eq!(asked("a.kt", "fun f() {\n  a() // one\n}\n", "fun f() {\n  a() // two\n}\n"), [1, 1]);
  // a line starts as a line of the other side and a line ends as one: it may be formatting. Asked, and it is not
  let (diff, asked) = read_counted("a.kt", "fun f() {\n  a(1,\n    2)\n}\n", "fun f() {\n  a(1, 3,\n    4, 2)\n}\n", Exact);
  assert_eq!((diff.marks.runs.len(), asked), (0, [1, 1]));
}

/// The files of the grammars: the test data of `code-symbols` and sources of the marks
fn corpus() -> Vec<(String, String)> {
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
  let mut files = Vec::new();
  let mut directories = vec![root.join("../code-symbols/testdata")];
  while let Some(directory) = directories.pop() {
    for entry in std::fs::read_dir(&directory).unwrap() {
      let path = entry.unwrap().path();
      if path.is_dir() {
        directories.push(path);
      } else if grammar_of(path.to_str().unwrap()).is_some() {
        files.push(path);
      }
    }
  }
  files.extend(["../diff-marks/src/marks.rs", "../diff-marks/src/marks_runs.rs", "src/syntax.rs"].map(|path| root.join(path)));
  files.sort();
  files
    .into_iter()
    .map(|path| (path.to_str().unwrap().to_string(), std::fs::read_to_string(&path).unwrap()))
    .collect()
}

/// Comments in the forms a line of them may lack what starts a comment: over several lines, nested, beside a `/*/`
const COMMENTS: [(&str, &str); 19] = [
  ("a.kt", "/* a\n b /* c\n d */ e\n f */\nval x = 1 /*/ g\n h */\n"),
  ("a.kt", "/*/*/ a\n b */ c\n d */\nval x = 1\n"),
  ("a.kt", "val x = 2 */* a\n b */ 3\n"),
  ("a.kt", "val s = \"*/\" /* a\n b\n c */\n/**\n d\n */\nfun f() {}\n"),
  ("a.kts", "#!/usr/bin/env kotlin\n/* a\n b */\nprintln(1)\n"),
  ("a.rs", "#!/usr/bin/env run\n/* a /* b\n c */ d\n e */\n//! f\nfn main() {} /*/ g\n h */\n"),
  ("A.java", "/* a\n b */\nclass A {\n  /** c\n   d\n   */\n  int x; // e\n}\n"),
  (
    "A.cs",
    "/* a\n b */\nclass A {\n  /// <summary>\n  /// c\n  /// </summary>\n  int x;\n  /** d\n   e */\n  int y;\n}\n",
  ),
  ("a.ts", "/* a\n b */\nconst x = 1 /*/ c\n d */\n// e\n"),
  ("a.js", "#!/usr/bin/env node\n<!-- a\nx = 1 /* b\n c */\n--> d\n"),
  ("defs.bzl", "# a\ndef f():\n    # b\n    return 1  # c\n"),
  // the comments of Razor, which the parser of C# reads as comments: over several lines, beside a `@*@`
  ("A.cs", "class A {\n  @* a\n  b *@\n  int x; @* c *@\n  @*@ d\n  e *@\n  int y; /* f @* g\n  h */\n}\n"),
  // a lone `\r` ends a line of the view and, to some lexers, not a line comment
  ("a.rs", "fn f() {\n  // a\rb\rc\n  d(); // e\rf\n}\n"),
  ("a.kt", "// a\rb\nval x = 1 // c\rd\re\n/* f */\rval y = 2\n"),
  ("a.kts", "#!/usr/bin/env kotlin\rprintln(1)\nprintln(2) // a\rb\n"),
  ("A.java", "// a\rb\nclass A {} // c\rd\n"),
  ("A.cs", "// a\rb\nclass A {} // c\rd\n"),
  ("a.ts", "// a\rb\nconst x = 1 // c\rd\n"),
  ("defs.bzl", "# a\rb = 1\nc = 2 # d\re\n"),
];

/// The half of the question before the tokens that asks for a comment: yes for every line a comment is in, in each
/// grammar, so no block with a comment is passed without its tokens
#[test]
fn every_line_of_a_comment_may_hold_a_comment_before_the_tokens_are_read() {
  let mut files = corpus();
  files.extend(COMMENTS.map(|(path, text)| (path.to_string(), text.to_string())));
  let mut comments = 0;
  for (path, text) in &files {
    let grammar = grammar_of(path).unwrap();
    let Some(tokens) = tokens(grammar, text) else { continue };
    let side = SideText::new(grammar, text);
    for token in tokens.all().iter().filter(|token| token.is_comment()) {
      comments += 1;
      for line in token.line..=token.end_line {
        assert!(may_hold_comment(&side, line..line + 1), "{path}: line {line} of the comment {:?}", token.text_in(text));
      }
    }
  }
  assert!(comments > 100, "{comments} comments");
  // a line after a comment that closed, and code alone, hold none
  let answers = |grammar: Grammar, text: &str| -> Vec<bool> {
    let side = SideText::new(grammar, text);
    (0..u32::try_from(text.lines().count()).unwrap())
      .map(|line| may_hold_comment(&side, line..line + 1))
      .collect()
  };
  let text = "/* a */\nval x = 1\n/* b\n c */\nval y = \"*\" + \"/\"\n";
  assert_eq!(answers(Grammar::Kotlin, text), [true, false, true, true, false]);
  // a `/*` inside a string (a glob) and a comment after it: where comments do not nest, the comment that closed
  // leaves the lines after it without one; where they nest, the first `/*` may still be open
  let glob = "const a = \"src/**/*.ts\"\nconst b = 1\n/* c */\nconst d = 2\n";
  assert_eq!(answers(Grammar::TypeScript, glob), [true, true, true, false]);
  assert_eq!(answers(Grammar::Rust, glob), [true, true, true, true]);
  // a comment of Razor that closed, in C# alone
  let razor = "@* a\n b *@\nclass A {}\n";
  assert_eq!(answers(Grammar::CSharp, razor), [true, true, false]);
  assert_eq!(answers(Grammar::Java, razor), [false, false, false]);
  // after a lone `\r`, only the lines of a `\n` line that holds what starts a comment before them
  let side = SideText::new(Grammar::Kotlin, "val a = 1\rval b = 2\n// c\rd\re\nval f = 3\rval g = 4\n");
  assert_eq!(
    (0..7).map(|line| may_hold_comment(&side, line..line + 1)).collect::<Vec<_>>(),
    [false, false, true, true, true, false, false]
  );
}

/// A seeded generator: the same edits in every run
struct Random(u64);

impl Random {
  fn below(&mut self, limit: usize) -> usize {
    self.0 ^= self.0 << 13;
    self.0 ^= self.0 >> 7;
    self.0 ^= self.0 << 17;
    usize::try_from(self.0 % limit as u64).unwrap()
  }
}

/// Whether a line holds what starts a line comment of a grammar
fn holds_line_comment(line: &str) -> bool {
  line.contains("//") || line.contains('#')
}

/// `text` after a few edits of its lines: indents, wrapped and joined lines, comments, blank lines, changed names,
/// lines wrapped into a block, a changed name on the line after a comment
fn edited(text: &str, random: &mut Random) -> String {
  let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
  for _ in 0..=random.below(6) {
    if lines.is_empty() {
      break;
    }
    let at = random.below(lines.len());
    match random.below(13) {
      0 => lines[at].insert_str(0, "  "),
      1 => lines[at] = lines[at].trim_start().to_string(),
      2 => {
        let spaces: Vec<usize> = lines[at].match_indices(' ').map(|(index, _)| index).filter(|index| *index > 0).collect();
        if !spaces.is_empty() {
          let space = spaces[random.below(spaces.len())];
          lines[at].replace_range(space..=space, "\n      ");
        }
      }
      3 if at + 1 < lines.len() => {
        let next = lines.remove(at + 1);
        lines[at] = format!("{} {}", lines[at].trim_end(), next.trim_start());
      }
      4 => lines[at].push_str(" // note"),
      5 => {
        if let Some(comment) = lines[at].find("//") {
          lines[at].truncate(comment + 2);
          lines[at].push_str(" worded again");
        }
      }
      6 => {
        if let Some(letter) = lines[at].find(|character: char| character.is_ascii_alphabetic()) {
          lines[at].replace_range(letter..=letter, "q");
        }
      }
      7 => drop(lines.remove(at)),
      8 => lines.insert(at, String::new()),
      9 => lines[at] = lines[at].replace(' ', "  "),
      10 => {
        let end = (at + 1 + random.below(6)).min(lines.len());
        for line in &mut lines[at..end] {
          line.insert_str(0, "    ");
        }
        lines.insert(end, "}".to_string());
        lines.insert(at, "if (x) {".to_string());
      }
      // the line after the next line with a line comment: after a lone `\r` it is in that comment for some lexers
      11 => {
        let after = (at..lines.len() - 1).find(|line| holds_line_comment(&lines[*line])).map(|line| line + 1);
        if let Some(letter) = after.and_then(|after| lines[after].find(|character: char| character.is_ascii_alphabetic()).map(|letter| (after, letter))) {
          lines[letter.0].replace_range(letter.1..=letter.1, "q");
        }
      }
      _ => lines[at] = format!("{} ", lines[at]),
    }
  }
  let mut text = lines.join("\n");
  text.push('\n');
  text
}

/// `text` with some of its lines at another place: up to twelve lines in a row, as they are, indented by two more
/// columns, or with a letter of one of them changed
fn moved(text: &str, random: &mut Random) -> String {
  let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
  if lines.len() < 8 {
    return text.to_string();
  }
  let count = 1 + random.below(12.min(lines.len() / 2));
  let from = random.below(lines.len() - count + 1);
  let mut taken: Vec<String> = lines.drain(from..from + count).collect();
  match random.below(4) {
    0 => taken.iter_mut().filter(|line| !line.trim().is_empty()).for_each(|line| line.insert_str(0, "  ")),
    1 => {
      let line = &mut taken[count / 2];
      if let Some(letter) = line.find(|character: char| character.is_ascii_alphabetic()) {
        line.replace_range(letter..=letter, "q");
      }
    }
    _ => {}
  }
  let to = random.below(lines.len() + 1);
  lines.splice(to..to, taken);
  let mut text = lines.join("\n");
  text.push('\n');
  text
}

/// `text` with one of its words written otherwise wherever it stands as a word: a name renamed in the whole file, a
/// local among them now and then. One time in four a place of it keeps the old word (a use left behind), one time
/// in four the new word is one the text has already (a name taken)
fn renamed(text: &str, random: &mut Random) -> String {
  let is_word = |character: char| character.is_alphanumeric() || character == '_';
  // the words of the text with what stands between them, in order
  let mut pieces: Vec<&str> = Vec::new();
  let mut rest = text;
  while !rest.is_empty() {
    let word = rest.chars().next().is_some_and(is_word);
    let end = rest.find(|character: char| is_word(character) != word).unwrap_or(rest.len());
    pieces.push(&rest[..end]);
    rest = &rest[end..];
  }
  let words: Vec<usize> = (0..pieces.len())
    .filter(|index| pieces[*index].chars().next().is_some_and(|first| is_word(first) && !first.is_numeric()))
    .collect();
  if words.is_empty() {
    return text.to_string();
  }
  let word = pieces[words[random.below(words.len())]];
  let taken = pieces[words[random.below(words.len())]];
  let new_word = if random.below(4) == 0 { taken.to_string() } else { format!("{word}Q") };
  let places: Vec<usize> = words.iter().copied().filter(|index| pieces[*index] == word).collect();
  let left = (random.below(4) == 0).then(|| places[random.below(places.len())]);
  let mut renamed = String::with_capacity(text.len() + places.len());
  for (index, piece) in pieces.iter().enumerate() {
    renamed.push_str(if *piece == word && Some(index) != left { &new_word } else { piece });
  }
  renamed
}

/// `text` with the line ends of `style`: 0 and 1 its own, 2 `\r\n` for each `\n`, 3 a lone `\r` after the lines
/// that hold a line comment or whose bytes add up to a multiple of three (by the line alone: a line both sides have
/// ends the same on both); `cut`: without its last line end
fn with_line_ends(text: &str, style: usize, cut: bool) -> String {
  let mut styled = String::with_capacity(text.len() + text.len() / 16);
  for line in text.split_inclusive('\n') {
    let content = line.strip_suffix('\n').unwrap_or(line);
    styled.push_str(content);
    if content.len() < line.len() {
      let sum: u32 = content.bytes().map(u32::from).sum();
      styled.push_str(match style {
        2 => "\r\n",
        3 if holds_line_comment(content) || sum.is_multiple_of(3) => "\r",
        _ => "\n",
      });
    }
  }
  if cut {
    let end = styled.trim_end_matches(['\r', '\n']).len();
    let last = styled[end..].chars().next_back().map_or(0, |_| if styled.ends_with("\r\n") { 2 } else { 1 });
    styled.truncate(styled.len() - last);
  }
  styled
}

/// What the navigation says of a rename of a diff, read apart from the code that found it: the places of each side
/// asked with the other name (the reading of `code_symbols::names_at_test::is_one_local`)
struct RenameFacts {
  /// The places of both sides are a value of its body alone, every name of which the scopes bind
  value: bool,
  /// For each place, whether it is the name of the binding and nothing else on both sides
  plain: Vec<bool>,
}

/// The facts of each rename of `diff`, a diff of the texts `texts` of a file at `path`. A rename holds when each
/// side says: every place is the same binding, its names are the places and no other, the places that declare it
/// are those the rename names, and for a name bound inside a body the other name changes no binding
fn rename_facts(path: &str, diff: &MarkedDiff, texts: [&str; 2]) -> Vec<RenameFacts> {
  let grammar = grammar_of(path).unwrap();
  diff
    .marks
    .renames
    .iter()
    .map(|rename| {
      let context = format!("{path}: {} of {}", show_rename(rename), show(diff));
      let count = rename.sites as usize;
      assert!(count > 0 && rename.old != rename.new && !rename.declared.is_empty(), "{context}");
      let mut facts = RenameFacts {
        value: rename.local,
        plain: vec![true; count],
      };
      for (side, (sites, other)) in [(&rename.old_sites, &rename.new), (&rename.new_sites, &rename.old)].into_iter().enumerate() {
        let sites = sites.as_chunks::<3>().0;
        assert_eq!(sites.len(), count, "{context}");
        let questions: Vec<NameQuestion<'_>> = sites
          .iter()
          .map(|site| NameQuestion {
            position: Position { line: site[0], column: site[1] },
            renamed_to: Some(other),
          })
          .collect();
        let found = names_at(grammar, texts[side], &questions);
        // a place inside a string (a name a format string captures) is a name of the binding and no place to ask at
        let entry = found
          .asked
          .iter()
          .flatten()
          .next()
          .copied()
          .unwrap_or_else(|| panic!("{context}: no name at a place of side {side}"));
        assert!(
          found.asked.iter().flatten().all(|asked| *asked == entry),
          "{context}: the places of side {side} are not one binding"
        );
        let name = &found.names[entry];
        for (asked, used) in found.asked.iter().zip(&name.uses) {
          assert!(asked.is_some() || used.site != Site::Plain, "{context}: no name at a plain place of side {side}");
        }
        assert_eq!(name.local.is_some(), rename.local, "{context}: side {side}");
        assert_eq!(
          name.clashes.as_ref().is_some_and(Vec::is_empty),
          rename.local,
          "{context}: side {side} clashes {:?}",
          name.clashes
        );
        let used: Vec<[u32; 3]> = name.uses.iter().map(|used| [used.start.line, used.start.column, used.end_column]).collect();
        assert_eq!(used, sites, "{context}: the names of side {side}");
        let declared: Vec<u32> = (0u32..)
          .zip(&name.uses)
          .filter(|(_, used)| used.access == Access::Declaration)
          .map(|(index, _)| index)
          .collect();
        assert_eq!(declared, rename.declared, "{context}: side {side}");
        facts.value &= name.local.is_some_and(LocalBinding::is_of_body_alone);
        for (plain, used) in facts.plain.iter_mut().zip(&name.uses) {
          *plain &= used.site == Site::Plain;
        }
      }
      facts
    })
    .collect()
}

/// The tokens of the two texts with the two names of every place of every rename of `marks` read as one: a text of
/// that place alone on both sides
fn renames_read_as_one(marks: &DiffMarks, old: &Tokens, new: &Tokens) -> [Tokens; 2] {
  let side = |tokens: &Tokens, new_side: bool| {
    let mut texts = Vec::new();
    for (number, rename) in (0u64..).zip(&marks.renames) {
      let sites = if new_side { &rename.new_sites } else { &rename.old_sites };
      for (place, site) in (0u64..).zip(sites.as_chunks::<3>().0) {
        // a place inside a string (a name a format string captures) is no token of its own, and in no run
        if let Some(token) = tokens.place_at(site[0], site[1]) {
          texts.push((token, 0x5eed_0000_0000_0000 | (number << 24) | place));
        }
      }
    }
    tokens.with_texts(texts)
  };
  [side(old, false), side(new, true)]
}

/// What holds of the marks of every diff as they are proven (`read_pair`): each run is in one block of two sides that is not cut off, in order and
/// without overlap, with lines on both sides; a run of formatting has the same tokens, a run of comments the same
/// code; a run of the kind `Renamed` has the same tokens once the two names of each place of a rename are read as
/// one, holds such a place, holds none that makes more of its name or declares a name that is not local, and is
/// quiet only when each of its places is a value of its body alone (`rename_facts`, the navigation asked again);
/// every run the first reading of the blocks finds, from the tokens of the texts as they are, lies in a run of its
/// kind: a rename takes no line that changed in its formatting or its comments alone; a
/// move has its old lines in one block that is a change and its new lines in another, none of them in a run or in
/// another move, and the block of its new lines has a start; a block has a start exactly when a line of it is to
/// read, a new line in no quiet run or an old line in no quiet run that no move took; and when no block of a diff
/// that ignores nothing (`exact`) has a start, the two texts are the same program but for the names of its renames
pub(crate) fn check_marks(path: &str, diff: &MarkedDiff, texts: [&str; 2], old: &Tokens, new: &Tokens, exact: bool) {
  let marks = &diff.marks;
  let facts = rename_facts(path, diff, texts);
  let [old_as_one, new_as_one] = renames_read_as_one(marks, old, new);
  let (mut old_end, mut new_end) = (0, 0);
  for run in &marks.runs {
    let context = format!("{path}: {run:?} of {}", show(diff));
    assert!(run.old_start < run.old_end && run.new_start < run.new_end, "{context}");
    assert!(run.old_start >= old_end && run.new_start >= new_end, "{context}");
    (old_end, new_end) = (run.old_end, run.new_end);
    let block = diff
      .blocks
      .iter()
      .find(|block| block.old_start <= run.old_start && run.old_end <= block.old_end && block.new_start <= run.new_start && run.new_end <= block.new_end);
    assert!(block.is_some_and(|block| block.kind != BlockKind::Ignored), "{context}");
    if run.kind == RunKind::Renamed {
      let answer = compare(old_as_one.in_lines(run.old_start..run.old_end), new_as_one.in_lines(run.new_start..run.new_end));
      assert_eq!(answer, Equality::Same, "{context}");
      // the places in the run: each is the rename and nothing else, on the line of the run on both sides
      let mut proven = true;
      let mut places = 0;
      for (rename, facts) in marks.renames.iter().zip(&facts) {
        for (place, (old_site, new_site)) in rename.old_sites.as_chunks::<3>().0.iter().zip(rename.new_sites.as_chunks::<3>().0).enumerate() {
          let inside = run.old_start <= old_site[0] && old_site[0] < run.old_end;
          assert_eq!(
            inside,
            run.new_start <= new_site[0] && new_site[0] < run.new_end,
            "{context}: place {place} of {}",
            show_rename(rename)
          );
          if inside {
            assert!(facts.plain[place], "{context}: place {place} of {} makes more of its name", show_rename(rename));
            assert!(
              rename.local || !rename.declared.contains(&u32::try_from(place).unwrap()),
              "{context}: a declaration of {}",
              show_rename(rename)
            );
            places += 1;
            proven &= facts.value;
          }
        }
      }
      assert!(places > 0, "{context}: no place of a rename");
      assert!(!run.quiet || (proven && has_proof(run.kind)), "{context}: quiet without a proof");
    } else {
      let answer = compare(old.in_lines(run.old_start..run.old_end), new.in_lines(run.new_start..run.new_end));
      let expected = if run.kind == RunKind::Formatting { Equality::Same } else { Equality::CodeOnly };
      assert_eq!((answer, run.quiet), (expected, has_proof(run.kind)), "{context}");
    }
  }
  // the first reading of the blocks, where the pair has runs at all
  if let Some(grammar) = pair_grammar(path, path).filter(|_| fits_inner(texts[0], diff.old_line_count) && fits_inner(texts[1], diff.new_line_count)) {
    let read = |new_side: bool| Some(Arc::new(if new_side { new.clone() } else { old.clone() }));
    let first = read_blocks(diff, &SideText::new(grammar, texts[0]), &SideText::new(grammar, texts[1]), false, &read);
    for run in first.runs.iter().flatten() {
      let held = marks
        .runs
        .iter()
        .any(|held| held.kind == run.kind && held.old_start <= run.old_start && run.old_end <= held.old_end && held.new_start <= run.new_start && run.new_end <= held.new_end);
      assert!(held, "{path}: {run:?} of the first reading is in no run of its kind of {}", show(diff));
    }
  }
  let in_run = |new_side: bool, line: u32, quiet_only: bool| {
    marks.runs.iter().filter(|run| run.quiet || !quiet_only).any(|run| {
      if new_side {
        run.new_start <= line && line < run.new_end
      } else {
        run.old_start <= line && line < run.old_end
      }
    })
  };
  let quiet = |new_side: bool, line: u32| in_run(new_side, line, true);
  let block_of = |new_side: bool, start: u32, end: u32| {
    diff.blocks.iter().position(|block| {
      let lines = if new_side { block.new_start..block.new_end } else { block.old_start..block.old_end };
      block.kind == BlockKind::Changed && lines.start <= start && end <= lines.end
    })
  };
  let (mut old_moved, mut new_moved) = (vec![false; diff.old_line_count as usize], vec![false; diff.new_line_count as usize]);
  let mut last_new = 0;
  for line_move in &marks.moves {
    let context = format!("{path}: {line_move:?} of {}", show(diff));
    // with the blank lines that moved beside it
    let (old_lines, new_lines) = (line_move.old_lines(), line_move.new_lines());
    assert!(line_move.len > 0 && new_lines.start >= last_new, "{context}");
    last_new = new_lines.end;
    let (old_block, new_block) = (block_of(false, old_lines.start, old_lines.end), block_of(true, new_lines.start, new_lines.end));
    assert!(old_block.is_some() && new_block.is_some() && old_block != new_block, "{context}");
    assert!(
      marks.starts.as_chunks::<3>().0.iter().any(|start| Some(start[0] as usize) == new_block),
      "{context}: no start where the lines stand"
    );
    for (old_line, new_line) in old_lines.zip(new_lines) {
      assert!(!in_run(false, old_line, false) && !in_run(true, new_line, false), "{context}: a line of a run");
      assert!(!old_moved[old_line as usize] && !new_moved[new_line as usize], "{context}: a line of two moves");
      (old_moved[old_line as usize], new_moved[new_line as usize]) = (true, true);
    }
  }
  let mut starts = marks.starts.as_chunks::<3>().0.iter().peekable();
  for (index, block) in diff.blocks.iter().enumerate() {
    let read = block.kind != BlockKind::Ignored
      && ((block.old_start..block.old_end).any(|line| !quiet(false, line) && !old_moved[line as usize]) || (block.new_start..block.new_end).any(|line| !quiet(true, line)));
    let start = starts.next_if(|start| start[0] as usize == index);
    assert_eq!(start.is_some(), read, "{path}: block {index} of {}", show(diff));
    if let Some(start) = start {
      assert!(!quiet(start[1] == 1, start[2]), "{path}: the start {start:?} of {}", show(diff));
      assert!(
        start[1] == 1 || !old_moved[start[2] as usize],
        "{path}: the start {start:?} of {} is a line that moved",
        show(diff)
      );
    }
  }
  assert_eq!(starts.next(), None, "{path}: {}", show(diff));
  if exact && marks.starts.is_empty() {
    assert_eq!(compare(old_as_one.all(), new_as_one.all()), Equality::Same, "{path}: nothing to read of {}", show(diff));
  }
}

/// Every file of the corpus against itself after random edits, under each policy, with its own line ends, with
/// `\r\n`, with lone `\r` among them and without the last one, and after the edits with some lines at another
/// place: the question before the tokens changes no run and no rename (`read`), and the marks hold what
/// `check_marks` asks
#[test]
fn the_marks_of_edited_files_are_proven_by_their_tokens() {
  let mut random = Random(0x9e37_79b9_7f4a_7c15);
  // the moves have their own numbers, so the edits are the edits they were before lines moved
  let mut moves_random = Random(0x2545_f491_4f6c_dd1d);
  // `MARKS_EDITS=600 cargo nextest run --release -p file-diffs -E 'test(the_marks_of_edited_files)'` asks more of them.
  // Twelve a file reach a changed line inside a comment that runs over a lone `\r`
  let edits = std::env::var("MARKS_EDITS").ok().and_then(|edits| edits.parse().ok()).unwrap_or(12);
  let (mut runs, mut quiet_files, mut moves, mut moves_with_runs) = (0, 0, 0, 0);
  for (path, text) in corpus() {
    let grammar = grammar_of(&path).unwrap();
    for _ in 0..edits {
      let ends = random.below(4);
      let new = with_line_ends(&edited(&text, &mut random), ends, random.below(8) == 0);
      let text = with_line_ends(&text, ends, random.below(8) == 0);
      let (Some(old_tokens), Some(new_tokens)) = (tokens(grammar, &text), tokens(grammar, &new)) else {
        continue;
      };
      for policy in [Exact, Ignore, IgnoreBlankLines] {
        let diff = read(&path, &text, &new, policy);
        check_marks(&path, &diff, [&text, &new], &old_tokens, &new_tokens, policy == Exact);
        runs += diff.marks.runs.len();
        quiet_files += usize::from(!diff.blocks.is_empty() && diff.marks.starts.is_empty());
      }
      // the same edits with some lines at another place
      let new = with_line_ends(&moved(&new.replace("\r\n", "\n").replace('\r', "\n"), &mut moves_random), ends, false);
      let Some(new_tokens) = tokens(grammar, &new) else { continue };
      let diff = read(&path, &text, &new, Exact);
      check_marks(&path, &diff, [&text, &new], &old_tokens, &new_tokens, true);
      moves += diff.marks.moves.len();
      moves_with_runs += usize::from(!diff.marks.moves.is_empty() && !diff.marks.runs.is_empty());
    }
  }
  // the edits do make runs, files with nothing left to read, moves, and files with moves beside runs
  assert!(runs > 100 && quiet_files > 5, "{runs} runs, {quiet_files} files without a start");
  assert!(moves > 100 && moves_with_runs > 20, "{moves} moves, {moves_with_runs} files with moves and runs");
}

/// Every file of the corpus against itself with a name renamed in all of it (`renamed`), alone and beside random
/// edits, with its own line ends, with `\r\n` and with lone `\r` among them: the question before the tokens changes
/// no run and no rename (`read`), and the marks hold what `check_marks` asks, the navigation asked again of each
/// rename among it
#[test]
fn the_renames_of_files_with_a_name_renamed_are_proven_by_their_bindings() {
  let mut random = Random(0x6a09_e667_f3bc_c908);
  // `MARKS_RENAMES=200 cargo nextest run --release -p file-diffs -E 'test(the_renames_of_files)'` asks more of them
  let count = std::env::var("MARKS_RENAMES").ok().and_then(|count| count.parse().ok()).unwrap_or(2);
  let (mut renames, mut local_renames, mut quiet_runs, mut labels) = (0, 0, 0, 0);
  for (path, text) in corpus() {
    let grammar = grammar_of(&path).unwrap();
    for _ in 0..count {
      let ends = random.below(4);
      for edits in [false, true] {
        let base = if edits { edited(&text, &mut random) } else { text.clone() };
        let new = with_line_ends(&renamed(&base, &mut random), ends, false);
        let old = with_line_ends(&text, ends, false);
        let (Some(old_tokens), Some(new_tokens)) = (tokens(grammar, &old), tokens(grammar, &new)) else {
          continue;
        };
        let diff = read(&path, &old, &new, Exact);
        check_marks(&path, &diff, [&old, &new], &old_tokens, &new_tokens, true);
        renames += diff.marks.renames.len();
        local_renames += diff.marks.renames.iter().filter(|rename| rename.local).count();
        quiet_runs += diff.marks.runs.iter().filter(|run| run.kind == RunKind::Renamed && run.quiet).count();
        labels += diff.marks.runs.iter().filter(|run| run.kind == RunKind::Renamed && !run.quiet).count();
      }
    }
  }
  // the renamed words do make renames, of names bound inside a body and of others, with runs that are passed and
  // runs that are labels
  assert!(
    renames > 100 && local_renames > 100 && quiet_runs > 10 && labels > 40,
    "{renames} renames, {local_renames} of them local, {quiet_runs} quiet runs and {labels} labels of renamed lines"
  );
}

/// `count` statements of Kotlin in a function, each a call of four arguments
fn calls(count: usize, indent: &str, arguments: &str) -> String {
  let mut text = String::from("fun f() {\n");
  for index in 0..count {
    writeln!(text, "{indent}call{}({arguments})", index % 7).unwrap();
  }
  text.push_str("}\n");
  text
}

/// The comparison of the tokens of a block is bound by work: a block whose sides differ in more tokens than
/// `MAX_TOKEN_EDITS`, a side of more tokens than `MAX_BLOCK_TOKENS` and a block the work left of its file does not
/// pay for have no run cut out of them, and a block whose sides are the same code needs no comparison at all
#[test]
fn a_block_over_a_limit_of_work_has_no_run_cut_out_of_it() {
  // one line indented again before lines whose arguments changed their order: eight tokens of each line differ
  let block = |lines: usize| {
    let old = calls(lines, "  ", "a, b, c, d").replacen("fun f() {\n", "fun f() {\n  first()\n", 1);
    let new = calls(lines, "    ", "d, c, b, a").replacen("fun f() {\n", "fun f() {\n    first()\n", 1);
    read("a.kt", &old, &new, Exact)
  };
  let small = block(MAX_TOKEN_EDITS / 60);
  assert_eq!(show(&small).split(" starts").next(), Some("blocks [1..36/1..36] runs [1..2/1..2 formatting quiet]"));
  let large = block(MAX_TOKEN_EDITS / 4);
  assert_eq!((large.blocks.len(), large.marks.runs.len(), large.marks.starts.as_slice()), (1, 0, &[0, 1, 1][..]));

  // the same lines by themselves: a block the work of its file does not pay for is not compared; one whose sides are
  // the same code is a run without a comparison
  let kotlin = |text: &str| tokens(Grammar::Kotlin, text).unwrap();
  let (old, new) = ("fun f() {\n  first()\n  call(a, b)\n}\n", "fun f() {\n    first()\n    call(b, a)\n}\n");
  let lines = BlockLines { old: 1..3, new: 1..3 };
  let runs = |work: &mut u64| runs_of_block(&kotlin(old), &kotlin(new), &lines, &ANY_TEXT, work, &mut |_, _| {}).map(|runs| runs.len());
  let mut work = MAX_FILE_TOKEN_WORK;
  assert_eq!(runs(&mut work), Some(1));
  let spent = MAX_FILE_TOKEN_WORK - work;
  assert!(spent > 0);
  assert_eq!(runs(&mut (spent - 1)), None);
  let same = BlockLines { old: 1..2, new: 1..2 };
  assert_eq!(
    runs_of_block(&kotlin(old), &kotlin(new), &same, &ANY_TEXT, &mut 0, &mut |_, _| {}).map(|runs| runs.len()),
    Some(1)
  );

  // a side of more tokens than the limit, a real change at its end: nothing is cut out of it
  let lines = MAX_BLOCK_TOKENS / 9 + 1;
  let (old, new) = (calls(lines, "  ", "a, b, c, d"), calls(lines, "    ", "a, b, c, d").replacen("}\n", "    last()\n}\n", 1));
  let over = read("a.kt", &old, &new, Exact);
  assert_eq!((over.blocks.len(), over.marks.runs.len()), (1, 0));
}

/// The `commit_marks` template of the E2E tests (`test_utils::repo_template`), through the service: each case is a
/// block of its own, and the marks this side decides are there. The layout the later tests of the views rely on
#[test]
fn the_marks_template_has_each_case_in_a_block_of_its_own() {
  let directory = tempfile::tempdir().unwrap();
  test_utils::repo_template::templates::commit_marks().build(directory.path()).unwrap();
  let git = GitCommandExecutor::new();
  let path = directory.path().to_str().unwrap();
  let head = git.execute_command(&["rev-parse", "HEAD"], path).unwrap();
  let files = unhurried()
    .read(&git, path, &ObjectCache::disabled(), &DiffSource::Commit { id: head }, Exact.into())
    .unwrap();
  let shown: Vec<(&str, String)> = files
    .iter()
    .map(|file| match &file.content {
      DiffContent::Text(diff) => (file.path.as_str(), show(&diff.into())),
      other => panic!("{}: {other:?}", file.path),
    })
    .collect();
  let expected = [
    (
      "src/Ledger.kt",
      // `accounts` moved in, the wrapped call, `describe` moved in, the three places of `total`, `accounts` moved
      // out, the comment, `isEmpty`, `describe` moved out, `reportTitle` moved out. `accounts` moved with the blank
      // line that was before it and is after it now; `describe` moved with the one after it, was indented and has an
      // edited line. The blocks the two left have no start. The three of the local `total`, which is `sum` now, are
      // each a run of a renamed name, quiet: nothing but the name changed between its places, and no block of them
      // has a start
      "blocks [5..5/5..10, 6..7/11..14, 11..11/18..25, 13..14/27..28, 16..17/30..31, 19..20/33..34, 21..26/35..35, 27..28/36..37, 32..33/41..42, 45..52/54..54, 53..58/55..55] \
       runs [6..7/11..14 formatting quiet, 13..14/27..28 renamed quiet, 16..17/30..31 renamed quiet, 19..20/33..34 renamed quiet, 27..28/36..37 comments] \
       moves [22..26->5..9 blanks 1+0, 45..52->18..25 indent +2 edited 1] \
       renames [total->sum local declared [0] at 13:8 16:8 19:11 / 27:8 30:8 33:11] \
       starts [0, 1, 5, 2, 1, 18, 7, 1, 36, 8, 1, 41, 10, 0, 53] \
       moved out [54..58->1:17]"
        .to_string(),
    ),
    // the two calls of `openStore`, which the file does not declare and `Store.kt` does: a rename by the other file,
    // each call a run of it, a label; `reportTitle` moved in from `Ledger.kt`, a label too: every block is read
    (
      "src/Report.kt",
      "blocks [4..5/4..5, 10..11/10..11, 16..16/16..21] runs [4..5/4..5 renamed, 10..11/10..11 renamed] starts [0, 1, 4, 1, 1, 10, 2, 1, 16] \
       moved in [17..21->0:54] used [openStore->connectStore declared [] at 4:16 10:16 / 4:16 10:16 in 2:18 Unavailable]"
        .to_string(),
    ),
    // the declaration of `openStore`, which is `connectStore` now: a rename with one place, read; no index was asked
    (
      "src/Store.kt",
      "blocks [18..19/18..19] runs [] renames [openStore->connectStore declared [0] at 18:4 / 18:4] starts [0, 1, 18] declared [0 Unavailable]".to_string(),
    ),
  ];
  assert_eq!(shown, expected);
}
