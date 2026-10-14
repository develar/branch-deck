//! Branch Deck never writes to the user's working tree (docs/design/git-operations.md, invariant 1).
//! This test scans the production sources for git argument lists whose subcommand checks out, resets or commits.
//! Test files and test-utility crates create fixture repositories with porcelain commands and are exempt.

use std::fs;
use std::path::{Path, PathBuf};

/// Subcommands that write to the working tree or the user's index. `apply` is not listed: `apply --cached` is fine
const DENIED_SUBCOMMANDS: [&str; 13] = [
  "checkout",
  "switch",
  "reset",
  "rebase",
  "stash",
  "commit",
  "cherry-pick",
  "merge",
  "restore",
  "clean",
  "am",
  "revert",
  "history",
];

/// Crates that only build fixture repositories for tests
const EXEMPT_CRATES: [&str; 3] = ["test-utils", "sync-test-utils", "integration-detection-tests"];

fn is_test_file(path: &Path) -> bool {
  let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
  name.ends_with("_test.rs") || name.ends_with("_tests.rs") || name == "tests.rs" || path.components().any(|c| c.as_os_str() == "tests")
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
  let Ok(entries) = fs::read_dir(dir) else { return };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      collect_rust_files(&path, files);
    } else if path.extension().is_some_and(|e| e == "rs") && !is_test_file(&path) {
      files.push(path);
    }
  }
}

/// Source without inline `#[cfg(test)] mod …` blocks (by convention they are the last item of a file)
fn production_part(source: &str) -> &str {
  let mut offset = 0;
  while let Some(index) = source[offset..].find("#[cfg(test)]") {
    let start = offset + index;
    let rest = source[start + "#[cfg(test)]".len()..].trim_start();
    if rest.starts_with("mod ") {
      return &source[..start];
    }
    offset = start + 1;
  }
  source
}

/// Read a string literal starting right after its opening quote; returns the literal and the rest
fn read_literal(text: &str) -> Option<(&str, &str)> {
  let end = text.find('"')?;
  Some((&text[..end], &text[end + 1..]))
}

/// The git subcommand of an argument list literal that starts at `text` (just after `[`), skipping `-c key=value` pairs
fn subcommand_of(mut text: &str) -> Option<&str> {
  loop {
    text = text.trim_start().strip_prefix('"')?;
    let (literal, rest) = read_literal(text)?;
    if literal != "-c" {
      return Some(literal);
    }
    // skip `, "key=value",`
    let rest = rest.trim_start().strip_prefix(',')?.trim_start().strip_prefix('"')?;
    let (_, rest) = read_literal(rest)?;
    text = rest.trim_start().strip_prefix(',')?;
  }
}

fn violations(source: &str) -> Vec<String> {
  let source = production_part(source);
  let mut found = Vec::new();
  for (index, _) in source.match_indices('[') {
    let Some(subcommand) = subcommand_of(&source[index + 1..]) else { continue };
    if DENIED_SUBCOMMANDS.contains(&subcommand) {
      let line = source[..index].lines().count();
      found.push(format!("line {line}: `{subcommand}`"));
    }
  }
  found
}

#[test]
fn production_code_never_writes_the_working_tree() {
  let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace directory");
  let mut files = Vec::new();
  collect_rust_files(&workspace.join("src-tauri/src"), &mut files);
  for entry in fs::read_dir(workspace.join("crates")).expect("crates directory").flatten() {
    let name = entry.file_name();
    if !EXEMPT_CRATES.iter().any(|exempt| name == *exempt) {
      collect_rust_files(&entry.path().join("src"), &mut files);
    }
  }
  assert!(files.len() > 20, "the scan found too few sources, is the workspace layout different? {}", files.len());

  let report: Vec<String> = files
    .iter()
    .flat_map(|file| {
      let source = fs::read_to_string(file).expect("readable source");
      violations(&source).into_iter().map(move |v| format!("{}: {v}", file.display()))
    })
    .collect();
  assert!(
    report.is_empty(),
    "git commands that write the working tree or the user's index (see docs/design/git-operations.md):\n{}",
    report.join("\n")
  );
}

#[test]
fn scanner_finds_denied_subcommands() {
  assert_eq!(violations(r#"run(&["reset", "--mixed"])"#).len(), 1);
  assert_eq!(violations("let args = vec![\n  \"commit\",\n  \"-a\",\n];").len(), 1);
  assert_eq!(violations(r#"run(&["-c", "x=y", "rebase", "--root"])"#).len(), 1);
  assert_eq!(violations(r#"run(&["commit-tree", "-p"]); run(&["merge-tree", "--write-tree"])"#), Vec::<String>::new());
  assert_eq!(violations("fn f() {}\n#[cfg(test)]\nmod tests {\n  run(&[\"checkout\"]);\n}"), Vec::<String>::new());
}
