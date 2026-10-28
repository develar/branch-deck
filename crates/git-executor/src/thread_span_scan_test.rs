//! A thread starts without a span, so a closure or a future handed to one would log its lines outside the operation of
//! its caller (`operation{operation_id=…}` of a `bd` run or a Tauri command, docs/design/async-patterns.md#operation-spans).
//! Every spawn of the production sources carries the caller's span: a closure through `sync_utils::span::in_current_span`,
//! a future through `tracing::Instrument::in_current_span`. A spawn is every call named `spawn` or `spawn_blocking` that
//! has an argument, whichever receiver or path it has (`tokio::spawn`, `thread::spawn`, `tauri::async_runtime::spawn`, a
//! `JoinSet`, a scope, a `Builder`), its argument read across lines; `Command::spawn()` has none. Like the git scans, a
//! file is exempt only with its reason and its exact number of hits.

use crate::source_scan_test::{Exemption, Source, check};

/// The heads of a spawn call
const SPAWNS: [&str; 2] = ["spawn_blocking(", "spawn("];

const EXEMPT: [Exemption; 7] = [
  Exemption {
    path: "crates/git-executor/src/git_command_executor.rs",
    hits: 5,
    reason: "the pipe threads of a git process log nothing (3); `GitCommand::spawn(bool)` starts the process, no thread (2)",
  },
  Exemption {
    path: "crates/git-executor/src/interactive_command.rs",
    hits: 2,
    reason: "the pipe threads of a git process log nothing",
  },
  Exemption {
    path: "crates/operations/src/registry_file.rs",
    hits: 1,
    reason: "the heartbeat thread of the operations registry serves every operation of the process, no caller's",
  },
  Exemption {
    path: "crates/sync-core/src/sync.rs",
    hits: 4,
    reason: "`SyncTask::spawn` runs its task through `spawn_blocking(in_current_span(…))` itself",
  },
  Exemption {
    path: "crates/test-server/src/lib.rs",
    hits: 2,
    reason: "the tasks of the mock server itself: its handler, the deletion of a repository",
  },
  Exemption {
    path: "crates/test-server/src/test_support.rs",
    hits: 1,
    reason: "the mock server started for a test",
  },
  Exemption {
    path: "crates/github/src/fake_github.rs",
    hits: 1,
    reason: "a fake server of the tests",
  },
];

/// The index in `line` of the `(` of every spawn call on it: a head of `SPAWNS` that is a name of its own (not `respawn(`)
/// and no declaration (`fn spawn(`)
fn spawn_calls(line: &str) -> impl Iterator<Item = usize> + '_ {
  SPAWNS.iter().flat_map(move |head| {
    line.match_indices(head).filter_map(move |(at, _)| {
      let before = &line[..at];
      let own_name = before.chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
      (own_name && !before.ends_with("fn ")).then_some(at + head.len() - 1)
    })
  })
}

/// The argument text of the call that opens at `open` (the index of its `(`), up to its closing parenthesis, without a
/// trailing comma; string and character literals are skipped
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
          return text[open + 1..index].trim().trim_end_matches(',').trim_end();
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
  let mut offset = 0;
  for (index, segment) in source.text.split_inclusive('\n').enumerate() {
    let line = segment.trim_end_matches(['\n', '\r']);
    if !line.trim_start().starts_with("//") {
      for column in spawn_calls(line) {
        let argument = argument_of(&source.text, offset + column);
        let spanned = argument.is_empty() || argument.starts_with("in_current_span(") || argument.ends_with(".in_current_span()");
        if !spanned {
          lines.push(index + 1);
        }
      }
    }
    offset += segment.len();
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
fn the_scan_reads_the_argument_of_a_spawn_past_strings_nested_calls_and_line_breaks() {
  let source = Source {
    path: "x.rs".to_owned(),
    text: [
      "tasks.spawn_blocking(in_current_span(move || run(\")\", '(')));",
      "tasks.spawn_blocking(move || run(\")\"));",
      "set.spawn(async move { get(\"(\").await }.in_current_span());",
      "set.spawn(async move { get(\"(\").await });",
      "std::thread::spawn(in_current_span(|| ()));",
      // a future as rustfmt lays a long one out: the argument on its own lines, a trailing comma
      "pending.spawn(",
      "  async move { get().await }",
      "  .in_current_span(),",
      ");",
      "pending.spawn(",
      "  async move { get().await },",
      ");",
      // a named future or closure, whichever path or receiver spawns it
      "tokio::spawn(work);",
      "tauri::async_runtime::spawn(task);",
      "std::thread::scope(|s| s.spawn(|| run()));",
      "std::thread::Builder::new().name(name).spawn(|| run())?;",
      // no spawn of a thread: a process, a declaration, another name
      "let child = command.spawn()?;",
      "pub fn spawn(&self, pipe_stdin: bool) -> Result<Spawned> {",
      "respawn(worker);",
    ]
    .join("\n"),
  };
  assert_eq!(unspanned_spawns(&source), vec![2, 4, 10, 13, 14, 15, 16]);
}
