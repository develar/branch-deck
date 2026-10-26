//! The git processes of the trial checks, by subcommand: the numbers behind the batching rule
//! (docs/design/git-operations.md, "Batch"), as `sync_core`'s `process_budget_test` pins those of the sync and the
//! rewrites. A change that runs fewer processes lowers the numbers here; one that runs more has to say why.

use crate::check::{CheckSession, check_plan};
use crate::landing_gate::landing_gate_test;
use crate::landing_gate::landing_needs_of;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::collections::{HashMap, HashSet};
use sync_core::sync::{SyncOptions, sync_branches};
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::{alpha_and_beta_on_origin, assert_processes, counting, empty_cache, processes, repo_on_origin};
use test_utils::sync_fixtures::TestReporter;

const PREFIX: &str = "test";

/// One sync of `repo`, which writes the copies the checks then reuse
async fn sync(repo: &TestRepo) {
  let git = GitCommandExecutor::new();
  sync_branches(&git, repo.path_str(), PREFIX, TestReporter::new(), SyncOptions::default()).await.unwrap();
}

/// `bd check` of the current grouping of `repo` on a counting executor, which it returns
fn counted_check(repo: &TestRepo, cache: &ObjectCache) -> GitCommandExecutor {
  let git = counting();
  check_plan(&git, repo.path_str(), PREFIX, None, cache).unwrap();
  git
}

/// `bd check` without a plan before the first sync, with an empty object cache and then with the one it filled
#[test]
fn a_check() {
  let (repo, _) = alpha_and_beta_on_origin();
  let (_dir, cache) = empty_cache();
  assert_processes(
    &counted_check(&repo, &cache),
    &[
      ("cat-file", 1),
      ("config", 1),
      ("fast-import", 2),
      ("for-each-ref", 1),
      ("log", 3),
      ("merge-tree", 2),
      ("remote", 1),
      ("rev-parse", 2),
    ],
  );

  assert_processes(
    &counted_check(&repo, &cache),
    &[
      ("cat-file", 2),
      ("config", 1),
      ("fast-import", 2),
      ("for-each-ref", 1),
      ("log", 2),
      ("remote", 1),
      ("rev-parse", 2),
    ],
  );
}

/// `bd check` after a sync, with the object cache that sync left empty: the copies of alpha and beta exist, and their
/// commits are read for reuse
#[tokio::test]
async fn a_check_of_synced_branches() {
  let (repo, _) = alpha_and_beta_on_origin();
  sync(&repo).await;
  let (_dir, cache) = empty_cache();
  assert_processes(
    &counted_check(&repo, &cache),
    &[("cat-file", 1), ("config", 1), ("for-each-ref", 1), ("log", 4), ("remote", 1), ("rev-parse", 2)],
  );
}

/// The landing needs of every branch after a sync, the fact the session of the repository computes: one session (the
/// snapshot, the config and the tips, as `bd check` reads them), one trial check of all branches, which reuses the copies
/// the sync wrote (no `fast-import`, no `merge-tree`), and the names the commits use and add, read once with the changed
/// lines; with the object cache the first filled, the second reads the changes and the summaries from it
#[tokio::test]
async fn the_landing_needs_of_every_branch() {
  let (repo, _) = landing_gate_test::repo();
  sync(&repo).await;
  let (_dir, cache) = empty_cache();
  let landing = |git: &GitCommandExecutor| {
    let needs = landing_needs_of(git, &cache, repo.path_str(), PREFIX, &HashSet::new()).unwrap();
    assert_eq!(needs.keys().collect::<Vec<_>>(), ["flow", "ui"]);
  };
  let git = counting();
  landing(&git);
  assert_processes(&git, &[("cat-file", 4), ("config", 1), ("for-each-ref", 1), ("log", 5), ("remote", 1), ("rev-parse", 2)]);
  let git = counting();
  landing(&git);
  assert_processes(&git, &[("cat-file", 2), ("config", 1), ("for-each-ref", 1), ("log", 3), ("remote", 1), ("rev-parse", 2)]);
}

/// A check session of `repo` on a counting executor
fn counted_session(repo: &TestRepo, cache: &ObjectCache) -> (GitCommandExecutor, CheckSession) {
  let git = counting();
  let session = CheckSession::open(&git, repo.path_str(), PREFIX, cache).unwrap();
  (git, session)
}

/// The processes of `run` on `git`, which may have counted others before
fn processes_of(git: &GitCommandExecutor, run: impl FnOnce()) -> Vec<(String, u32)> {
  let before: HashMap<String, u32> = processes(git).into_iter().collect();
  run();
  processes(git)
    .into_iter()
    .map(|(command, count)| (command.clone(), count - before.get(&command).copied().unwrap_or(0)))
    .filter(|(_, count)| *count > 0)
    .collect()
}

/// The checks of one session after a sync (the conflict fix tries many plans): the first reads the commits of the
/// existing copies, the next reuses them and the merges the first remembered
#[tokio::test]
async fn the_second_check_of_a_session() {
  let (repo, _) = alpha_and_beta_on_origin();
  sync(&repo).await;
  let (_dir, cache) = empty_cache();
  let (git, session) = counted_session(&repo, &cache);
  let first = processes_of(&git, || {
    session.check(None).unwrap();
  });
  assert_eq!(first, [("log".to_string(), 3)]);
  let second = processes_of(&git, || {
    session.check(None).unwrap();
  });
  assert_eq!(second, []);
  // a session of a rewrite (`bd check --fix` tries one) shares them, and the baseline commit
  let rewritten = session.with_rewrite(&[], &[]);
  let third = processes_of(&git, || {
    rewritten.check(None).unwrap();
  });
  assert_eq!(third, []);
}

/// The checks of a session on Kotlin sources, whose declaration searches read the summaries of the commits' files: the
/// first reads them once (the commits' changes with the blobs of their blocks, the trees above them, the blobs) with the
/// changed lines, which take the blobs of their blocks from the session's memory; the next
/// checks of the session, and of a session of a rewrite, which shares them, read none of them again: only the builds
/// (the copies, which no sync wrote) and the trees' differences and blobs of the search (`diff-tree`, `cat-file`)
#[test]
fn the_checks_of_a_session_on_sources() {
  let repo = repo_on_origin();
  let ledger = |modifier: &str| format!("package bridge\n\nclass Ledger {{\n  {modifier}fun cleanup() {{\n  }}\n}}\n");
  repo.create_commit_with_files(
    "Add the ledger",
    &[
      ("bridge/Ledger.kt", &ledger("")),
      (
        "contract/LedgerTest.kt",
        "package contract\n\nimport bridge.Ledger\n\nfun cleans(ledger: Ledger) {\n  ledger.cleanup()\n}\n",
      ),
    ],
  );
  repo.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
  repo.git(&["rm", "-q", "--", "contract/LedgerTest.kt"]);
  repo.git(&["commit", "-q", "-m", "(journey) The test goes"]);
  repo.create_commit_with_files("(suspend) The cleanup suspends", &[("bridge/Ledger.kt", &ledger("suspend "))]);
  let (_dir, cache) = empty_cache();
  let (git, session) = counted_session(&repo, &cache);
  let first = processes_of(&git, || {
    let report = session.check(None).unwrap();
    assert!(!report.builds_cleanly("suspend"), "{:?}", report.dangling);
  });
  assert_eq!(first, counts(&FIRST_SOURCES_CHECK));
  let second = processes_of(&git, || {
    session.check(None).unwrap();
  });
  let again = [("cat-file".to_string(), 2), ("diff-tree".to_string(), 1), ("fast-import".to_string(), 2)];
  assert_eq!(second, again);
  let rewritten = session.with_rewrite(&[], &[]);
  let third = processes_of(&git, || {
    rewritten.check(None).unwrap();
  });
  assert_eq!(third, again);
}

/// The processes of the first check of a branch that removes a declaration of a source file: the blobs of the search
/// and the summaries
const FIRST_SOURCES_CHECK: [(&str, u32); 5] = [("cat-file", 4), ("diff-tree", 1), ("fast-import", 2), ("log", 3), ("merge-tree", 1)];

/// `expected` as `processes_of` gives it
fn counts(expected: &[(&str, u32)]) -> Vec<(String, u32)> {
  expected.iter().map(|(command, count)| ((*command).to_string(), *count)).collect()
}

/// The check of a Rust item a branch removes, with the crates known: the line search reads the file it was removed from
/// for the line of the declaration (`declared_in`) in the same `cat-file` as the blobs of the search, and the crates
/// come with the summaries
#[test]
fn the_check_of_a_removed_rust_item() {
  let repo = repo_on_origin();
  repo.create_commit_with_files(
    "Add the crates",
    &[
      ("guest/Cargo.toml", "[package]\nname = \"guest\"\n"),
      ("guest/src/lib.rs", "pub fn collect() -> u32 {\n  1\n}\n"),
      ("guest/src/sum.rs", "pub fn total() -> u32 {\n  crate::collect()\n}\n"),
      ("vm/Cargo.toml", "[package]\nname = \"vm\"\n"),
      ("vm/src/lib.rs", "pub fn names() -> Vec<u32> {\n  (0..2).collect()\n}\n"),
    ],
  );
  repo.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
  repo.create_commit_with_files("(cleanup) Drop collect", &[("guest/src/lib.rs", "pub fn other() {}\n")]);
  repo.create_commit_with_files(
    "(vm) Count plainly",
    &[
      ("guest/src/sum.rs", "pub fn total() -> u32 {\n  1\n}\n"),
      ("vm/src/lib.rs", "pub fn names() -> Vec<u32> {\n  vec![0, 1]\n}\n"),
    ],
  );
  let (_dir, cache) = empty_cache();
  let (git, session) = counted_session(&repo, &cache);
  let processes = processes_of(&git, || {
    let report = session.check(None).unwrap();
    let found: Vec<(&str, Option<u32>)> = report.dangling["cleanup"].iter().map(|it| (it.file.as_str(), it.declared_in.line)).collect();
    assert_eq!(found, [("guest/src/sum.rs", Some(1))]);
  });
  assert_eq!(processes, counts(&FIRST_SOURCES_CHECK));
}
