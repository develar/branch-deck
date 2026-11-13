//! The renames of a diff (`marks_renames.rs`): what is a rename, which of its places are passed, which are a label and
//! which are read, in each grammar family; what is none; the limit of the work.

use crate::marks::{MAX_FILE_TOKEN_WORK, MAX_RENAME_WORK, has_proof, is_quiet, marks_of, pair_grammar};
use crate::marks_renames::{Candidates, may_hold_rename, renames_within, with_the_first};
use crate::marks_runs::{BlockLines, runs_of_block};
use crate::marks_test::{ANY_TEXT, check_marks, read_pair, show_rename};
use code_symbols::navigate::resolution_work;
use code_tokens::tokens;
use code_tokens::{Grammar, grammar_of};
use line_diff::model::{DiffBudget, LineRun, Rename, RunKind, TextDiff, diff_texts};
use line_diff::policy::WhitespacePolicy;
use pretty_assertions::assert_eq;
use std::fmt::Write as _;
use std::sync::Arc;

/// The diff of two texts of a file at `path` with its marks as they are proven (`marks_test::read_pair`: a run is
/// quiet here where its proof holds, also one of a kind that is not switched on for a view), and the names it leaves
/// to the other files. The marks hold what holds of every diff (`check_marks`: the navigation is
/// asked again of each rename)
fn read(path: &str, old: &str, new: &str) -> (TextDiff, Vec<Rename>) {
  let (diff, undeclared, _) = read_pair(path, old, new, WhitespacePolicy::Default);
  if let Some(grammar) = grammar_of(path)
    && let (Some(old_tokens), Some(new_tokens)) = (tokens(grammar, old), tokens(grammar, new))
  {
    check_marks(path, &diff, [old, new], &old_tokens, &new_tokens, true);
  }
  (diff, undeclared)
}

/// The marks of a diff as `marks_test::show` words them, then the names the pair leaves to the other files
fn show(diff: &TextDiff, undeclared: &[Rename]) -> String {
  let mut shown = crate::marks_test::show(diff);
  for rename in undeclared {
    write!(shown, " undeclared [{}]", show_rename(rename)).unwrap();
  }
  shown
}

/// Each case, a file with its two texts, against what `show` gives for it; every failure in one message
fn check_cases(cases: &[(&str, &str, &str, &str)]) {
  let mut failed = String::new();
  for (path, old, new, expected) in cases {
    let (diff, undeclared) = read(path, old, new);
    let found = show(&diff, &undeclared);
    if found != *expected {
      writeln!(failed, "{path}: {old:?} -> {new:?}\n  found    {found}\n  expected {expected}").unwrap();
    }
  }
  assert!(failed.is_empty(), "\n{failed}");
}

/// A function of each grammar with a local `total`: declared, added to in a loop, then read three times
const LOCALS: [(&str, &str); 9] = [
  (
    "a.kt",
    "fun f(xs: List<Int>): Int {\n  var total = 0\n  for (x in xs) {\n    total += x\n  }\n  println(total)\n  check(total >= 0)\n  return total\n}\n",
  ),
  (
    "a.kts",
    "fun f(xs: List<Int>): Int {\n  var total = 0\n  for (x in xs) {\n    total += x\n  }\n  println(total)\n  check(total >= 0)\n  return total\n}\n",
  ),
  (
    "A.java",
    "class A {\n  int f(int[] xs) {\n    int total = 0;\n    for (int x : xs) {\n      total += x;\n    }\n    log(total);\n    check(total >= 0);\n    return total;\n  }\n}\n",
  ),
  (
    "A.cs",
    "class A {\n  int F(int[] xs) {\n    int total = 0;\n    foreach (int x in xs) {\n      total += x;\n    }\n    last = total;\n    ready = total >= 0;\n    return total;\n  }\n}\n",
  ),
  (
    "defs.bzl",
    "def f(xs):\n    total = 0\n    for x in xs:\n        total += x\n    log(total)\n    check(total >= 0)\n    return total\n",
  ),
  (
    "a.rs",
    "fn f(xs: &[i32]) -> i32 {\n  let mut total = 0;\n  for x in xs {\n    total += x;\n  }\n  log(total);\n  check(total >= 0);\n  total\n}\n",
  ),
  (
    "a.ts",
    "function f(xs: number[]): number {\n  let total = 0\n  for (const x of xs) {\n    total += x\n  }\n  log(total)\n  check(total >= 0)\n  return total\n}\n",
  ),
  (
    "a.tsx",
    "function f(xs: number[]): number {\n  let total = 0\n  for (const x of xs) {\n    total += x\n  }\n  log(total)\n  check(total >= 0)\n  return total\n}\n",
  ),
  (
    "a.js",
    "function f(xs) {\n  let total = 0\n  for (const x of xs) {\n    total += x\n  }\n  log(total)\n  check(total >= 0)\n  return total\n}\n",
  ),
];

/// `text` with `from` written as `to` at its last place
fn replace_last(text: &str, from: &str, to: &str) -> String {
  let at = text.rfind(from).unwrap();
  format!("{}{to}{}", &text[..at], &text[at + from.len()..])
}

/// The diff of a function of `LOCALS` whose local was renamed, with what it shows and its one rename
fn read_renamed(path: &str, old: &str, new: &str) -> (TextDiff, String, Rename) {
  let (diff, undeclared) = read(path, old, new);
  let context = format!("{path}: {}", show(&diff, &undeclared));
  assert!(undeclared.is_empty(), "{context}");
  let [rename] = &diff.marks.renames[..] else { panic!("{context}") };
  let rename = rename.clone();
  (diff, context, rename)
}

/// A local renamed at each of its five places, in each grammar: one rename of a name bound inside a body, declared
/// at its first place; every changed line is in a run that is proven, so no block is left to read
#[test]
fn a_local_renamed_at_every_place_is_proven_in_each_grammar() {
  for (path, old) in LOCALS {
    let (diff, context, rename) = read_renamed(path, old, &old.replace("total", "sum"));
    assert_eq!(
      (rename.old.as_str(), rename.new.as_str(), rename.sites, rename.local, rename.declared.as_slice()),
      ("total", "sum", 5, true, &[0][..]),
      "{context}"
    );
    assert_eq!((rename.old_sites.len(), rename.new_sites.len()), (15, 15), "{context}");
    assert!(
      !diff.marks.runs.is_empty() && diff.marks.runs.iter().all(|run| run.kind == RunKind::Renamed && run.quiet),
      "{context}"
    );
    assert!(diff.marks.starts.is_empty(), "{context}");
  }
  check_cases(&[(
    "a.kt",
    LOCALS[0].1,
    &LOCALS[0].1.replace("total", "sum"),
    "blocks [1..2/1..2, 3..4/3..4, 5..8/5..8] runs [1..2/1..2 renamed quiet, 3..4/3..4 renamed quiet, 5..8/5..8 renamed quiet] \
     renames [total->sum local declared [0] at 1:6 3:4 5:10 6:8 7:9 / 1:6 3:4 5:10 6:8 7:9] starts []",
  )]);
}

/// The places of a rename are lines and UTF-16 columns of the two texts, the end of each name excluded, place `i`
/// of the old text beside place `i` of the new one
#[test]
fn the_places_of_a_rename_are_lines_and_columns_of_both_texts() {
  // a character of two UTF-16 units before the name, and a line more in the new text
  let old = "fun f(): Int {\n  val total = 1\n  log(\"\u{1d4b3}\", total)\n  return total\n}\n";
  let new = "fun f(): Int {\n  val sum = 1\n  log(\n    \"\u{1d4b3}\", sum)\n  return sum\n}\n";
  let (diff, _) = read("a.kt", old, new);
  assert_eq!(
    diff.marks.renames,
    [Rename {
      old: "total".to_string(),
      new: "sum".to_string(),
      sites: 3,
      local: true,
      declared: vec![0],
      old_sites: vec![1, 6, 11, 2, 12, 17, 3, 9, 14],
      new_sites: vec![1, 6, 9, 3, 10, 13, 4, 9, 12],
    }]
  );
}

/// A use the change left behind is no place of a rename, in each grammar: the old name still stands at the last
/// place of the local, so nothing is a rename and every block is read
#[test]
fn a_local_with_a_use_left_behind_is_no_rename_in_each_grammar() {
  for (path, old) in LOCALS {
    let new = replace_last(&old.replace("total", "sum"), "sum", "total");
    let (diff, undeclared) = read(path, old, &new);
    let context = format!("{path}: {}", show(&diff, &undeclared));
    assert!(diff.marks.renames.is_empty() && diff.marks.runs.is_empty() && undeclared.is_empty(), "{context}");
    assert_eq!(diff.marks.starts.len(), diff.blocks.len() * 3, "{context}");
  }
}

/// A place on a line with another change is a place of the rename and in no run, in each grammar: that line is
/// read. It stands between the declaration of the name and its last use, where it may give the name another value,
/// so the lines that changed in the name alone are labels: every block is read
#[test]
fn a_place_on_a_line_with_another_change_is_read_in_each_grammar() {
  for (path, old) in LOCALS {
    let (diff, context, rename) = read_renamed(path, old, &old.replace("total", "sum").replace(">= 0", "> 0"));
    assert_eq!((rename.sites, rename.local), (5, true), "{context}");
    // the fourth place is the one on the line of the other change
    let old_line = rename.old_sites[9];
    assert!(
      !diff.marks.runs.is_empty() && diff.marks.runs.iter().all(|run| run.kind == RunKind::Renamed && !run.quiet),
      "{context}"
    );
    assert!(
      !diff.marks.runs.iter().any(|run| run.old_start <= old_line && old_line < run.old_end),
      "{context}: the line of the other change is in a run"
    );
    // each block is read from its first line
    let starts: Vec<u32> = (0u32..).zip(&diff.blocks).flat_map(|(index, block)| [index, 1, block.new_start]).collect();
    assert_eq!(diff.marks.starts, starts, "{context}");
  }
}

/// What is no rename, though a name stands where another stood: nothing is marked, and every block is read. Each
/// case has a name of each side at one place of a block, so each is asked of the bindings (two names swapped inside
/// one block are not: `a_block_whose_sides_hold_the_same_words_gives_no_candidate`)
#[test]
fn a_use_left_behind_a_name_taken_and_two_names_swapped_are_no_rename() {
  check_cases(&[
    // two locals of one body that took the name of each other, declared and read in blocks of their own: each is
    // asked with the name of the other, which the body has already
    (
      "a.kt",
      "fun f() {\n  val a = 1\n  keep()\n  val b = 2\n  keep()\n  use(a, b)\n}\n",
      "fun f() {\n  val b = 1\n  keep()\n  val a = 2\n  keep()\n  use(b, a)\n}\n",
      "blocks [1..2/1..2, 3..4/3..4, 5..6/5..6] runs [] starts [0, 1, 1, 1, 1, 3, 2, 1, 5]",
    ),
    (
      "A.java",
      "class A {\n  void f() {\n    int a = 1;\n    keep();\n    int b = 2;\n    keep();\n    use(a, b);\n  }\n}\n",
      "class A {\n  void f() {\n    int b = 1;\n    keep();\n    int a = 2;\n    keep();\n    use(b, a);\n  }\n}\n",
      "blocks [2..3/2..3, 4..5/4..5, 6..7/6..7] runs [] starts [0, 1, 2, 1, 1, 4, 2, 1, 6]",
    ),
    (
      "A.cs",
      "class A {\n  void F() {\n    int a = 1;\n    Keep();\n    int b = 2;\n    Keep();\n    Use(a, b);\n  }\n}\n",
      "class A {\n  void F() {\n    int b = 1;\n    Keep();\n    int a = 2;\n    Keep();\n    Use(b, a);\n  }\n}\n",
      "blocks [2..3/2..3, 4..5/4..5, 6..7/6..7] runs [] starts [0, 1, 2, 1, 1, 4, 2, 1, 6]",
    ),
    (
      "defs.bzl",
      "def f():\n    a = 1\n    keep()\n    b = 2\n    keep()\n    use(a, b)\n",
      "def f():\n    b = 1\n    keep()\n    a = 2\n    keep()\n    use(b, a)\n",
      "blocks [1..2/1..2, 3..4/3..4, 5..6/5..6] runs [] starts [0, 1, 1, 1, 1, 3, 2, 1, 5]",
    ),
    (
      "a.ts",
      "function f() {\n  const a = 1\n  keep()\n  const b = 2\n  keep()\n  use(a, b)\n}\n",
      "function f() {\n  const b = 1\n  keep()\n  const a = 2\n  keep()\n  use(b, a)\n}\n",
      "blocks [1..2/1..2, 3..4/3..4, 5..6/5..6] runs [] starts [0, 1, 1, 1, 1, 3, 2, 1, 5]",
    ),
    (
      "a.rs",
      "fn f() {\n  let a = 1;\n  keep();\n  let b = 2;\n  keep();\n  hold(a, b);\n}\n",
      "fn f() {\n  let b = 1;\n  keep();\n  let a = 2;\n  keep();\n  hold(b, a);\n}\n",
      "blocks [1..2/1..2, 3..4/3..4, 5..6/5..6] runs [] starts [0, 1, 1, 1, 1, 3, 2, 1, 5]",
    ),
    // the new name is a parameter the body reads: that read would become the local's
    (
      "a.kt",
      "fun f(sum: Int): Int {\n  val total = 1\n  log(sum)\n  return total\n}\n",
      "fun f(sum: Int): Int {\n  val sum = 1\n  log(sum)\n  return sum\n}\n",
      "blocks [1..2/1..2, 3..4/3..4] runs [] starts [0, 1, 1, 1, 1, 3]",
    ),
    // a field the method reads
    (
      "A.java",
      "class A {\n  int sum;\n\n  int f() {\n    int total = 1;\n    log(sum);\n    return total;\n  }\n}\n",
      "class A {\n  int sum;\n\n  int f() {\n    int sum = 1;\n    log(sum);\n    return sum;\n  }\n}\n",
      "blocks [4..5/4..5, 6..7/6..7] runs [] starts [0, 1, 4, 1, 1, 6]",
    ),
    // a constant of the file
    (
      "a.ts",
      "const sum = 0\n\nfunction f() {\n  const total = 1\n  log(sum)\n  return total\n}\n",
      "const sum = 0\n\nfunction f() {\n  const sum = 1\n  log(sum)\n  return sum\n}\n",
      "blocks [3..4/3..4, 5..6/5..6] runs [] starts [0, 1, 3, 1, 1, 5]",
    ),
    (
      "a.rs",
      "fn f(sum: i32) -> i32 {\n  let total = 1;\n  log(sum);\n  total\n}\n",
      "fn f(sum: i32) -> i32 {\n  let sum = 1;\n  log(sum);\n  sum\n}\n",
      "blocks [1..2/1..2, 3..4/3..4] runs [] starts [0, 1, 1, 1, 1, 3]",
    ),
    // a nearer declaration of the new name takes a use of the local
    (
      "a.kt",
      "fun f(): Int {\n  val total = 1\n  run {\n    val sum = 2\n    log(sum + total)\n  }\n  return total\n}\n",
      "fun f(): Int {\n  val sum = 1\n  run {\n    val sum = 2\n    log(sum + sum)\n  }\n  return sum\n}\n",
      "blocks [1..2/1..2, 4..5/4..5, 6..7/6..7] runs [] starts [0, 1, 1, 1, 1, 4, 2, 1, 6]",
    ),
    // two locals that became one name
    (
      "a.kt",
      "fun f() {\n  val a = 1\n  val b = 2\n  first(a)\n  second(b)\n}\n",
      "fun f() {\n  val c = 1\n  val c = 2\n  first(c)\n  second(c)\n}\n",
      "blocks [1..5/1..5] runs [] starts [0, 1, 1]",
    ),
    // a use that got a third name
    (
      "a.kt",
      "fun f() {\n  val a = 1\n  first(a)\n  second(a)\n}\n",
      "fun f() {\n  val b = 1\n  first(b)\n  second(c)\n}\n",
      "blocks [1..4/1..4] runs [] starts [0, 1, 1]",
    ),
    // a name the language binds without a declaration
    (
      "a.kt",
      "fun f(xs: List<Int>) {\n  xs.forEach { total ->\n    log(total)\n  }\n}\n",
      "fun f(xs: List<Int>) {\n  xs.forEach { it ->\n    log(it)\n  }\n}\n",
      "blocks [1..3/1..3] runs [] starts [0, 1, 1]",
    ),
    // a member whose use through another instance kept the old name
    (
      "a.kt",
      "class A {\n  val total = 0\n\n  fun f() = total + 1\n\n  fun g(a: A) = a.total\n}\n",
      "class A {\n  val sum = 0\n\n  fun f() = sum + 1\n\n  fun g(a: A) = a.total\n}\n",
      "blocks [1..2/1..2, 3..4/3..4] runs [] starts [0, 1, 1, 1, 1, 3]",
    ),
    // one of two calls of a function declared elsewhere
    (
      "a.kt",
      "fun f() {\n  openStore(1)\n  openStore(2)\n}\n",
      "fun f() {\n  connectStore(1)\n  openStore(2)\n}\n",
      "blocks [1..2/1..2] runs [] starts [0, 1, 1]",
    ),
  ]);
}

/// What a view gets of a renamed local (`marks_of`): the rename and its runs, quiet, since the kind `renamed` is
/// switched on (`is_quiet`): no block has a start. No run is left that is proven and a label all the same
/// (`PairMarks::proven`), so the marks as proven (`PairMarks::as_proven`) are the marks of the view
#[test]
fn the_runs_of_a_renamed_local_are_quiet_since_their_kind_is_switched_on() {
  for (path, old) in LOCALS {
    let new = old.replace("total", "sum");
    let diff = diff_texts(old, &new, WhitespacePolicy::Default, &DiffBudget::NONE);
    let grammar = grammar_of(path);
    let read = |new_side: bool| tokens(grammar?, if new_side { &new } else { old }).map(Arc::new);
    let found = marks_of(&diff, old, &new, grammar, WhitespacePolicy::Default, false, read);
    let shown = &found.marks;
    assert!(is_quiet(RunKind::Renamed) && has_proof(RunKind::Renamed));
    assert_eq!(found.proven, [] as [u32; 0], "{path}");
    assert_eq!(*shown, found.as_proven(&diff.blocks), "{path}");
    assert_eq!(shown.renames.len(), 1, "{path}");
    assert!(
      !shown.runs.is_empty() && shown.runs.iter().all(|run| run.kind == RunKind::Renamed && run.quiet),
      "{path}: {:?}",
      shown.runs
    );
    // nothing of it is read
    assert_eq!(shown.starts, [] as [u32; 0], "{path}");
  }
}

/// Two locals with as many names at the same places are a rename only where a line changed in the name alone. Found
/// on real commits (`brd commits --mark-stats`): `val isRoot = …` gone and `val nameComponent = …` in its place, each
/// read once where the other was, is one variable in the place of another, and nothing calls it a rename
#[test]
fn a_local_none_of_whose_lines_changed_in_its_name_alone_is_no_rename() {
  check_cases(&[
    (
      "a.kt",
      "fun f(value: Row) {\n  val isRoot = value is Root\n  cell.add(if (isRoot) rootName else name)\n}\n",
      "fun f(value: Row) {\n  val nameComponent = pick(value)\n  cell.add(nameComponent)\n}\n",
      "blocks [1..3/1..3] runs [] starts [0, 1, 1]",
    ),
    (
      "A.java",
      "class A {\n  void f(Path temp) {\n    Launch v3 = parse(TEXT);\n    check(ready(v3), resolve(v3));\n  }\n}\n",
      "class A {\n  void f(Path temp) {\n    Path checkout = temp.resolve(\"ultimate\");\n    check(checkout.resolve(\"out\"), file(checkout));\n  }\n}\n",
      "blocks [2..4/2..4] runs [] starts [0, 1, 2] undeclared [resolve->file declared [] at 3:21 / 3:35]",
    ),
    (
      "a.ts",
      "function f(items: Item[]) {\n  const first = items[0]\n  return label(first.name)\n}\n",
      "function f(items: Item[]) {\n  const count = items.length\n  return label(String(count))\n}\n",
      "blocks [1..3/1..3] runs [] starts [0, 1, 1]",
    ),
    (
      "a.rs",
      "fn f(items: &[Item]) -> String {\n  let first = &items[0];\n  label(&first.name)\n}\n",
      "fn f(items: &[Item]) -> String {\n  let count = items.len();\n  label(&count.to_string())\n}\n",
      "blocks [1..3/1..3] runs [] starts [0, 1, 1]",
    ),
    // a line beside it that changed in its formatting alone is the quiet run it is without the pair of names
    (
      "a.kt",
      "fun f(value: Row) {\n  val isRoot = value is Root\n  log(value, 1)\n  cell.add(if (isRoot) rootName else name)\n}\n",
      "fun f(value: Row) {\n  val nameComponent = pick(value)\n  log(\n    value, 1\n  )\n  cell.add(nameComponent)\n}\n",
      "blocks [1..4/1..6] runs [2..3/2..5 formatting quiet] starts [0, 1, 1]",
    ),
  ]);
}

/// A rename of a local is a label where its declaration or a line of one of its uses changed in more than the name:
/// lines of its member, which proves it only whole (the test below reads the other lines of the member). Where the
/// declaration changed otherwise (found on real commits: `val subscription = subscriptions[id]` became `val channel
/// = subscriptions[id]?.selectedChannel`), the name binds another value: `if (channel != null)` asks another
/// question than `if (subscription != null)` did, with the same tokens, so the runs of such a rename are labels. So
/// are they where the declaration is of several lines and a later one changed, and where a line of a use changed
/// otherwise. Two locals renamed in the lines of each other are proven together or labels together, and a blank
/// line put into the function is no change of it
#[test]
fn a_rename_of_a_local_is_a_label_where_its_declaration_or_a_line_of_a_use_changed_otherwise() {
  check_cases(&[
    // the declaration got another initializer: a rename by its `if`, a label, every block read
    (
      "a.kt",
      "fun f(id: String) {\n  val subscription = subscriptions[id]\n  if (subscription != null) {\n    add(\"channel\", subscription.selectedChannel)\n  }\n}\n",
      "fun f(id: String) {\n  val channel = subscriptions[id]?.selectedChannel\n  if (channel != null) {\n    add(\"channel\", channel)\n  }\n}\n",
      "blocks [1..4/1..4] runs [2..3/2..3 renamed] renames [subscription->channel local declared [0] at 1:6 2:6 3:19 / 1:6 2:6 3:19] starts [0, 1, 1]",
    ),
    // another type: what the call is given is another thing
    (
      "A.java",
      "class A {\n  void f(Event event) {\n    Storage storage = event.storage();\n    log(1);\n    body(tool, storage);\n  }\n}\n",
      "class A {\n  void f(Event event) {\n    Model model = event.model();\n    log(1);\n    body(tool, model);\n  }\n}\n",
      "blocks [2..3/2..3, 4..5/4..5] runs [4..5/4..5 renamed] renames [storage->model local declared [0] at 2:12 4:15 / 2:10 4:15] starts [0, 1, 2, 1, 1, 4] undeclared [Storage->Model declared [] at 2:4 / 2:4] undeclared [storage->model declared [] at 2:28 / 2:24]",
    ),
    (
      "a.ts",
      "function f(id: string) {\n  const session = find(id)\n  log(1)\n  return use(session)\n}\n",
      "function f(id: string) {\n  const conn = connect(id)\n  log(1)\n  return use(conn)\n}\n",
      "blocks [1..2/1..2, 3..4/3..4] runs [3..4/3..4 renamed] renames [session->conn local declared [0] at 1:8 3:13 / 1:8 3:13] starts [0, 1, 1, 1, 1, 3] undeclared [find->connect declared [] at 1:18 / 1:15]",
    ),
    (
      "a.rs",
      "fn f(id: &str) -> Reply {\n  let session = find(id);\n  log(1);\n  send(session)\n}\n",
      "fn f(id: &str) -> Reply {\n  let conn = connect(id);\n  log(1);\n  send(conn)\n}\n",
      "blocks [1..2/1..2, 3..4/3..4] runs [3..4/3..4 renamed] renames [session->conn local declared [0] at 1:6 3:7 / 1:6 3:7] starts [0, 1, 1, 1, 1, 3] undeclared [find->connect declared [] at 1:16 / 1:13]",
    ),
    // a declaration that changed in its name alone, and its last use on a line with another change: that line is
    // read, and the lines before it are labels
    (
      "a.kt",
      "fun f(id: String) {\n  val subscription = subscriptions[id]\n  check(subscription)\n  add(subscription, 1)\n}\n",
      "fun f(id: String) {\n  val channel = subscriptions[id]\n  check(channel)\n  add(channel, 2)\n}\n",
      "blocks [1..4/1..4] runs [1..3/1..3 renamed] renames [subscription->channel local declared [0] at 1:6 2:8 3:6 / 1:6 2:8 3:6] starts [0, 1, 1]",
    ),
    // a declaration that reads the same but for another renamed local is proven with it
    (
      "a.kt",
      "fun f() {\n  val first = one()\n  val second = first\n  use(second)\n}\n",
      "fun f() {\n  val head = one()\n  val next = head\n  use(next)\n}\n",
      "blocks [1..4/1..4] runs [1..4/1..4 renamed quiet] renames [first->head local declared [0] at 1:6 2:15 / 1:6 2:13, second->next local declared [0] at 2:6 3:6 / 2:6 3:6] starts []",
    ),
    // and is a label with it: `next` is given another value, through a name whose declaration changed
    (
      "a.kt",
      "fun f() {\n  val first = one()\n  val second = first\n  log(1)\n  use(second)\n}\n",
      "fun f() {\n  val head = two()\n  val next = head\n  log(1)\n  use(next)\n}\n",
      "blocks [1..3/1..3, 4..5/4..5] runs [2..3/2..3 renamed, 4..5/4..5 renamed] renames [first->head local declared [0] at 1:6 2:15 / 1:6 2:13, second->next local declared [0] at 2:6 4:6 / 2:6 4:6] starts [0, 1, 1, 1, 1, 4] undeclared [one->two declared [] at 1:14 / 1:13]",
    ),
    // a declaration of several lines whose first line changed in the name alone and a later one otherwise, with a
    // line that stays between them: found on real commits as `val scope = if (project != null) {` that became `val
    // content = if (project != null) {` with other branches
    (
      "a.kt",
      "fun f(project: Project?) {\n  val scope = if (project != null) {\n    keep()\n    \"project\"\n  } else \"app\"\n  log(1)\n  notify(scope)\n}\n",
      "fun f(project: Project?) {\n  val content = if (project != null) {\n    keep()\n    text(project)\n  } else \"app\"\n  log(1)\n  notify(content)\n}\n",
      "blocks [1..2/1..2, 3..4/3..4, 6..7/6..7] runs [1..2/1..2 renamed, 6..7/6..7 renamed] renames [scope->content local declared [0] at 1:6 6:9 / 1:6 6:9] starts [0, 1, 1, 1, 1, 3, 2, 1, 6]",
    ),
    // a line that changed after the last place of the name is a line of its function all the same
    (
      "a.kt",
      "fun f(id: String) {\n  val subscription = subscriptions[id]\n  check(subscription)\n  log(1)\n  add(1)\n}\n",
      "fun f(id: String) {\n  val channel = subscriptions[id]\n  check(channel)\n  log(1)\n  add(2)\n}\n",
      "blocks [1..3/1..3, 4..5/4..5] runs [1..3/1..3 renamed] renames [subscription->channel local declared [0] at 1:6 2:8 / 1:6 2:8] starts [0, 1, 1, 1, 1, 4]",
    ),
    // a blank line put between the places is no line that changed
    (
      "a.kt",
      "fun f(id: String) {\n  val subscription = subscriptions[id]\n  log(1)\n  check(subscription)\n}\n",
      "fun f(id: String) {\n  val channel = subscriptions[id]\n\n  log(1)\n  check(channel)\n}\n",
      "blocks [1..2/1..3, 3..4/4..5] runs [1..2/1..3 renamed quiet, 3..4/4..5 renamed quiet] renames [subscription->channel local declared [0] at 1:6 3:8 / 1:6 4:8] starts []",
    ),
    // a parameter whose type changed with its name
    (
      "A.java",
      "class A {\n  void f(boolean strict) {\n    log(1);\n    run(strict);\n  }\n}\n",
      "class A {\n  void f(Mode mode) {\n    log(1);\n    run(mode);\n  }\n}\n",
      "blocks [1..2/1..2, 3..4/3..4] runs [3..4/3..4 renamed] renames [strict->mode local declared [0] at 1:17 3:8 / 1:14 3:8] starts [0, 1, 1, 1, 1, 3]",
    ),
  ]);
}

/// The code of a local is the member around it, whole: what a name holds comes from anywhere in its function. A
/// lambda is given to a call on a value that stands on the lines before its parameter; a value goes on over the
/// lines after the name it is given to; the type of what a body reads stands in the head of its function; a
/// statement after the last place of a name gives a value to a variable the name is made of in the next turn of
/// its loop; and a line no character stands on is a line of a string where a string of several lines holds it.
/// Where such a line changed, the name may hold another value, and the lines that changed in the name alone are
/// labels (the first six cases found by a review of the measured runs). A comment that changed, formatting,
/// another local renamed with it, the name of the function itself and what changed in another function leave the
/// rename proven.
///
/// The head of each type around the member is code of its locals too (the parameters of a primary constructor that
/// the value of a property reads, the components of a record, the bounds of the type parameters), and a function
/// that is a value in the head of a type or the value of an `export default` has no statement of its own around it,
/// so its member is never one statement of its body (found by a second review, which gave such a local its value
/// on the line beside the renamed ones): the cases at the end
#[test]
fn a_rename_of_a_local_is_proven_only_where_its_member_changed_in_names_alone() {
  check_cases(&[
    // the parameter of a lambda given to a call on another value: the first line of its statement
    (
      "a.kt",
      "fun f(users: List<User>, admins: List<User>) {\n  users\n    .filter { it.active }\n    .forEach { user ->\n      send(user)\n    }\n}\n",
      "fun f(users: List<User>, admins: List<User>) {\n  admins\n    .filter { it.active }\n    .forEach { admin ->\n      send(admin)\n    }\n}\n",
      "blocks [1..2/1..2, 3..5/3..5] runs [3..5/3..5 renamed] renames [user->admin local declared [0] at 3:15 4:11 / 3:15 4:11] starts [0, 1, 1, 1, 1, 3]",
    ),
    // and on what another lambda of the chain before it gives (found: `.mapNotNull { it.registry }` became
    // `.mapNotNull { it.catalogOrNull }` before `.collect { catalog -> sweep(catalog) }`)
    (
      "a.kt",
      "suspend fun f(states: Flow<State>) {\n  states\n    .mapNotNull {\n      it.registry\n    }\n    .collect { registry ->\n      sweep(registry)\n    }\n}\n",
      "suspend fun f(states: Flow<State>) {\n  states\n    .mapNotNull {\n      it.catalogOrNull\n    }\n    .collect { catalog ->\n      sweep(catalog)\n    }\n}\n",
      "blocks [3..4/3..4, 5..7/5..7] runs [5..7/5..7 renamed] renames [registry->catalog local declared [0] at 5:15 6:12 / 5:15 6:12] starts [0, 1, 3, 1, 1, 5] undeclared [registry->catalogOrNull declared [] at 3:9 / 3:9]",
    ),
    (
      "a.rs",
      "fn f(config: &Config) -> Vec<&Host> {\n  config\n    .projects\n    .iter()\n    .filter(|project| project.enabled)\n    .collect()\n}\n",
      "fn f(config: &Config) -> Vec<&Host> {\n  config\n    .hosts\n    .iter()\n    .filter(|host| host.enabled)\n    .collect()\n}\n",
      "blocks [2..3/2..3, 4..5/4..5] runs [4..5/4..5 renamed] renames [project->host local declared [0] at 4:13 4:22 / 4:13 4:19] starts [0, 1, 2, 1, 1, 4] undeclared [projects->hosts declared [] at 2:5 / 2:5]",
    ),
    // the head of the function gives what the lambda is called on another type (found: `Collection<String>` became
    // `Collection<AgentSessionId>`, and `.map { sessionId -> Seed(sessionId) }` is given another thing)
    (
      "a.kt",
      "fun Collection<String>.seeds(): Set<Seed> {\n  return asSequence()\n    .map { threadId -> Seed(threadId) }\n    .toSet()\n}\n",
      "fun Collection<SessionId>.seeds(): Set<Seed> {\n  return asSequence()\n    .map { sessionId -> Seed(sessionId) }\n    .toSet()\n}\n",
      "blocks [0..1/0..1, 2..3/2..3] runs [2..3/2..3 renamed] renames [threadId->sessionId local declared [0] at 2:11 2:28 / 2:11 2:29] starts [0, 1, 0, 1, 1, 2] undeclared [String->SessionId declared [] at 0:15 / 0:15]",
    ),
    // an assignment that goes on over a line after the last place of the name, in a loop: the lines before it read
    // what it gave the name
    (
      "A.java",
      "class A {\n  void f(List<String> items) {\n    String last = null;\n    for (String item : items) {\n      if (last != null) emit(last);\n      last = describe(item,\n          mode);\n    }\n  }\n}\n",
      "class A {\n  void f(List<String> items) {\n    String previous = null;\n    for (String item : items) {\n      if (previous != null) emit(previous);\n      previous = describe(item,\n          other);\n    }\n  }\n}\n",
      "blocks [2..3/2..3, 4..7/4..7] runs [2..3/2..3 renamed, 4..6/4..6 renamed] renames [last->previous local declared [0] at 2:11 4:10 4:29 5:6 / 2:11 4:10 4:33 5:6] starts [0, 1, 2, 1, 1, 4] undeclared [mode->other declared [] at 6:10 / 6:10]",
    ),
    // a declaration after the function that reads the name, with a value on two lines
    (
      "a.ts",
      "function f() {\n  const show = () => {\n    console.log(total)\n  }\n  let total = compute(1,\n    2)\n  show()\n}\n",
      "function f() {\n  const show = () => {\n    console.log(sum)\n  }\n  let sum = compute(1,\n    3)\n  show()\n}\n",
      "blocks [2..3/2..3, 4..6/4..6] runs [2..3/2..3 renamed, 4..5/4..5 renamed] renames [total->sum local declared [1] at 2:16 4:6 / 2:16 4:6] starts [0, 1, 2, 1, 1, 4]",
    ),
    // a blank line put into a string of several lines is a line of the string
    (
      "a.kt",
      "fun f(sink: Sink) {\n  val text = \"\"\"\n    first\n    second\n  \"\"\"\n  sink.write(text)\n}\n",
      "fun f(sink: Sink) {\n  val body = \"\"\"\n    first\n\n    second\n  \"\"\"\n  sink.write(body)\n}\n",
      "blocks [1..2/1..2, 3..3/3..4, 5..6/6..7] runs [1..2/1..2 renamed, 5..6/6..7 renamed] renames [text->body local declared [0] at 1:6 5:13 / 1:6 6:13] starts [0, 1, 1, 1, 1, 3, 2, 1, 6]",
    ),
    // a statement after the last place of the name, in the loop of its places: the next turn reads what it wrote
    (
      "a.kt",
      "fun f(items: List<Item>) {\n  var current = first()\n  for (item in items) {\n    val label = current.name\n    show(label)\n    current = next(item, 1)\n  }\n}\n",
      "fun f(items: List<Item>) {\n  var current = first()\n  for (item in items) {\n    val title = current.name\n    show(title)\n    current = next(item, 2)\n  }\n}\n",
      "blocks [3..6/3..6] runs [3..5/3..5 renamed] renames [label->title local declared [0] at 3:8 4:9 / 3:8 4:9] starts [0, 1, 3]",
    ),
    // and any line of the function that changed otherwise: nothing says that it gives the name nothing
    (
      "A.java",
      "class A {\n  void f(int count) {\n    run(count);\n    log(1);\n  }\n}\n",
      "class A {\n  void f(int size) {\n    run(size);\n    log(2);\n  }\n}\n",
      "blocks [1..4/1..4] runs [1..3/1..3 renamed] renames [count->size local declared [0] at 1:13 2:8 / 1:13 2:8] starts [0, 1, 1]",
    ),
    (
      "a.ts",
      "function f(\n  count: number,\n  scale = 1,\n) {\n  run(count)\n}\n",
      "function f(\n  size: number,\n  scale = 2,\n) {\n  run(size)\n}\n",
      "blocks [1..3/1..3, 4..5/4..5] runs [1..2/1..2 renamed, 4..5/4..5 renamed] renames [count->size local declared [0] at 1:2 4:6 / 1:2 4:6] starts [0, 1, 1, 1, 1, 4]",
    ),
    // what changed in another function says nothing of the name: proven, with that line read
    (
      "a.kt",
      "fun f(users: List<User>) {\n  users.forEach { user ->\n    send(user)\n  }\n}\n\nfun g() {\n  log(1)\n}\n",
      "fun f(users: List<User>) {\n  users.forEach { admin ->\n    send(admin)\n  }\n}\n\nfun g() {\n  log(2)\n}\n",
      "blocks [1..3/1..3, 7..8/7..8] runs [1..3/1..3 renamed quiet] renames [user->admin local declared [0] at 1:18 2:9 / 1:18 2:9] starts [1, 1, 7]",
    ),
    // a comment of the function worded again, a call wrapped and another local renamed with it: proven; the
    // comment is read
    (
      "a.kt",
      "fun f(users: List<User>) {\n  // every user\n  val count = users.size\n  users.forEach { user ->\n    send(user, count)\n  }\n  done(1, 2)\n}\n",
      "fun f(users: List<User>) {\n  // every admin\n  val total = users.size\n  users.forEach { admin ->\n    send(admin, total)\n  }\n  done(\n    1, 2)\n}\n",
      "blocks [1..5/1..5, 6..7/6..8] runs [1..2/1..2 comments, 2..5/2..5 renamed quiet, 6..7/6..8 formatting quiet] renames [count->total local declared [0] at 2:6 4:15 / 2:6 4:16, user->admin local declared [0] at 3:18 4:9 / 3:18 4:9] starts [0, 1, 1]",
    ),
    // the function got another name with its local: the name it is known by gives the local nothing
    (
      "a.kt",
      "fun timeout(): Int {\n  val base = 5\n  return base\n}\n\nfun g() = timeout()\n",
      "fun limit(): Int {\n  val unit = 5\n  return unit\n}\n\nfun g() = limit()\n",
      "blocks [0..3/0..3, 5..6/5..6] runs [1..3/1..3 renamed quiet, 5..6/5..6 renamed] renames [timeout->limit declared [0] at 0:4 5:10 / 0:4 5:10, base->unit local declared [0] at 1:6 2:9 / 1:6 2:9] starts [0, 1, 0, 1, 1, 5]",
    ),
    // a function that also calls itself by its new name is read by a name whose proof is another's: a label
    (
      "a.kt",
      "fun timeout(depth: Int): Int {\n  val base = 5\n  return if (depth > 0) timeout(depth - 1) else base\n}\n",
      "fun limit(depth: Int): Int {\n  val unit = 5\n  return if (depth > 0) limit(depth - 1) else unit\n}\n",
      "blocks [0..3/0..3] runs [1..3/1..3 renamed] renames [timeout->limit declared [0] at 0:4 2:24 / 0:4 2:24, base->unit local declared [0] at 1:6 2:48 / 1:6 2:46] starts [0, 1, 0]",
    ),
    // a function that is the value of an `export default` has no statement of its own around it: the export is its
    // member, not the statement of its body that holds the name (found by a review of the rule: the value came
    // from the line beside it)
    (
      "a.ts",
      "export default async (request, backup) => {\n  const rows = await load(request)\n  prepare()\n  for (const row of rows) {\n    publish(row)\n  }\n}\n",
      "export default async (request, backup) => {\n  const rows = await load(backup)\n  prepare()\n  for (const entry of rows) {\n    publish(entry)\n  }\n}\n",
      "blocks [1..2/1..2, 3..5/3..5] runs [3..5/3..5 renamed] renames [row->entry local declared [0] at 3:13 4:12 / 3:13 4:12] starts [0, 1, 1, 1, 1, 3]",
    ),
    // and a method of an object that is such a value
    (
      "a.js",
      "export default {\n  methods: {\n    notify(users, admins) {\n      const list = users.filter(active)\n      prepare()\n      list.forEach(user => {\n        send(user)\n      })\n    },\n  },\n}\n",
      "export default {\n  methods: {\n    notify(users, admins) {\n      const list = admins.filter(active)\n      prepare()\n      list.forEach(member => {\n        send(member)\n      })\n    },\n  },\n}\n",
      "blocks [3..4/3..4, 5..7/5..7] runs [5..7/5..7 renamed] renames [user->member local declared [0] at 5:19 6:13 / 5:19 6:13] starts [0, 1, 3, 1, 1, 5]",
    ),
    // a function that is a value in the head of a type is of no member: an argument of a constant of an enum, of
    // the constructor of a supertype, of a decorator of a class
    (
      "A.java",
      "enum Step {\n  LOAD((source, backup) -> {\n    var rows = source.rows();\n    prepare();\n    for (Row row : rows) publish(row);\n  });\n\n  Step(Loader loader) {}\n}\n",
      "enum Step {\n  LOAD((source, backup) -> {\n    var rows = backup.rows();\n    prepare();\n    for (Row entry : rows) publish(entry);\n  });\n\n  Step(Loader loader) {}\n}\n",
      "blocks [2..3/2..3, 4..5/4..5] runs [4..5/4..5 renamed] renames [row->entry local declared [0] at 4:13 4:33 / 4:13 4:35] starts [0, 1, 2, 1, 1, 4]",
    ),
    (
      "a.kt",
      "class Job(source: Source, backup: Source) : Base({\n  val rows = source.rows()\n  prepare()\n  rows.forEach { row ->\n    publish(row)\n  }\n}) {\n  fun other() = 1\n}\n",
      "class Job(source: Source, backup: Source) : Base({\n  val rows = backup.rows()\n  prepare()\n  rows.forEach { entry ->\n    publish(entry)\n  }\n}) {\n  fun other() = 1\n}\n",
      "blocks [1..2/1..2, 3..5/3..5] runs [3..5/3..5 renamed] renames [row->entry local declared [0] at 3:17 4:12 / 3:17 4:12] starts [0, 1, 1, 1, 1, 3]",
    ),
    (
      "a.ts",
      "@Component({\n  factory: (source, backup) => {\n    const rows = source.rows()\n    prepare()\n    for (const row of rows) publish(row)\n  },\n})\nexport class Job {\n  other() {}\n}\n",
      "@Component({\n  factory: (source, backup) => {\n    const rows = backup.rows()\n    prepare()\n    for (const entry of rows) publish(entry)\n  },\n})\nexport class Job {\n  other() {}\n}\n",
      "blocks [2..3/2..3, 4..5/4..5] runs [4..5/4..5 renamed] renames [row->entry local declared [0] at 4:15 4:36 / 4:15 4:38] starts [0, 1, 2, 1, 1, 4]",
    ),
    // the head of the type around the member is code of its locals: the parameter of the primary constructor that
    // the value of a property reads got another type, so `entry.name` reads another thing
    (
      "a.kt",
      "class Report(rows: List<Row>) {\n  val names = rows.map { row ->\n    row.name\n  }\n}\n",
      "class Report(rows: List<Other>) {\n  val names = rows.map { entry ->\n    entry.name\n  }\n}\n",
      "blocks [0..3/0..3] runs [1..3/1..3 renamed] renames [row->entry local declared [0] at 1:25 2:4 / 1:25 2:4] starts [0, 1, 0] undeclared [Row->Other declared [] at 0:24 / 0:24]",
    ),
    // the component of a record that its compact constructor reads
    (
      "A.java",
      "record Report(List<Row> rows) {\n  Report {\n    rows.forEach(row -> log(row));\n  }\n}\n",
      "record Report(List<Other> rows) {\n  Report {\n    rows.forEach(entry -> log(entry));\n  }\n}\n",
      "blocks [0..1/0..1, 2..3/2..3] runs [2..3/2..3 renamed] renames [row->entry local declared [0] at 2:17 2:28 / 2:17 2:30] starts [0, 1, 0, 1, 1, 2] undeclared [Row->Other declared [] at 0:19 / 0:19]",
    ),
    // and the bound of a type parameter that a member names
    (
      "a.rs",
      "impl<T: Row> Ledger<T> {\n  fn names(&self) -> Vec<Name> {\n    self.rows.iter().map(|row| row.name()).collect()\n  }\n}\n",
      "impl<T: Other> Ledger<T> {\n  fn names(&self) -> Vec<Name> {\n    self.rows.iter().map(|entry| entry.name()).collect()\n  }\n}\n",
      "blocks [0..1/0..1, 2..3/2..3] runs [2..3/2..3 renamed] renames [row->entry local declared [0] at 2:26 2:31 / 2:26 2:33] starts [0, 1, 0, 1, 1, 2] undeclared [Row->Other declared [] at 0:8 / 0:8]",
    ),
    (
      "a.ts",
      "class Ledger<T extends Row> {\n  names(rows: T[]) {\n    return rows.map(row => row.name)\n  }\n}\n",
      "class Ledger<T extends Other> {\n  names(rows: T[]) {\n    return rows.map(entry => entry.name)\n  }\n}\n",
      "blocks [0..1/0..1, 2..3/2..3] runs [2..3/2..3 renamed] renames [row->entry local declared [0] at 2:20 2:27 / 2:20 2:29] starts [0, 1, 0, 1, 1, 2] undeclared [Row->Other declared [] at 0:23 / 0:23]",
    ),
    // another member of the type is none of that code: proven, with its line read
    (
      "a.kt",
      "class Report(rows: List<Row>) {\n  val names = rows.map { row ->\n    row.name\n  }\n\n  fun size() = 1\n}\n",
      "class Report(rows: List<Row>) {\n  val names = rows.map { entry ->\n    entry.name\n  }\n\n  fun size() = 2\n}\n",
      "blocks [1..3/1..3, 5..6/5..6] runs [1..3/1..3 renamed quiet] renames [row->entry local declared [0] at 1:25 2:4 / 1:25 2:4] starts [1, 1, 5]",
    ),
  ]);
}

/// A place where the name is more than the name of its binding is in no run, so its line is read; and since it is a
/// line that changed otherwise between the places of the name, the runs of the other places are labels: a pattern, a
/// delegate or a function that takes the name may bind another value under the new name, and a shorthand, a
/// `nameof` or a macro makes another value of it
#[test]
fn a_place_that_makes_more_of_its_name_than_its_binding_is_read() {
  check_cases(&[
    // declared by a pattern that takes a value apart
    (
      "a.kt",
      "fun f(pair: Pair<Int, Int>): Int {\n  val (total, rest) = pair\n  log(rest)\n  return total\n}\n",
      "fun f(pair: Pair<Int, Int>): Int {\n  val (sum, rest) = pair\n  log(rest)\n  return sum\n}\n",
      "blocks [1..2/1..2, 3..4/3..4] runs [3..4/3..4 renamed] renames [total->sum local declared [0] at 1:7 3:9 / 1:7 3:9] starts [0, 1, 1, 1, 1, 3]",
    ),
    (
      "A.java",
      "class A {\n  int f(Object o) {\n    if (o instanceof Point(int total, int y)) {\n      log(y);\n      return total;\n    }\n    return 0;\n  }\n}\n",
      "class A {\n  int f(Object o) {\n    if (o instanceof Point(int sum, int y)) {\n      log(y);\n      return sum;\n    }\n    return 0;\n  }\n}\n",
      "blocks [2..3/2..3, 4..5/4..5] runs [4..5/4..5 renamed] renames [total->sum local declared [0] at 2:31 4:13 / 2:31 4:13] starts [0, 1, 2, 1, 1, 4]",
    ),
    (
      "a.ts",
      "function f(pair: number[]) {\n  const [total, rest] = pair\n  log(rest)\n  return total\n}\n",
      "function f(pair: number[]) {\n  const [sum, rest] = pair\n  log(rest)\n  return sum\n}\n",
      "blocks [1..2/1..2, 3..4/3..4] runs [3..4/3..4 renamed] renames [total->sum local declared [0] at 1:9 3:9 / 1:9 3:9] starts [0, 1, 1, 1, 1, 3]",
    ),
    (
      "a.rs",
      "fn f(pair: (i32, i32)) -> i32 {\n  let (total, rest) = pair;\n  log(rest);\n  total\n}\n",
      "fn f(pair: (i32, i32)) -> i32 {\n  let (sum, rest) = pair;\n  log(rest);\n  sum\n}\n",
      "blocks [1..2/1..2, 3..4/3..4] runs [3..4/3..4 renamed] renames [total->sum local declared [0] at 1:7 3:2 / 1:7 3:2] starts [0, 1, 1, 1, 1, 3]",
    ),
    // the name is also the property the pattern reads
    (
      "a.ts",
      "function f(props: Props) {\n  const { total } = props\n  log(total)\n  return total\n}\n",
      "function f(props: Props) {\n  const { sum } = props\n  log(sum)\n  return sum\n}\n",
      "blocks [1..4/1..4] runs [2..4/2..4 renamed] renames [total->sum local declared [0] at 1:10 2:6 3:9 / 1:10 2:6 3:9] starts [0, 1, 1]",
    ),
    // and the property an object gets
    (
      "a.ts",
      "function f() {\n  const total = 1\n  log(total)\n  return { total }\n}\n",
      "function f() {\n  const sum = 1\n  log(sum)\n  return { sum }\n}\n",
      "blocks [1..4/1..4] runs [1..3/1..3 renamed] renames [total->sum local declared [0] at 1:8 2:6 3:11 / 1:8 2:6 3:11] starts [0, 1, 1]",
    ),
    (
      "a.rs",
      "fn f(p: Point) -> i32 {\n  let Point { total } = p;\n  log(total);\n  total\n}\n",
      "fn f(p: Point) -> i32 {\n  let Point { sum } = p;\n  log(sum);\n  sum\n}\n",
      "blocks [1..4/1..4] runs [2..4/2..4 renamed] renames [total->sum local declared [0] at 1:14 2:6 3:2 / 1:14 2:6 3:2] starts [0, 1, 1]",
    ),
    (
      "a.rs",
      "fn f() -> Point {\n  let total = 1;\n  log(total);\n  Point { total }\n}\n",
      "fn f() -> Point {\n  let sum = 1;\n  log(sum);\n  Point { sum }\n}\n",
      "blocks [1..4/1..4] runs [1..3/1..3 renamed] renames [total->sum local declared [0] at 1:6 2:6 3:10 / 1:6 2:6 3:10] starts [0, 1, 1]",
    ),
    (
      "A.cs",
      "class A {\n  object F() {\n    int total = 1;\n    last = total;\n    return new { total };\n  }\n}\n",
      "class A {\n  object F() {\n    int sum = 1;\n    last = sum;\n    return new { sum };\n  }\n}\n",
      "blocks [2..5/2..5] runs [2..4/2..4 renamed] renames [total->sum local declared [0] at 2:8 3:11 4:17 / 2:8 3:11 4:17] starts [0, 1, 2]",
    ),
    // among the tokens of a macro, and a name a format string captures: no token of its own
    (
      "a.rs",
      "fn f() -> i32 {\n  let total = 1;\n  println!(\"{}\", total);\n  println!(\"{total}\");\n  total\n}\n",
      "fn f() -> i32 {\n  let sum = 1;\n  println!(\"{}\", sum);\n  println!(\"{sum}\");\n  sum\n}\n",
      "blocks [1..5/1..5] runs [1..2/1..2 renamed, 4..5/4..5 renamed] renames [total->sum local declared [0] at 1:6 2:17 3:13 4:2 / 1:6 2:17 3:13 4:2] starts [0, 1, 1]",
    ),
    // a call of a value: only the types tell whether it is the local
    (
      "a.kt",
      "fun f() {\n  val total = { 1 }\n  log(total)\n  total()\n}\n",
      "fun f() {\n  val sum = { 1 }\n  log(sum)\n  sum()\n}\n",
      "blocks [1..4/1..4] runs [1..3/1..3 renamed] renames [total->sum local declared [0] at 1:6 2:6 3:2 / 1:6 2:6 3:2] starts [0, 1, 1]",
    ),
    // the delegate of a property gets its name where it is declared: the key the map is read by, the name a task
    // is registered under
    (
      "a.kt",
      "fun f(m: Map<String, Int>): Int {\n  val total by m\n  println(total)\n  return total\n}\n",
      "fun f(m: Map<String, Int>): Int {\n  val sum by m\n  println(sum)\n  return sum\n}\n",
      "blocks [1..4/1..4] runs [2..4/2..4 renamed] renames [total->sum local declared [0] at 1:6 2:10 3:9 / 1:6 2:10 3:9] starts [0, 1, 1]",
    ),
    (
      "build.gradle.kts",
      "tasks {\n  val integrationTest by registering(Test::class) {\n    group = \"verification\"\n  }\n  check {\n    dependsOn(integrationTest)\n  }\n}\n",
      "tasks {\n  val integTest by registering(Test::class) {\n    group = \"verification\"\n  }\n  check {\n    dependsOn(integTest)\n  }\n}\n",
      "blocks [1..2/1..2, 5..6/5..6] runs [5..6/5..6 renamed] renames [integrationTest->integTest local declared [0] at 1:6 5:14 / 1:6 5:14] starts [0, 1, 1, 1, 1, 5]",
    ),
    (
      "a.kt",
      "class A {\n  init {\n    val total by lazy { 1 }\n    use(total)\n  }\n}\n",
      "class A {\n  init {\n    val sum by lazy { 1 }\n    use(sum)\n  }\n}\n",
      "blocks [2..4/2..4] runs [3..4/3..4 renamed] renames [total->sum local declared [0] at 2:8 3:8 / 2:8 3:8] starts [0, 1, 2]",
    ),
    // a function and a class written without a name take the name of the variable they are given to
    (
      "a.ts",
      "function f() {\n  const total = () => 1\n  log(total)\n  return total\n}\n",
      "function f() {\n  const sum = () => 1\n  log(sum)\n  return sum\n}\n",
      "blocks [1..4/1..4] runs [2..4/2..4 renamed] renames [total->sum local declared [0] at 1:8 2:6 3:9 / 1:8 2:6 3:9] starts [0, 1, 1]",
    ),
    (
      "a.js",
      "function f() {\n  const Total = class {}\n  log(Total)\n  return Total\n}\n",
      "function f() {\n  const Sum = class {}\n  log(Sum)\n  return Sum\n}\n",
      "blocks [1..4/1..4] runs [2..4/2..4 renamed] renames [Total->Sum local declared [0] at 1:8 2:6 3:9 / 1:8 2:6 3:9] starts [0, 1, 1]",
    ),
    // inside a `with`, the name may be a property of the object
    (
      "a.js",
      "function f(o) {\n  const total = 1\n  with (o) {\n    use(total)\n  }\n  return total\n}\n",
      "function f(o) {\n  const sum = 1\n  with (o) {\n    use(sum)\n  }\n  return sum\n}\n",
      "blocks [1..2/1..2, 3..4/3..4, 5..6/5..6] runs [1..2/1..2 renamed, 5..6/5..6 renamed] renames [total->sum local declared [0] at 1:8 3:8 5:9 / 1:8 3:8 5:9] starts [0, 1, 1, 1, 1, 3, 2, 1, 5]",
    ),
    // the spelling of the name is a value
    (
      "A.cs",
      "class A {\n  string F() {\n    int total = 1;\n    last = total;\n    return nameof(total);\n  }\n}\n",
      "class A {\n  string F() {\n    int sum = 1;\n    last = sum;\n    return nameof(sum);\n  }\n}\n",
      "blocks [2..5/2..5] runs [2..4/2..4 renamed] renames [total->sum local declared [0] at 2:8 3:11 4:18 / 2:8 3:11 4:18] starts [0, 1, 2]",
    ),
    // C# may hand the text of an argument, and of what a method is called on, to what is called
    // (`[CallerArgumentExpression]`): the exception and the failed assertion name another text now
    (
      "A.cs",
      "class A {\n  int F(string path) {\n    var total = Load(path);\n    ArgumentNullException.ThrowIfNull(total);\n    Debug.Assert(total > 0);\n    total.ShouldBe(1);\n    return total;\n  }\n}\n",
      "class A {\n  int F(string path) {\n    var sum = Load(path);\n    ArgumentNullException.ThrowIfNull(sum);\n    Debug.Assert(sum > 0);\n    sum.ShouldBe(1);\n    return sum;\n  }\n}\n",
      "blocks [2..7/2..7] runs [2..3/2..3 renamed, 6..7/6..7 renamed] renames [total->sum local declared [0] at 2:8 3:38 4:17 5:4 6:11 / 2:8 3:38 4:17 5:4 6:11] \
       starts [0, 1, 2]",
    ),
  ]);
}

/// A name bound inside a body is proven only when it is a value of that body alone, every name of which the scopes
/// bind. A parameter that a call may name, that an annotation or an attribute macro may read, a declaration that is
/// no value (a local function), and a local of a text the scopes read in part (a C# file with an `#if`, a Rust body
/// with a `#[cfg]` inside, a script with a direct `eval`) are renames whose runs are labels: every block of them is
/// read
#[test]
fn a_local_that_code_outside_its_body_may_name_is_a_label() {
  check_cases(&[
    // a parameter of a named function of Kotlin: a call may name it
    (
      "a.kt",
      "fun f(total: Int): Int {\n  log(total)\n  return total\n}\n",
      "fun f(sum: Int): Int {\n  log(sum)\n  return sum\n}\n",
      "blocks [0..3/0..3] runs [0..3/0..3 renamed] renames [total->sum local declared [0] at 0:6 1:6 2:9 / 0:6 1:6 2:9] starts [0, 1, 0]",
    ),
    // and where a call does, the name of the argument is no name of the parameter: it is left to the other files
    (
      "a.kt",
      "fun g(total: Int) = total\n\nfun f() {\n  g(total = 1)\n}\n",
      "fun g(sum: Int) = sum\n\nfun f() {\n  g(sum = 1)\n}\n",
      "blocks [0..1/0..1, 3..4/3..4] runs [0..1/0..1 renamed] renames [total->sum local declared [0] at 0:6 0:20 / 0:6 0:18] starts [0, 1, 0, 1, 1, 3] undeclared [total->sum declared [] at 3:4 / 3:4]",
    ),
    // a parameter that is called: that place is read
    (
      "a.kt",
      "fun f(total: () -> Int): Int {\n  log(total)\n  return total()\n}\n",
      "fun f(sum: () -> Int): Int {\n  log(sum)\n  return sum()\n}\n",
      "blocks [0..3/0..3] runs [0..2/0..2 renamed] renames [total->sum local declared [0] at 0:6 1:6 2:9 / 0:6 1:6 2:9] starts [0, 1, 0]",
    ),
    (
      "A.cs",
      "class A {\n  int F(int total) {\n    last = total;\n    return total;\n  }\n}\n",
      "class A {\n  int F(int sum) {\n    last = sum;\n    return sum;\n  }\n}\n",
      "blocks [1..4/1..4] runs [1..4/1..4 renamed] renames [total->sum local declared [0] at 1:12 2:11 3:11 / 1:12 2:11 3:11] starts [0, 1, 1]",
    ),
    (
      "defs.bzl",
      "def f(total):\n    log(total)\n    return total\n",
      "def f(sum):\n    log(sum)\n    return sum\n",
      "blocks [0..3/0..3] runs [0..3/0..3 renamed] renames [total->sum local declared [0] at 0:6 1:8 2:11 / 0:6 1:8 2:11] starts [0, 1, 0]",
    ),
    // under an annotation a framework may read
    (
      "A.java",
      "@Service\nclass A {\n  int f(int total) {\n    log(total);\n    return total;\n  }\n}\n",
      "@Service\nclass A {\n  int f(int sum) {\n    log(sum);\n    return sum;\n  }\n}\n",
      "blocks [2..5/2..5] runs [2..5/2..5 renamed] renames [total->sum local declared [0] at 2:12 3:8 4:11 / 2:12 3:8 4:11] starts [0, 1, 2]",
    ),
    // under an attribute macro
    (
      "a.rs",
      "#[tauri::command]\nfn f(total: i32) -> i32 {\n  log(total);\n  total\n}\n",
      "#[tauri::command]\nfn f(sum: i32) -> i32 {\n  log(sum);\n  sum\n}\n",
      "blocks [1..4/1..4] runs [1..4/1..4 renamed] renames [total->sum local declared [0] at 1:5 2:6 3:2 / 1:5 2:6 3:2] starts [0, 1, 1]",
    ),
    // a file compiled by conditions: a use in the branch the parser does not take is no name of the tree
    (
      "A.cs",
      "class A {\n  int total = 5;\n  int F() {\n    int total = 1;\n#if DEBUG\n    Log(sum);\n#endif\n    return total;\n  }\n}\n",
      "class A {\n  int total = 5;\n  int F() {\n    int sum = 1;\n#if DEBUG\n    Log(sum);\n#endif\n    return sum;\n  }\n}\n",
      "blocks [3..4/3..4, 7..8/7..8] runs [3..4/3..4 renamed, 7..8/7..8 renamed] renames [total->sum local declared [0] at 3:8 7:11 / 3:8 7:11] starts [0, 1, 3, 1, 1, 7]",
    ),
    (
      "A.cs",
      "class A {\n  int F() {\n    int total = 1;\n#if NET8_0_OR_GREATER\n    Fast(1);\n#else\n    Slow(1);\n#endif\n    return total;\n  }\n}\n",
      "class A {\n  int F() {\n    int sum = 1;\n#if NET8_0_OR_GREATER\n    Fast(1);\n#else\n    Slow(1);\n#endif\n    return sum;\n  }\n}\n",
      "blocks [2..3/2..3, 8..9/8..9] runs [2..3/2..3 renamed, 8..9/8..9 renamed] renames [total->sum local declared [0] at 2:8 8:11 / 2:8 8:11] starts [0, 1, 2, 1, 1, 8]",
    ),
    // a body with a `#[cfg]` inside it: the second declaration hides the first only where both are compiled
    (
      "a.rs",
      "fn f() -> u32 {\n  #[cfg(unix)]\n  let total = 1;\n  #[cfg(windows)]\n  let total = 2;\n  total\n}\n",
      "fn f() -> u32 {\n  #[cfg(unix)]\n  let total = 1;\n  #[cfg(windows)]\n  let sum = 2;\n  sum\n}\n",
      "blocks [4..6/4..6] runs [4..6/4..6 renamed] renames [total->sum local declared [0] at 4:6 5:2 / 4:6 5:2] starts [0, 1, 4]",
    ),
    // a script that runs a text where the variable is seen
    (
      "a.js",
      "function f(code) {\n  const total = 1\n  keep(total)\n  return eval(code)\n}\n",
      "function f(code) {\n  const sum = 1\n  keep(sum)\n  return eval(code)\n}\n",
      "blocks [1..3/1..3] runs [1..3/1..3 renamed] renames [total->sum local declared [0] at 1:8 2:7 / 1:8 2:7] starts [0, 1, 1]",
    ),
    // a function declared in a body
    (
      "a.kt",
      "fun f(): Int {\n  fun total() = 1\n  log(1)\n  return total()\n}\n",
      "fun f(): Int {\n  fun sum() = 1\n  log(1)\n  return sum()\n}\n",
      "blocks [1..2/1..2, 3..4/3..4] runs [1..2/1..2 renamed, 3..4/3..4 renamed] renames [total->sum local declared [0] at 1:6 3:9 / 1:6 3:9] starts [0, 1, 1, 1, 1, 3]",
    ),
    (
      "a.ts",
      "function f(): number {\n  function total() { return 1 }\n  log(1)\n  return total()\n}\n",
      "function f(): number {\n  function sum() { return 1 }\n  log(1)\n  return sum()\n}\n",
      "blocks [1..2/1..2, 3..4/3..4] runs [1..2/1..2 renamed, 3..4/3..4 renamed] renames [total->sum local declared [0] at 1:11 3:9 / 1:11 3:9] starts [0, 1, 1, 1, 1, 3]",
    ),
    // the parameter of a lambda, and a parameter where no call names one, are values of their bodies alone
    (
      "a.kt",
      "fun f(xs: List<Int>) {\n  xs.forEach { total ->\n    log(total)\n  }\n}\n",
      "fun f(xs: List<Int>) {\n  xs.forEach { sum ->\n    log(sum)\n  }\n}\n",
      "blocks [1..3/1..3] runs [1..3/1..3 renamed quiet] renames [total->sum local declared [0] at 1:15 2:8 / 1:15 2:8] starts []",
    ),
    (
      "A.java",
      "class A {\n  int f(int total) {\n    log(total);\n    return total;\n  }\n}\n",
      "class A {\n  int f(int sum) {\n    log(sum);\n    return sum;\n  }\n}\n",
      "blocks [1..4/1..4] runs [1..4/1..4 renamed quiet] renames [total->sum local declared [0] at 1:12 2:8 3:11 / 1:12 2:8 3:11] starts []",
    ),
    (
      "a.ts",
      "function f(total: number): number {\n  log(total)\n  return total\n}\n",
      "function f(sum: number): number {\n  log(sum)\n  return sum\n}\n",
      "blocks [0..3/0..3] runs [0..3/0..3 renamed quiet] renames [total->sum local declared [0] at 0:11 1:6 2:9 / 0:11 1:6 2:9] starts []",
    ),
    (
      "a.rs",
      "fn f(total: i32) -> i32 {\n  log(total);\n  total\n}\n",
      "fn f(sum: i32) -> i32 {\n  log(sum);\n  sum\n}\n",
      "blocks [0..3/0..3] runs [0..3/0..3 renamed quiet] renames [total->sum local declared [0] at 0:5 1:6 2:2 / 0:5 1:6 2:2] starts []",
    ),
  ]);
}

/// A name that no body binds, declared in the file: every name of its spelling changed. Its uses are runs that are
/// labels, until a reader of the whole repository confirms them; its declaration is in no run
#[test]
fn a_name_that_is_not_local_is_a_label_and_its_declaration_is_read() {
  check_cases(&[
    // a property, read bare and through an instance
    (
      "a.kt",
      "class A {\n  val total = 0\n\n  fun f() = total + 1\n\n  fun g(a: A) = a.total\n}\n",
      "class A {\n  val sum = 0\n\n  fun f() = sum + 1\n\n  fun g(a: A) = a.sum\n}\n",
      "blocks [1..2/1..2, 3..4/3..4, 5..6/5..6] runs [3..4/3..4 renamed, 5..6/5..6 renamed] renames [total->sum declared [0] at 1:6 3:12 5:18 / 1:6 3:12 5:18] starts [0, 1, 1, 1, 1, 3, 2, 1, 5]",
    ),
    (
      "A.java",
      "class A {\n  int total;\n\n  int f() {\n    return total + 1;\n  }\n\n  int g(A a) {\n    return a.total;\n  }\n}\n",
      "class A {\n  int sum;\n\n  int f() {\n    return sum + 1;\n  }\n\n  int g(A a) {\n    return a.sum;\n  }\n}\n",
      "blocks [1..2/1..2, 4..5/4..5, 8..9/8..9] runs [4..5/4..5 renamed, 8..9/8..9 renamed] renames [total->sum declared [0] at 1:6 4:11 8:13 / 1:6 4:11 8:13] starts [0, 1, 1, 1, 1, 4, 2, 1, 8]",
    ),
    (
      "a.ts",
      "class A {\n  total = 0\n\n  f() {\n    return this.total + 1\n  }\n}\n",
      "class A {\n  sum = 0\n\n  f() {\n    return this.sum + 1\n  }\n}\n",
      "blocks [1..2/1..2, 4..5/4..5] runs [4..5/4..5 renamed] renames [total->sum declared [0] at 1:2 4:16 / 1:2 4:16] starts [0, 1, 1, 1, 1, 4]",
    ),
    (
      "a.rs",
      "struct A {\n  total: i32,\n}\n\nfn f(a: &A) -> i32 {\n  a.total + 1\n}\n",
      "struct A {\n  sum: i32,\n}\n\nfn f(a: &A) -> i32 {\n  a.sum + 1\n}\n",
      "blocks [1..2/1..2, 5..6/5..6] runs [5..6/5..6 renamed] renames [total->sum declared [0] at 1:2 5:4 / 1:2 5:4] starts [0, 1, 1, 1, 1, 5]",
    ),
    // a parameter of a constructor that declares a property, read in the constructor and through `this`
    (
      "a.ts",
      "export class A {\n  constructor(public total: number) {\n    use(total)\n  }\n\n  f() {\n    return this.total + 1\n  }\n}\n",
      "export class A {\n  constructor(public sum: number) {\n    use(sum)\n  }\n\n  f() {\n    return this.sum + 1\n  }\n}\n",
      "blocks [1..3/1..3, 6..7/6..7] runs [2..3/2..3 renamed, 6..7/6..7 renamed] renames [total->sum declared [0] at 1:21 2:8 6:16 / 1:21 2:8 6:16] starts [0, 1, 1, 1, 1, 6]",
    ),
    // a function
    (
      "a.kt",
      "fun total(): Int = 1\n\nfun f(): Int {\n  return total() + 1\n}\n",
      "fun sum(): Int = 1\n\nfun f(): Int {\n  return sum() + 1\n}\n",
      "blocks [0..1/0..1, 3..4/3..4] runs [3..4/3..4 renamed] renames [total->sum declared [0] at 0:4 3:9 / 0:4 3:9] starts [0, 1, 0, 1, 1, 3]",
    ),
    (
      "A.java",
      "class A {\n  int total() {\n    return 1;\n  }\n\n  int f() {\n    return total() + 1;\n  }\n}\n",
      "class A {\n  int sum() {\n    return 1;\n  }\n\n  int f() {\n    return sum() + 1;\n  }\n}\n",
      "blocks [1..2/1..2, 6..7/6..7] runs [6..7/6..7 renamed] renames [total->sum declared [0] at 1:6 6:11 / 1:6 6:11] starts [0, 1, 1, 1, 1, 6]",
    ),
    (
      "a.ts",
      "function total(): number {\n  return 1\n}\n\nexport function f(): number {\n  return total() + 1\n}\n",
      "function sum(): number {\n  return 1\n}\n\nexport function f(): number {\n  return sum() + 1\n}\n",
      "blocks [0..1/0..1, 5..6/5..6] runs [5..6/5..6 renamed] renames [total->sum declared [0] at 0:9 5:9 / 0:9 5:9] starts [0, 1, 0, 1, 1, 5]",
    ),
    (
      "a.rs",
      "fn total() -> i32 {\n  1\n}\n\nfn f() -> i32 {\n  total() + 1\n}\n",
      "fn sum() -> i32 {\n  1\n}\n\nfn f() -> i32 {\n  sum() + 1\n}\n",
      "blocks [0..1/0..1, 5..6/5..6] runs [5..6/5..6 renamed] renames [total->sum declared [0] at 0:3 5:2 / 0:3 5:2] starts [0, 1, 0, 1, 1, 5]",
    ),
    // a class with a use; the type it names is declared elsewhere
    (
      "a.kt",
      "class RowMapper {\n  val field1 = 1\n  fun map(state: String): Row = Row(state)\n}\n\nfun mapper() = RowMapper()\n",
      "class SessionRowMapper {\n  val field1 = 1\n  fun map(state: String): SessionRow = SessionRow(state)\n}\n\nfun mapper() = SessionRowMapper()\n",
      "blocks [0..1/0..1, 2..3/2..3, 5..6/5..6] runs [5..6/5..6 renamed] renames [RowMapper->SessionRowMapper declared [0] at 0:6 5:15 / 0:6 5:15] starts [0, 1, 0, 1, 1, 2, 2, 1, 5] undeclared [Row->SessionRow declared [] at 2:26 2:32 / 2:26 2:39]",
    ),
    // a property and a local of its spelling, both renamed alike: two renames, the local one passed
    (
      "a.kt",
      "class A {\n  val total = 0\n\n  fun f() = total + 1\n\n  fun g(): Int {\n    val total = 2\n    return total\n  }\n}\n",
      "class A {\n  val sum = 0\n\n  fun f() = sum + 1\n\n  fun g(): Int {\n    val sum = 2\n    return sum\n  }\n}\n",
      "blocks [1..2/1..2, 3..4/3..4, 6..8/6..8] runs [3..4/3..4 renamed, 6..8/6..8 renamed quiet] renames [total->sum declared [0] at 1:6 3:12 / 1:6 3:12, total->sum local declared [0] at 6:8 7:11 / 6:8 7:11] starts [0, 1, 1, 1, 1, 3]",
    ),
  ]);
}

/// A name that changed at every place of the file and is declared in none of them is no rename the file can prove:
/// nothing is marked, and the pair hands the name out for the reader of the other files
#[test]
fn a_name_the_file_does_not_declare_is_left_to_the_other_files() {
  check_cases(&[(
    "a.kt",
    "fun f() {\n  openStore(1)\n  openStore(2)\n}\n",
    "fun f() {\n  connectStore(1)\n  connectStore(2)\n}\n",
    "blocks [1..3/1..3] runs [] starts [0, 1, 1] undeclared [openStore->connectStore declared [] at 1:2 2:2 / 1:2 2:2]",
  )]);
}

/// A rename is of a binding, not of a spelling: two locals of one name are two renames, whatever each became
#[test]
fn two_bindings_of_one_spelling_are_two_renames() {
  check_cases(&[
    // two loop variables that got two names
    (
      "a.kt",
      "fun f(xs: List<Int>) {\n  for (i in xs) {\n    first(i)\n  }\n  for (i in xs) {\n    second(i)\n  }\n}\n",
      "fun f(xs: List<Int>) {\n  for (index in xs) {\n    first(index)\n  }\n  for (row in xs) {\n    second(row)\n  }\n}\n",
      "blocks [1..3/1..3, 4..6/4..6] runs [1..3/1..3 renamed quiet, 4..6/4..6 renamed quiet] renames [i->index local declared [0] at 1:7 2:10 / 1:7 2:10, i->row local declared [0] at 4:7 5:11 / 4:7 5:11] starts []",
    ),
    // and the same name
    (
      "a.kt",
      "fun f(xs: List<Int>) {\n  for (i in xs) {\n    first(i)\n  }\n  for (i in xs) {\n    second(i)\n  }\n}\n",
      "fun f(xs: List<Int>) {\n  for (index in xs) {\n    first(index)\n  }\n  for (index in xs) {\n    second(index)\n  }\n}\n",
      "blocks [1..3/1..3, 4..6/4..6] runs [1..3/1..3 renamed quiet, 4..6/4..6 renamed quiet] renames [i->index local declared [0] at 1:7 2:10 / 1:7 2:10, i->index local declared [0] at 4:7 5:11 / 4:7 5:11] starts []",
    ),
    // the locals of two functions that took the name of each other: neither sees the other
    (
      "a.kt",
      "fun f() {\n  val a = 1\n  first(a)\n}\n\nfun g() {\n  val b = 2\n  second(b)\n}\n",
      "fun f() {\n  val b = 1\n  first(b)\n}\n\nfun g() {\n  val a = 2\n  second(a)\n}\n",
      "blocks [1..3/1..3, 6..8/6..8] runs [1..3/1..3 renamed quiet, 6..8/6..8 renamed quiet] renames [a->b local declared [0] at 1:6 2:8 / 1:6 2:8, b->a local declared [0] at 6:6 7:9 / 6:6 7:9] starts []",
    ),
  ]);
}

/// The run of a rename is cut by the tokens: the lines a call with the name was wrapped over are one run, and so
/// are two lines of the name with a blank line the change put between them
#[test]
fn lines_wrapped_with_a_renamed_name_are_one_run() {
  check_cases(&[
    (
      "a.kt",
      "fun f(): Int {\n  val total = 1\n  return compute(total, total)\n}\n",
      "fun f(): Int {\n  val sum = 1\n  return compute(\n    sum,\n    sum\n  )\n}\n",
      "blocks [1..3/1..6] runs [1..3/1..6 renamed quiet] renames [total->sum local declared [0] at 1:6 2:17 2:24 / 1:6 3:4 4:4] starts []",
    ),
    // a name inside a string template is a name
    (
      "a.kt",
      "fun f(): String {\n  val total = 1\n  log(\"$total\")\n  return \"${total}\"\n}\n",
      "fun f(): String {\n  val sum = 1\n  log(\"$sum\")\n  return \"${sum}\"\n}\n",
      "blocks [1..4/1..4] runs [1..4/1..4 renamed quiet] renames [total->sum local declared [0] at 1:6 2:8 3:12 / 1:6 2:8 3:12] starts []",
    ),
    (
      "a.kt",
      "fun f(): Int {\n  val total = 1\n  return total\n}\n",
      "fun f(): Int {\n  val sum = 1\n\n  return sum\n}\n",
      "blocks [1..3/1..4] runs [1..3/1..4 renamed quiet] renames [total->sum local declared [0] at 1:6 2:9 / 1:6 3:9] starts []",
    ),
  ]);
}

/// A line that changed in its formatting alone is a line of formatting beside a renamed name too: the run of the
/// rename holds the lines of its places and no other, so what the tokens of the texts prove stays quiet whatever
/// the rename is, proven or a label
#[test]
fn lines_of_formatting_beside_a_renamed_name_are_a_run_of_their_own() {
  // twelve statements of Java, indented inside a new `if`, the seventh a use of the local
  let steps = |indent: &str, name: &str| -> String {
    (0..12).fold(String::new(), |mut text, step| {
      if step == 6 {
        writeln!(text, "{indent}use({name});").unwrap();
      } else {
        writeln!(text, "{indent}step{step}({step});").unwrap();
      }
      text
    })
  };
  let indented_old = format!(
    "class A {{\n  int f(boolean on) {{\n    int total = 1;\n{}    return total;\n  }}\n}}\n",
    steps("    ", "total")
  );
  let indented_new = format!(
    "class A {{\n  int f(boolean on) {{\n    int sum = 1;\n    if (on) {{\n{}    }}\n    return sum;\n  }}\n}}\n",
    steps("      ", "sum")
  );
  check_cases(&[
    // a call wrapped between two lines of the name
    (
      "a.kt",
      "fun f(): Int {\n  val total = 1\n  log(1, 2)\n  return total\n}\n",
      "fun f(): Int {\n  val sum = 1\n  log(1,\n    2)\n  return sum\n}\n",
      "blocks [1..4/1..5] runs [1..2/1..2 renamed quiet, 2..3/2..4 formatting quiet, 3..4/4..5 renamed quiet] \
       renames [total->sum local declared [0] at 1:6 3:9 / 1:6 4:9] starts []",
    ),
    // and before them
    (
      "a.kt",
      "fun f(): Int {\n  log(1, 2)\n  val total = 1\n  return total\n}\n",
      "fun f(): Int {\n  log(1,\n    2)\n  val sum = 1\n  return sum\n}\n",
      "blocks [1..4/1..5] runs [1..2/1..3 formatting quiet, 2..4/3..5 renamed quiet] renames [total->sum local declared [0] at 2:6 3:9 / 3:6 4:9] starts []",
    ),
    // lines indented inside a new `if`, one of them with the name: the indented lines are passed. The `if` stands
    // between the declaration of the name and its last use, so the lines of the name are labels, and the reading
    // starts at the first of them
    (
      "A.java",
      &indented_old,
      &indented_new,
      "blocks [2..16/2..18] runs [2..3/2..3 renamed, 3..9/4..10 formatting quiet, 9..10/10..11 renamed, 10..15/11..16 formatting quiet, 15..16/17..18 renamed] \
       renames [total->sum local declared [0] at 2:8 9:8 15:11 / 2:8 10:10 17:11] starts [0, 1, 2]",
    ),
    // two calls wrapped, each before a line of the name
    (
      "A.java",
      "class A {\n  int f() {\n    log(1, 2);\n    int total = 1;\n    more(3, 4);\n    return total;\n  }\n}\n",
      "class A {\n  int f() {\n    log(1,\n        2);\n    int sum = 1;\n    more(3,\n        4);\n    return sum;\n  }\n}\n",
      "blocks [2..6/2..8] runs [2..3/2..4 formatting quiet, 3..4/4..5 renamed quiet, 4..5/5..7 formatting quiet, 5..6/7..8 renamed quiet] \
       renames [total->sum local declared [0] at 3:8 5:11 / 4:8 7:11] starts []",
    ),
    // a blank line the change put between the two goes with the formatting, after it and before it
    (
      "a.kt",
      "fun f(): Int {\n  val total = 1\n  log(1, 2)\n  return total\n}\n",
      "fun f(): Int {\n  val sum = 1\n  log(1,\n    2)\n\n  return sum\n}\n",
      "blocks [1..4/1..6] runs [1..2/1..2 renamed quiet, 2..3/2..5 formatting quiet, 3..4/5..6 renamed quiet] \
       renames [total->sum local declared [0] at 1:6 3:9 / 1:6 5:9] starts []",
    ),
    (
      "a.kt",
      "fun f(): Int {\n  val total = 1\n  log(1, 2)\n  return total\n}\n",
      "fun f(): Int {\n  val sum = 1\n\n  log(1,\n    2)\n  return sum\n}\n",
      "blocks [1..4/1..6] runs [1..2/1..2 renamed quiet, 2..3/2..5 formatting quiet, 3..4/5..6 renamed quiet] \
       renames [total->sum local declared [0] at 1:6 3:9 / 1:6 5:9] starts []",
    ),
    // a rename that is a label for good takes no line of formatting either: the use of a member
    (
      "a.kt",
      "class A {\n  val total = 1\n  fun keep() = 0\n  fun f(): Int {\n    log(1, 2)\n    return total\n  }\n}\n",
      "class A {\n  val sum = 1\n  fun keep() = 0\n  fun f(): Int {\n    log(1,\n      2)\n    return sum\n  }\n}\n",
      "blocks [1..2/1..2, 4..6/4..7] runs [4..5/4..6 formatting quiet, 5..6/6..7 renamed] renames [total->sum declared [0] at 1:6 5:11 / 1:6 6:11] starts [0, 1, 1, 1, 1, 6]",
    ),
    // a parameter a call may name
    (
      "a.kt",
      "fun f(total: Int): Int {\n  log(1, 2)\n  return total\n}\n",
      "fun f(sum: Int): Int {\n  log(1,\n    2)\n  return sum\n}\n",
      "blocks [0..3/0..4] runs [0..1/0..1 renamed, 1..2/1..3 formatting quiet, 2..3/3..4 renamed] renames [total->sum local declared [0] at 0:6 2:9 / 0:6 3:9] starts [0, 1, 0]",
    ),
    // a local of a file compiled by conditions
    (
      "A.cs",
      "#if DEBUG\nusing System.Diagnostics;\n#endif\nclass A {\n  int F() {\n    Log(1, 2);\n    int total = 1;\n    return total;\n  }\n}\n",
      "#if DEBUG\nusing System.Diagnostics;\n#endif\nclass A {\n  int F() {\n    Log(1,\n        2);\n    int sum = 1;\n    return sum;\n  }\n}\n",
      "blocks [5..8/5..9] runs [5..6/5..7 formatting quiet, 6..8/7..9 renamed] renames [total->sum local declared [0] at 6:8 7:11 / 7:8 8:11] starts [0, 1, 7]",
    ),
    // and of a text with a syntax error
    (
      "A.java",
      "class A {\n  int f() {\n    log(1, 2);\n    int total = 1;\n    return total;\n  }\n\n  int broken( {\n}\n",
      "class A {\n  int f() {\n    log(1,\n        2);\n    int sum = 1;\n    return sum;\n  }\n\n  int broken( {\n}\n",
      "blocks [2..5/2..6] runs [2..3/2..4 formatting quiet, 3..5/4..6 renamed] renames [total->sum local declared [0] at 3:8 4:11 / 4:8 5:11] starts [0, 1, 4]",
    ),
  ]);
}

/// The two readings of a block align its tokens each by itself: with the names read as one, a line with three places
/// pairs with itself, where the tokens of the texts paired the call that changed places with it. What the first
/// reading proved stays, and the run of the second that crosses it is none: the lines of the places are read, from
/// the old one. They stand between the declaration of the name and its last use, so the two lines that changed in
/// the name alone are labels
#[test]
fn a_run_of_the_first_reading_stays_where_the_second_aligns_the_lines_otherwise() {
  check_cases(&[(
    "a.kt",
    "fun f(): Int {\n  val total = 1\n  keep()\n  a(total, total, total)\n  b(1, 2)\n  keep()\n  return total\n}\n",
    "fun f(): Int {\n  val sum = 1\n  keep()\n  b(1,\n    2)\n  a(sum, sum, sum)\n  keep()\n  return sum\n}\n",
    "blocks [1..2/1..2, 3..5/3..6, 6..7/7..8] runs [1..2/1..2 renamed, 4..5/3..5 formatting quiet, 6..7/7..8 renamed] \
     renames [total->sum local declared [0] at 1:6 3:4 3:11 3:18 6:9 / 1:6 5:4 5:9 5:14 7:9] starts [0, 1, 1, 1, 0, 3, 2, 1, 7]",
  )]);

  // the runs of a block, each `old lines / new lines` of its kind
  let run = |old: std::ops::Range<u32>, new: std::ops::Range<u32>, kind: RunKind| LineRun {
    old_start: old.start,
    old_end: old.end,
    new_start: new.start,
    new_end: new.end,
    kind,
    quiet: has_proof(kind),
  };
  let (formatting, comments, renamed) = (RunKind::Formatting, RunKind::Comments, RunKind::Renamed);
  for (first, second, expected) in [
    // the second reading holds each run of the first, one of them with the blank line beside it, and adds its own
    (
      vec![run(2..3, 2..4, formatting), run(5..6, 6..7, comments)],
      vec![run(1..2, 1..2, renamed), run(2..4, 2..5, formatting), run(4..5, 5..6, renamed), run(5..6, 6..7, comments)],
      vec![run(1..2, 1..2, renamed), run(2..4, 2..5, formatting), run(4..5, 5..6, renamed), run(5..6, 6..7, comments)],
    ),
    // a run of a rename over the lines of a run of the first reading, and one of another kind there, are none
    (
      vec![run(2..3, 2..4, formatting), run(6..7, 7..8, comments)],
      vec![run(1..4, 1..5, renamed), run(6..8, 7..9, formatting)],
      vec![run(2..3, 2..4, formatting), run(6..7, 7..8, comments)],
    ),
    // a run that stands before a run of the first reading on one side and after it on the other is none; the runs
    // apart from it stay
    (
      vec![run(4..5, 3..5, formatting)],
      vec![run(1..2, 1..2, renamed), run(3..4, 5..6, renamed), run(6..7, 7..8, renamed)],
      vec![run(1..2, 1..2, renamed), run(4..5, 3..5, formatting), run(6..7, 7..8, renamed)],
    ),
    // a block the first reading cut nothing out of, and one the second finds nothing in
    (Vec::new(), vec![run(1..2, 1..2, renamed)], vec![run(1..2, 1..2, renamed)]),
    (vec![run(1..2, 1..3, formatting)], Vec::new(), vec![run(1..2, 1..3, formatting)]),
  ] {
    assert_eq!(with_the_first(first.clone(), second.clone()), expected, "{first:?} and {second:?}");
  }
}

/// A line whose comment changed with its name is no run of either kind: its code is not the same without the rename,
/// and its comment is not the same with it. Where that line declares the name, no run says that the declaration reads
/// the same, and the lines that read the name are labels
#[test]
fn a_comment_that_changed_beside_a_renamed_name_is_read() {
  check_cases(&[
    (
      "a.kt",
      "fun f(): Int {\n  val total = 1 // the total\n  return total\n}\n",
      "fun f(): Int {\n  val sum = 1 // the sum\n  return sum\n}\n",
      "blocks [1..3/1..3] runs [2..3/2..3 renamed] renames [total->sum local declared [0] at 1:6 2:9 / 1:6 2:9] starts [0, 1, 1]",
    ),
    // a comment on a line of its own is a run of comments, as without a rename
    (
      "a.kt",
      "fun f(): Int {\n  // the total\n  val total = 1\n  return total\n}\n",
      "fun f(): Int {\n  // the sum\n  val sum = 1\n  return sum\n}\n",
      "blocks [1..4/1..4] runs [1..2/1..2 comments, 2..4/2..4 renamed quiet] renames [total->sum local declared [0] at 2:6 3:9 / 2:6 3:9] starts [0, 1, 1]",
    ),
    // and so beside a line of the name whose comment changed too, which is read
    (
      "a.kt",
      "fun f(): Int {\n  // the total\n  val total = 1 // x\n  return total\n}\n",
      "fun f(): Int {\n  // the sum\n  val sum = 1 // y\n  return sum\n}\n",
      "blocks [1..4/1..4] runs [1..2/1..2 comments, 3..4/3..4 renamed] renames [total->sum local declared [0] at 2:6 3:9 / 2:6 3:9] starts [0, 1, 1]",
    ),
  ]);
}

/// A text with a syntax error has the tokens and the bindings its parser gives it, and none of them is proven: what
/// stands where the parser gave up is no name to the scopes, so a use left behind there does not show. A rename
/// beside the error, and one on the broken line of both sides, is a label, and every block of it is read. A text oxc
/// reads no program of has no bindings, and no rename
#[test]
fn a_file_with_a_syntax_error_is_read_as_its_parser_reads_it() {
  check_cases(&[
    // the old name still stands in what the parser gave up on
    (
      "A.java",
      "class A {\n  int total = 5;\n  int f() {\n    int total = 1;\n    x = ) total (;\n    return total;\n  }\n}\n",
      "class A {\n  int total = 5;\n  int f() {\n    int sum = 1;\n    x = ) total (;\n    return sum;\n  }\n}\n",
      "blocks [3..4/3..4, 5..6/5..6] runs [3..4/3..4 renamed, 5..6/5..6 renamed] renames [total->sum local declared [0] at 3:8 5:11 / 3:8 5:11] starts [0, 1, 3, 1, 1, 5]",
    ),
    (
      "a.kt",
      "fun f(): Int {\n  val total = 1\n  return total\n}\n\nfun broken( {\n",
      "fun f(): Int {\n  val sum = 1\n  return sum\n}\n\nfun broken( {\n",
      "blocks [1..3/1..3] runs [1..3/1..3 renamed] renames [total->sum local declared [0] at 1:6 2:9 / 1:6 2:9] starts [0, 1, 1]",
    ),
    (
      "A.java",
      "class A {\n  int f() {\n    int total = 1;\n    return total;\n  }\n\n  int broken( {\n}\n",
      "class A {\n  int f() {\n    int sum = 1;\n    return sum;\n  }\n\n  int broken( {\n}\n",
      "blocks [2..4/2..4] runs [2..4/2..4 renamed] renames [total->sum local declared [0] at 2:8 3:11 / 2:8 3:11] starts [0, 1, 2]",
    ),
    (
      "a.rs",
      "fn f() -> i32 {\n  let total = 1;\n  total\n}\n\nfn broken( {\n",
      "fn f() -> i32 {\n  let sum = 1;\n  sum\n}\n\nfn broken( {\n",
      "blocks [1..3/1..3] runs [1..3/1..3 renamed] renames [total->sum local declared [0] at 1:6 2:2 / 1:6 2:2] starts [0, 1, 1]",
    ),
    // the broken line holds a place
    (
      "a.kt",
      "fun f(): Int {\n  val total = 1\n  log(total\n  return total\n}\n",
      "fun f(): Int {\n  val sum = 1\n  log(sum\n  return sum\n}\n",
      "blocks [1..4/1..4] runs [1..4/1..4 renamed] renames [total->sum local declared [0] at 1:6 2:6 3:9 / 1:6 2:6 3:9] starts [0, 1, 1]",
    ),
    (
      "a.ts",
      "function f(): number {\n  const total = 1\n  return total\n}\n\nfunction broken( {\n",
      "function f(): number {\n  const sum = 1\n  return sum\n}\n\nfunction broken( {\n",
      "blocks [1..3/1..3] runs [] starts [0, 1, 1]",
    ),
  ]);
}

/// There is no rule of characters: a file of no grammar has no rename. A name renamed in lines that moved is in
/// another block than it was, so it is none either
#[test]
fn a_file_of_no_grammar_and_lines_that_moved_have_no_rename() {
  check_cases(&[
    (
      "a.txt",
      "let total = 1\nuse(total)\n",
      "let sum = 1\nuse(sum)\n",
      "blocks [0..2/0..2] runs [] starts [0, 1, 0]",
    ),
    (
      "a.py",
      "def f():\n    total = 1\n    return total\n",
      "def f():\n    sum = 1\n    return sum\n",
      "blocks [1..3/1..3] runs [] starts [0, 1, 1]",
    ),
    (
      "a.kt",
      "fun a() {\n  first()\n}\n\nfun f(): Int {\n  val total = compute(1, 2, 3)\n  return total\n}\n\nfun b() {\n  second()\n}\n",
      "fun a() {\n  first()\n}\n\nfun b() {\n  second()\n}\n\nfun f(): Int {\n  val sum = compute(1, 2, 3)\n  return sum\n}\n",
      "blocks [4..9/4..4, 12..12/7..12] runs [] starts [0, 0, 4, 1, 1, 7]",
    ),
  ]);
}

/// A pair whose sides are read by two grammars has no rename, as it has no run
#[test]
fn a_pair_of_two_grammars_has_no_rename() {
  let (old, new) = (LOCALS[0].1, LOCALS[0].1.replace("total", "sum"));
  let diff = diff_texts(old, &new, WhitespacePolicy::Default, &DiffBudget::NONE);
  let read = |grammar: Option<Grammar>| {
    let found = marks_of(&diff, old, &new, grammar, WhitespacePolicy::Default, false, |new_side| {
      tokens(grammar?, if new_side { &new } else { old }).map(Arc::new)
    });
    (found.as_proven(&diff.blocks), found.undeclared)
  };
  let (marks, undeclared) = read(pair_grammar("a.kt", "a.kts"));
  assert_eq!((marks.runs.len(), marks.renames.len(), undeclared.len(), marks.starts.len()), (0, 0, 0, 9));
  let (marks, _) = read(pair_grammar("a.kt", "a.kt"));
  assert_eq!((marks.runs.len(), marks.renames.len(), marks.starts.len()), (3, 1, 0));
}

/// A block whose two sides hold the same words gives no candidate, which is also the question it is asked before the
/// tokens: a file whose blocks all hold the same words, start and end no line alike and hold no comment is not parsed
#[test]
fn a_block_whose_sides_hold_the_same_words_gives_no_candidate() {
  for (old, new, expected) in [
    ("val total = 1", "val sum = 1", true),
    // one word more often
    ("use(a, b)", "use(a, a, b)", true),
    // the same words in another order, other numbers, other marks, a word of digits first
    ("use(a, b)", "use(b, a)", false),
    ("val a = 1 + b", "val a = 20 - b", false),
    ("a(0x1f, 2L)", "a(0x2f, 3L)", false),
    // a letter of another script, `_` and `$` are of a word
    ("val größe = 1", "val grösse = 1", true),
    ("a_b + $c", "a_c + $c", true),
    ("", "", false),
  ] {
    assert_eq!(may_hold_rename(old, new), expected, "{old:?} -> {new:?}");
  }
  // two names that took the place of each other in one block: no candidate, so no side is parsed
  let (diff, _, asked) = read_pair(
    "a.kt",
    "fun f() {\n  val a = 1\n  val b = 2\n  use(a, b)\n}\n",
    "fun f() {\n  val b = 1\n  val a = 2\n  use(b, a)\n}\n",
    WhitespacePolicy::Default,
  );
  assert_eq!((diff.marks.renames.len(), asked), (0, [0, 0]));
  // a renamed local: each side is parsed once, for its tokens
  let (diff, _, asked) = read_pair("a.kt", LOCALS[0].1, &LOCALS[0].1.replace("total", "sum"), WhitespacePolicy::Default);
  assert_eq!((diff.marks.renames.len(), asked), (1, [1, 1]));
}

/// A file of `functions` functions of Kotlin or of Java, each with a local of its own, declared and returned, and
/// `calls` lines between the two that do not name it
fn functions_with_a_local(grammar: Grammar, functions: usize, calls: usize, name: &str) -> String {
  let kotlin = grammar == Grammar::Kotlin;
  let end = if kotlin { "" } else { ";" };
  let mut text = String::from(if kotlin { "" } else { "class A {\n" });
  for index in 0..functions {
    let declared = if kotlin { "fun f" } else { "int f" };
    writeln!(text, "{declared}{index}(){} {{", if kotlin { ": Int" } else { "" }).unwrap();
    writeln!(text, "  {} {name}{index} = {index}{end}", if kotlin { "val" } else { "int" }).unwrap();
    for _ in 0..calls {
      writeln!(text, "  log({index}, 1, 2, 3, 4, 5, 6, 7, 8, 9){end}").unwrap();
    }
    writeln!(text, "  return {name}{index}{end}\n}}\n").unwrap();
  }
  if !kotlin {
    text.push_str("}\n");
  }
  text
}

/// The diff of two texts of `grammar` with its marks as they are proven, read once
fn marks_read_once(grammar: Grammar, old: &str, new: &str) -> TextDiff {
  let mut diff = diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget::NONE);
  let read = |new_side: bool| tokens(grammar, if new_side { new } else { old }).map(Arc::new);
  diff.marks = marks_of(&diff, old, new, Some(grammar), WhitespacePolicy::Default, false, read).as_proven(&diff.blocks);
  diff
}

/// The old names of the renames of two texts of `grammar` within `work`, from the candidates of their blocks, and
/// the resolutions of a name each text took
fn renames_within_work(grammar: Grammar, old: &str, new: &str, work: u64) -> (Vec<String>, [usize; 2]) {
  let diff = diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget::NONE);
  let (old_tokens, new_tokens) = (tokens(grammar, old).unwrap(), tokens(grammar, new).unwrap());
  let mut candidates = Candidates::default();
  let mut token_work = MAX_FILE_TOKEN_WORK;
  for block in &diff.blocks {
    let lines = BlockLines {
      old: block.old_start..block.old_end,
      new: block.new_start..block.new_end,
    };
    let _ = runs_of_block(&old_tokens, &new_tokens, &lines, &ANY_TEXT, &mut token_work, &mut |old, new| candidates.take(old, new));
  }
  let found = renames_within(grammar, [old, new], [&old_tokens, &new_tokens], &diff.blocks, &candidates, work);
  (found.found.into_iter().map(|found| found.rename.old).collect(), found.resolutions)
}

/// The names of a file are asked within a limit of work, never of time: a resolution of a name costs the tokens of
/// its text, or an eighth of them where it reads the names of one spelling, one resolution per spelling and one more
/// per binding, and the first place of each pair of names is asked before the second of any. Two hundred renamed
/// locals of a file that the work pays for are two hundred renames; in a file it does not pay for, the first locals
/// of the text are, as many as it pays for, and the others are read
#[test]
fn the_names_asked_are_bound_by_work() {
  // 200 locals of Kotlin with two places each: 400 candidates, 200 spellings and 200 bindings of each text
  let kotlin = Grammar::Kotlin;
  let (old, new) = (functions_with_a_local(kotlin, 200, 0, "total"), functions_with_a_local(kotlin, 200, 0, "sum"));
  let cost = |grammar: Grammar, old: &str, new: &str| {
    let tokens = |text: &str| tokens(grammar, text).unwrap().all().len();
    resolution_work(grammar, tokens(old)) + resolution_work(grammar, tokens(new))
  };
  let diff = marks_read_once(kotlin, &old, &new);
  assert_eq!((diff.marks.renames.len(), diff.marks.starts.len()), (200, 0));
  let (renames, resolutions) = renames_within_work(kotlin, &old, &new, MAX_RENAME_WORK);
  assert_eq!((renames.len(), resolutions), (200, [400, 400]));
  assert!(MAX_RENAME_WORK / cost(kotlin, &old, &new) > 400);

  // work for 150 resolutions of each text: the first 75 locals of the text, a spelling and a binding each
  let (renames, resolutions) = renames_within_work(kotlin, &old, &new, cost(kotlin, &old, &new) * 150);
  assert_eq!((renames.len(), resolutions), (75, [150, 150]));
  assert_eq!((renames[0].as_str(), renames[74].as_str()), ("total0", "total74"));
  // for less than one resolution: nothing is asked
  assert_eq!(renames_within_work(kotlin, &old, &new, cost(kotlin, &old, &new) - 1), (Vec::new(), [0, 0]));

  // a file the work does not pay for: 100 locals of Java, six lines of calls between the two places of each, where
  // a resolution walks the tree. The locals asked are the first of the text, and the blocks of the others are read
  let java = Grammar::Java;
  let (old, new) = (functions_with_a_local(java, 100, 6, "total"), functions_with_a_local(java, 100, 6, "sum"));
  let paid = usize::try_from(MAX_RENAME_WORK / cost(java, &old, &new)).unwrap() / 2;
  assert!((8..100).contains(&paid), "{paid} locals");
  let diff = marks_read_once(java, &old, &new);
  let names: Vec<&str> = diff.marks.renames.iter().map(|rename| rename.old.as_str()).collect();
  assert_eq!(names, (0..paid).map(|index| format!("total{index}")).collect::<Vec<_>>());
  assert!(diff.marks.renames.iter().all(|rename| rename.sites == 2 && rename.local));
  // each place is a block of its own, and those of the locals that were not asked have a start
  assert_eq!((diff.blocks.len(), diff.marks.starts.len() / 3), (200, 200 - 2 * paid));

  // the places of one spelling that are many bindings cost a resolution each, after the one of the spelling: a
  // local of one name in each function, renamed alike
  let alike = |name: &str| functions_with_a_local(kotlin, 60, 0, name).replace(|character: char| character.is_ascii_digit(), "");
  let (old, new) = (alike("total").replace("fun f()", "fun f(x: Int)"), alike("sum").replace("fun f()", "fun f(x: Int)"));
  let (renames, resolutions) = renames_within_work(kotlin, &old, &new, MAX_RENAME_WORK);
  assert_eq!((renames.len(), resolutions), (60, [61, 61]));
}

/// The blocks of a rename are read again within the work the first reading of the blocks left of the file's
/// (`MAX_FILE_TOKEN_WORK` is the work of both): a block that what is left does not pay for keeps the runs it has,
/// none of them a run of the rename. A block that is the same code once the names are read as one costs nothing.
/// The second reading tells the lines of a rename from the lines of formatting by itself: also in a block the first
/// reading cut nothing out of
#[test]
fn the_blocks_of_a_rename_are_cut_again_within_the_work_the_file_has_left() {
  let kotlin = Grammar::Kotlin;
  // the declaration renamed, a call wrapped, and the last place on a line with another change
  let old = "fun f(): Int {\n  val total = 1\n  log(1, 2)\n  return total\n}\n";
  let renamed = "fun f(): Int {\n  val sum = 1\n  log(1,\n    2)\n  return sum\n}\n";
  let edited = renamed.replace("return sum", "return sum + 1");
  // the runs of the block after its second reading with `left` of the work, and what that reading spent; `first`:
  // with the runs of its first reading, else as a block that reading cut nothing out of
  let cut = |new: &str, left: u64, first: bool| -> (Vec<String>, u64) {
    let diff = diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget::NONE);
    let (old_tokens, new_tokens) = (tokens(kotlin, old).unwrap(), tokens(kotlin, new).unwrap());
    let [block] = &diff.blocks[..] else { panic!("{:?}", diff.blocks) };
    let lines = BlockLines {
      old: block.old_start..block.old_end,
      new: block.new_start..block.new_end,
    };
    let mut candidates = Candidates::default();
    let mut work = MAX_FILE_TOKEN_WORK;
    let read = runs_of_block(&old_tokens, &new_tokens, &lines, &ANY_TEXT, &mut work, &mut |old, new| candidates.take(old, new));
    let mut runs = vec![if first { read.unwrap() } else { Vec::new() }];
    let found = renames_within(kotlin, [old, new], [&old_tokens, &new_tokens], &diff.blocks, &candidates, MAX_RENAME_WORK);
    assert_eq!(found.found.len(), 1);
    let mut work = left;
    found.cut(&diff.blocks, [&old_tokens, &new_tokens], &|_, _| true, &mut work, &mut runs);
    let shown = |run: &LineRun| format!("{}..{}/{}..{} {:?}", run.old_start, run.old_end, run.new_start, run.new_end, run.kind);
    (runs[0].iter().map(shown).collect(), left - work)
  };
  // the block with another change is compared again: the line of the declaration is a run of the rename, beside the
  // wrapped call the first reading found
  let (runs, spent) = cut(&edited, MAX_FILE_TOKEN_WORK, true);
  assert_eq!(runs, ["1..2/1..2 Renamed", "2..3/2..4 Formatting"]);
  assert!(spent > 0);
  // with one step less than that comparison costs, the block keeps what the first reading found: the wrapped call
  assert_eq!(cut(&edited, spent - 1, true), (vec!["2..3/2..4 Formatting".to_string()], 0));
  // the same code once the names are read as one: the runs without a comparison, whatever is left
  let both = ["1..2/1..2 Renamed", "2..3/2..4 Formatting", "3..4/4..5 Renamed"].map(str::to_string).to_vec();
  assert_eq!(cut(renamed, 0, true), (both.clone(), 0));
  // a block the first reading cut nothing out of has the same runs: the wrapped call is no line of the rename
  assert_eq!(cut(renamed, 0, false), (both, 0));
  assert_eq!(cut(&edited, MAX_FILE_TOKEN_WORK, false).0, ["1..2/1..2 Renamed", "2..3/2..4 Formatting"]);
}

/// The names of a file are read on the stack of a worker of the diffs, 2 MiB, as its tokens are: a text nested
/// deeper than its parser reads has no tokens and is asked no name, and one within that depth has its rename
#[test]
fn a_deeply_nested_file_is_read_on_the_stack_of_a_worker() {
  // the start of a function, a block to nest, the declaration and the read of the local, the ends
  let grammars = [
    ("a.kt", "fun f() {\n", "run {\n", "val total = 1\nlog(total)\n", "}\n", "}\n"),
    ("A.java", "class A {\n  void f() {\n", "{\n", "int total = 1;\nlog(total);\n", "}\n", "  }\n}\n"),
    ("A.cs", "class A {\n  void F() {\n", "{\n", "int total = 1;\nLog(total);\n", "}\n", "  }\n}\n"),
    ("a.rs", "fn f() {\n", "{\n", "let total = 1;\nlog(total);\n", "}\n", "}\n"),
    ("a.ts", "function f() {\n", "{\n", "const total = 1\nlog(total)\n", "}\n", "}\n"),
    ("defs.bzl", "def f():\n    total = 1\n    log(total, ", "(", "", ")", ")\n"),
  ];
  let mut found = 0;
  for (path, head, open, body, close, tail) in grammars {
    for depth in [60, 240, 900, 3_600] {
      let old = format!("{head}{}{body}{}{tail}", open.repeat(depth), close.repeat(depth));
      let new = old.replace("total", "sum");
      let worker = std::thread::Builder::new().stack_size(2 << 20).spawn(move || {
        let (diff, _, _) = read_pair(path, &old, &new, WhitespacePolicy::Default);
        (diff.marks.renames.len(), tokens(grammar_of(path).unwrap(), &old).is_some())
      });
      let (renames, has_tokens) = worker.unwrap().join().unwrap();
      assert_eq!(renames, usize::from(has_tokens), "{path} nested {depth} times");
      found += renames;
    }
  }
  // every grammar has a depth with tokens, and Rust and TypeScript, read on a stack of their own, have every depth
  assert!(found >= 6 + 3 + 3, "{found} renames");
}
