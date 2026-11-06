//! The counts of the process budget tests (docs/design/git-operations.md, "Batch"): an executor that counts the git
//! processes it starts, the assert of those counts by subcommand, the git processes of a whole test process (fixtures
//! included), the small fixed repositories the budgets count on and an empty object cache.

use crate::env_vars::EnvVars;
use crate::git_test_utils::TestRepo;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::{CacheConfig, ObjectCache};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

/// An executor that counts every git process it starts
#[must_use]
pub fn counting() -> GitCommandExecutor {
  GitCommandExecutor::new().with_process_stats("test")
}

/// The processes `git` counted, by subcommand, sorted by name
///
/// # Panics
/// When `git` counts nothing (not made by `counting`)
#[must_use]
pub fn processes(git: &GitCommandExecutor) -> Vec<(String, u32)> {
  git
    .process_stats()
    .expect("a counting executor")
    .by_command()
    .into_iter()
    .map(|(subcommand, stats)| (subcommand, stats.count))
    .collect()
}

/// `git` counted exactly `expected`: `(subcommand, processes)`, sorted by subcommand
///
/// # Panics
/// When the counts differ, with the diff of both lists
#[track_caller]
pub fn assert_processes(git: &GitCommandExecutor, expected: &[(&str, u32)]) {
  let expected: Vec<(String, u32)> = expected.iter().map(|(subcommand, count)| ((*subcommand).to_string(), *count)).collect();
  assert_eq!(processes(git), expected);
}

/// Add process counts to `processes`, retaining its sorted order
#[must_use]
pub fn with_added_processes(processes: &[(&'static str, u32)], additions: &[(&'static str, u32)]) -> Vec<(&'static str, u32)> {
  let additions: Vec<(&'static str, i64)> = additions.iter().map(|(command, count)| (*command, i64::from(*count))).collect();
  with_process_deltas(processes, &additions)
}

/// Apply signed process-count deltas to `processes`, retaining its sorted order
#[must_use]
pub fn with_process_deltas(processes: &[(&'static str, u32)], changes: &[(&'static str, i64)]) -> Vec<(&'static str, u32)> {
  let mut counts: BTreeMap<&'static str, i64> = processes.iter().map(|(command, count)| (*command, i64::from(*count))).collect();
  for (command, delta) in changes {
    *counts.entry(*command).or_default() += *delta;
  }
  counts
    .into_iter()
    .filter_map(|(command, count)| {
      assert!(count >= 0, "negative process count for {command}");
      (count > 0).then(|| (command, u32::try_from(count).unwrap()))
    })
    .collect()
}

/// Assert that every first-parent commit in `base..tip` has a signature Git verifies
#[track_caller]
pub fn assert_signed_commits(repo: &TestRepo, base: &str, tip: &str) {
  let range = format!("{base}..{tip}");
  let commits = repo.git(&["rev-list", &range]);
  assert!(!commits.trim().is_empty(), "the range {range} must contain commits");
  for commit in commits.lines() {
    repo.git(&["verify-commit", commit]);
  }
  let statuses = repo.git(&["log", "--format=%G?", &range]);
  assert_eq!(statuses.trim(), vec!["G"; commits.lines().count()].join("\n"));
}

/// The processes of a first sync with two virtual branches
pub const FIRST_SYNC: &[(&str, u32)] = &[
  ("cat-file", 1),
  ("config", 1),
  ("fast-import", 3),
  ("for-each-ref", 1),
  ("log", 2),
  ("merge-base", 1),
  ("merge-tree", 2),
  ("notes", 1),
  ("remote", 1),
  ("rev-list", 2),
  ("rev-parse", 1),
  ("show-ref", 1),
  ("symbolic-ref", 1),
  ("update-ref", 2),
  ("var", 1),
];

/// A repository whose `origin/master` is its initial commit
#[must_use]
pub fn repo_on_origin() -> TestRepo {
  let repo = TestRepo::new();
  let initial = repo.create_commit("Initial commit", "README.md", "# Test Project\n");
  repo.git(&["remote", "add", "origin", "."]);
  repo.git(&["update-ref", "refs/remotes/origin/master", &initial]);
  repo
}

/// `origin/master` at the initial commit, then three commits of `alpha` and three of `beta` in turns; returns them
#[must_use]
pub fn alpha_and_beta_on_origin() -> (TestRepo, Vec<String>) {
  let repo = repo_on_origin();
  let mut commits = Vec::new();
  for index in 1..=3 {
    commits.push(repo.create_commit(&format!("(alpha) Alpha {index}"), &format!("alpha-{index}.txt"), "alpha\n"));
    commits.push(repo.create_commit(&format!("(beta) Beta {index}"), &format!("beta-{index}.txt"), "beta\n"));
  }
  (repo, commits)
}

/// The object cache in `dir` as a test opens it: the app's config, its commits not waiting for the drive
/// (`CacheConfig::sync_commits`), as those of `bd` in a test (`Environment::flush_to_drive`)
#[must_use]
pub fn cache_in(dir: &std::path::Path) -> ObjectCache {
  let mut config = CacheConfig::default();
  config.sync_commits = false;
  ObjectCache::open_dir(dir, &config)
}

/// An empty object cache in a temporary directory, which the cache needs while it is open (`cache_in`)
///
/// # Panics
/// When no temporary directory can be created
#[must_use]
pub fn empty_cache() -> (tempfile::TempDir, ObjectCache) {
  let dir = tempfile::tempdir().unwrap();
  let cache = cache_in(dir.path());
  (dir, cache)
}

/// The subcommands of the git processes `run` starts, in the order they started, whoever starts them in this test
/// process: a fixture as well as the code under test, with an executor of its own or `git_command`.
/// `GIT_TRACE2_EVENT` names a trace file while `run` runs (`EnvVars`: nextest runs a process per test); a process git
/// starts for itself (a nested sid, `upload-pack` of a fetch) is left out
///
/// # Panics
/// When no temporary directory can be created
pub fn git_processes_of<T>(run: impl FnOnce() -> T) -> (T, Vec<String>) {
  let dir = tempfile::tempdir().unwrap();
  let trace = dir.path().join("trace2.json");
  let result = {
    let _env = EnvVars::set(&[("GIT_TRACE2_EVENT", &trace)]);
    run()
  };
  let text = std::fs::read_to_string(&trace).unwrap_or_default();
  let subcommands = text
    .lines()
    .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
    .filter(|event| event["event"] == "start" && !event["sid"].as_str().unwrap_or_default().contains('/'))
    .map(|event| subcommand(event["argv"].as_array().map_or(&[], Vec::as_slice)))
    .collect();
  (result, subcommands)
}

/// The subcommand of `git <argv>`: the first word after the program and its options (`-c <name>=<value>` included)
fn subcommand(argv: &[serde_json::Value]) -> String {
  let mut words = argv.iter().skip(1).filter_map(serde_json::Value::as_str);
  while let Some(word) = words.next() {
    if word == "-c" || word == "-C" {
      words.next();
    } else if !word.starts_with('-') {
      return word.to_string();
    }
  }
  String::new()
}
