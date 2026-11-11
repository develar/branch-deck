//! What the marks of a diff cost (docs/design/diff-view.md, "Marks"): the tokens of a text, the rule of the tokens,
//! `marks_of` of a pair, and a subject of 200 files through the service, to its first content and to the end of its
//! stream. `cargo bench -p file-diffs --bench marks_bench`, on a quiet machine.
//!
//! The pair is two states of a Kotlin file of 3,300 lines, 300 functions of which every fifth changed:
//! - `formatted`: a call wrapped over three lines and a comment worded again, so every changed block is a run and both
//!   sides are parsed;
//! - `changed`: a name changed, so no block may hold a run and nothing is parsed;
//! - `indented`: every function indented again, a name changed in the first: 300 blocks, one of them with a line to
//!   read beside its run.

use code_symbols::highlight::Grammar;
use code_symbols::tokens::{Tokens, compare, equal_by_tokens, tokens};
use criterion::{Criterion, criterion_group, criterion_main};
use file_diffs::marks::{MAX_BLOCK_TOKENS, MAX_TOKEN_EDITS, marks_of};
use file_diffs::model::{DiffSource, FileDiffEvent, GetFileDiffsParams};
use file_diffs::service::{DiffLimits, FileDiffs};
use object_cache::cache::ObjectCache;
use std::fmt::Write as _;
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};
use test_utils::git_test_utils::TestRepo;
use text_diff::model::{DiffBudget, TextDiff, diff_texts};
use text_diff::policy::WhitespacePolicy;

/// How a function of the generated file differs from its first state
#[derive(Clone, Copy, PartialEq, Eq)]
enum Edit {
  None,
  /// A call wrapped inside its brackets, and its comment worded again
  Formatted,
  /// A name changed
  Changed,
}

/// A Kotlin class of `functions` functions of eleven lines, `edit` saying how each differs
fn source(name: &str, functions: usize, indent: &str, edit: impl Fn(usize) -> Edit) -> String {
  let mut text = format!("package bench\n\nclass {name}(private val repository: Repository) {{\n");
  for index in 0..functions {
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
    writeln!(text, "{indent}    val open = items.filter {{ it.state == State.OPEN }}").unwrap();
    writeln!(text, "{indent}    if (open.isEmpty()) {{").unwrap();
    writeln!(text, "{indent}      return Response.empty(request.id)").unwrap();
    writeln!(text, "{indent}    }}").unwrap();
    let amount = if edit == Edit::Changed { "price" } else { "amount" };
    writeln!(text, "{indent}    val total = open.sumOf {{ it.{amount} }}").unwrap();
    writeln!(text, "{indent}    return Response(request.id, open, total)").unwrap();
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
  // every function indented again, a name changed in the first
  let indented = source("Service", 300, "  ", |index| if index == 0 { Edit::Changed } else { Edit::None });

  let mut group = c.benchmark_group("marks_of");
  group.bench_function("the diff alone, formatted", |b| b.iter(|| diff_of(black_box(&old), black_box(&formatted))));
  for (name, new) in [("formatted", &formatted), ("changed", &changed), ("indented", &indented)] {
    let diff = diff_of(&old, new);
    let (old_tokens, new_tokens) = (kotlin(&old), kotlin(new));
    let kept = |new_side: bool| Some(Arc::clone(if new_side { &new_tokens } else { &old_tokens }));
    let marks = marks_of(&diff, &old, new, Some(Grammar::Kotlin), kept);
    println!("{name}: {} blocks, {} runs, {} blocks to read", diff.blocks.len(), marks.runs.len(), marks.starts.len() / 3);
    // the tokens of both sides read for the marks: what a file costs before its content goes out
    group.bench_function(format!("{name}, tokens read"), |b| {
      b.iter(|| marks_of(&diff, &old, new, Some(Grammar::Kotlin), |new_side| Some(kotlin(if new_side { new } else { &old }))));
    });
    // the tokens in memory already: the question before the tokens and the runs alone
    group.bench_function(format!("{name}, tokens kept"), |b| b.iter(|| marks_of(&diff, &old, new, Some(Grammar::Kotlin), kept)));
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
    let marks = marks_of(&diff, &old, &new, Some(Grammar::Kotlin), kept);
    println!("{name}: {} blocks, {} tokens a side, {} runs", diff.blocks.len(), old_tokens.all().len(), marks.runs.len());
    group.bench_function(name, |b| b.iter(|| marks_of(&diff, &old, &new, Some(Grammar::Kotlin), kept)));
  }
  group.finish();
}

/// A commit of 200 Kotlin files, four of 3,300 lines and the others of 165, every fifth function of each changed as
/// `edit` says
fn subject(edit: Edit) -> (TestRepo, String) {
  let files = |edit: Edit| -> Vec<(String, String)> {
    (0..200)
      .map(|index| {
        let functions = if index % 50 == 0 { 300 } else { 15 };
        (format!("src/bench/Service{index}.kt"), source(&format!("Service{index}"), functions, "", every_fifth(edit)))
      })
      .collect()
  };
  let repo = TestRepo::new();
  let (before, after) = (files(Edit::None), files(edit));
  repo.create_commit_with_files("Initial", &pairs(&before));
  let commit = repo.create_commit_with_files("Change", &pairs(&after));
  (repo, commit)
}

fn pairs(files: &[(String, String)]) -> Vec<(&str, &str)> {
  files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect()
}

/// The time of one stream of the subject by a service with empty caches: to its first content, and to its end
fn stream(repo: &TestRepo, commit: &str) -> (Duration, Duration) {
  let diffs = FileDiffs::new(DiffLimits {
    file: Duration::from_secs(60),
    lines: Duration::from_secs(60),
  });
  let params = GetFileDiffsParams {
    repository_path: repo.path().to_str().expect("a UTF-8 path").to_string(),
    source: DiffSource::Commit { id: commit.to_string() },
    policy: WhitespacePolicy::Default,
    priority_paths: Vec::new(),
  };
  let start = Instant::now();
  let mut first = None;
  diffs
    .stream(repo.git_executor(), &ObjectCache::disabled(), &params, |event| {
      if matches!(event, FileDiffEvent::Content { .. }) {
        first.get_or_insert_with(|| start.elapsed());
      }
    })
    .expect("the diffs of the subject");
  (first.expect("a content"), start.elapsed())
}

/// The subject through the service, the workers and the parses as the app runs them. `changed` parses no file for its
/// marks (the question before the tokens answers no for every block), `formatted` parses both sides of every file:
/// the difference is what the marks cost when nothing is left out before the tokens
fn bench_stream(c: &mut Criterion) {
  let mut group = c.benchmark_group("stream, 200 files");
  group.sample_size(20);
  for (name, edit) in [("changed", Edit::Changed), ("formatted", Edit::Formatted)] {
    let (repo, commit) = subject(edit);
    group.bench_function(format!("{name}, to the first content"), |b| {
      b.iter_custom(|iterations| (0..iterations).map(|_| stream(&repo, &commit).0).sum());
    });
    group.bench_function(format!("{name}, to the end"), |b| {
      b.iter_custom(|iterations| (0..iterations).map(|_| stream(&repo, &commit).1).sum());
    });
  }
  group.finish();
}

criterion_group!(benches, bench_tokens, bench_marks, bench_limits, bench_stream);
criterion_main!(benches);
