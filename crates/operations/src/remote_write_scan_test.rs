//! Every write to a remote is an effect of an operation (docs/design/rust-domain-authority.md, s9-engine-remote,
//! "Критерий выхода"): the starts, restarts and cancels of Patronus robots, the `/patronus` comment, the writes of pull
//! requests, the state of a YouTrack issue and the leased push are called only from `crates/operations`, where the
//! admission, the witness and the outcome of an intent wrap them. Checked on the production sources, as the git usage
//! scan of git-executor (`git_usage_scan_test.rs`) checks git: each file that may call a write is named with its reason
//! and its exact number of calls, so a call elsewhere and a call gone both fail.

use std::fs;
use std::path::{Path, PathBuf};

/// A write and the files that call it: the files of `crates/operations` that run it as an effect, and the service whose
/// method sends the request of its client (its definition is no call)
struct Write {
  /// The call as the source writes it: a method (`.cancel_robot(`), or a function (`close_pull_request(`), after a
  /// character that neither a name nor a method call has before it
  call: &'static str,
  callers: &'static [(&'static str, usize, &'static str)],
}

const OPERATIONS: &str = "an effect of an operation";

const WRITES: &[Write] = &[
  Write {
    call: ".start_robot(",
    callers: &[
      ("crates/operations/src/robot_start.rs", 1, "the start of a Safe Push or a Dry Run through the API"),
      ("crates/patronus/src/actions.rs", 1, "`PatronusService::start_robot`, the request of its client"),
    ],
  },
  Write {
    call: ".restart_robot(",
    callers: &[
      ("crates/operations/src/land_execute.rs", 1, "the restart of a plan (`PlannedEffect::RestartRobot`)"),
      ("crates/patronus/src/actions.rs", 1, "`PatronusService::restart_robot`, the request of its client"),
    ],
  },
  Write {
    call: ".cancel_robot(",
    callers: &[
      (
        "crates/operations/src/robot.rs",
        1,
        "the cancel of a robot, one that finished as it ended (`robot::cancel`)",
      ),
      ("crates/patronus/src/actions.rs", 1, "`PatronusService::cancel_robot`, the request of its client"),
    ],
  },
  Write {
    call: "post_patronus_command(",
    callers: &[("crates/operations/src/robot_start.rs", 1, "the start as a `/patronus` comment")],
  },
  Write {
    call: "retarget_pull_request(",
    callers: &[("crates/operations/src/pull_request.rs", 1, OPERATIONS)],
  },
  Write {
    call: "close_pull_request(",
    callers: &[("crates/operations/src/close.rs", 1, OPERATIONS)],
  },
  Write {
    call: "submit_pull_request_review(",
    callers: &[("crates/operations/src/review.rs", 1, OPERATIONS)],
  },
  Write {
    call: "request_pull_request_reviewers(",
    callers: &[("crates/operations/src/land_execute.rs", 1, OPERATIONS)],
  },
  Write {
    call: "create_pull_request(",
    callers: &[("crates/operations/src/land_execute.rs", 1, "the creation of a plan (`PlannedEffect::CreatePullRequest`)")],
  },
  Write {
    call: ".set_state(",
    callers: &[("crates/operations/src/issue.rs", 1, "Mark Fixed and its undo (`Intent::SetIssueState`)")],
  },
  Write {
    call: ".push_leased(",
    callers: &[(
      "crates/operations/src/push.rs",
      2,
      "`operations::push::send`: the push, and once more over a remote that moved",
    )],
  },
];

#[test]
fn the_writes_of_the_remote_are_called_only_from_the_operations() {
  let sources = production_sources();
  let mut report = Vec::new();
  for write in WRITES {
    let mut counts = vec![0; write.callers.len()];
    for (path, text) in &sources {
      let lines = calls(text, write.call);
      match write.callers.iter().position(|(caller, _, _)| caller == path) {
        Some(index) => counts[index] += lines.len(),
        None => report.extend(lines.iter().map(|line| format!("  {}: {path}:{line}", write.call))),
      }
    }
    for ((path, hits, reason), count) in write.callers.iter().zip(counts) {
      if count != *hits {
        report.push(format!("  {}: {path}: {count} calls, but its entry says {hits} ({reason})", write.call));
      }
    }
  }
  assert!(
    report.is_empty(),
    "a write to a remote is called outside crates/operations, or an entry is stale:\n{}\nInstead: make it an effect of a plan \
     (`operations::engine::execute` of an intent), which admits it, checks its witness and tells its outcome",
    report.join("\n")
  );
}

/// The 1-based lines of `text` (not comments) that call `call`
fn calls(text: &str, call: &str) -> Vec<usize> {
  let starts_word = !call.starts_with('.');
  let mut lines = Vec::new();
  for (index, line) in text.lines().enumerate() {
    if line.trim_start().starts_with("//") {
      continue;
    }
    let found = line.match_indices(call).any(|(at, _)| {
      let before = &line[..at];
      // a function: not a part of a longer name, not a method of the same name
      let in_word = before.chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
      !starts_word || (!in_word && !before.trim_end().ends_with("fn"))
    });
    if found {
      lines.push(index + 1);
    }
  }
  lines
}

/// The production sources of the workspace by their path (`crates/operations/src/push.rs`): the `.rs` files of
/// `src-tauri` and of every crate but `test-utils`, without test files (`*_test.rs`, `tests/`), the modules a file
/// declares under `#[cfg(test)]` (`mod push_fixture;`) and its inline test modules
fn production_sources() -> Vec<(String, String)> {
  let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace directory");
  let mut files = Vec::new();
  rust_files(&workspace.join("src-tauri/src"), &mut files);
  for entry in fs::read_dir(workspace.join("crates")).expect("crates directory").flatten() {
    if entry.file_name() != "test-utils" {
      rust_files(&entry.path().join("src"), &mut files);
    }
  }
  let texts: Vec<(PathBuf, String)> = files
    .into_iter()
    .map(|file| {
      let text = fs::read_to_string(&file).expect("readable source");
      (file, text)
    })
    .collect();
  let test_modules: Vec<PathBuf> = texts.iter().flat_map(|(file, text)| test_module_files(file, text)).collect();
  let sources: Vec<(String, String)> = texts
    .into_iter()
    .filter(|(file, _)| !test_modules.contains(file))
    .map(|(file, text)| {
      (
        file.strip_prefix(&workspace).unwrap_or(&file).to_string_lossy().replace('\\', "/"),
        without_inline_tests(&text),
      )
    })
    .collect();
  assert!(sources.len() > 100, "the scan found too few sources, is the workspace layout different? {}", sources.len());
  sources
}

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
  let Ok(entries) = fs::read_dir(directory) else { return };
  for entry in entries.flatten() {
    let path = entry.path();
    let name = entry.file_name().to_string_lossy().into_owned();
    if path.is_dir() {
      if name != "tests" {
        rust_files(&path, files);
      }
    } else if path.extension().is_some_and(|extension| extension == "rs") && !name.ends_with("_test.rs") && !name.ends_with("_tests.rs") && name != "tests.rs" {
      files.push(path);
    }
  }
}

/// The files of the modules `text` declares under `#[cfg(test)]` without a body: `#[path = "x.rs"]` beside `file`, else
/// `<name>.rs` in the directory of its modules
fn test_module_files(file: &Path, text: &str) -> Vec<PathBuf> {
  let directory = file.parent().expect("a file in a directory");
  let stem = file.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default();
  let modules = if matches!(stem, "lib" | "main" | "mod") {
    directory.to_path_buf()
  } else {
    directory.join(stem)
  };
  let mut found = Vec::new();
  let mut lines = text.lines().map(str::trim);
  while let Some(line) = lines.next() {
    if line != "#[cfg(test)]" {
      continue;
    }
    let mut path = None;
    for next in lines.by_ref() {
      if let Some(value) = next.strip_prefix("#[path = \"").and_then(|rest| rest.strip_suffix("\"]")) {
        path = Some(directory.join(value));
      } else if !next.starts_with("#[") {
        let declaration = next.trim_start_matches("pub(crate) ").trim_start_matches("pub ");
        if let Some(name) = declaration.strip_prefix("mod ").and_then(|rest| rest.strip_suffix(';')) {
          found.push(path.take().unwrap_or_else(|| modules.join(format!("{name}.rs"))));
        }
        break;
      }
    }
  }
  found
}

/// `text` without its inline test modules (`#[cfg(test)] mod tests { … }`), each replaced by its line breaks so the
/// lines after it keep their numbers; the braces counted outside strings and line comments
fn without_inline_tests(text: &str) -> String {
  let mut kept = String::with_capacity(text.len());
  let mut rest = text;
  while let Some(at) = rest.find("#[cfg(test)]") {
    let after = &rest[at..];
    let Some(open) = after.find(['{', ';']).filter(|&open| after.as_bytes()[open] == b'{' && after[..open].contains("mod ")) else {
      kept.push_str(&rest[..at + "#[cfg(test)]".len()]);
      rest = &rest[at + "#[cfg(test)]".len()..];
      continue;
    };
    let end = open + closing_brace(&after[open..]).unwrap_or(after.len() - open - 1) + 1;
    kept.push_str(&rest[..at]);
    kept.extend(after[..end].matches('\n').map(|_| '\n'));
    rest = &after[end..];
  }
  kept.push_str(rest);
  kept
}

/// The index of the brace that closes the one `text` starts with, past string literals, braces as characters (`'{'`)
/// and line comments
fn closing_brace(text: &str) -> Option<usize> {
  let bytes = text.as_bytes();
  let (mut depth, mut index) = (0usize, 0);
  while index < bytes.len() {
    match bytes[index] {
      b'"' => index = quote_end(bytes, index),
      b'\'' if bytes.get(index + 2) == Some(&b'\'') => index += 2,
      b'/' if bytes.get(index + 1) == Some(&b'/') => index += text[index..].find('\n').unwrap_or(text.len() - index),
      b'{' => depth += 1,
      b'}' => {
        depth -= 1;
        if depth == 0 {
          return Some(index);
        }
      }
      _ => {}
    }
    index += 1;
  }
  None
}

/// The index of the quote that ends the string literal whose quote is at `open`, past its escapes
fn quote_end(bytes: &[u8], open: usize) -> usize {
  let escaped = |at: usize| bytes[..at].iter().rev().take_while(|byte| **byte == b'\\').count() % 2 == 1;
  (open + 1..bytes.len()).find(|&at| bytes[at] == b'"' && !escaped(at)).unwrap_or(bytes.len())
}
