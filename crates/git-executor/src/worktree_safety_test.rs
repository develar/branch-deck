//! Branch Deck never writes to the user's working tree (docs/design/git-operations.md, invariant 1).
//! This test scans the production sources (`source_scan_test.rs`) for git argument lists whose subcommand checks out,
//! resets or commits, and for `read-tree` with `-u` or `--reset`. Test files and test-utility crates create fixture
//! repositories with porcelain commands and are exempt.
//! The one exception, Update Project, writes the working tree in `WORKTREE_WRITE_EXEMPTIONS` only, with exactly the
//! listed number of `read-tree -u` argument lists, so the exemption can't grow unnoticed.

use crate::source_scan_test::{argument_lists, production_part, production_sources};

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
      Self::Subcommand { line, subcommand } => write!(f, "line {line}: `{subcommand}`"),
      Self::ReadTreeUpdate { line } => write!(f, "line {line}: `read-tree -u` or `--reset`"),
    }
  }
}

/// `-u`, a bundle of short options with `u` (`-mu`), or `--reset`
fn updates_worktree(argument: &str) -> bool {
  argument == "--reset" || (argument.starts_with('-') && !argument.starts_with("--") && argument.contains('u'))
}

fn violations(source: &str) -> Vec<Violation> {
  let mut found = Vec::new();
  for list in argument_lists(production_part(source)) {
    let line = list.line;
    match list.subcommand() {
      Some(subcommand) if DENIED_SUBCOMMANDS.contains(&subcommand) => found.push(Violation::Subcommand {
        line,
        subcommand: subcommand.to_string(),
      }),
      Some("read-tree") if list.has(updates_worktree) => found.push(Violation::ReadTreeUpdate { line }),
      _ => {}
    }
  }
  found
}

#[test]
fn production_code_never_writes_the_working_tree() {
  let mut report = Vec::new();
  let mut exempt_found = vec![0; WORKTREE_WRITE_EXEMPTIONS.len()];
  for source in production_sources() {
    let exemption = WORKTREE_WRITE_EXEMPTIONS.iter().position(|(path, _)| source.path == *path);
    for violation in violations(&source.text) {
      match (exemption, &violation) {
        (Some(exemption), Violation::ReadTreeUpdate { .. }) => exempt_found[exemption] += 1,
        _ => report.push(format!("{}: {violation}", source.path)),
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
  assert_eq!(violations(r#"run(&["--no-pager", "reset"]); run(&["-C", dir, "checkout", "-"])"#).len(), 2);
  assert_eq!(violations(r#"run(&["commit-tree", "-p"]); run(&["merge-tree", "--write-tree"])"#), Vec::<Violation>::new());
  assert_eq!(violations("fn f() {}\n#[cfg(test)]\nmod tests {\n  run(&[\"checkout\"]);\n}"), Vec::<Violation>::new());
}
