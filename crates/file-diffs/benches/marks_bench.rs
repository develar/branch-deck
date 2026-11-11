//! What the marks of a diff cost (docs/design/diff-view.md, "Marks"): the tokens of a text, the rule of the tokens,
//! `marks_of` of a pair, the renames of a file at the limit of their work, the moves of a file of 10,000 lines, of
//! the file a search costs most in and of a chain of many short moves, and a subject of 200 files, with and without
//! renamed locals and moved functions, within a file and between two: its moves alone, the pass over its files
//! alone, and through the service to its first content and to the end of its stream, compared and from memory.
//! `cargo bench -p file-diffs --bench marks_bench`, on a quiet machine.
//!
//! The pair is two states of a Kotlin file of 3,300 lines, 300 functions of which every fifth changed:
//! - `formatted`: a call wrapped over three lines and a comment worded again, so every changed block is a run and both
//!   sides are parsed;
//! - `operator`: an operator changed, so no block may hold a run or a renamed name and nothing is parsed;
//! - `changed`: a name of a member changed, which may be a rename: both sides are parsed for their tokens and, since
//!   the name stands where another stood, for their names; the file declares neither, so nothing is marked;
//! - `renamed`: a local renamed at its four places, so both sides are parsed for their tokens and their names, and
//!   the blocks are cut again: every changed block is a run of the rename (a label while its kind is off, so every
//!   block is still read: the cost is the same);
//! - `indented`: every function indented again, a name changed in the first: 300 blocks, one of them with a line to
//!   read beside its run.

use code_symbols::highlight::Grammar;
use code_symbols::navigate::resolution_work;
use code_symbols::tokens::{Tokens, compare, equal_by_tokens, tokens};
use criterion::{Criterion, criterion_group, criterion_main};
use file_diffs::leftovers::Leftovers;
use file_diffs::marks::{MAX_BLOCK_TOKENS, MAX_RENAME_WORK, MAX_TOKEN_EDITS, marks_of};
use file_diffs::marks_subject::{IndexAnswer, PairFacts, Subject};
use file_diffs::model::{DiffSource, FileDiffEvent, GetFileDiffsParams};
use file_diffs::service::{DiffLimits, FileDiffs};
use object_cache::cache::ObjectCache;
use std::fmt::Write as _;
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};
use test_utils::git_test_utils::TestRepo;
use text_diff::model::{BlockKind, DiffBlock, DiffBudget, TextDiff, diff_texts};
use text_diff::moves::{MAX_KEY_CANDIDATES, MAX_MOVE_LINES, MAX_MOVE_WORK, find_moves, find_moves_counted};
use text_diff::policy::WhitespacePolicy;

/// How a function of the generated file differs from its first state
#[derive(Clone, Copy, PartialEq, Eq)]
enum Edit {
  None,
  /// A call wrapped inside its brackets, and its comment worded again
  Formatted,
  /// The name of a member changed
  Changed,
  /// An operator changed
  Operator,
  /// A local renamed at its four places
  Renamed,
}

/// A Kotlin class of `functions` functions of eleven lines, `edit` saying how each differs
fn source(name: &str, functions: usize, indent: &str, edit: impl Fn(usize) -> Edit) -> String {
  source_of(name, 0..functions, indent, edit)
}

/// A Kotlin class of the functions `functions`, by their numbers and in their order
fn source_of(name: &str, functions: impl Iterator<Item = usize>, indent: &str, edit: impl Fn(usize) -> Edit) -> String {
  let mut text = format!("package bench\n\nclass {name}(private val repository: Repository) {{\n");
  for index in functions {
    let edit = edit(index);
    writeln!(text, "{indent}  fun handle{index}(request: Request, limit: Int): Response {{").unwrap();
    if edit == Edit::Formatted {
      writeln!(
        text,
        "{indent}    val items = repository.find(\n{indent}      request.id,\n{indent}      limit\n{indent}    )"
      )
      .unwrap();
      writeln!(text, "{indent}    // Only the items that are still open.").unwrap();
    } else {
      writeln!(text, "{indent}    val items = repository.find(request.id, limit)").unwrap();
      writeln!(text, "{indent}    // Keeps the items that are still open.").unwrap();
    }
    let open = if edit == Edit::Renamed { "kept" } else { "open" };
    let equal = if edit == Edit::Operator { "!=" } else { "==" };
    writeln!(text, "{indent}    val {open} = items.filter {{ it.state {equal} State.OPEN }}").unwrap();
    writeln!(text, "{indent}    if ({open}.isEmpty()) {{").unwrap();
    writeln!(text, "{indent}      return Response.empty(request.id)").unwrap();
    writeln!(text, "{indent}    }}").unwrap();
    let amount = if edit == Edit::Changed { "price" } else { "amount" };
    writeln!(text, "{indent}    val total = {open}.sumOf {{ it.{amount} }}").unwrap();
    writeln!(text, "{indent}    return Response(request.id, {open}, total)").unwrap();
    writeln!(text, "{indent}  }}\n").unwrap();
  }
  text.push_str("}\n");
  text
}

fn every_fifth(edit: Edit) -> impl Fn(usize) -> Edit {
  move |index| if index % 5 == 0 { edit } else { Edit::None }
}

fn diff_of(old: &str, new: &str) -> TextDiff {
  diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget::NONE)
}

fn kotlin(text: &str) -> Arc<Tokens> {
  Arc::new(tokens(Grammar::Kotlin, text).expect("a text with tokens"))
}

/// The tokens of one side, and the rule of the tokens over a block and over the whole pair
fn bench_tokens(c: &mut Criterion) {
  let old = source("Service", 300, "", |_| Edit::None);
  let new = source("Service", 300, "", every_fifth(Edit::Formatted));
  let (old_tokens, new_tokens) = (kotlin(&old), kotlin(&new));
  println!(
    "the pair: {} and {} lines, {} and {} tokens",
    old.lines().count(),
    new.lines().count(),
    old_tokens.all().len(),
    new_tokens.all().len()
  );

  let mut group = c.benchmark_group("tokens");
  group.bench_function("a side of 3,300 lines", |b| b.iter(|| tokens(Grammar::Kotlin, black_box(&old))));
  group.bench_function("compare, the whole pair", |b| b.iter(|| compare(black_box(old_tokens.all()), black_box(new_tokens.all()))));
  let diff = diff_of(&old, &new);
  let block = &diff.blocks[0];
  group.bench_function("equal_by_tokens, one block", |b| {
    b.iter(|| equal_by_tokens(&old_tokens, block.old_start..block.old_end, &new_tokens, block.new_start..block.new_end));
  });
  group.finish();
}

/// `marks_of` of the pair, with what the diff itself costs beside it
fn bench_marks(c: &mut Criterion) {
  let old = source("Service", 300, "", |_| Edit::None);
  let formatted = source("Service", 300, "", every_fifth(Edit::Formatted));
  let changed = source("Service", 300, "", every_fifth(Edit::Changed));
  let operator = source("Service", 300, "", every_fifth(Edit::Operator));
  let renamed = source("Service", 300, "", every_fifth(Edit::Renamed));
  // every function indented again, a name changed in the first
  let indented = source("Service", 300, "  ", |index| if index == 0 { Edit::Changed } else { Edit::None });

  let mut group = c.benchmark_group("marks_of");
  group.bench_function("the diff alone, formatted", |b| b.iter(|| diff_of(black_box(&old), black_box(&formatted))));
  for (name, new) in [
    ("formatted", &formatted),
    ("operator", &operator),
    ("changed", &changed),
    ("renamed", &renamed),
    ("indented", &indented),
  ] {
    let diff = diff_of(&old, new);
    let (old_tokens, new_tokens) = (kotlin(&old), kotlin(new));
    let kept = |new_side: bool| Some(Arc::clone(if new_side { &new_tokens } else { &old_tokens }));
    let marks = marks_of(&diff, &old, new, Some(Grammar::Kotlin), WhitespacePolicy::Default, false, kept).marks;
    println!(
      "{name}: {} blocks, {} runs, {} renames, {} blocks to read",
      diff.blocks.len(),
      marks.runs.len(),
      marks.renames.len(),
      marks.starts.len() / 3
    );
    // the tokens of both sides read for the marks: what a file costs before its content goes out
    group.bench_function(format!("{name}, tokens read"), |b| {
      b.iter(|| {
        marks_of(&diff, &old, new, Some(Grammar::Kotlin), WhitespacePolicy::Default, false, |new_side| {
          Some(kotlin(if new_side { new } else { &old }))
        })
      });
    });
    // the tokens in memory already: the question before the tokens and the runs alone
    group.bench_function(format!("{name}, tokens kept"), |b| {
      b.iter(|| marks_of(&diff, &old, new, Some(Grammar::Kotlin), WhitespacePolicy::Default, false, kept));
    });
  }
  group.finish();
}

/// A file of `functions` functions of `grammar` (Kotlin, Java or C#), each with a local of its own named `name` and
/// its number: declared, then read on `uses` lines and returned
fn locals_source(grammar: Grammar, functions: usize, uses: usize, name: &str) -> String {
  let kotlin = grammar == Grammar::Kotlin;
  let mut text = String::from(if kotlin { "package bench\n\n" } else { "class Bench {\n" });
  for index in 0..functions {
    if kotlin {
      writeln!(text, "fun handle{index}(): Int {{\n  val {name}{index} = {index}").unwrap();
    } else {
      writeln!(text, "  int handle{index}() {{\n    int {name}{index} = {index};").unwrap();
    }
    for _ in 0..uses {
      writeln!(text, "    log({name}{index}, {index}){}", if kotlin { "" } else { ";" }).unwrap();
    }
    writeln!(text, "    return {name}{index}{}\n  }}\n", if kotlin { "" } else { ";" }).unwrap();
  }
  if !kotlin {
    text.push_str("}\n");
  }
  text
}

/// The renames of a file at the limit of their work (`MAX_RENAME_WORK`): 200 locals renamed, each read on twelve
/// lines, in a file of some 3,400 lines. Each local is a spelling and a binding of its own, two resolutions of each
/// text; the work pays for some two hundred resolutions of a Kotlin text, where a resolution reads the names of
/// one spelling, and for some twenty-six of a Java or a C# text, where it walks the tree: the most the names of one
/// file cost before its content goes out
fn bench_renames(c: &mut Criterion) {
  let mut group = c.benchmark_group("marks_of, renamed locals at the limit of their work");
  group.sample_size(20);
  for (name, grammar) in [("Kotlin", Grammar::Kotlin), ("Java", Grammar::Java), ("C#", Grammar::CSharp)] {
    let (old, new) = (locals_source(grammar, 200, 12, "total"), locals_source(grammar, 200, 12, "sum"));
    let diff = diff_of(&old, &new);
    let read = |text: &str| Arc::new(tokens(grammar, text).expect("a text with tokens"));
    let (old_tokens, new_tokens) = (read(&old), read(&new));
    let kept = |new_side: bool| Some(Arc::clone(if new_side { &new_tokens } else { &old_tokens }));
    let marks = marks_of(&diff, &old, &new, Some(grammar), WhitespacePolicy::Default, false, kept).marks;
    println!(
      "{name}: {} lines, {} tokens a side, {} blocks, {} renames by {} resolutions a side, {} blocks to read",
      old.lines().count(),
      old_tokens.all().len(),
      diff.blocks.len(),
      marks.renames.len(),
      MAX_RENAME_WORK / (resolution_work(grammar, old_tokens.all().len()) + resolution_work(grammar, new_tokens.all().len())),
      marks.starts.len() / 3
    );
    group.bench_function(format!("{name}, tokens kept"), |b| {
      b.iter(|| marks_of(&diff, &old, &new, Some(grammar), WhitespacePolicy::Default, false, kept));
    });
  }
  group.finish();
}

/// A Kotlin class of the functions `functions`, by their numbers and in their order, each of eleven lines that are
/// its own (its number is in most of them); `edited` says which have another word in one line
fn own_source(functions: impl Iterator<Item = usize>, edited: impl Fn(usize) -> bool) -> String {
  let mut text = String::from("package bench\n\nclass Service(private val repository: Repository) {\n");
  for index in functions {
    let amount = if edited(index) { "price" } else { "amount" };
    writeln!(text, "  fun handle{index}(request: Request{index}, limit: Int): Response {{").unwrap();
    writeln!(text, "    val items{index} = repository.find{index}(request.id, limit)").unwrap();
    writeln!(text, "    // Keeps the items of {index} that are still open.").unwrap();
    writeln!(text, "    val open{index} = items{index}.filter {{ it.state == State.OPEN }}").unwrap();
    writeln!(text, "    if (open{index}.isEmpty()) {{").unwrap();
    writeln!(text, "      return Response.empty(request.id)").unwrap();
    writeln!(text, "    }}").unwrap();
    writeln!(text, "    val total{index} = open{index}.sumOf {{ it.{amount} }}").unwrap();
    writeln!(text, "    return Response(request.id, open{index}, total{index})").unwrap();
    writeln!(text, "  }}\n").unwrap();
  }
  text.push_str("}\n");
  text
}

/// The moves of a file of 10,000 lines, 910 functions, from the blocks of its diff: what every file pays for them
/// when none of its lines moved, and what a file with moved functions pays
fn bench_moves(c: &mut Criterion) {
  const FUNCTIONS: usize = 910;
  let old = own_source(0..FUNCTIONS, |_| false);
  // every 23rd function at the end of the class, in their order
  let (stay, leave) = (|index: &usize| index % 23 != 7, |index: &usize| index % 23 == 7);
  let at_the_end = |edited: bool| own_source((0..FUNCTIONS).filter(stay).chain((0..FUNCTIONS).filter(leave)), move |index| edited && leave(&index));
  let cases = [
    // a word changed in every fifth function: blocks, and no line that stands at two places
    ("no line moved", own_source(0..FUNCTIONS, |index| index % 5 == 0)),
    ("40 functions moved", at_the_end(false)),
    ("40 functions moved, a line of each edited", at_the_end(true)),
    // the two halves of the class change places: one move of 5,000 lines
    ("half the file moved", own_source((FUNCTIONS / 2..FUNCTIONS).chain(0..FUNCTIONS / 2), |_| false)),
  ];
  let mut group = c.benchmark_group("moves, 10,000 lines");
  // what the comparison of such a pair costs, beside its moves
  group.bench_function("the diff alone, 40 functions moved", |b| b.iter(|| diff_of(black_box(&old), black_box(&cases[1].1))));
  for (name, new) in &cases {
    let diff = diff_of(&old, new);
    let (moves, work) = find_moves_counted(&old, new, &diff.blocks, &[], WhitespacePolicy::Default, false);
    println!(
      "{name}: {} blocks, {} moves of {} lines, {} edited lines, {work:?}",
      diff.blocks.len(),
      moves.len(),
      moves.iter().map(|line_move| line_move.len).sum::<u32>(),
      moves.iter().map(|line_move| line_move.edited.len()).sum::<usize>(),
    );
    group.bench_function(*name, |b| {
      b.iter(|| find_moves(black_box(&old), black_box(new), &diff.blocks, &[], WhitespacePolicy::Default, false));
    });
  }
  group.finish();
}

/// The lines `old` deleted before ten lines that stay, and the lines `new`, as many, added after them: the two texts
/// and the two blocks of their diff
fn moved_sides(old: &[String], new: &[String]) -> (String, String, [DiffBlock; 2]) {
  let side = |lines: &[String], stay_first: bool| {
    let mut text = String::new();
    let stay = |text: &mut String| (0..10).for_each(|index| writeln!(text, "stay_{index}();").unwrap());
    if stay_first {
      stay(&mut text);
    }
    for line in lines {
      writeln!(text, "{line}").unwrap();
    }
    if !stay_first {
      stay(&mut text);
    }
    text
  };
  let count = u32::try_from(old.len()).unwrap();
  let block = |old: std::ops::Range<u32>, new: std::ops::Range<u32>| DiffBlock {
    old_start: old.start,
    old_end: old.end,
    new_start: new.start,
    new_end: new.end,
    kind: BlockKind::Changed,
    inline_pairs: false,
    old_ranges: Vec::new(),
    new_ranges: Vec::new(),
  };
  (side(old, false), side(new, true), [block(0..count, 0..0), block(count + 10..count + 10, 10..count + 10)])
}

/// The file the search for moves costs most in: `lines` lines deleted in one block and added in another, every line
/// standing `MAX_KEY_CANDIDATES` times on each side and no two lines in a row on both, so every pair of two lines of
/// a key starts a chain of its own that ends at once
fn repeated_sides(lines: usize, line: impl Fn(usize) -> String) -> (String, String, [DiffBlock; 2]) {
  let keys = lines / MAX_KEY_CANDIDATES;
  let side = |key: &dyn Fn(usize) -> usize| (0..lines).map(|index| line(key(index))).collect::<Vec<String>>();
  // seven keys on from line to line: a line that follows another on one side never does on the other
  moved_sides(&side(&|index| index % keys), &side(&|index| index * 7 % keys))
}

/// The search for moves at its limits: a file of as many lines as a side may have, each standing as often as a line
/// that starts a chain may. The search ends at `MAX_MOVE_WORK` pairs: what a file costs at the worst
fn bench_moves_limits(c: &mut Criterion) {
  // lines that are nothing alike: the text of a line is a number of its own, five times
  let unlike = |key: usize| format!("{:08x}", key * 2_654_435_761 % 0xffff_fffb).repeat(5);
  // lines that differ in a number: every pair of lines that ends a chain may be an edited line of it
  let alike = |key: usize| format!("    registry.register(handler_{key:05}, {});", "option, ".repeat(11));
  type Line<'a> = &'a dyn Fn(usize) -> String;
  let cases: [(&str, usize, Line<'_>); 3] = [
    ("65,536 lines of 40 characters", MAX_MOVE_LINES, &unlike),
    ("65,536 lines of 120 characters, alike", MAX_MOVE_LINES, &alike),
    ("10,000 lines of 40 characters", 10_000, &unlike),
  ];
  let mut group = c.benchmark_group("moves, every line 16 times");
  group.sample_size(10);
  for (name, lines, line) in &cases {
    let (old, new, blocks) = repeated_sides(*lines, line);
    let (moves, work) = find_moves_counted(&old, &new, &blocks, &[], WhitespacePolicy::Default, false);
    println!("{name}: {} moves, {work:?} of {MAX_MOVE_WORK}", moves.len());
    group.bench_function(*name, |b| {
      b.iter(|| find_moves(black_box(&old), black_box(&new), &blocks, &[], WhitespacePolicy::Default, false));
    });
  }
  group.finish();
}

/// A chain of many short moves, in a file of as many lines as a side may have: the longest row of the chain that may
/// be a move is a line or two, and what is left of the chain after it is nearly all of it. Read again for each
/// move, such a chain cost seconds; it is read once for all of them (`Finder::many_moves`)
fn bench_moves_short(c: &mut Criterion) {
  const LINES: usize = 65_000;
  // jscpd:ignore-start: the files of `a_chain_of_many_short_moves_is_read_once_for_all_of_them` of text-diff, whose
  // tests a benchmark of another crate cannot share
  // a list of YAML, each item of five lines, indented again by two columns a level in place of four
  let items = |step: usize| -> Vec<String> {
    (0..LINES / 5)
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
  // every second line under another parent, the lines between them at their indent
  let alternate = |indent: &str| -> Vec<String> {
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
  // a line of its own, a line with another number that a chain grows through unread, and two blank lines
  let groups = |number: usize| -> Vec<String> {
    (0..LINES / 4)
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
  // jscpd:ignore-end
  let cases = [
    ("a list of YAML indented again, 13,000 items of five lines", items(4), items(2), true),
    ("every second line under another parent", alternate(""), alternate("  "), true),
    ("a line of its own among lines that are alike and blank ones", groups(1), groups(2), false),
  ];
  let mut group = c.benchmark_group("moves, a chain of many short moves, 65,000 lines");
  group.sample_size(10);
  for (name, old, new, indented) in &cases {
    let (old, new, blocks) = moved_sides(old, new);
    let (moves, work) = find_moves_counted(&old, &new, &blocks, &[], WhitespacePolicy::Default, *indented);
    println!("{name}: {} moves, {work:?} of {MAX_MOVE_WORK}", moves.len());
    group.bench_function(*name, |b| {
      b.iter(|| find_moves(black_box(&old), black_box(&new), &blocks, &[], WhitespacePolicy::Default, *indented));
    });
  }
  group.finish();
}

/// One block at the limits of its comparison: `lines` calls on each side, indented again, the arguments of the first
/// `reversed` of them in another order
fn limit_pair(lines: usize, reversed: usize) -> (String, String) {
  let side = |indent: &str, reversed: usize| {
    let mut text = String::from("fun f() {\n");
    for index in 0..lines {
      let arguments = if index < reversed { "d, c, b, a" } else { "a, b, c, d" };
      writeln!(text, "{indent}call{}({arguments})", index % 7).unwrap();
    }
    text.push_str("}\n");
    text
  };
  (side("  ", 0), side("    ", reversed))
}

/// The worst a block costs: a side of `MAX_BLOCK_TOKENS` tokens that differs in nearly `MAX_TOKEN_EDITS` of them, and
/// one that differs in more, whose comparison gives up
fn bench_limits(c: &mut Criterion) {
  // ten tokens a line
  let lines = MAX_BLOCK_TOKENS / 11;
  let mut group = c.benchmark_group("marks_of, a block at its limits");
  group.sample_size(20);
  // eight tokens of a line with its arguments in another order differ
  for (name, reversed) in [
    ("within the edits", MAX_TOKEN_EDITS / 8 - 16),
    ("over the edits", MAX_TOKEN_EDITS / 8 + 64),
    ("every line", lines),
  ] {
    let (old, new) = limit_pair(lines, reversed);
    let diff = diff_of(&old, &new);
    let (old_tokens, new_tokens) = (kotlin(&old), kotlin(&new));
    let kept = |new_side: bool| Some(Arc::clone(if new_side { &new_tokens } else { &old_tokens }));
    let marks = marks_of(&diff, &old, &new, Some(Grammar::Kotlin), WhitespacePolicy::Default, false, kept).marks;
    println!("{name}: {} blocks, {} tokens a side, {} runs", diff.blocks.len(), old_tokens.all().len(), marks.runs.len());
    group.bench_function(name, |b| {
      b.iter(|| marks_of(&diff, &old, &new, Some(Grammar::Kotlin), WhitespacePolicy::Default, false, kept));
    });
  }
  group.finish();
}

/// The 200 Kotlin files of a subject, four of 3,300 lines and the others of 165, before and after its commit:
/// `sides` are the two texts of a file, by the name of its class and the number of its functions
fn subject_files(sides: &dyn Fn(&str, usize) -> [String; 2]) -> [Vec<(String, String)>; 2] {
  let (mut before, mut after) = (Vec::new(), Vec::new());
  for index in 0..200 {
    let functions = if index % 50 == 0 { 300 } else { 15 };
    let path = format!("src/bench/Service{index}.kt");
    let [old, new] = sides(&format!("Service{index}"), functions);
    before.push((path.clone(), old));
    after.push((path, new));
  }
  [before, after]
}

/// The commit of a subject (`subject_files`)
fn subject(sides: &dyn Fn(&str, usize) -> [String; 2]) -> (TestRepo, String) {
  let [before, after] = subject_files(sides);
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &pairs(&before));
  let commit = repo.create_commit_with_files("Change", &pairs(&after));
  (repo, commit)
}

fn pairs(files: &[(String, String)]) -> Vec<(&str, &str)> {
  files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect()
}

/// A service whose time limits no file of a subject reaches
fn service() -> FileDiffs {
  FileDiffs::new(DiffLimits {
    file: Duration::from_secs(60),
    lines: Duration::from_secs(60),
  })
}

/// The time of one stream of the subject by a service with empty caches: to its first content, and to its end
fn stream(repo: &TestRepo, commit: &str) -> (Duration, Duration) {
  stream_of(&service(), repo, commit)
}

/// The time of one stream of the subject by `diffs`: to its first content, and to its end. From the memory of a
/// service that streamed the subject before, the first content waits for what the files say of each other
fn stream_of(diffs: &FileDiffs, repo: &TestRepo, commit: &str) -> (Duration, Duration) {
  let params = GetFileDiffsParams {
    repository_path: repo.path().to_str().expect("a UTF-8 path").to_string(),
    source: DiffSource::Commit { id: commit.to_string() },
    policy: WhitespacePolicy::Default,
    priority_paths: Vec::new(),
  };
  let start = Instant::now();
  let mut first = None;
  diffs
    .stream(repo.git_executor(), &ObjectCache::disabled(), None, &params, |event| {
      if matches!(event, FileDiffEvent::Content { .. }) {
        first.get_or_insert_with(|| start.elapsed());
      }
    })
    .expect("the diffs of the subject");
  (first.expect("a content"), start.elapsed())
}

/// The two texts of a file of a subject, by the name of its class and the number of its functions
type Sides = Box<dyn Fn(&str, usize) -> [String; 2]>;

/// The subjects of the stream. `operator` parses no file for its marks (the question before the tokens answers no
/// for every block), `formatted` parses both sides of every file for their tokens, `changed` and `renamed` for their
/// tokens and their names: the difference is what the marks cost when nothing is left out before the tokens, and what
/// the names add. `moved` has every fifth function of each file at the end of its class, the lines of a function its
/// own: three moves of eleven lines a file, sixty in the four large ones. `moved between files` has those functions
/// at the end of the class of the next file, each function of the subject a text of its own: the moves between two
/// files that the pass over the files of the subject finds. A function that left a small file is two of them, around
/// the two lines every function has (`return Response.empty(request.id)` and the brace after it): the pair of its
/// file took those for a move within the file, to a function that arrived there, before the pass saw the other file
fn subjects() -> [(&'static str, Sides); 6] {
  let edited = |edit: Edit| -> Sides { Box::new(move |name, functions| [source(name, functions, "", |_| Edit::None), source(name, functions, "", every_fifth(edit))]) };
  let moved: Sides = Box::new(|_, functions| {
    let (stay, leave) = (|index: &usize| index % 5 != 2, |index: &usize| index % 5 == 2);
    [
      own_source(0..functions, |_| false),
      own_source((0..functions).filter(stay).chain((0..functions).filter(leave)), |_| false),
    ]
  });
  let moved_across: Sides = Box::new(|name, functions| {
    // the functions of a file, by numbers no other file has, and the ones that leave it
    let own = |file: usize, functions: usize| file * 1_000..file * 1_000 + functions;
    let (stay, leave) = (|index: &usize| index % 5 != 2, |index: &usize| index % 5 == 2);
    let file: usize = name.trim_start_matches("Service").parse().expect("the number of the file");
    let before = (file + 199) % 200;
    let arrive = own(before, if before % 50 == 0 { 300 } else { 15 }).filter(leave);
    [
      own_source(own(file, functions), |_| false),
      own_source(own(file, functions).filter(stay).chain(arrive), |_| false),
    ]
  });
  [
    ("operator", edited(Edit::Operator)),
    ("changed", edited(Edit::Changed)),
    ("formatted", edited(Edit::Formatted)),
    ("renamed", edited(Edit::Renamed)),
    ("moved", moved),
    ("moved between files", moved_across),
  ]
}

/// What the files of a subject say of each other, from what their pairs left: the pass over the 200 files alone, on
/// one thread, as a stream runs it once every pair is known (before the first content when the caches had them all)
fn bench_subject_pass(c: &mut Criterion) {
  let mut group = c.benchmark_group("the pass over the 200 files of a subject");
  for (name, sides) in subjects() {
    let [before, after] = subject_files(&sides);
    let read: Vec<(TextDiff, Leftovers)> = before
      .iter()
      .zip(&after)
      .map(|((_, old), (_, new))| {
        let mut diff = diff_of(old, new);
        let tokens_of = |new_side: bool| tokens(Grammar::Kotlin, if new_side { new } else { old }).map(Arc::new);
        let pair = marks_of(&diff, old, new, Some(Grammar::Kotlin), WhitespacePolicy::Default, false, tokens_of);
        let (marks, left) = Leftovers::of(&diff.blocks, pair, old, new);
        diff.marks = marks;
        (diff, left)
      })
      .collect();
    let facts: Vec<Option<PairFacts<'_>>> = before
      .iter()
      .zip(&read)
      .map(|((path, _), (diff, left))| {
        Some(PairFacts {
          old_path: Some(path),
          new_path: Some(path),
          diff,
          left,
        })
      })
      .collect();
    let subject = Subject::of(&facts);
    let marks = subject.marks(IndexAnswer::Unavailable);
    let moved: usize = marks
      .iter()
      .flatten()
      .filter_map(|marks| marks.subject.as_ref())
      .map(|subject| subject.moved_in.len())
      .sum();
    let left: usize = read.iter().flat_map(|(_, left)| left.lines.iter().flatten()).map(|row| row.keys.len()).sum();
    println!(
      "{name}: {left} lines left by the pairs, {moved} moves between files, {} files the subject says something of, work {:?}",
      marks.iter().flatten().count(),
      subject.work
    );
    group.bench_function(name, |b| b.iter(|| black_box(Subject::of(&facts).marks(IndexAnswer::Unavailable)).len()));
  }
  group.finish();
}

/// The moves of every file of a subject, one file after another on one thread, from the blocks of its diff: what
/// the moves add to the work of a stream, whatever the load of the machine does to the stream itself
fn bench_subject_moves(c: &mut Criterion) {
  let mut group = c.benchmark_group("moves, the 200 files of a subject");
  for (name, sides) in subjects() {
    let [before, after] = subject_files(&sides);
    let diffs: Vec<TextDiff> = before.iter().zip(&after).map(|((_, old), (_, new))| diff_of(old, new)).collect();
    let moves_of = |index: usize| find_moves(&before[index].1, &after[index].1, &diffs[index].blocks, &[], WhitespacePolicy::Default, false);
    let found: Vec<usize> = (0..diffs.len()).map(|index| moves_of(index).len()).collect();
    let work = (0..diffs.len()).fold((0, 0), |(search, compared), index| {
      let work = find_moves_counted(&before[index].1, &after[index].1, &diffs[index].blocks, &[], WhitespacePolicy::Default, false).1;
      (search + work.search, compared + u64::from(work.compared))
    });
    println!(
      "{name}: {} moves in {} files, {} blocks, {} pairs of lines read, {} compared by word",
      found.iter().sum::<usize>(),
      found.iter().filter(|moves| **moves > 0).count(),
      diffs.iter().map(|diff| diff.blocks.len()).sum::<usize>(),
      work.0,
      work.1
    );
    group.bench_function(name, |b| b.iter(|| (0..diffs.len()).map(|index| black_box(moves_of(index)).len()).sum::<usize>()));
    group.bench_function(format!("{name}, the diffs alone"), |b| {
      b.iter(|| {
        before
          .iter()
          .zip(&after)
          .map(|((_, old), (_, new))| black_box(diff_of(old, new)).blocks.len())
          .sum::<usize>()
      });
    });
  }
  group.finish();
}

/// The subjects through the service, the workers and the parses as the app runs them, to the first content and to
/// the end of the stream
fn bench_stream(c: &mut Criterion) {
  let mut group = c.benchmark_group("stream, 200 files");
  group.sample_size(20);
  for (name, sides) in subjects() {
    let (repo, commit) = subject(&sides);
    group.bench_function(format!("{name}, to the first content"), |b| {
      b.iter_custom(|iterations| (0..iterations).map(|_| stream(&repo, &commit).0).sum());
    });
    group.bench_function(format!("{name}, to the end"), |b| {
      b.iter_custom(|iterations| (0..iterations).map(|_| stream(&repo, &commit).1).sum());
    });
    // every pair in memory: the first content comes after the pass over the files of the subject
    let warm = service();
    stream_of(&warm, &repo, &commit);
    group.bench_function(format!("{name}, from memory, to the first content"), |b| {
      b.iter_custom(|iterations| (0..iterations).map(|_| stream_of(&warm, &repo, &commit).0).sum());
    });
    group.bench_function(format!("{name}, from memory, to the end"), |b| {
      b.iter_custom(|iterations| (0..iterations).map(|_| stream_of(&warm, &repo, &commit).1).sum());
    });
  }
  group.finish();
}

criterion_group!(
  benches,
  bench_tokens,
  bench_marks,
  bench_renames,
  bench_moves,
  bench_moves_limits,
  bench_moves_short,
  bench_limits,
  bench_subject_moves,
  bench_subject_pass,
  bench_stream
);
criterion_main!(benches);
