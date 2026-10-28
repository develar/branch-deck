//! Branch Deck never writes to the user's working tree (docs/design/git-operations.md, invariant 1).
//! This test scans the production sources (`source_scan_test.rs`) for git argument lists whose subcommand checks out,
//! resets or commits, and for `read-tree` with `-u` or `--reset`. Test files and test-utility crates create fixture
//! repositories with porcelain commands and are exempt.
//! The one exception, Update Project, writes the working tree in `WORKTREE_WRITE_EXEMPTIONS` only, with exactly the
//! listed number of `read-tree -u` argument lists, so the exemption can't grow unnoticed; the moves of HEAD that the
//! files follow are called only from `MOVE_CALLERS`, each with its count. A slot of the lease pool is not the user's
//! working tree: `SLOT_WRITE_EXEMPTIONS` allows its writes in one file, each subcommand with its count. `apply`
//! writes the files unless it has `--cached` or only reads the patch (`--numstat`, `--stat`, `--check`): the one apply
//! to the user's working tree is `CLIENT_WRITE_EXEMPTIONS`, the changes of a remote run the user asked for.

use crate::source_scan_test::{argument_lists, production_part, production_sources};

/// Subcommands that write to the working tree or the user's index. `apply` is not listed: `apply --cached` is fine, and
/// a writing `apply` is `Violation::Apply`
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

/// The functions of `update_branch.rs` that move HEAD and let the files follow through `worktree_update.rs`
const MOVE_FUNCTIONS: [&str; 6] = [
  "move_branch_and_files(",
  "move_branch_and_files_carrying(",
  "move_branch_and_files_with(",
  "move_with_index_lock(",
  "files_follow(",
  "apply_move_undo_with(",
];

/// The files that name `MOVE_FUNCTIONS` (declarations and calls), with their number of uses: Update Project and its
/// undo, the finish of a resolve session, `bd pick`, the unapply and the reapply of a virtual branch and their undo
/// (docs/design/git-operations.md, invariant 1)
const MOVE_CALLERS: [(&str, usize); 5] = [
  ("crates/git-ops/src/update_branch.rs", 13),
  ("crates/git-ops/src/update_resolve.rs", 2),
  ("crates/git-ops/src/pick.rs", 2),
  ("crates/git-ops/src/unapply.rs", 2),
  ("crates/git-ops/src/reapply.rs", 1),
];

/// The file of the lease pool that writes a slot, pool property (docs/design/git-operations.md, "Lease"), with the number of
/// argument lists of each subcommand: the checkout of every file of a new slot (detached, or on a new branch), the move
/// of a clean slot by the diff (`reset --merge`), the clean of a cloned slot, and the discard of a return that the agent
/// confirmed or of a give-back (`reset --hard`, `clean`)
const SLOT_WRITE_EXEMPTIONS: [(&str, &str, usize); 3] = [
  ("crates/git-ops/src/lease_slot.rs", "checkout", 2),
  ("crates/git-ops/src/lease_slot.rs", "reset", 2),
  ("crates/git-ops/src/lease_slot.rs", "clean", 2),
];

/// The file that applies the patch of the changes of a remote run to the user's working tree, on the user's request
/// (`bd-host run --apply`, docs/design/git-operations.md, "Host slots"), with the number of its argument lists
const CLIENT_WRITE_EXEMPTIONS: [(&str, &str, usize); 1] = [("crates/git-ops/src/host_client.rs", "apply", 1)];

/// The options of an `apply` that only reads the patch or writes the index alone
const APPLY_READS: [&str; 4] = ["--cached", "--numstat", "--stat", "--check"];

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
  let text = production_part(source);
  for list in argument_lists(&text) {
    let line = list.line;
    match list.subcommand() {
      Some(subcommand) if DENIED_SUBCOMMANDS.contains(&subcommand) => found.push(Violation::Subcommand {
        line,
        subcommand: subcommand.to_string(),
      }),
      Some("read-tree") if list.has(updates_worktree) => found.push(Violation::ReadTreeUpdate { line }),
      Some("apply") if !list.has(|argument| APPLY_READS.contains(&argument)) => found.push(Violation::Subcommand {
        line,
        subcommand: "apply".to_string(),
      }),
      _ => {}
    }
  }
  found
}

#[test]
fn production_code_never_writes_the_working_tree() {
  let mut report = Vec::new();
  let mut exempt_found = vec![0; WORKTREE_WRITE_EXEMPTIONS.len()];
  let mut slot_found = vec![0; SLOT_WRITE_EXEMPTIONS.len()];
  let mut client_found = vec![0; CLIENT_WRITE_EXEMPTIONS.len()];
  for source in production_sources() {
    let exemption = WORKTREE_WRITE_EXEMPTIONS.iter().position(|(path, _)| source.path == *path);
    for violation in violations(&source.text) {
      let exempt_in = |exemptions: &[(&str, &str, usize)]| match &violation {
        Violation::Subcommand { subcommand, .. } => exemptions.iter().position(|(path, exempt, _)| source.path == *path && subcommand == exempt),
        Violation::ReadTreeUpdate { .. } => None,
      };
      let (slot_write, client_write) = (exempt_in(&SLOT_WRITE_EXEMPTIONS), exempt_in(&CLIENT_WRITE_EXEMPTIONS));
      match (exemption, slot_write, client_write, &violation) {
        (_, Some(slot_write), _, _) => slot_found[slot_write] += 1,
        (_, _, Some(client_write), _) => client_found[client_write] += 1,
        (Some(exemption), _, _, Violation::ReadTreeUpdate { .. }) => exempt_found[exemption] += 1,
        _ => report.push(format!("{}: {violation}", source.path)),
      }
    }
  }
  for ((path, subcommand, expected), found) in SLOT_WRITE_EXEMPTIONS.iter().zip(slot_found) {
    assert_eq!(
      found, *expected,
      "{path} must have exactly {expected} `{subcommand}` argument lists; a change to the slot writes needs a review of docs/design/git-operations.md"
    );
  }
  for ((path, subcommand, expected), found) in CLIENT_WRITE_EXEMPTIONS.iter().zip(client_found) {
    assert_eq!(
      found, *expected,
      "{path} must have exactly {expected} `{subcommand}` argument lists; another write of the user's working tree needs a review of docs/design/git-operations.md"
    );
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
fn only_the_named_files_move_head_with_the_files() {
  let mut found = Vec::new();
  for source in production_sources() {
    let uses: usize = source
      .code_lines()
      .map(|(_, line)| MOVE_FUNCTIONS.iter().map(|function| line.matches(function).count()).sum::<usize>())
      .sum();
    if uses > 0 {
      found.push((source.path, uses));
    }
  }
  found.sort();
  let mut expected: Vec<(String, usize)> = MOVE_CALLERS.iter().map(|(path, uses)| ((*path).to_string(), *uses)).collect();
  expected.sort();
  assert_eq!(
    found, expected,
    "a new caller of a move that the files follow needs a review of docs/design/git-operations.md (invariant 1) and AGENTS.md"
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
  assert_eq!(violations(r#"run(&["apply", "--whitespace=nowarn", patch])"#).len(), 1);
  assert_eq!(
    violations(r#"run(&["apply", "--cached", patch]); run(&["apply", "--numstat", patch])"#),
    Vec::<Violation>::new()
  );
}
