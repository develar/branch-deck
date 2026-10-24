//! On Windows a removal or a rename fails while another process holds what it changes: a git or a shell whose current
//! directory is in it, a scanner or an editor with a file of it open (a sharing error). Every removal and rename of the
//! production sources goes through `sync_utils::fs_retry`, which waits such a holder out for about a second, so no
//! caller is left to fail on the first try, or to delete half a directory. A call is `fs::remove_dir_all(`,
//! `fs::remove_dir(`, `fs::remove_file(` or `fs::rename(` of `std::fs` (as `std::fs::…` or after `use std::fs`); like
//! the git scans, a file is exempt only with its reason and its exact number of hits.

use crate::source_scan_test::{Exemption, Source, check};

/// The calls of `std::fs` that a holder fails on Windows
const CALLS: [&str; 4] = ["fs::remove_dir_all(", "fs::remove_dir(", "fs::remove_file(", "fs::rename("];

const EXEMPT: [Exemption; 2] = [
  Exemption {
    path: "crates/sync-utils/src/fs_retry.rs",
    hits: 4,
    reason: "the helper itself",
  },
  Exemption {
    path: "crates/object-cache/src/semaphore_key.rs",
    hits: 1,
    reason: "macOS only (the System V semaphore of an LMDB lock file), where no holder fails a removal",
  },
];

/// The lines of `source` with a call of `CALLS` of `std::fs`: not one of another module (`fs_retry::remove_file(`)
fn unretried_calls(source: &Source) -> Vec<usize> {
  source
    .code_lines()
    .filter(|(_, line)| {
      CALLS.iter().any(|call| {
        line.match_indices(call).any(|(at, _)| {
          let before = &line[..at];
          before.ends_with("std::") || before.chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == ':'))
        })
      })
    })
    .map(|(number, _)| number)
    .collect()
}

#[test]
fn every_removal_and_rename_waits_out_a_holder_on_windows() {
  check(
    "a removal or a rename of std::fs fails at once on Windows while a process holds what it changes",
    "call the same function of `sync_utils::fs_retry`",
    &EXEMPT,
    unretried_calls,
  );
}

#[test]
fn the_scan_tells_a_call_of_std_fs_from_one_of_another_module() {
  let source = Source {
    path: "x.rs".to_owned(),
    text: [
      "std::fs::remove_dir_all(&dir)?;",
      "let _ = fs::remove_file(path);",
      "fs_retry::remove_file(path)?;",
      "sync_utils::fs_retry::rename(from, to)?;",
      "crate::fs_retry::remove_dir(path)?;",
      "// std::fs::remove_file(path) in a comment",
      "if is_dir { std::fs::remove_dir(path) } else { fs_retry::remove_file(path) }",
    ]
    .join("\n"),
  };
  assert_eq!(unretried_calls(&source), [1, 2, 7]);
}
