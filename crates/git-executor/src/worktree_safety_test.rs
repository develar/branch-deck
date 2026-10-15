//! Branch Deck never writes to the user's working tree (docs/design/git-operations.md, invariant 1).
//! This test scans the production sources for git argument lists whose subcommand checks out, resets or commits, and
//! for `read-tree` with `-u` or `--reset`. Test files and test-utility crates create fixture repositories with porcelain
//! commands and are exempt.
//! The one exception, Update Project, writes the working tree in `WORKTREE_WRITE_EXEMPTIONS` only, with exactly the
//! listed number of `read-tree -u` argument lists, so the exemption can't grow unnoticed.

use std::fs;
use std::path::{Path, PathBuf};

/// Subcommands that write to the working tree or the user's index. `apply` is not listed: `apply --cached` is fine
const DENIED_SUBCOMMANDS: [&str; 14] = [
  "checkout",
  "checkout-index",
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

/// Files allowed to update the working tree with `read-tree -u`, with the number of such argument lists in each: the dry
/// run on a copy of the index and the update itself (docs/design/git-operations.md, "Update project")
const WORKTREE_WRITE_EXEMPTIONS: [(&str, usize); 1] = [("crates/git-ops/src/worktree_update.rs", 2)];

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

/// A git argument list that writes the working tree
#[derive(Debug, PartialEq, Eq)]
enum Violation {
  /// A denied subcommand
  Subcommand { line: usize, subcommand: String },
  /// `read-tree` with `-u` or `--reset`, which update the working tree
  ReadTreeUpdate { line: usize },
}

impl std::fmt::Display for Violation {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Violation::Subcommand { line, subcommand } => write!(f, "line {line}: `{subcommand}`"),
      Violation::ReadTreeUpdate { line } => write!(f, "line {line}: `read-tree -u` or `--reset`"),
    }
  }
}

/// The string literals of an argument list that starts at `text` (just after `[`), up to its closing `]`
fn literals_of(text: &str) -> Vec<&str> {
  let mut literals = Vec::new();
  let mut rest = text;
  while let Some(index) = rest.find(['"', ']']) {
    if rest.as_bytes()[index] == b']' {
      break;
    }
    let Some((literal, after)) = read_literal(&rest[index + 1..]) else { break };
    literals.push(literal);
    rest = after;
  }
  literals
}

/// `-u`, a bundle of short options with `u` (`-mu`), or `--reset`
fn updates_worktree(argument: &str) -> bool {
  argument == "--reset" || (argument.starts_with('-') && !argument.starts_with("--") && argument.contains('u'))
}

fn violations(source: &str) -> Vec<Violation> {
  let source = production_part(source);
  let mut found = Vec::new();
  for (index, _) in source.match_indices('[') {
    let Some(subcommand) = subcommand_of(&source[index + 1..]) else { continue };
    let line = source[..index].lines().count();
    if DENIED_SUBCOMMANDS.contains(&subcommand) {
      found.push(Violation::Subcommand {
        line,
        subcommand: subcommand.to_string(),
      });
    } else if subcommand == "read-tree" && literals_of(&source[index + 1..]).into_iter().any(updates_worktree) {
      found.push(Violation::ReadTreeUpdate { line });
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

  let mut report = Vec::new();
  let mut exempt_found = vec![0; WORKTREE_WRITE_EXEMPTIONS.len()];
  for file in &files {
    let source = fs::read_to_string(file).expect("readable source");
    let relative = file.strip_prefix(&workspace).unwrap_or(file);
    let exemption = WORKTREE_WRITE_EXEMPTIONS.iter().position(|(path, _)| relative == Path::new(path));
    for violation in violations(&source) {
      match (exemption, &violation) {
        (Some(exemption), Violation::ReadTreeUpdate { .. }) => exempt_found[exemption] += 1,
        _ => report.push(format!("{}: {violation}", file.display())),
      }
    }
  }
  for ((path, expected), found) in WORKTREE_WRITE_EXEMPTIONS.iter().zip(exempt_found) {
    assert_eq!(
      found, *expected,
      "{path} must have exactly {expected} `read-tree -u` argument lists; a change to the working-tree exemption needs a review of docs/design/git-operations.md"
    );
  }
  assert!(
    report.is_empty(),
    "git commands that write the working tree or the user's index (see docs/design/git-operations.md):\n{}",
    report.join("\n")
  );
}

#[test]
fn scanner_finds_read_tree_updates() {
  let read_tree_update = |source: &str| violations(source).into_iter().filter(|v| matches!(v, Violation::ReadTreeUpdate { .. })).count();
  assert_eq!(read_tree_update(r#"run(&["read-tree", "-m", "-u", old, new])"#), 1);
  assert_eq!(read_tree_update(r#"run(&["read-tree", "-mu", old, new])"#), 1);
  assert_eq!(read_tree_update(r#"run(&["read-tree", "--reset", "-u", tree])"#), 1);
  assert_eq!(read_tree_update("let args = [\n  \"read-tree\",\n  \"-m\",\n  \"-u\",\n];"), 1);
  // the two-way merge of the index only, and literals after the list ends
  assert_eq!(read_tree_update(r#"run(&["read-tree", "-m", "-i", old, new]); run(&["update-index", "-u"])"#), 0);
  assert_eq!(read_tree_update(r#"run(&["read-tree", "--no-recurse-submodules", "-m", old, new])"#), 0);
  assert_eq!(violations(r#"run(&["checkout-index", "-a"])"#).len(), 1);
}

#[test]
fn scanner_finds_denied_subcommands() {
  assert_eq!(violations(r#"run(&["reset", "--mixed"])"#).len(), 1);
  assert_eq!(violations("let args = vec![\n  \"commit\",\n  \"-a\",\n];").len(), 1);
  assert_eq!(violations(r#"run(&["-c", "x=y", "rebase", "--root"])"#).len(), 1);
  assert_eq!(violations(r#"run(&["commit-tree", "-p"]); run(&["merge-tree", "--write-tree"])"#), Vec::<Violation>::new());
  assert_eq!(violations("fn f() {}\n#[cfg(test)]\nmod tests {\n  run(&[\"checkout\"]);\n}"), Vec::<Violation>::new());
}
