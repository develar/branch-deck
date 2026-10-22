//! A thread starts without a span, so a closure or a future handed to one would log its lines outside the operation of
//! its caller (`operation{operation_id=…}` of a `bd` run or a Tauri command, docs/design/async-patterns.md#operation-spans).
//! Every spawn of the production sources carries the caller's span: a closure through `sync_utils::span::in_current_span`,
//! a future through `tracing::Instrument::in_current_span`. Like the git scans, a file is exempt only with its reason and
//! its exact number of hits.

use crate::source_scan_test::{Exemption, Source, check};

/// What hands work to another thread, each with the index of its `(`: a closure to a blocking or scoped thread, a future
/// to `tokio::spawn` or a `JoinSet`
const SPAWNS: [(&str, usize); 4] = [("spawn_blocking(", 14), ("thread::spawn(", 13), ("scope.spawn(", 11), ("spawn(async", 5)];

const EXEMPT: [Exemption; 7] = [
  Exemption {
    path: "crates/git-executor/src/git_command_executor.rs",
    hits: 3,
    reason: "the pipe threads of a git process log nothing",
  },
  Exemption {
    path: "crates/git-executor/src/interactive_command.rs",
    hits: 2,
    reason: "the pipe threads of a git process log nothing",
  },
  Exemption {
    path: "crates/test-server/src/lib.rs",
    hits: 3,
    reason: "the tasks of the mock server itself: its handler, the templates, the deletion of a repository",
  },
  Exemption {
    path: "crates/test-server/src/test_support.rs",
    hits: 1,
    reason: "the mock server started for a test",
  },
  Exemption {
    path: "crates/test-server/src/tauri_command_bridge.rs",
    hits: 1,
    reason: "the mock server runs a command outside an operation",
  },
  Exemption {
    path: "crates/github/src/fake_github.rs",
    hits: 1,
    reason: "a fake server of the tests",
  },
  Exemption {
    path: "crates/patronus/src/fake_patronus.rs",
    hits: 1,
    reason: "a fake server of the tests",
  },
];

/// The argument text of the call that opens at `open` (the index of its `(`), up to its closing parenthesis; string and
/// character literals are skipped
fn argument_of(text: &str, open: usize) -> &str {
  let bytes = text.as_bytes();
  let mut depth = 0usize;
  let mut index = open;
  while index < bytes.len() {
    match bytes[index] {
      b'"' => {
        index += 1;
        while index < bytes.len() && bytes[index] != b'"' {
          index += if bytes[index] == b'\\' { 2 } else { 1 };
        }
      }
      b'\'' if index + 2 < bytes.len() && (bytes[index + 2] == b'\'' || (bytes[index + 1] == b'\\' && bytes.get(index + 3) == Some(&b'\''))) => {
        index += if bytes[index + 1] == b'\\' { 3 } else { 2 };
      }
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return text[open + 1..index].trim();
        }
      }
      _ => {}
    }
    index += 1;
  }
  panic!("no closing parenthesis for the call at {open}");
}

/// The lines of `source` whose spawn carries no span of the caller
fn unspanned_spawns(source: &Source) -> Vec<usize> {
  let mut lines = Vec::new();
  for (line_number, line) in source.code_lines() {
    let Some(column) = SPAWNS.iter().filter_map(|(spawn, open)| line.find(spawn).map(|at| at + open)).min() else {
      continue;
    };
    let line_start = source.text.lines().take(line_number - 1).map(|line| line.len() + 1).sum::<usize>();
    let argument = argument_of(&source.text, line_start + column);
    if !(argument.starts_with("in_current_span(") || argument.ends_with(".in_current_span()")) {
      lines.push(line_number);
    }
  }
  lines
}

#[test]
fn every_closure_or_future_handed_to_another_thread_carries_the_span_of_its_caller() {
  check(
    "a thread starts without a span, so these spawns lose the operation of their caller",
    "wrap the closure in `sync_utils::span::in_current_span(…)`, or end the future with `.in_current_span()` (`tracing::Instrument`)",
    &EXEMPT,
    unspanned_spawns,
  );
}

#[test]
fn the_scan_reads_the_argument_of_a_spawn_past_strings_and_nested_calls() {
  let source = Source {
    path: "x.rs".to_owned(),
    text: [
      "tasks.spawn_blocking(in_current_span(move || run(\")\", '(')));",
      "tasks.spawn_blocking(move || run(\")\"));",
      "set.spawn(async move { get(\"(\").await }.in_current_span());",
      "set.spawn(async move { get(\"(\").await });",
      "std::thread::spawn(in_current_span(|| ()));",
    ]
    .join("\n"),
  };
  assert_eq!(unspanned_spawns(&source), vec![2, 4]);
}
